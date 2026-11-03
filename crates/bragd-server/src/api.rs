//! The game service's API: what Bragd asks of the Bragd server besides signing in and Call
//! signalling, which go to Supabase itself. Every answer is JSON; an error is
//! `{"error": "<code>"}`, and Bragd shows the code's text in the user's language.

use postgres::Transaction;
use serde_json::{Value, json};

use crate::db::{self, Pool};
use crate::jwt::{Caller, Verifier};

/// What the app is told before signing in (`GET /v1/config`).
#[derive(Clone, Debug, Default)]
pub struct Public {
    /// The Supabase project's address and publishable key, for Auth and Realtime.
    pub supabase_url: String,
    pub supabase_key: String,
    /// The OAuth providers turned on in the project, such as `github`.
    pub oauth: Vec<String>,
}

pub struct Service {
    pub pool: Pool,
    pub verifier: Verifier,
    pub public: Public,
}

/// An answer that is not a success.
#[derive(Debug)]
pub struct Error(pub u16, pub &'static str);

impl From<postgres::Error> for Error {
    fn from(e: postgres::Error) -> Error {
        use postgres::error::SqlState;
        match e.code() {
            Some(c) if *c == SqlState::UNIQUE_VIOLATION => Error(409, "taken"),
            Some(c) if *c == SqlState::INSUFFICIENT_PRIVILEGE => Error(403, "refused"),
            Some(c) if *c == SqlState::CHECK_VIOLATION || *c == SqlState::NOT_NULL_VIOLATION => Error(400, "bad_request"),
            Some(c) if *c == SqlState::FOREIGN_KEY_VIOLATION => Error(400, "no_profile"),
            _ => {
                eprintln!("database: {e}");
                Error(500, "server")
            }
        }
    }
}

pub(crate) type Res = Result<Value, Error>;

/// The largest request body read.
pub const MAX_BODY: usize = 64 * 1024;

impl Service {
    /// Answers `method path` with `body`, for the caller of the `Authorization` header.
    pub fn handle(&self, method: &str, path: &str, authorization: Option<&str>, body: &str) -> (u16, Value) {
        let out = self.route(method, path, authorization, body);
        match out {
            Ok(v) => (200, v),
            Err(Error(status, code)) => (status, json!({ "error": code })),
        }
    }

    fn route(&self, method: &str, path: &str, authorization: Option<&str>, body: &str) -> Res {
        let path = path.split('?').next().unwrap_or_default();
        if (method, path) == ("GET", "/v1/config") {
            let p = &self.public;
            return Ok(json!({ "supabase_url": p.supabase_url, "supabase_key": p.supabase_key, "oauth": p.oauth }));
        }
        let token = authorization.and_then(|a| a.strip_prefix("Bearer ")).ok_or(Error(401, "signed_out"))?;
        let caller = self.verifier.check(token).map_err(|_| Error(401, "signed_out"))?;
        let body: Value = if body.trim().is_empty() { json!({}) } else { serde_json::from_str(body).map_err(|_| Error(400, "bad_request"))? };
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        match (method, parts.as_slice()) {
            ("GET", ["v1", "state"]) => self.state(&caller),
            ("PUT", ["v1", "me"]) => self.me(&caller, &body),
            ("POST", ["v1", rest @ ..]) if matches!(rest.first(), Some(&("friends" | "invites" | "blocks" | "reports" | "clubs"))) => self.friends(&caller, rest, &body),
            ("POST", ["v1", "games"]) => self.challenge(&caller, &body),
            ("POST", ["v1", "games", id, what]) => {
                let id = uuid::Uuid::parse_str(id).map_err(|_| Error(404, "no_game"))?;
                self.game_action(&caller, id, what, &body)
            }
            _ => Err(Error(404, "not_found")),
        }
    }

    /// The caller's profile and their Games: open challenges, Games under way, and those that
    /// ended in the last day.
    fn state(&self, caller: &Caller) -> Res {
        self.pool.as_user(caller, |tx| {
            let me = tx.query_opt("select name from public.profiles where id = auth.uid()", &[])?.map(|r| r.get::<_, String>(0));
            let rows = tx.query(
                "select g.id, f.name, s.name, g.first = auth.uid(), g.challenger = auth.uid(), g.variant, g.status, \
                 coalesce(g.result, ''), coalesce(g.reason, ''), g.draw_offer is not null and g.draw_offer <> auth.uid(), \
                 g.draw_offer = auth.uid(), \
                 coalesce((select array_agg(m.entry order by m.ply) from public.moves m where m.game_id = g.id), '{}') \
                 from public.games g join public.profiles f on f.id = g.first join public.profiles s on s.id = g.second \
                 where g.status in ('offered', 'playing') or g.updated_at > now() - interval '1 day' \
                 order by g.updated_at desc limit 50",
                &[],
            )?;
            let games: Vec<Value> = rows
                .iter()
                .map(|r| {
                    let first: bool = r.get(3);
                    let theirs: String = if first { r.get(2) } else { r.get(1) };
                    json!({
                        "id": r.get::<_, uuid::Uuid>(0).to_string(), "opponent": theirs, "first": first,
                        "challenger": r.get::<_, bool>(4), "variant": r.get::<_, String>(5), "status": r.get::<_, String>(6),
                        "result": r.get::<_, String>(7), "reason": r.get::<_, String>(8),
                        "draw_offered": r.get::<_, bool>(9), "draw_asked": r.get::<_, Option<bool>>(10).unwrap_or(false),
                        "moves": r.get::<_, Vec<String>>(11),
                    })
                })
                .collect();
            let mut out = crate::friends::state(tx)?;
            out["me"] = json!(me);
            out["games"] = json!(games);
            Ok(out)
        })
    }

    /// Makes or renames the caller's profile (`name`), and sets whether members of their Clubs
    /// count as Friends (`club_friends`).
    fn me(&self, caller: &Caller, body: &Value) -> Res {
        self.pool.as_user(caller, |tx| {
            if let Some(name) = body.get("name") {
                let name = name.as_str().map(str::trim).filter(|n| !n.is_empty()).ok_or(Error(400, "bad_request"))?;
                tx.execute(
                    "insert into public.profiles (id, name) values (auth.uid(), $1) on conflict (id) do update set name = excluded.name",
                    &[&name],
                )?;
            }
            if let Some(on) = body["club_friends"].as_bool() {
                let n = tx.execute("update public.profiles set club_friends = $1 where id = auth.uid()", &[&on])?;
                if n == 0 {
                    return Err(Error(400, "no_profile"));
                }
            }
            Ok(json!({}))
        })
    }

    /// Challenges the player named `opponent` to a Game of `variant`, the caller playing the
    /// `side` asked for (`first`, `second` or `random`).
    fn challenge(&self, caller: &Caller, body: &Value) -> Res {
        let variant = text(body, "variant")?;
        playable(variant)?;
        let opponent = text(body, "opponent")?;
        let first = match body["side"].as_str().unwrap_or("random") {
            "first" => true,
            "second" => false,
            _ => random_bit(),
        };
        self.pool.as_user(caller, |tx| {
            let them: uuid::Uuid = tx
                .query_opt("select id from public.profiles where name = $1 and id <> auth.uid()", &[&opponent])?
                .ok_or(Error(404, "no_user"))?
                .get(0);
            let (a, b) = if first { (caller.id, them) } else { (them, caller.id) };
            let row = tx.query_one(
                "insert into public.games (first, second, challenger, variant) values ($1, $2, auth.uid(), $3) returning id",
                &[&a, &b, &variant],
            )?;
            Ok(json!({ "id": row.get::<_, uuid::Uuid>(0).to_string() }))
        })
    }

    fn game_action(&self, caller: &Caller, id: uuid::Uuid, what: &str, body: &Value) -> Res {
        self.pool.as_user(caller, |tx| {
            // Seen as the caller first: a Game they are not in is not there for them. Then
            // locked, and read again, so two requests on one Game take turns.
            tx.query_opt("select 1 from public.games where id = $1", &[&id])?.ok_or(Error(404, "no_game"))?;
            db::referee(tx)?;
            let row = tx.query_one("select first, second, challenger, variant, status, draw_offer from public.games where id = $1 for update", &[&id])?;
            let g = Game {
                id,
                first: row.get(0),
                second: row.get(1),
                challenger: row.get(2),
                variant: row.get(3),
                status: row.get(4),
                draw_offer: row.get(5),
            };
            g.act(tx, caller.id, what, body)
        })
    }
}

/// A Game as the referee acts on it.
struct Game {
    id: uuid::Uuid,
    first: uuid::Uuid,
    second: uuid::Uuid,
    challenger: uuid::Uuid,
    variant: String,
    status: String,
    draw_offer: Option<uuid::Uuid>,
}

impl Game {
    fn act(&self, tx: &mut Transaction, me: uuid::Uuid, what: &str, body: &Value) -> Res {
        let challenged = me != self.challenger;
        match (what, self.status.as_str()) {
            ("accept", "offered") if challenged => self.set(tx, "playing", None, None),
            ("decline", "offered") if challenged => self.set(tx, "declined", None, None),
            ("cancel", "offered") if !challenged => self.set(tx, "cancelled", None, None),
            ("move", "playing") => self.play(tx, me, body),
            ("resign", "playing") => {
                let result = if me == self.first { "0-1" } else { "1-0" };
                self.set(tx, "over", Some(result), Some("resign"))
            }
            ("draw", "playing") => {
                let yes = body["yes"].as_bool().unwrap_or(false);
                let theirs = self.draw_offer.is_some_and(|o| o != me);
                match (yes, theirs) {
                    (true, true) => self.set(tx, "over", Some("½-½"), Some("agreement")),
                    (true, false) => self.offer(tx, Some(me)),
                    (false, true) => self.offer(tx, None),
                    (false, false) => Err(Error(409, "no_draw")),
                }
            }
            ("accept" | "decline" | "cancel" | "move" | "resign" | "draw", _) => Err(Error(409, "not_now")),
            _ => Err(Error(404, "not_found")),
        }
    }

    fn set(&self, tx: &mut Transaction, status: &str, result: Option<&str>, reason: Option<&str>) -> Res {
        tx.execute(
            "update public.games set status = $2, result = $3, reason = $4, draw_offer = null, updated_at = now() where id = $1",
            &[&self.id, &status, &result, &reason],
        )?;
        Ok(json!({ "status": status }))
    }

    fn offer(&self, tx: &mut Transaction, by: Option<uuid::Uuid>) -> Res {
        tx.execute("update public.games set draw_offer = $2, updated_at = now() where id = $1", &[&self.id, &by])?;
        Ok(json!({ "status": self.status }))
    }

    /// Plays `entry` at `ply` for `me`, once Bragd's rules say it is legal and theirs to play.
    fn play(&self, tx: &mut Transaction, me: uuid::Uuid, body: &Value) -> Res {
        let entry = text(body, "entry")?;
        let entries: Vec<String> = tx.query("select entry from public.moves where game_id = $1 order by ply", &[&self.id])?.iter().map(|r| r.get(0)).collect();
        if body["ply"].as_u64() != Some(entries.len() as u64) {
            return Err(Error(409, "stale"));
        }
        let now = bragd_games::replay(&self.variant, None, &entries).map_err(|_| Error(500, "server"))?;
        let mover = match now.to_move {
            Some(s) if s.index == 0 => self.first,
            Some(_) => self.second,
            None => return Err(Error(409, "not_now")),
        };
        if mover != me {
            return Err(Error(409, "not_your_turn"));
        }
        let next = bragd_games::apply(&self.variant, None, &entries, entry).map_err(|_| Error(400, "illegal"))?;
        let ply = entries.len() as i32;
        tx.execute("insert into public.moves (game_id, ply, entry) values ($1, $2, $3)", &[&self.id, &ply, &entry])?;
        match &next.outcome {
            Some(o) => self.set(tx, "over", Some(&o.result), Some(&o.reason)),
            None => self.offer(tx, None),
        }
    }
}

/// A Variant a Game on the Bragd server may be played in: one Bragd has the rules of, with no
/// dice. shortcut: the dice Families need the referee to throw for both players; until it does,
/// they are refused here.
fn playable(variant: &str) -> Result<(), Error> {
    use bragd_games::Family;
    let v = bragd_games::variant(variant).map_err(|_| Error(400, "variant"))?;
    match v.family() {
        Family::Backgammon | Family::Ur | Family::Senet => Err(Error(400, "variant")),
        _ => Ok(()),
    }
}

pub(crate) fn text<'a>(body: &'a Value, field: &str) -> Result<&'a str, Error> {
    body[field].as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or(Error(400, "bad_request"))
}

fn random_bit() -> bool {
    let mut b = [0u8; 1];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut b).is_ok() && b[0] & 1 == 1
}
