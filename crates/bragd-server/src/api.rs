//! The game service's API: what Bragd asks of the Bragd server besides signing in and Call
//! signalling, which go to Supabase itself. Every answer is JSON; an error is
//! `{"error": "<code>"}`, and Bragd shows the code's text in the user's language.

use postgres::Transaction;
use bragd_games::Awaiting;
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
    pub turn: Turn,
}

/// The STUN and TURN servers a Call may use, and the secret TURN shares with the game service.
#[derive(Clone, Debug, Default)]
pub struct Turn {
    /// `stun:` and `turn:`/`turns:` addresses.
    pub urls: Vec<String>,
    /// Empty: no TURN, only STUN.
    pub secret: String,
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
            ("GET", ["v1", "turn"]) => Ok(self.ice(&caller)),
            ("PUT", ["v1", "me"]) => self.me(&caller, &body),
            ("DELETE", ["v1", "me"]) => self.forget(&caller, &body),
            ("POST", ["v1", rest @ ..]) if matches!(rest.first(), Some(&("friends" | "invites" | "blocks" | "reports" | "clubs"))) => self.friends(&caller, rest, &body),
            ("POST", ["v1", "games"]) => self.challenge(&caller, &body),
            ("POST", ["v1", "seeks"]) => self.seek(&caller, &body),
            ("DELETE", ["v1", "seeks"]) => self.pool.as_user(&caller, |tx| {
                tx.execute("delete from public.seeks where player = auth.uid()", &[])?;
                Ok(json!({}))
            }),
            ("POST", ["v1", "seeks", id, "accept"]) => {
                let id = uuid::Uuid::parse_str(id).map_err(|_| Error(404, "no_seek"))?;
                self.take(&caller, id)
            }
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
                "select g.id, f.name, s.name, coalesce(g.first = auth.uid(), false), coalesce(g.challenger = auth.uid(), false), g.variant, g.status, \
                 coalesce(g.result, ''), coalesce(g.reason, ''), g.draw_offer is not null and g.draw_offer <> auth.uid(), \
                 g.draw_offer = auth.uid(), \
                 coalesce((select array_agg(m.entry order by m.ply) from public.moves m where m.game_id = g.id), '{}'), \
                 case when g.first = auth.uid() then g.chat_first else g.chat_second end, \
                 case when g.first = auth.uid() then g.chat_second else g.chat_first end, \
                 case when g.first = auth.uid() then g.keep_first else g.keep_second end, \
                 case when g.first = auth.uid() then g.keep_second else g.keep_first end, \
                 coalesce(bragd.are_friends(auth.uid(), case when g.first = auth.uid() then g.second else g.first end), false), \
                 case when g.first = auth.uid() then g.second else g.first end, \
                 (select coalesce(json_agg(json_build_object('mine', c.sender = auth.uid(), 'text', c.text) order by c.id), '[]'::json) \
                  from (select * from public.chat where game_id = g.id order by id desc limit 100) c), \
                 g.initial, g.clock_ms, g.increment_ms, g.rated, g.left_first, g.left_second, g.clock_side, \
                 coalesce((extract(epoch from now() - g.turn_started) * 1000)::bigint, 0) \
                 from public.games g left join public.profiles f on f.id = g.first left join public.profiles s on s.id = g.second \
                 where (g.first is null or f.id is not null) and (g.second is null or s.id is not null) \
                 and (g.status in ('offered', 'playing') or g.updated_at > now() - interval '1 day') \
                 order by g.updated_at desc limit 50",
                &[],
            )?;
            let games: Vec<Value> = rows
                .iter()
                .map(|r| {
                    let first: bool = r.get(3);
                    // None: a deleted player, who left the Game to the caller.
                    let theirs: Option<String> = if first { r.get(2) } else { r.get(1) };
                    json!({
                        "id": r.get::<_, uuid::Uuid>(0).to_string(), "opponent": theirs, "first": first,
                        "challenger": r.get::<_, bool>(4), "variant": r.get::<_, String>(5), "status": r.get::<_, String>(6),
                        "result": r.get::<_, String>(7), "reason": r.get::<_, String>(8),
                        "draw_offered": r.get::<_, bool>(9), "draw_asked": r.get::<_, Option<bool>>(10).unwrap_or(false),
                        "moves": r.get::<_, Vec<String>>(11),
                        "chat": { "mine": r.get::<_, bool>(12), "theirs": r.get::<_, bool>(13), "keep_mine": r.get::<_, bool>(14),
                            "keep_theirs": r.get::<_, bool>(15), "lines": r.get::<_, Value>(18) },
                        "friend": r.get::<_, bool>(16), "opponent_id": r.get::<_, Option<uuid::Uuid>>(17).map(|u| u.to_string()),
                        "initial": r.get::<_, Option<String>>(19),
                        "rated": r.get::<_, bool>(22),
                        "clock": clock(r.get(6), r.get(20), r.get(21), [r.get(23), r.get(24)], r.get(25), r.get(26)),
                    })
                })
                .collect();
            let ratings: Vec<Value> = tx
                .query("select family, rating, deviation, games from public.ratings where player = auth.uid() order by family", &[])?
                .iter()
                .map(|r| json!({ "family": r.get::<_, String>(0), "rating": r.get::<_, f64>(1), "deviation": r.get::<_, f64>(2), "games": r.get::<_, i32>(3) }))
                .collect();
            // shortcut: a seek lasts an hour unless taken or taken back; renew it from the app if players wait longer.
            let seeks: Vec<Value> = tx
                .query(
                    "select s.id, p.name, s.variant, s.clock_ms, s.increment_ms, s.rated, s.player = auth.uid() \
                     from public.seeks s join public.profiles p on p.id = s.player \
                     where s.created_at > now() - interval '1 hour' order by s.created_at desc limit 50",
                    &[],
                )?
                .iter()
                .map(|r| {
                    json!({ "id": r.get::<_, uuid::Uuid>(0).to_string(), "name": r.get::<_, String>(1), "variant": r.get::<_, String>(2),
                        "minutes": r.get::<_, Option<i32>>(3).map(|ms| ms / 60_000), "increment": r.get::<_, i32>(4) / 1000,
                        "rated": r.get::<_, bool>(5), "mine": r.get::<_, bool>(6) })
                })
                .collect();
            let mut out = crate::friends::state(tx)?;
            out["ratings"] = json!(ratings);
            out["seeks"] = json!(seeks);
            out["me"] = json!(me);
            out["id"] = json!(caller.id.to_string());
            out["games"] = json!(games);
            Ok(out)
        })
    }

    /// The ICE servers for the caller's Calls: STUN, and TURN with credentials of their own when
    /// a secret is set. The WebRTC `iceServers` shape.
    fn ice(&self, caller: &Caller) -> Value {
        let (stun, turn): (Vec<&String>, Vec<&String>) = self.turn.urls.iter().partition(|u| u.starts_with("stun:"));
        let mut servers = vec![];
        if !stun.is_empty() {
            servers.push(json!({ "urls": stun }));
        }
        if !turn.is_empty() && !self.turn.secret.is_empty() {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let (username, credential) = crate::turn::credentials(&self.turn.secret, &caller.id.to_string(), now);
            servers.push(json!({ "urls": turn, "username": username, "credential": credential }));
        }
        json!({ "ice_servers": servers, "ttl": crate::turn::TTL })
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

    /// Deletes the caller's account and everything with their id
    /// (`supabase/migrations/…_delete_account.sql`). With `keep_games`, the Games that ended stay
    /// with the other players, against a deleted player.
    fn forget(&self, caller: &Caller, body: &Value) -> Res {
        let keep = body["keep_games"].as_bool().unwrap_or(false);
        self.pool.as_user(caller, |tx| {
            db::referee(tx)?;
            tx.execute("select bragd.forget($1, $2)", &[&caller.id, &keep])?;
            Ok(json!({}))
        })
    }

    /// Challenges the player named `opponent` to a Game of `variant`, the caller playing the
    /// `side` asked for (`first`, `second` or `random`).
    fn challenge(&self, caller: &Caller, body: &Value) -> Res {
        let variant = text(body, "variant")?;
        playable(variant)?;
        let opponent = text(body, "opponent")?;
        let (clock_ms, increment_ms) = time_control(body)?;
        let rated = body["rated"].as_bool().unwrap_or(false);
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
                "insert into public.games (first, second, challenger, variant, clock_ms, increment_ms, rated) values ($1, $2, auth.uid(), $3, $4, $5, $6) returning id",
                &[&a, &b, &variant, &clock_ms, &increment_ms, &rated],
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
            Game::load(tx, id)?.act(tx, caller.id, what, body)
        })
    }

    /// Offers a Game of `variant` to any player, with a time control and rated or not as asked.
    /// It replaces the caller's earlier seek.
    fn seek(&self, caller: &Caller, body: &Value) -> Res {
        let variant = text(body, "variant")?;
        playable(variant)?;
        let (clock_ms, increment_ms) = time_control(body)?;
        let rated = body["rated"].as_bool().unwrap_or(false);
        self.pool.as_user(caller, |tx| {
            tx.execute("delete from public.seeks where player = auth.uid()", &[])?;
            let row = tx.query_one(
                "insert into public.seeks (player, variant, clock_ms, increment_ms, rated) values (auth.uid(), $1, $2, $3, $4) returning id",
                &[&variant, &clock_ms, &increment_ms, &rated],
            )?;
            Ok(json!({ "id": row.get::<_, uuid::Uuid>(0).to_string() }))
        })
    }

    /// Takes the seek `id`: a Game starts between its player and the caller, Sides drawn at random.
    fn take(&self, caller: &Caller, id: uuid::Uuid) -> Res {
        self.pool.as_user(caller, |tx| {
            // Seen as the caller: a seek between players who block each other is not there.
            let seeker: uuid::Uuid = tx
                .query_opt("select player from public.seeks where id = $1 and created_at > now() - interval '1 hour'", &[&id])?
                .ok_or(Error(404, "no_seek"))?
                .get(0);
            if seeker == caller.id {
                return Err(Error(409, "own_seek"));
            }
            tx.query_opt("select 1 from public.profiles where id = auth.uid()", &[])?.ok_or(Error(400, "no_profile"))?;
            db::referee(tx)?;
            // Deleted as it is taken, so two players taking it at once start one Game.
            let row = tx
                .query_opt("delete from public.seeks where id = $1 returning variant, clock_ms, increment_ms, rated", &[&id])?
                .ok_or(Error(404, "no_seek"))?;
            let variant: String = row.get(0);
            playable(&variant)?;
            let (a, b) = if random_bit() { (seeker, caller.id) } else { (caller.id, seeker) };
            let game: uuid::Uuid = tx
                .query_one(
                    "insert into public.games (first, second, challenger, variant, clock_ms, increment_ms, rated) values ($1, $2, $3, $4, $5, $6, $7) returning id",
                    &[&a, &b, &seeker, &variant, &row.get::<_, Option<i32>>(1), &row.get::<_, i32>(2), &row.get::<_, bool>(3)],
                )?
                .get(0);
            Game::load(tx, game)?.start(tx)?;
            Ok(json!({ "id": game.to_string() }))
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
    /// Who moves first, when the rules decided it at the start (backgammon's opening throw).
    initial: Option<String>,
    /// The time control in milliseconds, `None` without a clock, and the increment.
    clock_ms: Option<i32>,
    increment_ms: i32,
    rated: bool,
    /// Each Side's time when its clock last started, the Side whose clock runs, and the
    /// milliseconds since it started, by the database's clock.
    left: [Option<i32>; 2],
    clock_side: Option<i16>,
    elapsed: i64,
}

impl Game {
    /// The Game `id`, locked until the transaction ends. The caller has stepped out of their role.
    fn load(tx: &mut Transaction, id: uuid::Uuid) -> Result<Game, Error> {
        let row = tx.query_one(
            "select first, second, challenger, variant, status, draw_offer, initial, clock_ms, increment_ms, rated, left_first, left_second, clock_side, \
             coalesce((extract(epoch from now() - turn_started) * 1000)::bigint, 0) from public.games where id = $1 for update",
            &[&id],
        )?;
        // A Game left by a deleted player has ended: nothing more is done in it.
        let (Some(first), Some(second), Some(challenger)) = (row.get(0), row.get(1), row.get(2)) else { return Err(Error(409, "not_now")) };
        Ok(Game {
            id,
            first,
            second,
            challenger,
            variant: row.get(3),
            status: row.get(4),
            draw_offer: row.get(5),
            initial: row.get(6),
            clock_ms: row.get(7),
            increment_ms: row.get(8),
            rated: row.get(9),
            left: [row.get(10), row.get(11)],
            clock_side: row.get(12),
            elapsed: row.get(13),
        })
    }

    /// The Game starts: the referee's first throws, and both clocks, the first mover's running.
    fn start(&self, tx: &mut Transaction) -> Res {
        self.set(tx, "playing", None, None)?;
        let now = self.throws(tx, false)?;
        if self.clock_ms.is_some() {
            let side = now.to_move.map(|s| s.index as i16);
            tx.execute(
                "update public.games set left_first = clock_ms, left_second = clock_ms, clock_side = $2, turn_started = now() where id = $1",
                &[&self.id, &side],
            )?;
        }
        Ok(json!({ "status": "playing" }))
    }

    /// The Side whose time ran out, if one did.
    fn flagged(&self) -> Option<u8> {
        let side = self.clock_side? as usize;
        (i64::from(self.left[side]?) - self.elapsed <= 0).then_some(side as u8)
    }

    fn act(&self, tx: &mut Transaction, me: uuid::Uuid, what: &str, body: &Value) -> Res {
        // The clock is checked before anything else: a move after the flag fell is not played.
        if self.status == "playing"
            && let Some(side) = self.flagged()
        {
            return self.end(tx, Some(1 - side), "time");
        }
        let challenged = me != self.challenger;
        match (what, self.status.as_str()) {
            ("accept", "offered") if challenged => self.start(tx),
            ("decline", "offered") if challenged => self.set(tx, "declined", None, None),
            ("cancel", "offered") if !challenged => self.set(tx, "cancelled", None, None),
            ("move", "playing") => self.play(tx, me, body),
            // Backgammon: the player may double before their throw, so the referee waits to be asked.
            ("throw", "playing") => {
                let (now, _) = self.now(tx)?;
                if now.awaiting != Awaiting::Throw {
                    return Err(Error(409, "not_now"));
                }
                if self.seat(&now) != Some(me) {
                    return Err(Error(409, "not_your_turn"));
                }
                let next = self.throws(tx, true)?;
                self.tick(tx, &now, &next)?;
                self.after(tx, &next)
            }
            // Chat: the caller's yes, or closing it for both; keeping it; a line.
            ("chat", "playing") => {
                let on = body["on"].as_bool().unwrap_or(false);
                let sql = match (on, me == self.first) {
                    (true, true) => "update public.games set chat_first = true where id = $1",
                    (true, false) => "update public.games set chat_second = true where id = $1",
                    (false, _) => "update public.games set chat_first = false, chat_second = false where id = $1",
                };
                tx.execute(sql, &[&self.id])?;
                Ok(json!({ "chat": on }))
            }
            ("keep", "playing") => {
                let on = body["on"].as_bool().unwrap_or(false);
                let sql = if me == self.first { "update public.games set keep_first = $2 where id = $1" } else { "update public.games set keep_second = $2 where id = $1" };
                tx.execute(sql, &[&self.id, &on])?;
                Ok(json!({ "keep": on }))
            }
            ("say", "playing") => {
                let line = text(body, "text")?;
                // As the caller: the policy lets a line in only while both said yes.
                tx.batch_execute("set local role authenticated")?;
                tx.execute("insert into public.chat (game_id, text) values ($1, $2)", &[&self.id, &line]).map_err(|e| match Error::from(e) {
                    Error(403, _) => Error(409, "chat_closed"),
                    other => other,
                })?;
                Ok(json!({}))
            }
            ("resign", "playing") => self.end(tx, Some(if me == self.first { 1 } else { 0 }), "resign"),
            ("draw", "playing") => {
                // Race Games are always won: an agreement to a draw is not a result there.
                if !bragd_games::variant(&self.variant).map_err(|_| Error(500, "server"))?.family().draws() {
                    return Err(Error(409, "not_now"));
                }
                let yes = body["yes"].as_bool().unwrap_or(false);
                let theirs = self.draw_offer.is_some_and(|o| o != me);
                match (yes, theirs) {
                    (true, true) => self.end(tx, None, "agreement"),
                    (true, false) => self.offer(tx, Some(me)),
                    (false, true) => self.offer(tx, None),
                    (false, false) => Err(Error(409, "no_draw")),
                }
            }
            ("accept" | "decline" | "cancel" | "move" | "throw" | "flag" | "resign" | "draw" | "chat" | "keep" | "say", _) => Err(Error(409, "not_now")),
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

    /// Ends the Game won by the Side `winner`, drawn when `None`, and rates it if it is rated.
    fn end(&self, tx: &mut Transaction, winner: Option<u8>, reason: &str) -> Res {
        let result = match winner {
            None => "½-½",
            Some(0) => "1-0",
            Some(_) => "0-1",
        };
        self.over(tx, winner, result, reason)
    }

    fn over(&self, tx: &mut Transaction, winner: Option<u8>, result: &str, reason: &str) -> Res {
        let out = self.set(tx, "over", Some(result), Some(reason))?;
        if self.rated {
            self.rate(tx, winner)?;
        }
        Ok(out)
    }

    /// Both players' Glicko-2 ratings in the Game's Family after this Game, a rating period of
    /// its own (docs/adr/0007-ratings-on-the-bragd-server.md).
    fn rate(&self, tx: &mut Transaction, winner: Option<u8>) -> Result<(), Error> {
        use bragd_clubs::rating::{Game as Rated, Rating, glicko2};
        let family = bragd_games::variant(&self.variant).map_err(|_| Error(500, "server"))?.family().key();
        let players = [self.first, self.second];
        tx.execute("insert into public.ratings (player, family) values ($1, $3), ($2, $3) on conflict do nothing", &[&players[0], &players[1], &family])?;
        let mut before = [Rating::default(); 2];
        for (b, p) in before.iter_mut().zip(&players) {
            let r = tx.query_one("select rating, deviation, volatility from public.ratings where player = $1 and family = $2 for update", &[p, &family])?;
            *b = Rating { rating: r.get(0), deviation: r.get(1), volatility: r.get(2) };
        }
        for (i, p) in players.iter().enumerate() {
            let score = match winner {
                None => 0.5,
                Some(w) if usize::from(w) == i => 1.0,
                Some(_) => 0.0,
            };
            let after = glicko2(before[i], &[Rated { opponent: before[1 - i], score }]);
            tx.execute(
                "update public.ratings set rating = $3, deviation = $4, volatility = $5, games = games + 1 where player = $1 and family = $2",
                &[p, &family, &after.rating, &after.deviation, &after.volatility],
            )?;
        }
        Ok(())
    }

    /// Charges the time the mover (`before`'s Side to move) used, adds the increment once the
    /// turn passed to the other Side, and starts the clock of whoever moves next.
    fn tick(&self, tx: &mut Transaction, before: &bragd_games::State, next: &bragd_games::State) -> Result<(), Error> {
        let (Some(_), Some(side)) = (self.clock_ms, before.to_move.map(|s| s.index)) else { return Ok(()) };
        let to = next.to_move.map(|s| s.index);
        let added = if to.is_some_and(|t| t != side) { i64::from(self.increment_ms) } else { 0 };
        let left = i64::from(self.left[side as usize].unwrap_or(0)) - self.elapsed + added;
        let left = i32::try_from(left.max(0)).unwrap_or(i32::MAX);
        let sql = if side == 0 {
            "update public.games set left_first = $2, clock_side = $3, turn_started = now() where id = $1"
        } else {
            "update public.games set left_second = $2, clock_side = $3, turn_started = now() where id = $1"
        };
        tx.execute(sql, &[&self.id, &left, &to.map(i16::from)])?;
        Ok(())
    }

    fn offer(&self, tx: &mut Transaction, by: Option<uuid::Uuid>) -> Res {
        tx.execute("update public.games set draw_offer = $2, updated_at = now() where id = $1", &[&self.id, &by])?;
        Ok(json!({ "status": self.status }))
    }

    /// The Game's record, and the rules' view of it.
    fn now(&self, tx: &mut Transaction) -> Result<(bragd_games::State, Vec<String>), Error> {
        let entries: Vec<String> = tx.query("select entry from public.moves where game_id = $1 order by ply", &[&self.id])?.iter().map(|r| r.get(0)).collect();
        let now = bragd_games::replay(&self.variant, self.initial.as_deref(), &entries).map_err(|_| Error(500, "server"))?;
        Ok((now, entries))
    }

    /// The player whose turn it is, or `None` once the Game is over.
    fn seat(&self, now: &bragd_games::State) -> Option<uuid::Uuid> {
        now.to_move.map(|s| if s.index == 0 { self.first } else { self.second })
    }

    /// Plays `entry` at `ply` for `me`, once Bragd's rules say it is legal and theirs to play.
    fn play(&self, tx: &mut Transaction, me: uuid::Uuid, body: &Value) -> Res {
        let entry = text(body, "entry")?;
        let (now, entries) = self.now(tx)?;
        if body["ply"].as_u64() != Some(entries.len() as u64) {
            return Err(Error(409, "stale"));
        }
        match self.seat(&now) {
            None => return Err(Error(409, "not_now")),
            Some(p) if p != me => return Err(Error(409, "not_your_turn")),
            Some(_) => {}
        }
        // The dice are the referee's: while a throw is awaited, a player plays only what the rules
        // offer before it, such as a double.
        if now.awaiting == Awaiting::Throw && !now.moves.iter().any(|m| m.wire == entry) {
            return Err(Error(400, "illegal"));
        }
        bragd_games::apply(&self.variant, self.initial.as_deref(), &entries, entry).map_err(|_| Error(400, "illegal"))?;
        tx.execute("insert into public.moves (game_id, ply, entry) values ($1, $2, $3)", &[&self.id, &(entries.len() as i32), &entry])?;
        let next = self.throws(tx, false)?;
        self.tick(tx, &now, &next)?;
        self.after(tx, &next)
    }

    /// The Game over by the rules, or the turn passed on.
    fn after(&self, tx: &mut Transaction, next: &bragd_games::State) -> Res {
        match &next.outcome {
            Some(o) => self.over(tx, o.winner.map(|w| w.index), &o.result, &o.reason),
            None => self.offer(tx, None),
        }
    }

    /// The referee's throws, from the operating system's random source, recorded as entries:
    /// backgammon's opening throw, which also seats who starts, then each throw awaited until a
    /// player has a move to make. Where the player may double first, the referee throws only when
    /// they ask (`asked`).
    fn throws(&self, tx: &mut Transaction, mut asked: bool) -> Result<bragd_games::State, Error> {
        let (_, mut entries) = self.now(tx)?;
        let family = bragd_games::variant(&self.variant).map_err(|_| Error(500, "server"))?.family();
        let mut initial = self.initial.clone();
        let record = |tx: &mut Transaction, entries: &mut Vec<String>, entry: String| -> Result<(), Error> {
            tx.execute("insert into public.moves (game_id, ply, entry) values ($1, $2, $3)", &[&self.id, &(entries.len() as i32), &entry])?;
            entries.push(entry);
            Ok(())
        };
        if let Some((first, roll)) = bragd_core::throw::opening(family, &entries) {
            tx.execute("update public.games set initial = $2 where id = $1", &[&self.id, &first])?;
            initial = Some(first);
            record(tx, &mut entries, roll)?;
        }
        // A guard against a loop only: turns passed by throws that allow no move end long before.
        for _ in 0..1000 {
            let now = bragd_games::replay(&self.variant, initial.as_deref(), &entries).map_err(|_| Error(500, "server"))?;
            // Resigning before a throw is the `resign` action's; only a double waits for the player.
            if now.outcome.is_some() || now.awaiting != Awaiting::Throw || (now.moves.iter().any(|m| m.wire == "double") && !asked) {
                return Ok(now);
            }
            asked = false;
            let t = bragd_core::throw::throw(family, &now.throws).ok_or(Error(500, "server"))?;
            record(tx, &mut entries, t)?;
        }
        Err(Error(500, "server"))
    }
}

/// A Variant a Game on the Bragd server may be played in: one Bragd has the rules of.
fn playable(variant: &str) -> Result<(), Error> {
    bragd_games::variant(variant).map(|_| ()).map_err(|_| Error(400, "variant"))
}

/// A time control from `minutes` and `increment` (seconds), as `(clock_ms, increment_ms)`; no
/// clock without minutes. The table's checks bound them.
fn time_control(body: &Value) -> Result<(Option<i32>, i32), Error> {
    let ms = |field: &str, unit: i64| body[field].as_i64().map(|v| i32::try_from(v.saturating_mul(unit)).map_err(|_| Error(400, "bad_request"))).transpose();
    Ok((ms("minutes", 60_000)?, ms("increment", 1000)?.unwrap_or(0)))
}

/// A Game's clock as the app shows it: each Side's time now, and the Side whose clock runs.
fn clock(status: String, clock_ms: Option<i32>, increment_ms: i32, left: [Option<i32>; 2], side: Option<i16>, elapsed: i64) -> Value {
    if clock_ms.is_none() {
        return Value::Null;
    }
    let running = side.filter(|_| status == "playing");
    let now = |i: usize| {
        let l = i64::from(left[i].or(clock_ms).unwrap_or(0));
        if running == Some(i as i16) { (l - elapsed).max(0) } else { l }
    };
    json!({ "left": [now(0), now(1)], "running": running, "increment": increment_ms })
}

pub(crate) fn text<'a>(body: &'a Value, field: &str) -> Result<&'a str, Error> {
    body[field].as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or(Error(400, "bad_request"))
}

fn random_bit() -> bool {
    let mut b = [0u8; 1];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut b).is_ok() && b[0] & 1 == 1
}
