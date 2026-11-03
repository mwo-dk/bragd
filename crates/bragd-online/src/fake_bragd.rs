//! A fake Bragd server on 127.0.0.1, for tests and screenshots: the game service's API and the
//! part of Supabase Auth Bragd uses, in memory, on one port. The account is BragdTester (password
//! `bragd-fake`). Anna is a Friend and challenges to chess, Bo is a Friend through the Club
//! Skakklubben, and Carl asks to be a Friend. In a Game, the opponent answers each move with the
//! first legal one, by Bragd's rules.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::fake::{Fake, Reply, Request};

/// The token the fake hands out and accepts.
pub const TOKEN: &str = "fake-access";
pub const REFRESH: &str = "fake-refresh";
pub const PASSWORD: &str = "bragd-fake";

#[derive(Clone, Debug)]
pub struct Game {
    pub id: String,
    pub opponent: String,
    pub first: bool,
    pub challenger: bool,
    pub variant: String,
    pub status: String,
    pub result: String,
    pub reason: String,
    pub moves: Vec<String>,
    /// Who moves first, as backgammon's opening throw decided.
    pub initial: Option<String>,
    pub draw_offered: bool,
    pub friend: bool,
    pub chat_mine: bool,
    pub chat_theirs: bool,
    pub keep_mine: bool,
    /// (mine, text)
    pub lines: Vec<(bool, String)>,
}

/// What the fake holds; tests read and change it.
#[derive(Clone, Debug)]
pub struct World {
    pub me: Option<String>,
    pub friends: Vec<(String, bool)>,
    pub asked: Vec<String>,
    pub asking: Vec<String>,
    pub blocked: Vec<String>,
    pub club_friends: bool,
    pub games: Vec<Game>,
    pub reports: Vec<(String, String)>,
    /// The account was deleted, and whether its ended Games were left to the others.
    pub deleted: Option<bool>,
    /// The opponent says yes to a Chat at once, and greets: for the screenshots.
    pub agree: bool,
    next: u32,
}

impl Default for World {
    fn default() -> World {
        World {
            me: Some("BragdTester".into()),
            friends: vec![("Anna".into(), false), ("Bo".into(), true)],
            asked: vec!["Carl".into()],
            asking: vec![],
            blocked: vec![],
            club_friends: true,
            games: vec![Game {
                id: "g-anna".into(),
                opponent: "Anna".into(),
                first: true,
                challenger: false,
                variant: "chess".into(),
                status: "offered".into(),
                result: String::new(),
                reason: String::new(),
                moves: vec![],
                initial: None,
                draw_offered: false,
                friend: true,
                chat_mine: false,
                chat_theirs: false,
                keep_mine: false,
                lines: vec![],
            }],
            reports: vec![],
            deleted: None,
            agree: false,
            next: 1,
        }
    }
}

impl World {
    fn state(&self) -> Value {
        let games: Vec<Value> = self
            .games
            .iter()
            .map(|g| {
                json!({ "id": g.id, "opponent": g.opponent, "first": g.first, "challenger": g.challenger, "variant": g.variant,
                    "status": g.status, "result": g.result, "reason": g.reason, "moves": g.moves, "initial": g.initial, "draw_offered": g.draw_offered, "draw_asked": false,
                    "friend": g.friend, "opponent_id": format!("id-{}", g.opponent.to_lowercase()),
                    "chat": { "mine": g.chat_mine, "theirs": g.chat_theirs, "keep_mine": g.keep_mine, "keep_theirs": false,
                        "lines": g.lines.iter().map(|(m, t)| json!({ "mine": m, "text": t })).collect::<Vec<_>>() } })
            })
            .collect();
        let clubs = json!([{ "id": "club-1", "name": "Skakklubben", "code": "", "members": 12 }]);
        json!({
            "me": self.me, "id": "id-bragdtester", "games": games, "asked": self.asked, "asking": self.asking, "blocked": self.blocked,
            "friends": self.friends.iter().map(|(n, c)| json!({ "name": n, "club": c })).collect::<Vec<_>>(),
            "clubs": clubs, "club_friends": self.club_friends,
        })
    }
}

/// The fake on a free port.
pub fn bragd() -> (Fake, Arc<Mutex<World>>) {
    bragd_on(0)
}

pub fn bragd_on(port: u16) -> (Fake, Arc<Mutex<World>>) {
    let world: Arc<Mutex<World>> = Arc::default();
    let w = world.clone();
    let fake = Fake::start_with_socket(port, move |r, fake| reply(r, fake, &w), realtime);
    (fake, world)
}

fn ok(v: Value) -> Reply {
    Reply::Json(200, v.to_string())
}

fn err(status: u16, code: &str) -> Reply {
    Reply::Json(status, json!({ "error": code }).to_string())
}

fn tokens() -> Reply {
    ok(json!({ "access_token": TOKEN, "refresh_token": REFRESH, "expires_in": 3600, "token_type": "bearer" }))
}

/// The players a Call may be between: BragdTester and the Friends, by their ids in `/v1/state`.
const CALLERS: [&str; 3] = ["id-bragdtester", "id-anna", "id-bo"];

/// Supabase Realtime, as far as Call signalling uses it: a join of a private topic `call:<a>:<b>`
/// is answered ok for two of the callers in order with the fake's token, and refused otherwise; a
/// broadcast goes to every socket, as from the other side.
fn realtime(text: &str, fake: &Fake) {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let topic = v["topic"].as_str().unwrap_or_default();
    match v["event"].as_str() {
        Some("phx_join") => {
            let pair = topic.strip_prefix("realtime:call:").and_then(|p| p.split_once(':'));
            let ok = v["payload"]["access_token"] == TOKEN
                && v["payload"]["config"]["private"] == true
                && pair.is_some_and(|(a, b)| a < b && CALLERS.contains(&a) && CALLERS.contains(&b));
            let status = if ok { "ok" } else { "error" };
            fake.tell(&json!({ "topic": topic, "event": "phx_reply", "ref": v["ref"], "payload": { "status": status, "response": {} } }).to_string());
        }
        Some("broadcast") => fake.tell(&json!({ "topic": topic, "event": "broadcast", "ref": null, "payload": v["payload"] }).to_string()),
        Some("heartbeat") => fake.tell(&json!({ "topic": "phoenix", "event": "phx_reply", "ref": v["ref"], "payload": { "status": "ok", "response": {} } }).to_string()),
        _ => {}
    }
}

fn reply(r: &Request, fake: &Fake, world: &Mutex<World>) -> Reply {
    let body: Value = serde_json::from_str(&r.body).unwrap_or(Value::Null);
    let path = r.path.as_str();
    match (r.method.as_str(), path) {
        (_, "/realtime/v1/websocket") if r.ws_key.is_some() => return Reply::Socket,
        ("GET", "/v1/config") => return ok(json!({ "supabase_url": fake.url, "supabase_key": "fake-publishable", "oauth": ["github"] })),
        ("POST", "/auth/v1/token") => {
            return match r.query.as_str() {
                "grant_type=password" if body["password"] == PASSWORD => tokens(),
                "grant_type=password" => Reply::Json(400, json!({ "error_code": "invalid_credentials", "msg": "Invalid login credentials" }).to_string()),
                "grant_type=refresh_token" if body["refresh_token"] == REFRESH => tokens(),
                "grant_type=pkce" if body["auth_code"] == "fake-code" && body["code_verifier"].is_string() => tokens(),
                _ => Reply::Json(400, json!({ "error_code": "bad_jwt" }).to_string()),
            };
        }
        ("POST", "/auth/v1/signup") => return ok(json!({ "id": "new-user", "email": body["email"] })),
        ("POST", "/auth/v1/logout") => return Reply::Json(204, String::new()),
        _ => {}
    }
    if r.auth.as_deref() != Some(&format!("Bearer {TOKEN}")) {
        return err(401, "signed_out");
    }
    let mut w = world.lock().unwrap_or_else(|e| e.into_inner());
    let agree = w.agree;
    let name = || body["name"].as_str().unwrap_or_default().to_string();
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    match (r.method.as_str(), parts.as_slice()) {
        ("GET", ["v1", "state"]) => return ok(w.state()),
        ("GET", ["v1", "turn"]) => return ok(json!({ "ice_servers": [], "ttl": 43200 })),
        ("PUT", ["v1", "me"]) => {
            if let Some(n) = body["name"].as_str() {
                w.me = Some(n.to_string());
            }
            if let Some(c) = body["club_friends"].as_bool() {
                w.club_friends = c;
            }
        }
        ("DELETE", ["v1", "me"]) => {
            w.deleted = Some(body["keep_games"].as_bool().unwrap_or(false));
            w.me = None;
        }
        ("POST", ["v1", "friends"]) => {
            let n = name();
            if let Some(i) = w.asked.iter().position(|a| *a == n) {
                w.asked.remove(i);
                w.friends.push((n, false));
            } else {
                w.asking.push(n);
            }
        }
        ("POST", ["v1", "friends", "remove"]) => {
            let n = name();
            w.friends.retain(|(f, _)| *f != n);
            w.asked.retain(|f| *f != n);
            w.asking.retain(|f| *f != n);
        }
        ("POST", ["v1", "invites"]) => return ok(json!({ "code": "fake-invite-code" })),
        ("POST", ["v1", "invites", "redeem"]) => return ok(json!({ "name": "Dorte" })),
        ("POST", ["v1", "blocks"]) => {
            let n = name();
            w.friends.retain(|(f, _)| *f != n);
            w.asked.retain(|f| *f != n);
            w.blocked.push(n);
        }
        ("POST", ["v1", "blocks", "remove"]) => {
            let n = name();
            w.blocked.retain(|f| *f != n);
        }
        ("POST", ["v1", "reports"]) => {
            let entry = (name(), body["reason"].as_str().unwrap_or_default().to_string());
            w.reports.push(entry);
        }
        ("POST", ["v1", "clubs"]) => return ok(json!({ "id": "club-2", "code": "fake-club-code" })),
        ("POST", ["v1", "clubs", "join"]) => return ok(json!({ "name": "Skakklubben" })),
        ("POST", ["v1", "clubs", "leave"]) => {}
        ("POST", ["v1", "games"]) => {
            let id = format!("g{}", w.next);
            w.next += 1;
            let first = body["side"] != "second";
            w.games.push(Game {
                id: id.clone(),
                opponent: name_of(&body, "opponent"),
                first,
                challenger: true,
                variant: name_of(&body, "variant"),
                status: "offered".into(),
                result: String::new(),
                reason: String::new(),
                moves: vec![],
                initial: None,
                draw_offered: false,
                friend: false,
                chat_mine: false,
                chat_theirs: false,
                keep_mine: false,
                lines: vec![],
            });
            return ok(json!({ "id": id }));
        }
        ("POST", ["v1", "games", id, what]) => {
            let Some(g) = w.games.iter_mut().find(|g| g.id == *id) else { return err(404, "no_game") };
            match *what {
                "accept" => {
                    g.status = "playing".into();
                    answer(g);
                }
                "decline" => g.status = "declined".into(),
                "cancel" => g.status = "cancelled".into(),
                "resign" => {
                    g.status = "over".into();
                    g.result = if g.first { "0-1" } else { "1-0" }.into();
                    g.reason = "resign".into();
                }
                "draw" => g.draw_offered = false,
                "chat" if body["on"] == true => {
                    g.chat_mine = true;
                    if agree {
                        g.chat_theirs = true;
                        g.lines.push((false, "Good luck, have fun!".into()));
                    }
                }
                "chat" => (g.chat_mine, g.chat_theirs) = (false, false),
                "keep" => g.keep_mine = body["on"] == true,
                "say" if g.chat_mine && g.chat_theirs => g.lines.push((true, name_of(&body, "text"))),
                "say" => return err(409, "chat_closed"),
                // The referee's throw: the first the rules allow.
                "throw" => {
                    let Ok(st) = bragd_games::replay(&g.variant, g.initial.as_deref(), &g.moves) else { return err(500, "server") };
                    let Some(t) = st.throws.first() else { return err(409, "not_now") };
                    g.moves.push(t.clone());
                }
                "move" => {
                    let entry = body["entry"].as_str().unwrap_or_default();
                    match bragd_games::apply(&g.variant, g.initial.as_deref(), &g.moves, entry) {
                        Ok(_) => {
                            g.moves.push(entry.to_string());
                            answer(g);
                        }
                        Err(_) => return err(400, "illegal"),
                    }
                }
                _ => return err(404, "not_found"),
            }
        }
        _ => return err(404, "not_found"),
    }
    ok(json!({}))
}

fn name_of(body: &Value, field: &str) -> String {
    body[field].as_str().unwrap_or_default().to_string()
}

/// The opponent's move when it is theirs: the first legal one.
fn answer(g: &mut Game) {
    let Ok(st) = bragd_games::replay(&g.variant, g.initial.as_deref(), &g.moves) else { return };
    let mine = if g.first { 0 } else { 1 };
    if let (Some(seat), Some(mv)) = (st.to_move, st.moves.first())
        && seat.index != mine
    {
        g.moves.push(mv.wire.clone());
    }
}
