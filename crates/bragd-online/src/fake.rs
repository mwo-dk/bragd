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
