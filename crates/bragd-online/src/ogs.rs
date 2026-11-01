//! OGS (online-go.com): its REST API for the account and challenges, and its realtime socket
//! for the Games. The socket carries JSON arrays, `[command, data]` from Bragd and
//! `[event, data]` back, with a Game's events named after it: `game/123/move`. Bragd's go rules
//! check each move before it is sent; OGS keeps the clocks and counts the score.
//!
//! OGS's points are two letters from the top-left corner (`dd`), Bragd's a column letter
//! without `i` and a row from the bottom (`d16` on 19×19); a pass is `..` there and `pass` here.

use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bragd_core::t;
use rules_go::Point;
use serde_json::{Value, json};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use crate::board::{self, Time};
use crate::oauth::{self, Oauth};
use crate::server::{OGS, Server};
use crate::session::{ChallengeView, ChatLine, GameView, SERVICE, View};
use crate::{Can, Play};

/// Bragd's name with OGS. OGS knows its clients beforehand: the owner registers Bragd at
/// online-go.com/oauth2/applications/ as a public client with [`REDIRECT_PORT`]'s redirect and
/// puts its id here (#235). Until then signing in says so, and nothing is sent.
pub const CLIENT_ID: &str = "";
/// The redirect OGS was told: `http://127.0.0.1:47265/`.
pub const REDIRECT_PORT: u16 = 47265;
/// Bragd's id for the tests' fake, which takes any.
const FAKE_CLIENT_ID: &str = "bragd-fake";

const OAUTH: Oauth = Oauth { authorize: "/oauth2/authorize/", token: "/oauth2/token/", client_id: CLIENT_ID, scopes: "read write" };

/// Bragd's Variants and their boards.
pub const VARIANTS: [(&str, u32); 3] = [("go-9", 9), ("go-13", 13), ("go-19", 19)];
/// Byoyomi with five periods, as OGS's live Games mostly are.
pub const TIMES: [&str; 4] = ["5|5x30", "10|5x30", "20|5x30", "30|5x30"];
/// How long the browser has for the sign-in.
const SIGN_IN: Duration = Duration::from_secs(300);

/// Bragd's entry for OGS's point `x`, `y` on a board `size` wide: `pass` for -1.
pub fn entry(x: i64, y: i64, size: u32) -> Option<String> {
    if x < 0 || y < 0 {
        return Some("pass".into());
    }
    let (x, y, size) = (u8::try_from(x).ok()?, u8::try_from(y).ok()?, u8::try_from(size).ok()?);
    if x >= size || y >= size {
        return None;
    }
    Some(Point::new(x, size - 1 - y)?.name())
}

/// OGS's move for Bragd's `entry`: two letters, `..` for a pass.
pub fn ogs_move(entry: &str, size: u32) -> Option<String> {
    if entry == "pass" {
        return Some("..".into());
    }
    let p = Point::parse(entry)?;
    let size = u8::try_from(size).ok()?;
    if p.column() >= size || p.row() >= size {
        return None;
    }
    let letter = |n: u8| (b'a' + n) as char;
    Some(format!("{}{}", letter(p.column()), letter(size - 1 - p.row())))
}

/// OGS's stone list (`aadd`), as Bragd's points.
fn points(stones: &str, size: u32) -> Vec<String> {
    let b = stones.as_bytes();
    b.as_chunks::<2>().0.iter().filter_map(|c| entry((c[0] as i64) - b'a' as i64, (c[1] as i64) - b'a' as i64, size)).collect()
}

/// A time control as OGS sends it, worded: any of its systems.
pub fn time_label(tc: &Value) -> String {
    let n = |k: &str| tc[k].as_f64().unwrap_or(0.0).round() as u64;
    let m = |k: &str| n(k) / 60;
    match tc["system"].as_str().or(tc["time_control"].as_str()).unwrap_or("") {
        "byoyomi" => t!("online.minutes_periods", "m" => m("main_time"), "n" => n("periods"), "s" => n("period_time")),
        "fischer" => t!("online.minutes_plus", "m" => m("initial_time"), "s" => n("time_increment")),
        "canadian" => t!("online.tc.canadian", "m" => m("main_time"), "n" => n("stones_per_period"), "p" => m("period_time")),
        "simple" => t!("online.tc.simple", "s" => n("per_move")),
        "absolute" => t!("online.tc.absolute", "m" => m("total_time")),
        _ => t!("online.tc.none"),
    }
}

/// A Player as the Game names them.
#[derive(Clone, Debug, Default, PartialEq)]
struct Player {
    id: i64,
    name: String,
    /// Kyu and dan as OGS ranks them: 30k is 0, 1d is 30.
    ranking: Option<f64>,
    bot: bool,
}

fn rank(r: f64) -> String {
    let r = r.floor() as i64;
    if r >= 30 { format!("{}d", r - 29) } else { format!("{}k", 30 - r) }
}

impl Player {
    fn of(v: &Value) -> Player {
        Player {
            id: v["id"].as_i64().unwrap_or(0),
            name: v["username"].as_str().unwrap_or_default().to_string(),
            ranking: v["ranking"].as_f64(),
            bot: v["ui_class"].as_str().is_some_and(|c| c.split_whitespace().any(|w| w == "bot")),
        }
    }

    fn named(&self) -> String {
        match self.ranking {
            Some(r) => format!("{} ({})", self.name, rank(r)),
            None => self.name.clone(),
        }
    }
}

/// A Game being played, as OGS has told it.
#[derive(Clone, Debug, Default)]
struct Game {
    id: i64,
    size: u32,
    ranked: bool,
    /// Black, then White: Black moves first and is seat 0.
    players: [Player; 2],
    seat: Option<u8>,
    entries: Vec<String>,
    /// `play`, `stone removal` or `finished`.
    phase: String,
    /// The stones OGS marks dead while the score is counted.
    removed: String,
    winner: Option<i64>,
    outcome: String,
    /// Black's and White's time left in milliseconds, and the byoyomi period in seconds.
    clocks: [u64; 2],
    period: u64,
    /// Black's and White's byoyomi periods left.
    periods: [u64; 2],
    /// The seat whose clock runs, and since when.
    running: Option<u8>,
    since: Option<Instant>,
}

impl Game {
    fn of(d: &Value, me: i64) -> Game {
        let size = d["width"].as_u64().unwrap_or(19) as u32;
        let player = |k: &str| {
            let mut p = Player::of(&d["players"][k]);
            if p.id == 0 {
                p.id = d[&format!("{k}_player_id")].as_i64().unwrap_or(0);
            }
            p
        };
        let players = [player("black"), player("white")];
        let seat = players.iter().position(|p| p.id == me && me != 0).map(|s| s as u8);
        let entries = d["moves"].as_array().map(|m| m.iter().filter_map(|mv| entry(mv[0].as_i64()?, mv[1].as_i64()?, size)).collect()).unwrap_or_default();
        let mut g = Game {
            id: d["game_id"].as_i64().unwrap_or(0),
            size,
            ranked: d["ranked"].as_bool().unwrap_or(false),
            players,
            seat,
            entries,
            phase: d["phase"].as_str().unwrap_or("play").to_string(),
            removed: d["removed"].as_str().unwrap_or_default().to_string(),
            winner: d["winner"].as_i64(),
            outcome: d["outcome"].as_str().unwrap_or_default().to_string(),
            ..Default::default()
        };
        g.clock(&d["clock"]);
        g
    }

    /// A clock message: each side's thinking time and period, and who is to move.
    fn clock(&mut self, c: &Value) {
        if !c.is_object() {
            return;
        }
        let left = |k: &str| {
            let t = &c[k];
            let secs = t["thinking_time"].as_f64().or(t.as_f64()).unwrap_or(0.0);
            (secs * 1000.0) as u64
        };
        self.clocks = [left("black_time"), left("white_time")];
        self.period = c["black_time"]["period_time"].as_f64().unwrap_or(0.0) as u64;
        self.periods = [c["black_time"]["periods"].as_u64().unwrap_or(0), c["white_time"]["periods"].as_u64().unwrap_or(0)];
        let current = c["current_player"].as_i64();
        self.running = self.players.iter().position(|p| Some(p.id) == current).map(|s| s as u8);
        self.since = Some(Instant::now());
    }

    fn over(&self) -> bool {
        self.phase == "finished"
    }

    fn variant(&self) -> Option<&'static str> {
        VARIANTS.iter().find(|(_, s)| *s == self.size).map(|(v, _)| *v)
    }

    fn mine(&self) -> bool {
        self.phase == "play" && self.seat.is_some_and(|s| self.entries.len() % 2 == s as usize)
    }

    fn view(&self, base: &str) -> GameView {
        let variant = self.variant();
        let running = if self.phase == "play" { self.running } else { None };
        let mut clocks = self.clocks;
        if let (Some(s), Some(since)) = (running, self.since) {
            clocks[s as usize] = clocks[s as usize].saturating_sub(since.elapsed().as_millis() as u64);
        }
        let opponent = &self.players[if self.seat == Some(0) { 1 } else { 0 }];
        GameView {
            id: self.id.to_string(),
            url: format!("{base}/game/{}", self.id),
            variant,
            variant_name: format!("{0}×{0}", self.size),
            rated: self.ranked,
            against_person: !opponent.bot,
            names: [self.players[0].named(), self.players[1].named()],
            seat: self.seat,
            clocks,
            byoyomi: self.period,
            periods: self.periods,
            running,
            over: self.over(),
            result: self.result(),
            offered: (false, false),
            offering: (false, false),
            opponent_gone: false,
            state: variant.and_then(|v| bragd_games::replay(v, None, &self.entries).ok()),
            initial: None,
            entries: self.entries.clone(),
            dead: (self.phase == "stone removal").then(|| points(&self.removed, self.size)),
        }
    }

    fn result(&self) -> String {
        if !self.over() {
            return String::new();
        }
        let why = if self.outcome.is_empty() { t!("online.status.other") } else { self.outcome.clone() };
        match self.players.iter().position(|p| Some(p.id) == self.winner) {
            Some(0) => format!("{} · {why}", t!("play.wins", "side" => t!("side.black"))),
            Some(_) => format!("{} · {why}", t!("play.wins", "side" => t!("side.white"))),
            None => why,
        }
    }
}

/// A challenge as OGS lists or announces it.
#[derive(Clone, Debug, PartialEq)]
struct Challenge {
    id: i64,
    /// The Game it starts, when Bragd knows it.
    game: i64,
    incoming: bool,
    who: String,
    size: u32,
    ranked: bool,
    time: String,
}

type Ws = WebSocket<MaybeTlsStream<TcpStream>>;

/// The realtime socket's outbox: the thread that reads the socket also writes what is put here,
/// between reads.
struct Socket {
    out: Mutex<Sender<String>>,
}

impl Socket {
    fn send(&self, command: &str, data: Value) -> Result<(), String> {
        let text = json!([command, data]).to_string();
        lock(&self.out).send(text).map_err(|_| t!("online.error.lost", "server" => OGS.name, "why" => "-"))
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// OGS's REST API with the user's token.
struct Rest {
    base: String,
    token: String,
    agent: ureq::Agent,
}

impl Rest {
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn bearer(&self) -> String {
        format!("Bearer {}", self.token)
    }

    fn get(&self, path: &str) -> Result<Value, String> {
        board::read(&OGS, self.agent.get(self.url(path)).header("Authorization", self.bearer()).header("Accept", "application/json").call())
    }

    fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        board::read(&OGS, self.agent.post(self.url(path)).header("Authorization", self.bearer()).send_json(body))
    }

    fn delete(&self, path: &str) -> Result<Value, String> {
        board::read(&OGS, self.agent.delete(self.url(path)).header("Authorization", self.bearer()).call())
    }
}

#[derive(Default)]
struct Inner {
    rest: Option<Arc<Rest>>,
    socket: Option<Arc<Socket>>,
    /// The user's id and name.
    me: Option<(i64, String)>,
    signing_in: Option<Arc<AtomicBool>>,
    /// The socket's stop switch.
    events: Option<Arc<AtomicBool>>,
    /// The open challenge waiting for an opponent: its id and its keep-alive's stop switch.
    seek: Option<(i64, Arc<AtomicBool>)>,
    challenges: Vec<Challenge>,
    game: Option<Game>,
    chat_on: bool,
    chat: Vec<ChatLine>,
    error: Option<String>,
    tried: bool,
}

impl Inner {
    fn stop_all(&mut self) {
        for s in [self.events.take(), self.seek.take().map(|s| s.1)].into_iter().flatten() {
            s.store(true, Ordering::Relaxed);
        }
        self.socket = None;
    }
}

/// OGS for this window. Clones share it.
#[derive(Clone)]
pub struct Ogs {
    base: String,
    client_id: &'static str,
    port: u16,
    inner: Arc<Mutex<Inner>>,
}

impl Ogs {
    /// OGS at `base`: its real address, or a fake one in tests (`client_id` and `port` then the
    /// fake's).
    pub fn new(base: &str) -> Ogs {
        let fake = board::on_this_machine(base);
        Ogs {
            base: base.trim_end_matches('/').to_string(),
            client_id: if fake && CLIENT_ID.is_empty() { FAKE_CLIENT_ID } else { CLIENT_ID },
            port: if fake { 0 } else { REDIRECT_PORT },
            inner: Arc::default(),
        }
    }

    /// OGS at the address Bragd plays at.
    pub fn of() -> Ogs {
        Ogs::new(&OGS.base())
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        lock(&self.inner)
    }

    fn fail(&self, e: String) -> String {
        let mut i = self.lock();
        if e == t!("online.error.signed_out", "server" => OGS.name) {
            let _ = bragd_model::key::set_in(SERVICE, &self.base, "");
            i.stop_all();
            i.rest = None;
            i.me = None;
        }
        i.error = Some(e.clone());
        e
    }

    fn rest(&self) -> Result<Arc<Rest>, String> {
        self.lock().rest.clone().ok_or_else(|| t!("online.error.not_signed_in", "server" => OGS.name))
    }

    fn socket(&self) -> Result<Arc<Socket>, String> {
        self.lock().socket.clone().ok_or_else(|| t!("online.error.not_signed_in", "server" => OGS.name))
    }

    fn connect(&self, token: &str) -> Result<(), String> {
        if bragd_core::offline() && !board::on_this_machine(&self.base) {
            return Err(self.fail(t!("online.error.offline", "server" => OGS.name)));
        }
        let rest = Arc::new(Rest { base: self.base.clone(), token: token.to_string(), agent: board::agent() });
        // The page's settings give the socket's key for this account.
        let cfg = rest.get("/api/v1/ui/config").map_err(|e| self.fail(e))?;
        let (id, name) = (cfg["user"]["id"].as_i64().unwrap_or(0), cfg["user"]["username"].as_str().unwrap_or_default().to_string());
        let jwt = cfg["user_jwt"].as_str().unwrap_or_default().to_string();
        if id <= 0 || jwt.is_empty() || cfg["user"]["anonymous"].as_bool() == Some(true) {
            return Err(self.fail(t!("online.error.signed_out", "server" => OGS.name)));
        }
        let ws = self.dial().map_err(|e| self.fail(e))?;
        let (tx, outbox) = channel();
        let socket = Arc::new(Socket { out: Mutex::new(tx) });
        let device: String = format!("bragd-{}", std::process::id());
        socket.send("authenticate", json!({"jwt": jwt, "device_id": device, "client": "bragd", "client_version": env!("CARGO_PKG_VERSION")})).map_err(|e| self.fail(e))?;
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut i = self.lock();
            i.rest = Some(rest.clone());
            i.socket = Some(socket.clone());
            i.me = Some((id, name));
            i.events = Some(stop.clone());
            i.error = None;
        }
        let this = self.clone();
        std::thread::spawn(move || this.listen(ws, &outbox, &socket, &stop));
        self.refresh(&rest).map_err(|e| self.fail(e))
    }

    /// The socket at OGS's address: `wss://` for `https://`.
    fn dial(&self) -> Result<Ws, String> {
        let url = if let Some(rest) = self.base.strip_prefix("https://") { format!("wss://{rest}/") } else { format!("ws://{}/", self.base.trim_start_matches("http://")) };
        let lost = |e: &dyn std::fmt::Display| t!("online.error.unreachable", "server" => OGS.name, "why" => e);
        let (mut ws, _) = tungstenite::connect(url.as_str()).map_err(|e| lost(&e))?;
        // The reader waits this long at most, so a move to send never waits longer.
        let timeout = Some(Duration::from_millis(100));
        match ws.get_mut() {
            MaybeTlsStream::Plain(s) => s.set_read_timeout(timeout),
            MaybeTlsStream::Rustls(s) => s.get_mut().set_read_timeout(timeout),
            _ => Ok(()),
        }
        .map_err(|e| lost(&e))?;
        Ok(ws)
    }

    /// Reads the socket until `stop`, writes its outbox between reads, and pings it every ten
    /// seconds as OGS asks.
    fn listen(&self, mut ws: Ws, outbox: &Receiver<String>, socket: &Socket, stop: &AtomicBool) {
        let mut pinged = Instant::now();
        let error = 'run: loop {
            if stop.load(Ordering::Relaxed) {
                let _ = ws.close(None);
                return;
            }
            while let Ok(text) = outbox.try_recv() {
                if let Err(e) = ws.send(Message::text(text)) {
                    break 'run t!("online.error.lost", "server" => OGS.name, "why" => e);
                }
            }
            if pinged.elapsed() >= Duration::from_secs(10) {
                pinged = Instant::now();
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
                if let Err(e) = socket.send("net/ping", json!({"client": now, "drift": 0, "latency": 0})) {
                    break e;
                }
            }
            match ws.read() {
                Ok(Message::Text(text)) => {
                    if let Ok(Value::Array(a)) = serde_json::from_str::<Value>(&text)
                        && let (Some(event), Some(data)) = (a.first().and_then(Value::as_str), a.get(1))
                    {
                        self.event(event, data);
                    }
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(e) => break t!("online.error.lost", "server" => OGS.name, "why" => e),
            }
        };
        if !stop.load(Ordering::Relaxed) {
            let mut i = self.lock();
            i.events = None;
            i.socket = None;
            i.error = Some(error);
        }
    }

    /// The challenges made to the user, as OGS lists them.
    fn refresh(&self, rest: &Rest) -> Result<(), String> {
        let me = self.lock().me.as_ref().map_or(0, |m| m.0);
        let v = rest.get("/api/v1/me/challenges/")?;
        let list: Vec<Challenge> = v["results"]
            .as_array()
            .map(|a| a.iter().filter(|c| c["challenger"]["id"].as_i64() != Some(me)).filter_map(|c| challenge(c, &c["challenger"], &c["game"])).collect())
            .unwrap_or_default();
        let mut i = self.lock();
        i.challenges.retain(|c| !c.incoming);
        i.challenges.extend(list);
        Ok(())
    }

    fn event(&self, event: &str, data: &Value) {
        if let Some(rest) = event.strip_prefix("game/") {
            if let Some((id, what)) = rest.split_once('/')
                && let Ok(id) = id.parse::<i64>()
            {
                self.game_event(id, what, data);
            }
            return;
        }
        match (event, data["type"].as_str()) {
            ("notification", Some("challenge")) => {
                if let Some(c) = challenge(data, &data["user"], data) {
                    let mut i = self.lock();
                    i.challenges.retain(|x| x.id != c.id);
                    i.challenges.push(c);
                }
            }
            ("notification", Some("gameStarted" | "yourMove")) | ("active_game", _) => {
                let id = data["game_id"].as_i64().or(data["id"].as_i64()).unwrap_or(0);
                let finished = data["phase"].as_str() == Some("finished");
                if id > 0 && !finished {
                    self.watch(id);
                }
            }
            ("notification", Some("gameOfferRejected")) => {
                let mut i = self.lock();
                let game = data["game_id"].as_i64();
                i.challenges.retain(|c| c.incoming || Some(c.game) != game);
                let why = data["message"].as_str().unwrap_or_default();
                i.error = Some(t!("online.error.declined", "server" => OGS.name, "why" => why));
            }
            _ => {}
        }
    }

    /// Follows the Game `id` in place of any other, from its full state on.
    fn watch(&self, id: i64) {
        let (socket, old, chat) = {
            let mut i = self.lock();
            if i.game.as_ref().is_some_and(|g| g.id == id) {
                return;
            }
            if let Some((_, s)) = i.seek.take() {
                s.store(true, Ordering::Relaxed);
            }
            let old = i.game.take().map(|g| g.id);
            i.challenges.retain(|c| c.game != id);
            i.game = Some(Game { id, phase: "play".into(), ..Default::default() });
            i.chat.clear();
            (i.socket.clone(), old, i.chat_on)
        };
        let Some(socket) = socket else { return };
        if let Some(old) = old {
            let _ = socket.send("game/disconnect", json!({"game_id": old}));
        }
        let _ = socket.send("game/connect", json!({"game_id": id, "chat": chat}));
    }

    fn game_event(&self, id: i64, what: &str, d: &Value) {
        let mut i = self.lock();
        let me = i.me.as_ref().map_or(0, |m| m.0);
        let chat_on = i.chat_on;
        let Some(g) = i.game.as_mut().filter(|g| g.id == id) else { return };
        match what {
            "gamedata" => *g = Game::of(d, me),
            "move" => {
                let mv = &d["move"];
                let n = d["move_number"].as_u64().map(|n| n as usize);
                if let Some(e) = entry(mv[0].as_i64().unwrap_or(-1), mv[1].as_i64().unwrap_or(-1), g.size) {
                    // A move already had (the full state came after it) is not added twice.
                    if n.is_none_or(|n| n == g.entries.len() + 1) {
                        g.entries.push(e);
                    }
                }
            }
            "clock" => g.clock(d),
            "phase" => g.phase = d.as_str().unwrap_or("play").to_string(),
            "removed_stones" => g.removed = d["all_removed"].as_str().unwrap_or_default().to_string(),
            "removed_stones_accepted" if d["phase"].as_str() == Some("finished") => {
                g.phase = "finished".into();
                g.winner = d["winner"].as_i64();
                g.outcome = d["outcome"].as_str().unwrap_or_default().to_string();
            }
            "chat" if chat_on => {
                let line = &d["line"];
                if let Some(text) = line["body"].as_str() {
                    i.chat.push(ChatLine { user: line["username"].as_str().unwrap_or_default().to_string(), text: text.to_string() });
                }
            }
            _ => {}
        }
    }

    /// The Game's body for a seek or a challenge: Chinese rules, as Bragd's go keeps, no
    /// handicap, OGS's komi, and byoyomi.
    fn body(&self, variant: &str, time: &str, rated: bool, color: &str) -> Result<Value, String> {
        let size = VARIANTS.iter().find(|(v, _)| *v == variant).map(|(_, s)| *s).ok_or_else(|| t!("online.error.variant", "site" => OGS.site))?;
        let t = Time::parse(time).filter(|_| TIMES.contains(&time)).ok_or_else(|| t!("online.error.time", "server" => OGS.name))?;
        let tc = json!({"system": "byoyomi", "time_control": "byoyomi", "speed": "live", "main_time": t.minutes * 60, "period_time": t.byoyomi, "periods": t.periods, "pause_on_weekends": false});
        Ok(json!({
            "min_ranking": -1000, "max_ranking": 1000, "challenger_color": color,
            "game": {"name": "Bragd", "rules": "chinese", "ranked": rated, "width": size, "height": size, "handicap": 0, "komi_auto": "automatic",
                "disable_analysis": false, "initial_state": null, "private": false, "time_control": "byoyomi", "time_control_parameters": tc, "pause_on_weekends": false},
        }))
    }

    /// Keeps the challenge `id` (of Game `game`) open while it waits, as OGS asks once a second,
    /// until `stop`, or until it has left the list.
    fn keep(&self, id: i64, game: i64, stop: Arc<AtomicBool>) {
        let this = self.clone();
        std::thread::spawn(move || {
            let waits = |i: &Inner| i.seek.as_ref().is_some_and(|s| s.0 == id) || i.challenges.iter().any(|c| c.id == id && !c.incoming);
            while !stop.load(Ordering::Relaxed) && waits(&this.lock()) {
                match this.socket() {
                    Ok(s) => {
                        let _ = s.send("challenge/keepalive", json!({"challenge_id": id, "game_id": game}));
                    }
                    Err(_) => return,
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    }

    fn game_id(&self) -> Result<(Arc<Socket>, i64), String> {
        let id = self.lock().game.as_ref().map(|g| g.id).ok_or_else(|| t!("online.error.no_game"))?;
        Ok((self.socket()?, id))
    }

}

/// A challenge from its fields: `who` made it, `game` holds the board and the time.
fn challenge(c: &Value, who: &Value, game: &Value) -> Option<Challenge> {
    let id = c["challenge_id"].as_i64().or(c["id"].as_i64())?;
    let tc = if game["time_control_parameters"].is_object() { &game["time_control_parameters"] } else { &game["time_control"] };
    Some(Challenge {
        id,
        game: c["game_id"].as_i64().unwrap_or(0),
        incoming: true,
        who: Player::of(who).named(),
        size: game["width"].as_u64().unwrap_or(19) as u32,
        ranked: game["ranked"].as_bool().unwrap_or(false),
        time: time_label(tc),
    })
}

impl Play for Ogs {
    fn server(&self) -> &'static Server {
        &OGS
    }

    fn variants(&self) -> Vec<&'static str> {
        VARIANTS.iter().map(|(v, _)| *v).collect()
    }

    fn times(&self) -> Vec<(String, String)> {
        TIMES.iter().filter_map(|t| Some((t.to_string(), Time::parse(t)?.label()))).collect()
    }

    fn colours(&self) -> [&'static str; 2] {
        ["black", "white"]
    }

    fn can(&self) -> Can {
        Can { takeback: false, cancel: true, abort: true, draw: false }
    }

    fn ready(&self) -> bool {
        !self.client_id.is_empty()
    }

    fn view(&self) -> View {
        let i = self.lock();
        let challenges = i
            .challenges
            .iter()
            .map(|c| ChallengeView {
                id: c.id.to_string(),
                incoming: c.incoming,
                who: c.who.clone(),
                variant: VARIANTS.iter().find(|(_, s)| *s == c.size).map(|(v, _)| *v),
                variant_name: format!("{0}×{0}", c.size),
                rated: c.ranked,
                time: c.time.clone(),
            })
            .collect();
        View {
            signed_in: i.me.as_ref().map(|m| m.1.clone()),
            signing_in: i.signing_in.is_some(),
            connected: i.events.is_some(),
            seeking: i.seek.is_some(),
            challenges,
            game: i.game.as_ref().filter(|g| g.size > 0).map(|g| g.view(&self.base)),
            chat: if i.chat_on { i.chat.clone() } else { vec![] },
            error: i.error.clone(),
        }
    }

    fn live(&self) -> Option<(bool, bool, bool)> {
        let i = self.lock();
        let g = i.game.as_ref().filter(|g| g.size > 0)?;
        let opponent = &g.players[if g.seat == Some(0) { 1 } else { 0 }];
        Some((g.ranked, !opponent.bot, g.over()))
    }

    fn open(&self) -> Result<(), String> {
        {
            let mut i = self.lock();
            if i.rest.is_some() || i.tried {
                return Ok(());
            }
            i.tried = true;
        }
        let Some(token) = OGS.token(&self.base) else { return Ok(()) };
        self.connect(&token)
    }

    fn reconnect(&self) -> Result<(), String> {
        {
            let mut i = self.lock();
            i.stop_all();
            i.rest = None;
            i.error = None;
            i.tried = false;
        }
        self.open()
    }

    fn sign_in(&self, open: &dyn Fn(&str) -> Result<(), String>) -> Result<(), String> {
        if self.client_id.is_empty() {
            return Err(self.fail(t!("online.error.not_registered", "server" => OGS.name)));
        }
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut i = self.lock();
            if i.signing_in.is_some() {
                return Ok(());
            }
            i.signing_in = Some(stop.clone());
            i.error = None;
        }
        let o = Oauth { client_id: self.client_id, ..OAUTH };
        let r = (|| {
            if bragd_core::offline() && !board::on_this_machine(&self.base) {
                return Err(t!("online.error.offline", "server" => OGS.name));
            }
            let pkce = oauth::Pkce::new();
            let lb = oauth::Loopback::bind(self.port)?;
            open(&oauth::authorize_url(&o, &self.base, &lb.redirect, &pkce))?;
            let code = lb.wait(OGS.name, &pkce.state, &stop, SIGN_IN)?;
            let token = oauth::exchange(&OGS, &o, &board::agent(), &self.base, &code, &pkce, &lb.redirect)?;
            bragd_model::key::set_in(SERVICE, &self.base, &token)?;
            Ok(token)
        })();
        self.lock().signing_in = None;
        match r {
            Ok(token) => self.connect(&token),
            Err(e) => Err(self.fail(e)),
        }
    }

    fn cancel_sign_in(&self) {
        if let Some(s) = self.lock().signing_in.take() {
            s.store(true, Ordering::Relaxed);
        }
    }

    fn sign_out(&self) -> Result<(), String> {
        let rest = {
            let mut i = self.lock();
            i.stop_all();
            let r = i.rest.take();
            *i = Inner { chat_on: i.chat_on, ..Inner::default() };
            r
        };
        if let Some(r) = rest {
            let form = [("token", r.token.as_str()), ("client_id", self.client_id)];
            let _ = board::read(&OGS, r.agent.post(r.url("/oauth2/revoke_token/")).send_form(form));
        }
        bragd_model::key::set_in(SERVICE, &self.base, "")
    }

    fn seek(&self, variant: &str, time: &str, rated: bool) -> Result<(), String> {
        let body = self.body(variant, time, rated, "automatic")?;
        let r = self.rest()?.post("/api/v1/challenges/", &body).map_err(|e| self.fail(e))?;
        let (id, game) = (r["challenge"].as_i64().unwrap_or(0), r["game"].as_i64().unwrap_or(0));
        let stop = Arc::new(AtomicBool::new(false));
        if let Some((_, old)) = self.lock().seek.replace((id, stop.clone())) {
            old.store(true, Ordering::Relaxed);
        }
        self.keep(id, game, stop);
        Ok(())
    }

    fn seek_cancel(&self) {
        let seek = self.lock().seek.take();
        if let Some((id, stop)) = seek {
            stop.store(true, Ordering::Relaxed);
            if let Ok(r) = self.rest() {
                let _ = r.delete(&format!("/api/v1/challenges/{id}/"));
            }
        }
    }

    fn challenge(&self, user: &str, variant: &str, time: &str, rated: bool, color: &str) -> Result<(), String> {
        let user = user.trim();
        if user.is_empty() || !user.chars().all(|c| c.is_alphanumeric() || "_-.".contains(c)) {
            return Err(t!("online.error.user", "server" => OGS.name));
        }
        let color = match color {
            "random" => "automatic",
            c if self.colours().contains(&c) => c,
            _ => return Err(t!("online.error.reply", "server" => OGS.name)),
        };
        let body = self.body(variant, time, rated, color)?;
        let rest = self.rest()?;
        let found = rest.get(&format!("/api/v1/players/?username={user}&page_size=1")).map_err(|e| self.fail(e))?;
        let player = found["results"].as_array().and_then(|a| a.iter().find(|p| p["username"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(user)))).cloned();
        let Some(player) = player else { return Err(t!("online.error.no_user", "server" => OGS.name, "user" => user)) };
        let pid = player["id"].as_i64().unwrap_or(0);
        let r = rest.post(&format!("/api/v1/players/{pid}/challenge"), &body).map_err(|e| self.fail(e))?;
        let (id, game) = (r["challenge"].as_i64().unwrap_or(0), r["game"].as_i64().unwrap_or(0));
        let stop = Arc::new(AtomicBool::new(false));
        let size = body["game"]["width"].as_u64().unwrap_or(19) as u32;
        let c = Challenge { id, game, incoming: false, who: Player::of(&player).named(), size, ranked: rated, time: time_label(&body["game"]["time_control_parameters"]) };
        self.lock().challenges.push(c);
        // Kept open until it is answered: the Game starting, or OGS saying no, takes it away.
        self.keep(id, game, stop);
        Ok(())
    }

    fn answer(&self, id: &str, what: &str) -> Result<(), String> {
        let n: i64 = id.parse().map_err(|_| t!("online.error.reply", "server" => OGS.name))?;
        let rest = self.rest()?;
        let r = match what {
            "accept" => rest.post(&format!("/api/v1/me/challenges/{n}/accept"), &json!({})),
            "decline" | "cancel" => rest.delete(&format!("/api/v1/me/challenges/{n}")),
            _ => return Err(t!("online.error.reply", "server" => OGS.name)),
        };
        let r = r.map_err(|e| self.fail(e))?;
        self.lock().challenges.retain(|c| c.id != n);
        if let Some(game) = r["game"].as_i64().or(r["game_id"].as_i64()) {
            self.watch(game);
        }
        Ok(())
    }

    fn play(&self, entry: &str) -> Result<(), String> {
        let (mv, id) = {
            let i = self.lock();
            let g = i.game.as_ref().filter(|g| g.size > 0).ok_or_else(|| t!("online.error.no_game"))?;
            if !g.mine() {
                return Err(t!("online.error.not_your_turn"));
            }
            let variant = g.variant().ok_or_else(|| t!("online.error.variant", "site" => OGS.site))?;
            bragd_games::apply(variant, None, &g.entries, entry).map_err(|e| e.to_string())?;
            (ogs_move(entry, g.size).ok_or_else(|| t!("online.error.not_your_turn"))?, g.id)
        };
        self.socket()?.send("game/move", json!({"game_id": id, "move": mv}))
    }

    fn draw(&self, _yes: bool) -> Result<(), String> {
        Err(t!("online.error.no_draw", "server" => OGS.name))
    }

    fn takeback(&self, _yes: bool) -> Result<(), String> {
        Err(t!("online.error.no_takeback", "server" => OGS.name))
    }

    fn resign(&self) -> Result<(), String> {
        let (s, id) = self.game_id()?;
        s.send("game/resign", json!({"game_id": id}))
    }

    fn abort(&self) -> Result<(), String> {
        let (s, id) = self.game_id()?;
        s.send("game/cancel", json!({"game_id": id}))
    }

    fn accept_score(&self) -> Result<(), String> {
        let (s, id) = self.game_id()?;
        let stones = self.lock().game.as_ref().map(|g| g.removed.clone()).unwrap_or_default();
        s.send("game/removed_stones/accept", json!({"game_id": id, "stones": stones, "strict_seki_mode": false}))
    }

    fn leave(&self) {
        let mut i = self.lock();
        if i.game.as_ref().is_some_and(Game::over) {
            let id = i.game.take().map(|g| g.id);
            i.chat.clear();
            if let (Some(s), Some(id)) = (i.socket.clone(), id) {
                let _ = s.send("game/disconnect", json!({"game_id": id}));
            }
        }
    }

    fn set_chat(&self, on: bool) -> Result<(), String> {
        let (socket, id) = {
            let mut i = self.lock();
            if i.chat_on == on {
                return Ok(());
            }
            i.chat_on = on;
            i.chat.clear();
            (i.socket.clone(), i.game.as_ref().map(|g| g.id))
        };
        match (socket, id) {
            // Connected again with chat, OGS sends the chat so far.
            (Some(s), Some(id)) if on => s.send("game/connect", json!({"game_id": id, "chat": true})),
            _ => Ok(()),
        }
    }

    fn say(&self, text: &str) -> Result<(), String> {
        let text = text.trim();
        if !self.lock().chat_on {
            return Err(t!("online.error.chat_off"));
        }
        if text.is_empty() {
            return Ok(());
        }
        let text: String = text.chars().take(140).collect();
        let (s, id) = self.game_id()?;
        let n = self.lock().game.as_ref().map_or(0, |g| g.entries.len());
        s.send("game/chat", json!({"game_id": id, "type": "main", "move_number": n, "body": text}))
    }
}

#[cfg(test)]
mod tests;
