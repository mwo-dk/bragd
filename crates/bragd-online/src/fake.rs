//! A fake Lichess on 127.0.0.1, for tests and screenshots: never the real server. It answers
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
}

/// What the handler answers.
pub enum Reply {
    /// A status and a JSON body.
    Json(u16, String),
    /// An NDJSON stream: the lines [`Fake::push`] gives its path, until [`Fake::close`].
    Stream,
}

type Handler = dyn Fn(&Request, &Fake) -> Reply + Send + Sync;

/// The fake server. Clones share it.
#[derive(Clone)]
pub struct Fake {
    pub url: String,
    seen: Arc<Mutex<Vec<Request>>>,
    streams: Arc<Mutex<HashMap<String, Sender<Option<String>>>>>,
    /// Lines pushed to a path before a stream on it opened: sent first when it does.
    waiting: Arc<Mutex<HashMap<String, Vec<String>>>>,
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
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("a port on 127.0.0.1");
        let url = format!("http://{}", listener.local_addr().expect("its address"));
        let fake = Fake { url, seen: Arc::default(), streams: Arc::default(), waiting: Arc::default() };
        let handler: Arc<Handler> = Arc::new(handler);
        let f = fake.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let (f, handler) = (f.clone(), handler.clone());
                std::thread::spawn(move || f.serve(conn, &*handler));
            }
        });
        fake
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

    fn serve(&self, conn: TcpStream, handler: &Handler) {
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
    let (mut len, mut auth) = (0usize, None);
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
            _ => {}
        }
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body).ok()?;
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    Some(Request { method, path: path.to_string(), query: query.to_string(), body: String::from_utf8_lossy(&body).into_owned(), auth })
}

/// A Game on the fake Lichess.
#[derive(Default)]
struct FakeGame {
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
        serde_json::json!({
            "type": "gameState", "moves": self.moves.join(" "), "wtime": 597_000, "btime": 600_000, "winc": 5000, "binc": 5000,
            "status": self.status, "winner": self.winner, "wdraw": self.wdraw, "bdraw": self.bdraw, "wtakeback": false, "btakeback": false,
        })
        .to_string()
    }

    fn full(&self) -> String {
        let mut state: serde_json::Value = serde_json::from_str(&self.state()).unwrap_or_default();
        state["type"] = "gameState".into();
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

fn lichess_reply(r: &Request, fake: &Fake, game: &Mutex<Option<FakeGame>>) -> Reply {
    let ok = || Reply::Json(200, r#"{"ok":true}"#.into());
    if r.path == "/api/token" && r.method == "POST" {
        return Reply::Json(200, format!(r#"{{"token_type":"Bearer","access_token":"{TOKEN}","expires_in":31536000}}"#));
    }
    if r.auth.as_deref() != Some(&format!("Bearer {TOKEN}")) {
        return Reply::Json(401, r#"{"error":"No such token"}"#.into());
    }
    let start = |id: &str, rated: bool| {
        let g = FakeGame { id: id.into(), rated, status: "started".into(), replies: vec!["e7e5", "b8c6", "g8f6", "f8c5"], ..Default::default() };
        fake.push(&format!("/api/board/game/stream/{id}"), &g.full());
        *lock(game) = Some(g);
        fake.push("/api/stream/event", &format!(r#"{{"type":"gameStart","game":{{"gameId":"{id}","id":"{id}","color":"white"}}}}"#));
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
