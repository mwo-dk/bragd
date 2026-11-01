//! A Helper as a Peer: its key, the Peers it has paired with (`peers.json`), the pairing code
//! it shows, the connections it takes, and the ones it makes.
//!
//! Over TLS (`tls`) each side says one JSON line and the other answers one, at most
//! `LINE_MAX` long, all within `DEADLINE`:
//!
//! - `hello` → `hello`: names and versions, between paired machines only.
//! - `pair` (the client's SPAKE2 message) → `spake` (the server's); `confirm` (the client's
//!   proof that it knows the code) → `paired` (the server's). Both proofs are HMACs keyed by
//!   the SPAKE2 key over a label and the TLS session's exporter, and the SPAKE2 identities are
//!   the two keys TLS saw, so the code binds exactly these two keys on exactly this connection.
//! - `unpaired`: the client has forgotten the server; the server forgets it too.
//! - `call` → `reply`: a request of the Helper's own (`Node::on_call`), between paired machines
//!   only. The reply may be up to `REPLY_MAX` long.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use rustls::pki_types::ServerName;
use rustls::sign::CertifiedKey;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use spake2::{Ed25519Group, Identity as Who, Password, Spake2};

use crate::keys::{self, Identity, Key};
use crate::tls::{self, Admit};
use crate::{PORT, Why};

/// How long a code shown stays good.
pub const CODE_TTL: Duration = Duration::from_secs(120);
/// How many tries a code takes before it is gone.
pub const CODE_TRIES: u32 = 5;
/// A line longer than this ends the connection.
const LINE_MAX: u64 = 64 * 1024;
/// A reply to a `call`, at most: a Peer paired with this one sends it.
const REPLY_MAX: u64 = 4 * 1024 * 1024;
/// The whole of a connection, handshake included: a Peer answers at once.
const DEADLINE: Duration = Duration::from_secs(5);
/// How long dialling a Peer may take.
const DIAL: Duration = Duration::from_secs(3);
/// Connections served at once, at most; more are closed at once.
const AT_ONCE: usize = 16;
const EXPORTER: &[u8] = b"EXPORTER-bragd-peer-pairing";

/// A machine paired with this one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Peer {
    /// Its public key, as hex: its id.
    pub key: String,
    pub name: String,
    /// Where it was last reached: `host:port`.
    pub addr: String,
    /// When it was paired, in seconds since 1970.
    pub paired: u64,
    /// When it last answered, and the Bragd version it said.
    #[serde(default)]
    pub seen: Option<u64>,
    #[serde(default)]
    pub version: Option<String>,
}

/// The code this machine shows.
struct Code {
    digits: String,
    until: Instant,
    tries: u32,
}

impl Code {
    fn open(&self) -> bool {
        Instant::now() < self.until && self.tries < CODE_TRIES
    }
}

/// What the TLS verifier and the connections share: the pinned keys and the code.
struct Trust {
    peers: Mutex<Vec<Peer>>,
    code: Mutex<Option<Code>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Trust {
    fn pinned(&self, key: &Key) -> bool {
        let hex = keys::hex(key);
        lock(&self.peers).iter().any(|p| p.key == hex)
    }
}

impl Admit for Trust {
    fn admits(&self, key: &Key) -> bool {
        self.pinned(key) || lock(&self.code).as_ref().is_some_and(Code::open)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Ask {
    Hello { name: String, version: String },
    Pair { name: String, port: u16, spake: String },
    Confirm { mac: String },
    Unpaired,
    Call { body: serde_json::Value },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "re", rename_all = "snake_case")]
enum Answer {
    Hello { name: String, version: String },
    Spake { spake: String },
    Paired { name: String, mac: String },
    Done,
    Reply { body: serde_json::Value },
    Refused { why: Why },
}

/// A Peer's line: TLS over TCP, one JSON line each way at a time.
struct Line<C> {
    tls: rustls::StreamOwned<C, TcpStream>,
    by: Instant,
}

impl<C, S> Line<C>
where
    C: std::ops::DerefMut<Target = rustls::ConnectionCommon<S>>,
    S: rustls::SideData,
{
    fn new(conn: C, sock: TcpStream) -> io::Result<Line<C>> {
        let mut line = Line { tls: rustls::StreamOwned::new(conn, sock), by: Instant::now() + DEADLINE };
        line.time()?;
        while line.tls.conn.is_handshaking() {
            line.tls.conn.complete_io(&mut line.tls.sock)?;
        }
        Ok(line)
    }

    fn time(&mut self) -> io::Result<()> {
        let left = self.by.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        self.tls.sock.set_read_timeout(Some(left))?;
        self.tls.sock.set_write_timeout(Some(left))
    }

    /// The key the other side showed, which the verifier has accepted.
    fn their_key(&self) -> io::Result<Key> {
        self.tls.conn.peer_certificates().and_then(|c| c.first()).and_then(|c| keys::from_spki(c)).ok_or_else(|| io::Error::other("no key"))
    }

    fn exporter(&self) -> io::Result<[u8; 32]> {
        self.tls.conn.export_keying_material([0u8; 32], EXPORTER, None).map_err(io::Error::other)
    }

    fn say(&mut self, what: &impl Serialize) -> io::Result<()> {
        self.time()?;
        let mut text = serde_json::to_string(what).map_err(io::Error::other)?;
        text.push('\n');
        self.tls.write_all(text.as_bytes())?;
        self.tls.flush()
    }

    fn hear<T: for<'a> Deserialize<'a>>(&mut self) -> io::Result<T> {
        self.hear_up_to(LINE_MAX)
    }

    fn hear_up_to<T: for<'a> Deserialize<'a>>(&mut self, max: u64) -> io::Result<T> {
        self.time()?;
        // A reader per line is enough: each side waits for the answer before it says more.
        let mut text = String::new();
        BufReader::new((&mut self.tls).take(max)).read_line(&mut text)?;
        if !text.ends_with('\n') {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        serde_json::from_str(&text).map_err(io::Error::other)
    }
}

/// HMAC-SHA-256 of `label` and the exporter, keyed by the SPAKE2 key.
fn proof(key: &[u8], label: &[u8], exporter: &[u8]) -> Hmac<Sha256> {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC takes any key");
    m.update(label);
    m.update(exporter);
    m
}

/// What went wrong with a connection to a Peer.
#[derive(Debug)]
pub enum Failed {
    /// The other machine said no.
    Refused(Why),
    /// It could not be reached, or the line broke.
    Line(io::Error),
}

impl From<io::Error> for Failed {
    fn from(e: io::Error) -> Self {
        Failed::Line(e)
    }
}

impl std::fmt::Display for Failed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failed::Refused(why) => write!(f, "refused: {why:?}"),
            Failed::Line(e) => write!(f, "{e}"),
        }
    }
}

/// What answers a `call`: the caller's id and its request, to the reply.
pub type Handler = dyn Fn(&str, serde_json::Value) -> serde_json::Value + Send + Sync;

/// `peers.json`'s format: a list of Peers is version 1. A later version is an object with its
/// `format`, which this one refuses, leaving the file as it is.
pub const FORMAT: bragd_core::format::Json = bragd_core::format::Json { file: "peers.json", migrations: &[] };

pub struct Node {
    dir: PathBuf,
    me: Identity,
    certified: Arc<CertifiedKey>,
    name: String,
    version: String,
    trust: Arc<Trust>,
    server: Arc<rustls::ServerConfig>,
    /// The port this machine listens on, told to a machine it pairs with.
    port: AtomicU16,
    /// Open connections, by the key of the machine on the other end.
    open: Mutex<Vec<(Key, TcpStream)>>,
    serving: AtomicUsize,
    handler: Mutex<Option<Arc<Handler>>>,
}

impl Node {
    /// The Peer `name` of Bragd `version`, with its key and Peers in `dir`.
    pub fn open(dir: &Path, name: &str, version: &str) -> io::Result<Arc<Node>> {
        std::fs::create_dir_all(dir)?;
        #[cfg(unix)]
        std::fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(0o700))?;
        let me = Identity::load_or_make(dir)?;
        let certified = me.certified()?;
        let peers = match std::fs::read(dir.join("peers.json")) {
            Ok(b) => {
                let mut v: serde_json::Value = serde_json::from_slice(&b).map_err(io::Error::other)?;
                FORMAT.upgrade(&mut v).map_err(|e| io::Error::other(e.to_string()))?;
                serde_json::from_value(v).map_err(io::Error::other)?
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => vec![],
            Err(e) => return Err(e),
        };
        let trust = Arc::new(Trust { peers: Mutex::new(peers), code: Mutex::new(None) });
        let server = tls::server(certified.clone(), trust.clone()).map_err(io::Error::other)?;
        Ok(Arc::new(Node { dir: dir.into(), me, certified, name: name.into(), version: version.into(), trust, server, port: AtomicU16::new(PORT), open: Mutex::new(vec![]), serving: AtomicUsize::new(0), handler: Mutex::new(None) }))
    }

    pub fn id(&self) -> String {
        self.me.id()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn peers(&self) -> Vec<Peer> {
        lock(&self.trust.peers).clone()
    }

    fn save(&self, peers: &[Peer]) -> io::Result<()> {
        let text = serde_json::to_vec_pretty(peers).map_err(io::Error::other)?;
        bragd_core::fs::replace(&self.dir.join("peers.json"), &text, true)
    }

    /// Changes the Peers and writes them down.
    fn update(&self, change: impl FnOnce(&mut Vec<Peer>)) -> io::Result<()> {
        let mut peers = lock(&self.trust.peers);
        change(&mut peers);
        self.save(&peers)
    }

    /// What answers the `call`s of paired machines from now on; until then they get `null`.
    pub fn on_call(&self, handler: Arc<Handler>) {
        *lock(&self.handler) = Some(handler);
    }

    /// A new code, good for `ttl` and `CODE_TRIES` tries, in place of any before.
    pub fn show_code(&self, ttl: Duration) -> io::Result<String> {
        // Uniform over 000000–999999: 2^32 is cut down to a whole number of millions.
        let digits = loop {
            let mut b = [0u8; 4];
            getrandom::fill(&mut b).map_err(|e| io::Error::other(e.to_string()))?;
            let n = u32::from_le_bytes(b);
            if n < 4_294_000_000 {
                break format!("{:06}", n % 1_000_000);
            }
        };
        *lock(&self.trust.code) = Some(Code { digits: digits.clone(), until: Instant::now() + ttl, tries: 0 });
        Ok(digits)
    }

    /// The code shown now, with its seconds and tries left.
    pub fn code(&self) -> Option<(String, u64, u32)> {
        let code = lock(&self.trust.code);
        code.as_ref().filter(|c| c.open()).map(|c| (c.digits.clone(), c.until.saturating_duration_since(Instant::now()).as_secs(), CODE_TRIES - c.tries))
    }

    pub fn hide_code(&self) {
        *lock(&self.trust.code) = None;
    }

    // ------------------------------------------------------------------ the server's side

    /// Listens on `addrs` (port 0: any) until the `Listening` is dropped. Returns the addresses
    /// it listens on.
    pub fn listen(self: &Arc<Self>, addrs: &[SocketAddr]) -> io::Result<Listening> {
        let stop = Arc::new(AtomicBool::new(false));
        let mut bound = vec![];
        let listeners = addrs.iter().map(TcpListener::bind).collect::<io::Result<Vec<_>>>()?;
        for l in listeners {
            l.set_nonblocking(true)?;
            let at = l.local_addr()?;
            self.port.store(at.port(), Ordering::Relaxed);
            bound.push(at);
            let (node, stop) = (self.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    match l.accept() {
                        Ok((sock, from)) => {
                            if node.serving.fetch_add(1, Ordering::SeqCst) >= AT_ONCE {
                                node.serving.fetch_sub(1, Ordering::SeqCst);
                                continue;
                            }
                            let node = node.clone();
                            std::thread::spawn(move || {
                                let _ = node.serve(sock, from);
                                node.serving.fetch_sub(1, Ordering::SeqCst);
                            });
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
                        Err(_) => std::thread::sleep(Duration::from_millis(500)),
                    }
                }
            });
        }
        Ok(Listening { stop, addrs: bound })
    }

    fn serve(&self, sock: TcpStream, from: SocketAddr) -> io::Result<()> {
        sock.set_nonblocking(false)?;
        let watch = sock.try_clone()?;
        let conn = rustls::ServerConnection::new(self.server.clone()).map_err(io::Error::other)?;
        // A key the verifier refused ends here, in the handshake.
        let mut line = Line::new(conn, sock)?;
        let theirs = line.their_key()?;
        lock(&self.open).push((theirs, watch));
        let done = self.answer(&mut line, theirs, from);
        lock(&self.open).retain(|(k, s)| !(*k == theirs && s.peer_addr().ok() == Some(from)));
        done
    }

    fn answer(&self, line: &mut Line<rustls::ServerConnection>, theirs: Key, from: SocketAddr) -> io::Result<()> {
        let pinned = self.trust.pinned(&theirs);
        match line.hear()? {
            Ask::Hello { name, version } if pinned => {
                let hex = keys::hex(&theirs);
                self.update(|ps| {
                    if let Some(p) = ps.iter_mut().find(|p| p.key == hex) {
                        (p.name, p.version, p.seen) = (name, Some(version), Some(bragd_core::fs::now()));
                    }
                })?;
                line.say(&Answer::Hello { name: self.name.clone(), version: self.version.clone() })
            }
            Ask::Unpaired if pinned => {
                let hex = keys::hex(&theirs);
                self.update(|ps| ps.retain(|p| p.key != hex))?;
                line.say(&Answer::Done)
            }
            Ask::Call { body } if pinned => {
                let handler = lock(&self.handler).clone();
                let body = handler.map_or(serde_json::Value::Null, |h| h(&keys::hex(&theirs), body));
                line.say(&Answer::Reply { body })
            }
            Ask::Pair { name, port, spake } => self.pair_in(line, theirs, name, SocketAddr::new(from.ip(), port), &spake),
            _ => line.say(&Answer::Refused { why: Why::NotPaired }),
        }
    }

    /// Takes one try of the code, if one is open.
    fn take_try(&self) -> Option<String> {
        let mut code = lock(&self.trust.code);
        let c = code.as_mut().filter(|c| c.open())?;
        c.tries += 1;
        Some(c.digits.clone())
    }

    fn pair_in(&self, line: &mut Line<rustls::ServerConnection>, theirs: Key, name: String, addr: SocketAddr, spake: &str) -> io::Result<()> {
        // Every start is a try, finished or not: guesses are online, and five at most.
        let Some(digits) = self.take_try() else { return line.say(&Answer::Refused { why: Why::NoCode }) };
        let (s, ours) = Spake2::<Ed25519Group>::start_b(&Password::new(digits.as_bytes()), &Who::new(&theirs), &Who::new(&self.me.public));
        line.say(&Answer::Spake { spake: keys::hex(&ours) })?;
        let Ask::Confirm { mac } = line.hear()? else { return Ok(()) };
        let exporter = line.exporter()?;
        let key = keys::unhex(spake).and_then(|m| s.finish(&m).ok());
        let right = key.as_ref().is_some_and(|k| keys::unhex(&mac).is_some_and(|m| proof(k, b"bragd peer client", &exporter).verify_slice(&m).is_ok()));
        if !right {
            return line.say(&Answer::Refused { why: Why::WrongCode });
        }
        let key = key.unwrap_or_default();
        // Single use: paired, the code is gone.
        self.hide_code();
        let peer = Peer { key: keys::hex(&theirs), name, addr: addr.to_string(), paired: bragd_core::fs::now(), seen: Some(bragd_core::fs::now()), version: None };
        self.update(|ps| {
            ps.retain(|p| p.key != peer.key);
            ps.push(peer);
        })?;
        let mac = keys::hex(&proof(&key, b"bragd peer server", &exporter).finalize().into_bytes());
        line.say(&Answer::Paired { name: self.name.clone(), mac })
    }

    // ------------------------------------------------------------------ the client's side

    fn dial(&self, addr: &str, server: Option<Key>) -> io::Result<Line<rustls::ClientConnection>> {
        let mut last = io::Error::other(format!("{addr}: no address"));
        for at in resolve(addr)? {
            match TcpStream::connect_timeout(&at, DIAL) {
                Ok(sock) => {
                    let config = tls::client(self.certified.clone(), server).map_err(io::Error::other)?;
                    let name = ServerName::try_from("peer.bragd").map_err(io::Error::other)?;
                    let conn = rustls::ClientConnection::new(config, name).map_err(io::Error::other)?;
                    return Line::new(conn, sock);
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Pairs with the machine at `addr` that shows `code`. Both keep each other's key.
    pub fn pair_with(&self, addr: &str, code: &str) -> Result<Peer, Failed> {
        let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        let mut line = self.dial(addr, None)?;
        let theirs = line.their_key()?;
        if theirs == self.me.public {
            return Err(Failed::Refused(Why::Itself));
        }
        let (s, ours) = Spake2::<Ed25519Group>::start_a(&Password::new(code.as_bytes()), &Who::new(&self.me.public), &Who::new(&theirs));
        line.say(&Ask::Pair { name: self.name.clone(), port: self.port.load(Ordering::Relaxed), spake: keys::hex(&ours) })?;
        let spake = match line.hear()? {
            Answer::Spake { spake } => spake,
            Answer::Refused { why } => return Err(Failed::Refused(why)),
            _ => return Err(Failed::Line(io::Error::other("unexpected answer"))),
        };
        let exporter = line.exporter()?;
        let key = keys::unhex(&spake).and_then(|m| s.finish(&m).ok()).ok_or(Failed::Refused(Why::WrongCode))?;
        line.say(&Ask::Confirm { mac: keys::hex(&proof(&key, b"bragd peer client", &exporter).finalize().into_bytes()) })?;
        let name = match line.hear()? {
            Answer::Paired { name, mac } if keys::unhex(&mac).is_some_and(|m| proof(&key, b"bragd peer server", &exporter).verify_slice(&m).is_ok()) => name,
            Answer::Refused { why } => return Err(Failed::Refused(why)),
            _ => return Err(Failed::Refused(Why::WrongCode)),
        };
        let peer = Peer { key: keys::hex(&theirs), name, addr: addr.into(), paired: bragd_core::fs::now(), seen: Some(bragd_core::fs::now()), version: None };
        self.update(|ps| {
            ps.retain(|p| p.key != peer.key);
            ps.push(peer.clone());
        })?;
        Ok(peer)
    }

    fn pinned_key(&self, id: &str) -> Option<(Key, String)> {
        lock(&self.trust.peers).iter().find(|p| p.key == id).and_then(|p| Some((keys::key_of(&p.key)?, p.addr.clone())))
    }

    /// Says hello to the Peer `id`; its name and version come back, and are kept.
    pub fn hello(&self, id: &str) -> Result<Peer, Failed> {
        let (key, addr) = self.pinned_key(id).ok_or(Failed::Refused(Why::NotPaired))?;
        let mut line = self.dial(&addr, Some(key))?;
        line.say(&Ask::Hello { name: self.name.clone(), version: self.version.clone() })?;
        let (name, version) = match line.hear()? {
            Answer::Hello { name, version } => (name, version),
            Answer::Refused { why } => return Err(Failed::Refused(why)),
            _ => return Err(Failed::Line(io::Error::other("unexpected answer"))),
        };
        let mut seen = None;
        self.update(|ps| {
            if let Some(p) = ps.iter_mut().find(|p| p.key == id) {
                (p.name, p.version, p.seen) = (name, Some(version), Some(bragd_core::fs::now()));
                seen = Some(p.clone());
            }
        })?;
        seen.ok_or(Failed::Refused(Why::NotPaired))
    }

    /// Asks the Peer `id` its Helper's `body` (`on_call` there): its reply.
    pub fn call(&self, id: &str, body: &serde_json::Value) -> Result<serde_json::Value, Failed> {
        let (key, addr) = self.pinned_key(id).ok_or(Failed::Refused(Why::NotPaired))?;
        let mut line = self.dial(&addr, Some(key))?;
        line.say(&Ask::Call { body: body.clone() })?;
        match line.hear_up_to(REPLY_MAX)? {
            Answer::Reply { body } => Ok(body),
            Answer::Refused { why } => Err(Failed::Refused(why)),
            _ => Err(Failed::Line(io::Error::other("unexpected answer"))),
        }
    }

    /// A Peer found at another address (mDNS): kept for the next time.
    pub fn moved(&self, id: &str, addr: &str) -> io::Result<()> {
        self.update(|ps| {
            if let Some(p) = ps.iter_mut().find(|p| p.key == id) {
                p.addr = addr.into();
            }
        })
    }

    /// Forgets the Peer `id`, closes its connections, and tells it, if it can be reached.
    pub fn unpair(&self, id: &str) -> io::Result<()> {
        let pinned = self.pinned_key(id);
        self.update(|ps| ps.retain(|p| p.key != id))?;
        let Some((key, addr)) = pinned else { return Ok(()) };
        for (_, s) in lock(&self.open).iter().filter(|(k, _)| *k == key) {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
        // Best effort: the other side refuses this key from now on whatever it hears.
        if let Ok(mut line) = self.dial(&addr, Some(key)) {
            let _ = line.say(&Ask::Unpaired).and_then(|()| line.hear::<Answer>());
        }
        Ok(())
    }
}

/// `host`, `host:port`, `a.b.c.d:port` or `[v6]:port`; the Peers' port when none is given.
pub fn resolve(addr: &str) -> io::Result<Vec<SocketAddr>> {
    let addr = addr.trim();
    if let Ok(a) = addr.parse::<SocketAddr>() {
        return Ok(vec![a]);
    }
    let found = match addr.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') && port.parse::<u16>().is_ok() => (host, port.parse().unwrap_or(PORT)).to_socket_addrs()?,
        _ => (addr.trim_matches(['[', ']']), PORT).to_socket_addrs()?,
    };
    Ok(found.collect())
}

/// Listening, until dropped.
pub struct Listening {
    stop: Arc<AtomicBool>,
    pub addrs: Vec<SocketAddr>,
}

impl Drop for Listening {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
