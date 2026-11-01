//! A fake Lichess, Lishogi and OGS on 127.0.0.1, for tests and screenshots: never the real
//! servers. OGS's realtime socket is served on the same port, as OGS does. It answers
//! each request through a handler, and streams NDJSON as Lichess does: chunked, one JSON object
//! a line, with an empty line now and then to keep the connection open.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A request the fake was sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    /// The path without its query.
    pub path: String,
    pub query: String,
    pub body: String,
    /// The `Authorization` header, if any.
    pub auth: Option<String>,
    /// `Sec-WebSocket-Key`: the request opens a socket.
    pub ws_key: Option<String>,
}

/// What the handler answers.
pub enum Reply {
    /// A status and a JSON body.
    Json(u16, String),
    /// An NDJSON stream: the lines [`Fake::push`] gives its path, until [`Fake::close`].
    Stream,
    /// A WebSocket: each text the client sends goes to the socket handler, and
    /// [`Fake::tell`] sends to every socket open.
    Socket,
}

type Handler = dyn Fn(&Request, &Fake) -> Reply + Send + Sync;
type SocketHandler = dyn Fn(&str, &Fake) + Send + Sync;

/// The fake server. Clones share it.
#[derive(Clone)]
pub struct Fake {
    pub url: String,
    seen: Arc<Mutex<Vec<Request>>>,
    streams: Arc<Mutex<HashMap<String, Sender<Option<String>>>>>,
    /// Lines pushed to a path before a stream on it opened: sent first when it does.
    waiting: Arc<Mutex<HashMap<String, Vec<String>>>>,
    sockets: Arc<Mutex<Vec<Sender<String>>>>,
    /// Every text a socket was sent.
    said: Arc<Mutex<Vec<String>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Fake {
    /// Starts a fake on a free port of 127.0.0.1; it runs until the program ends.
    pub fn start(handler: impl Fn(&Request, &Fake) -> Reply + Send + Sync + 'static) -> Fake {
        Self::start_on(0, handler)
    }

    /// The same on `port` (0: any free one).
    pub fn start_on(port: u16, handler: impl Fn(&Request, &Fake) -> Reply + Send + Sync + 'static) -> Fake {
        Self::start_with_socket(port, handler, |_, _| {})
    }

    /// The same, with `socket` answering what a WebSocket client says.
    pub fn start_with_socket(port: u16, handler: impl Fn(&Request, &Fake) -> Reply + Send + Sync + 'static, socket: impl Fn(&str, &Fake) + Send + Sync + 'static) -> Fake {
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("a port on 127.0.0.1");
        let url = format!("http://{}", listener.local_addr().expect("its address"));
        let fake = Fake { url, seen: Arc::default(), streams: Arc::default(), waiting: Arc::default(), sockets: Arc::default(), said: Arc::default() };
        let handler: Arc<Handler> = Arc::new(handler);
        let socket: Arc<SocketHandler> = Arc::new(socket);
        let f = fake.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let (f, handler, socket) = (f.clone(), handler.clone(), socket.clone());
                std::thread::spawn(move || f.serve(conn, &*handler, &*socket));
            }
        });
        fake
    }

    /// Sends `text` to every socket open.
    pub fn tell(&self, text: &str) {
        lock(&self.sockets).retain(|tx| tx.send(text.to_string()).is_ok());
    }

    /// Every text the sockets were sent.
    pub fn said(&self) -> Vec<String> {
        lock(&self.said).clone()
    }

    /// Every request so far.
    pub fn seen(&self) -> Vec<Request> {
        lock(&self.seen).clone()
    }

    /// Sends `line` on the stream of `path`, or keeps it for when that stream opens.
    pub fn push(&self, path: &str, line: &str) {
        let sent = lock(&self.streams).get(path).is_some_and(|tx| tx.send(Some(line.to_string())).is_ok());
        if !sent {
            lock(&self.waiting).entry(path.to_string()).or_default().push(line.to_string());
        }
    }

    /// Ends the stream of `path`, as Lichess does when a seek is taken or a Game ends.
    pub fn close(&self, path: &str) {
        if let Some(tx) = lock(&self.streams).remove(path) {
            let _ = tx.send(None);
        }
    }

    /// Whether a stream on `path` is open now.
    pub fn streaming(&self, path: &str) -> bool {
        lock(&self.streams).contains_key(path)
    }

    fn serve(&self, conn: TcpStream, handler: &Handler, socket: &SocketHandler) {
        let Some(req) = read_request(&conn) else { return };
        lock(&self.seen).push(req.clone());
        let mut out = conn;
        match handler(&req, self) {
            Reply::Json(status, body) => {
                let _ = write!(
                    out,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
            Reply::Socket => {
                let key = req.ws_key.unwrap_or_default();
                let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
                if write!(out, "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n").is_err() {
                    return;
                }
                let _ = out.set_read_timeout(Some(Duration::from_millis(20)));
                let mut ws = tungstenite::WebSocket::from_raw_socket(out, tungstenite::protocol::Role::Server, None);
                let (tx, rx) = channel::<String>();
                lock(&self.sockets).push(tx);
                loop {
                    while let Ok(text) = rx.try_recv() {
                        if ws.send(tungstenite::Message::text(text)).is_err() {
                            return;
                        }
                    }
                    match ws.read() {
                        Ok(tungstenite::Message::Text(t)) => {
                            lock(&self.said).push(t.to_string());
                            socket(&t, self);
                        }
                        Ok(tungstenite::Message::Close(_)) => return,
                        Ok(_) => {}
                        Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                        Err(_) => return,
                    }
                }
            }
            Reply::Stream => {
                let (tx, rx) = channel();
                for line in lock(&self.waiting).remove(&req.path).unwrap_or_default() {
                    let _ = tx.send(Some(line));
                }
                lock(&self.streams).insert(req.path.clone(), tx);
                let _ = write!(out, "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n");
                stream(&mut out, &rx);
                let _ = out.write_all(b"0\r\n\r\n");
                drop(rx);
                // A stream the client left is no longer one to push to (a newer one on the same
                // path still takes the empty line).
                let mut streams = lock(&self.streams);
                if streams.get(&req.path).is_some_and(|tx| tx.send(Some(String::new())).is_err()) {
                    streams.remove(&req.path);
                }
            }
        }
    }
}

/// Writes each line as a chunk, and an empty line every 50 ms so a client that stopped
/// listening is noticed. Ends when the stream is closed or the client went away.
fn stream(out: &mut TcpStream, rx: &Receiver<Option<String>>) {
    loop {
        let line = match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Some(l)) => l,
            Ok(None) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => String::new(),
        };
        let data = format!("{line}\n");
        if write!(out, "{:x}\r\n{data}\r\n", data.len()).and_then(|_| out.flush()).is_err() {
            return;
        }
    }
}

fn read_request(conn: &TcpStream) -> Option<Request> {
    let mut r = BufReader::new(conn);
    let mut first = String::new();
    r.read_line(&mut first).ok()?;
    let mut parts = first.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let (mut len, mut auth, mut ws_key) = (0usize, None, None);
    loop {
        let mut h = String::new();
        r.read_line(&mut h).ok()?;
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        let (k, v) = h.split_once(':')?;
        match k.trim().to_ascii_lowercase().as_str() {
            "content-length" => len = v.trim().parse().ok()?,
            "authorization" => auth = Some(v.trim().to_string()),
            "sec-websocket-key" => ws_key = Some(v.trim().to_string()),
            _ => {}
        }
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body).ok()?;
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    Some(Request { method, path: path.to_string(), query: query.to_string(), body: String::from_utf8_lossy(&body).into_owned(), auth, ws_key })
}

/// A Game on the fake Lichess or Lishogi.
#[derive(Default)]
struct FakeGame {
    lishogi: bool,
    id: String,
    rated: bool,
    moves: Vec<String>,
    status: String,
    winner: Option<String>,
    wdraw: bool,
    bdraw: bool,
    replies: Vec<&'static str>,
}

impl FakeGame {
    fn state(&self) -> String {
        if self.lishogi {
            let winner = self.winner.as_deref().map(|w| if w == "white" { "sente" } else { "gote" });
            return serde_json::json!({
                "type": "gameState", "moves": self.moves.join(" "), "btime": 597_000, "wtime": 600_000, "binc": 0, "winc": 0, "byo": 30_000,
                "status": self.status, "winner": winner, "sdraw": self.wdraw, "gdraw": self.bdraw,
            })
            .to_string();
        }
        serde_json::json!({
            "type": "gameState", "moves": self.moves.join(" "), "wtime": 597_000, "btime": 600_000, "winc": 5000, "binc": 5000,
            "status": self.status, "winner": self.winner, "wdraw": self.wdraw, "bdraw": self.bdraw, "wtakeback": false, "btakeback": false,
        })
        .to_string()
    }

    fn full(&self) -> String {
        let mut state: serde_json::Value = serde_json::from_str(&self.state()).unwrap_or_default();
        state["type"] = "gameState".into();
        if self.lishogi {
            let (me, anna) = (serde_json::json!({"id": "bragdtester", "name": "BragdTester", "rating": 1500}), serde_json::json!({"id": "anna", "name": "Anna", "rating": 1712}));
            return serde_json::json!({
                "type": "gameFull", "id": self.id, "rated": self.rated, "variant": {"key": "standard", "name": "Standard"},
                "clock": {"initial": 600_000, "increment": 0, "byoyomi": 30_000, "periods": 1}, "speed": "classical",
                "sente": me, "white": me, "gote": anna, "black": anna,
                "initialSfen": "startpos", "initialFen": "startpos", "state": state,
            })
            .to_string();
        }
        serde_json::json!({
            "type": "gameFull", "id": self.id, "rated": self.rated, "variant": {"key": "standard", "name": "Standard"},
            "clock": {"initial": 600_000, "increment": 5000}, "speed": "rapid",
            "white": {"id": "bragdtester", "name": "BragdTester", "rating": 1500},
            "black": {"id": "anna", "name": "Anna", "rating": 1712, "title": null},
            "initialFen": "startpos", "state": state,
        })
        .to_string()
    }
}

/// The token the fake hands out and expects.
pub const TOKEN: &str = "lip_fake";

/// A fake Lichess with one account, `BragdTester`, one challenge waiting from Anna, and Anna
/// taking any seek or accepted challenge at once. She plays White's opponent: e5, Nc6, Nf6, Bc5,
/// as long as the moves last, and her chat line answers the user's. A move is not checked here.
pub fn lichess() -> Fake {
    let game: Arc<Mutex<Option<FakeGame>>> = Arc::default();
    Fake::start(move |r, fake| lichess_reply(r, fake, &game))
}

/// The same fake on `port`.
pub fn lichess_on(port: u16) -> Fake {
    let game: Arc<Mutex<Option<FakeGame>>> = Arc::default();
    Fake::start_on(port, move |r, fake| lichess_reply(r, fake, &game))
}

/// A fake Lishogi: the same account and Anna, in shogi. As Lishogi does, it has no list of
/// challenges: Anna's comes on the event stream as it opens. She answers 3c3d, 8c8d, 4a3b.
pub fn lishogi() -> Fake {
    lishogi_on(0)
}

/// The same fake on `port`.
pub fn lishogi_on(port: u16) -> Fake {
    let game: Arc<Mutex<Option<FakeGame>>> = Arc::default();
    let f = Fake::start_on(port, move |r, fake| lishogi_reply(r, fake, &game));
    f.push(
        "/api/stream/event",
        r#"{"type":"challenge","challenge":{"id":"c1anna","challenger":{"id":"anna","name":"Anna","rating":1712},"destUser":{"id":"bragdtester","name":"BragdTester","rating":1500},"variant":{"key":"standard","name":"Standard"},"rated":true,"speed":"classical","timeControl":{"type":"clock","limit":600,"increment":0,"byoyomi":30,"periods":1,"show":"10|30(1)"},"color":"random"}}"#,
    );
    f
}

fn lishogi_reply(r: &Request, fake: &Fake, game: &Mutex<Option<FakeGame>>) -> Reply {
    let path: Vec<&str> = r.path.trim_start_matches('/').split('/').collect();
    match (r.method.as_str(), path.as_slice()) {
        // Lishogi has neither: the challenges come on the event stream, and none takes back.
        ("GET", ["api", "challenge"]) | ("POST", ["api", "challenge", _, "cancel"]) | ("POST", ["api", "board", "game", _, "takeback", _]) => {
            Reply::Json(404, r#"{"error":"Not found"}"#.into())
        }
        _ => reply(r, fake, game, true),
    }
}

fn lichess_reply(r: &Request, fake: &Fake, game: &Mutex<Option<FakeGame>>) -> Reply {
    reply(r, fake, game, false)
}

fn reply(r: &Request, fake: &Fake, game: &Mutex<Option<FakeGame>>, lishogi: bool) -> Reply {
    let ok = || Reply::Json(200, r#"{"ok":true}"#.into());
    if r.path == "/api/token" && r.method == "POST" {
        return Reply::Json(200, format!(r#"{{"token_type":"Bearer","access_token":"{TOKEN}","expires_in":31536000}}"#));
    }
    if r.auth.as_deref() != Some(&format!("Bearer {TOKEN}")) {
        return Reply::Json(401, r#"{"error":"No such token"}"#.into());
    }
    let start = |id: &str, rated: bool| {
        let replies = if lishogi { vec!["3c3d", "8c8d", "4a3b"] } else { vec!["e7e5", "b8c6", "g8f6", "f8c5"] };
        let g = FakeGame { lishogi, id: id.into(), rated, status: "started".into(), replies, ..Default::default() };
        fake.push(&format!("/api/board/game/stream/{id}"), &g.full());
        *lock(game) = Some(g);
        let color = if lishogi { "sente" } else { "white" };
        fake.push("/api/stream/event", &format!(r#"{{"type":"gameStart","game":{{"gameId":"{id}","id":"{id}","color":"{color}"}}}}"#));
    };
    let push = |g: &FakeGame| fake.push(&format!("/api/board/game/stream/{}", g.id), &g.state());
    let path: Vec<&str> = r.path.trim_start_matches('/').split('/').collect();
    match (r.method.as_str(), path.as_slice()) {
        ("DELETE", ["api", "token"]) => Reply::Json(204, String::new()),
        ("GET", ["api", "account"]) => Reply::Json(200, r#"{"id":"bragdtester","username":"BragdTester"}"#.into()),
        ("GET", ["api", "challenge"]) => Reply::Json(
            200,
            r#"{"in":[{"id":"c1anna","challenger":{"id":"anna","name":"Anna","rating":1712},"destUser":{"id":"bragdtester","name":"BragdTester","rating":1500},
                "variant":{"key":"standard","name":"Standard"},"rated":true,"speed":"rapid","timeControl":{"type":"clock","limit":600,"increment":5,"show":"10+5"},"color":"random"}],"out":[]}"#
                .into(),
        ),
        ("GET", ["api", "stream", "event"]) | ("GET", ["api", "board", "game", "stream", _]) => Reply::Stream,
        ("POST", ["api", "board", "seek"]) => {
            let rated = r.body.contains("rated=true");
            start("g1seek", rated);
            // Taken at once: the seek's stream ends.
            Reply::Json(200, String::new())
        }
        ("POST", ["api", "challenge", id, "accept"]) => {
            start(id, true);
            ok()
        }
        ("POST", ["api", "challenge", _, "decline" | "cancel"]) => ok(),
        ("POST", ["api", "challenge", user]) => Reply::Json(
            200,
            format!(r#"{{"id":"c2out","challenger":{{"id":"bragdtester","name":"BragdTester"}},"destUser":{{"id":"{user}","name":"{user}"}},"variant":{{"key":"standard","name":"Standard"}},"rated":false,"speed":"rapid","timeControl":{{"type":"clock","show":"10+5"}},"color":"random"}}"#),
        ),
        ("POST", ["api", "board", "game", _, rest @ ..]) => {
            let mut g = lock(game);
            let Some(g) = g.as_mut() else { return Reply::Json(404, r#"{"error":"No such game"}"#.into()) };
            match rest {
                ["move", m] => {
                    g.moves.push(m.to_string());
                    if !g.replies.is_empty() {
                        let reply = g.replies.remove(0);
                        g.moves.push(reply.into());
                    }
                    push(g);
                }
                ["draw", "yes"] => {
                    g.wdraw = true;
                    push(g);
                }
                ["resign"] => {
                    g.status = "resign".into();
                    g.winner = Some("black".into());
                    push(g);
                }
                ["abort"] => {
                    g.status = "aborted".into();
                    push(g);
                }
                ["chat"] => {
                    let text = crate::oauth::decode(r.body.split('&').find_map(|p| p.strip_prefix("text=")).unwrap_or_default());
                    let stream = format!("/api/board/game/stream/{}", g.id);
                    fake.push(&stream, &serde_json::json!({"type": "chatLine", "room": "player", "username": "BragdTester", "text": text}).to_string());
                    fake.push(&stream, r#"{"type":"chatLine","room":"player","username":"Anna","text":"Good luck, have fun!"}"#);
                }
                _ => {}
            }
            ok()
        }
        ("GET", ["api", "board", "game", _, "chat"]) => Reply::Json(200, r#"[{"user":"lichess","text":"Warning: be kind."},{"user":"Anna","text":"Hi!"}]"#.into()),
        _ => Reply::Json(404, r#"{"error":"Not found"}"#.into()),
    }
}

/// The token the fake OGS hands out and expects.
pub const OGS_TOKEN: &str = "ogs_fake";

/// A Game on the fake OGS: 19×19, the user Black against Anna.
#[derive(Default)]
struct OgsGame {
    id: i64,
    ranked: bool,
    moves: Vec<(i64, i64)>,
    replies: Vec<(i64, i64)>,
    phase: String,
    winner: Option<i64>,
    outcome: String,
}

impl OgsGame {
    fn data(&self) -> String {
        let moves: Vec<_> = self.moves.iter().map(|(x, y)| serde_json::json!([x, y, 1000])).collect();
        let current = if self.moves.len().is_multiple_of(2) { 1001 } else { 2002 };
        let side = |t: u64| serde_json::json!({"thinking_time": t, "periods": 5, "period_time": 30});
        serde_json::json!({"game_id": self.id, "phase": self.phase, "width": 19, "height": 19, "rules": "chinese", "ranked": self.ranked, "moves": moves,
            "players": {"black": {"id": 1001, "username": "BragdTester", "ranking": 20.0}, "white": {"id": 2002, "username": "Anna", "ranking": 24.3}},
            "clock": {"current_player": current, "black_time": side(597), "white_time": side(600)},
            "winner": self.winner, "outcome": self.outcome})
        .to_string()
    }
}

/// A fake OGS with one account, `BragdTester` (id 1001), and Anna (2002), whose challenge
/// waits, who takes any open challenge as soon as it is kept alive, and who is too busy for a
/// challenge of the user's. She answers q16, c16, q4, then passes; a pass of the user's she
/// answers with a pass, and the score is counted.
pub fn ogs() -> Fake {
    ogs_on(0)
}

/// The same fake on `port`.
pub fn ogs_on(port: u16) -> Fake {
    let game: Arc<Mutex<Option<OgsGame>>> = Arc::default();
    let g = game.clone();
    Fake::start_with_socket(port, move |r, fake| ogs_reply(r, fake, &game), move |text, fake| ogs_socket(text, fake, &g))
}

fn ogs_start(fake: &Fake, game: &Mutex<Option<OgsGame>>, id: i64, ranked: bool) {
    *lock(game) = Some(OgsGame { id, ranked, phase: "play".into(), replies: vec![(15, 3), (2, 3), (15, 15)], ..Default::default() });
    fake.tell(&serde_json::json!(["notification", {"type": "gameStarted", "game_id": id}]).to_string());
}

fn ogs_reply(r: &Request, fake: &Fake, game: &Mutex<Option<OgsGame>>) -> Reply {
    let json = |s: &str| Reply::Json(200, s.into());
    match (r.method.as_str(), r.path.as_str()) {
        (_, "/") if r.ws_key.is_some() => return Reply::Socket,
        ("POST", "/oauth2/token/") => return Reply::Json(200, format!(r#"{{"access_token":"{OGS_TOKEN}","token_type":"Bearer","expires_in":2592000,"refresh_token":"r","scope":"read write"}}"#)),
        ("POST", "/oauth2/revoke_token/") => return json("{}"),
        _ => {}
    }
    if r.auth.as_deref() != Some(&format!("Bearer {OGS_TOKEN}")) {
        return Reply::Json(401, r#"{"detail":"Authentication credentials were not provided."}"#.into());
    }
    match (r.method.as_str(), r.path.as_str()) {
        ("GET", "/api/v1/ui/config") => json(r#"{"user":{"id":1001,"username":"BragdTester","anonymous":false},"user_jwt":"jwt_fake"}"#),
        ("GET", "/api/v1/me/challenges/") => json(
            r#"{"results":[{"id":55,"challenger":{"id":2002,"username":"Anna","ranking":24.3},"challenged":{"id":1001,"username":"BragdTester"},"game":{"width":19,"ranked":true,"rules":"japanese","time_control_parameters":{"system":"byoyomi","main_time":600,"period_time":30,"periods":5}}}]}"#,
        ),
        ("POST", "/api/v1/me/challenges/55/accept") => {
            ogs_start(fake, game, 777, true);
            json("{}")
        }
        ("DELETE", "/api/v1/me/challenges/55") | ("DELETE", "/api/v1/challenges/66/") | ("DELETE", "/api/v1/me/challenges/67") => Reply::Json(204, String::new()),
        ("POST", "/api/v1/challenges/") => json(r#"{"challenge":66,"game":778}"#),
        ("GET", "/api/v1/players/") if r.query.to_lowercase().contains("username=anna&") => json(r#"{"results":[{"id":2002,"username":"Anna","ranking":24.3}]}"#),
        ("GET", "/api/v1/players/") => json(r#"{"results":[]}"#),
        ("POST", "/api/v1/players/2002/challenge") => json(r#"{"challenge":67,"game":779}"#),
        _ => Reply::Json(404, r#"{"detail":"Not found."}"#.into()),
    }
}

fn seek_taken(game: &Mutex<Option<OgsGame>>) -> bool {
    lock(game).as_ref().is_some_and(|g| g.id == 778)
}

fn ogs_socket(text: &str, fake: &Fake, game: &Mutex<Option<OgsGame>>) {
    let Ok(serde_json::Value::Array(a)) = serde_json::from_str::<serde_json::Value>(text) else { return };
    let (cmd, d) = (a.first().and_then(|c| c.as_str()).unwrap_or_default(), a.get(1).cloned().unwrap_or_default());
    let tell = |event: String, data: serde_json::Value| fake.tell(&serde_json::json!([event, data]).to_string());
    match cmd {
        "challenge/keepalive" => match d["challenge_id"].as_i64() {
            Some(66) if !seek_taken(game) => {
                // The seek's ranked flag is not on the socket: the fake's seek is casual.
                ogs_start(fake, game, 778, false);
            }
            Some(67) => tell("notification".into(), serde_json::json!({"type": "gameOfferRejected", "game_id": 779, "message": "Anna is busy"})),
            _ => {}
        },
        "game/connect" => {
            let id = d["game_id"].as_i64().unwrap_or(0);
            if let Some(g) = lock(game).as_ref().filter(|g| g.id == id) {
                tell(format!("game/{id}/gamedata"), serde_json::from_str(&g.data()).unwrap_or_default());
            }
            if d["chat"].as_bool() == Some(true) {
                tell(format!("game/{id}/chat"), serde_json::json!({"channel": "main", "line": {"username": "Anna", "body": "Hi!"}}));
            }
        }
        "game/move" => {
            let mut g = lock(game);
            let Some(g) = g.as_mut() else { return };
            let m = d["move"].as_str().unwrap_or("..").as_bytes().to_vec();
            let xy = |c: u8| if c == b'.' { -1 } else { (c - b'a') as i64 };
            let play = |g: &mut OgsGame, mv: (i64, i64)| {
                g.moves.push(mv);
                tell(format!("game/{}/move", g.id), serde_json::json!({"game_id": g.id, "move_number": g.moves.len(), "move": [mv.0, mv.1, 1000]}));
            };
            let mine = (xy(m[0]), xy(m[1]));
            play(g, mine);
            let reply = if mine.0 < 0 || g.replies.is_empty() { (-1, -1) } else { g.replies.remove(0) };
            play(g, reply);
            if mine.0 < 0 {
                g.phase = "stone removal".into();
                tell(format!("game/{}/phase", g.id), "stone removal".into());
                tell(format!("game/{}/removed_stones", g.id), serde_json::json!({"removed": true, "stones": "cd", "all_removed": "cd"}));
            }
        }
        "game/removed_stones/accept" => {
            if let Some(g) = lock(game).as_mut() {
                g.phase = "finished".into();
                tell(format!("game/{}/removed_stones_accepted", g.id), serde_json::json!({"player_id": 1001, "stones": "cd", "phase": "finished", "winner": 1001, "outcome": "7.5 points"}));
            }
        }
        "game/resign" => {
            if let Some(g) = lock(game).as_mut() {
                (g.phase, g.winner, g.outcome) = ("finished".into(), Some(2002), "Resignation".into());
                tell(format!("game/{}/gamedata", g.id), serde_json::from_str(&g.data()).unwrap_or_default());
            }
        }
        "game/chat" => {
            let id = d["game_id"].as_i64().unwrap_or(0);
            let body = d["body"].as_str().unwrap_or_default();
            tell(format!("game/{id}/chat"), serde_json::json!({"line": {"username": "BragdTester", "body": body}}));
            tell(format!("game/{id}/chat"), serde_json::json!({"line": {"username": "Anna", "body": "Good luck, have fun!"}}));
        }
        _ => {}
    }
}
