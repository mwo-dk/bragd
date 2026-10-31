//! The Helper's connection and lifetime. One Helper per user: it holds `helper.lock` in its
//! folder for as long as it runs, so a second one leaves at once. Windows reach it over a Unix
//! socket (loopback TCP on Windows), one JSON line per request and per reply. The address and
//! a token from the system's random source are in `helper.addr`, which only the user can read;
//! a connection whose first line does not bring the token is dropped.
//!
//! A window that finds no Helper starts one (`connect_or_start`). The Helper leaves ten minutes
//! after the last window has gone, unless it was started to stay (`--stay`, or
//! `[helper] start_with_session`). A Helper steps down only for a window of a newer version,
//! whose own Helper then starts: an older window never makes a newer Helper leave.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use bragd_core::config::DiskConfig;

use crate::jobs::{Job, Jobs, Work};
use crate::sources::{About, Memo, Options};
use crate::{VERSION, note, service};

/// How long the Helper stays after the last window has gone.
pub const LINGER: Duration = Duration::from_secs(600);
/// The wire format. One more when a request or a reply changes so that another version cannot
/// read it; added fields with defaults do not count.
pub const PROTOCOL: u32 = 1;
/// A version and protocol: other ones in tests.
pub type Ours = (&'static str, u32);
const OURS: Ours = (VERSION, PROTOCOL);
/// A request longer than this is nobody's: the connection is dropped.
const LINE_MAX: u64 = 1 << 20;
/// How long a connection has to say hello, all told; a window does so at once.
const HELLO_WAIT: Duration = Duration::from_secs(5);
/// Connections that have not said hello yet, at most: more are dropped at once, so nobody can
/// open threads without limit.
const HELLO_PENDING: usize = 64;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Hello { token: String, version: String, protocol: u32 },
    Status,
    Jobs,
    Start { work: Work },
    Cancel { id: u64 },
    /// A Foreign database into a Library: the default one when none is named.
    Import { path: PathBuf, library: Option<PathBuf>, source: Option<String> },
    DedupeNow { library: Option<PathBuf> },
    FetchNow { source: String },
    /// The user's yes to a Source's download over its cap: fetched now, that once.
    AllowDownload { source: String },
    Sources,
    /// A Source's options: on or off, its schedule, metered, disk cap, a custom URL, its Strip
    /// rule, what becomes of its downloads, and what Disk watch may do for it by itself.
    SetSource { source: String, options: Options },
    /// Everything Bragd keeps on disk, its limits, the free space, and the limits crossed.
    DiskReport,
    /// Disk watch's limits, written to `[disk]` in config.toml; the next check uses them.
    SetDiskLimits { limits: DiskConfig },
    /// A job that reports what a Source's Strip rule would take from its downloads and the
    /// Library, changing nothing (`Work::Strip`, its `Job::report`).
    StripDryRun { source: String },
    /// A job that applies a Source's Strip rule to its downloads and the Library.
    StripApply { source: String },
    /// A job that applies every Source's Strip rule to a Library: the default one when none is
    /// named.
    PruneLibrary { library: Option<PathBuf>, dry_run: bool },
    /// Leave, so a Helper with the settings as they are now comes.
    Restart,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "re", rename_all = "snake_case")]
pub enum Reply {
    /// `same`: the window may use this Helper. Otherwise it is an older one leaving for the
    /// window's, or, with `newer` (its version), a newer one the window cannot talk to.
    Hello { same: bool, newer: Option<String> },
    Status(Status),
    Jobs { jobs: Vec<Job> },
    Started { id: u64 },
    Sources { sources: Vec<(About, Options, Memo)> },
    Disk { report: crate::disk::Report },
    Done,
    Error { error: String },
}

/// How the Helper is doing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub version: String,
    pub pid: u32,
    /// Windows connected now.
    pub clients: usize,
    /// Stays when the last window has gone.
    pub stays: bool,
    /// Work waits for the machine to be plugged in.
    pub paused: bool,
    /// Jobs queued or running.
    pub active: usize,
    /// The disk limits crossed at Disk watch's last check: a notice each.
    #[serde(default)]
    pub disk: Vec<crate::disk::Crossed>,
}

/// 128 bits from the system's random source, as hex.
fn token() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Whether `theirs` is the token, taking the same time whatever the first wrong byte.
fn is_token(theirs: &str, token: &str) -> bool {
    theirs.len() == token.len() && theirs.bytes().zip(token.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

/// Whether version `a` comes after `b`: 2.10.0 after 2.9.1.
fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> { v.split('.').map(|p| p.split(|c: char| !c.is_ascii_digit()).next().and_then(|n| n.parse().ok()).unwrap_or(0)).collect() };
    parts(a) > parts(b)
}

/// What a Helper of `ours` does when a window of `version`, speaking `protocol`, says hello.
#[derive(Debug, PartialEq)]
pub enum Greet {
    /// A newer window: the Helper leaves, and the window starts its own.
    Leave,
    /// The same or an older window that speaks its protocol.
    Serve,
    /// An older window that does not: the Helper stays, and the window goes without it. It
    /// never hands over to an older Helper, which would leave again for the next newer window.
    Refuse,
}

pub fn greet(ours: Ours, version: &str, protocol: u32) -> Greet {
    if newer(version, ours.0) {
        Greet::Leave
    } else if protocol == ours.1 {
        Greet::Serve
    } else {
        Greet::Refuse
    }
}

/// Whether the Helper leaves now: no window for `linger`, no job running, and not one that
/// stays.
pub fn idle_exit(clients: usize, idle: Duration, busy: bool, stays: bool, linger: Duration) -> bool {
    !stays && clients == 0 && !busy && idle >= linger
}

/// `helper.lock`, held for as long as this process is the Helper.
pub struct Lock(#[allow(dead_code)] std::fs::File);

impl Lock {
    /// The lock in `dir`, or `None` when another Helper holds it.
    pub fn take(dir: &Path) -> io::Result<Option<Lock>> {
        let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join("helper.lock"))?;
        match f.try_lock() {
            Ok(()) => Ok(Some(Lock(f))),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(e)) => Err(e),
        }
    }
}

// ---------------------------------------------------------------- the platform's connection

#[cfg(unix)]
type Stream = std::os::unix::net::UnixStream;
#[cfg(not(unix))]
type Stream = std::net::TcpStream;

/// Listens: the listener and its address for `helper.addr`.
#[cfg(unix)]
fn bind(dir: &Path) -> io::Result<(std::os::unix::net::UnixListener, String)> {
    let path = dir.join("helper.sock");
    // Ours to replace: we hold the lock, so whoever made it has gone.
    let _ = std::fs::remove_file(&path);
    Ok((std::os::unix::net::UnixListener::bind(&path)?, path.to_string_lossy().into_owned()))
}

// shortcut: loopback TCP with the token on Windows, as upstream does; a named pipe needs a crate or raw Win32 calls.
#[cfg(not(unix))]
fn bind(_: &Path) -> io::Result<(std::net::TcpListener, String)> {
    let l = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let port = l.local_addr()?.port();
    Ok((l, port.to_string()))
}

fn dial(addr: &str) -> io::Result<Stream> {
    #[cfg(unix)]
    return Stream::connect(addr);
    #[cfg(not(unix))]
    {
        let port: u16 = addr.parse().map_err(io::Error::other)?;
        let s = Stream::connect_timeout(&(std::net::Ipv4Addr::LOCALHOST, port).into(), Duration::from_millis(300))?;
        s.set_nodelay(true)?;
        Ok(s)
    }
}

/// `text` into `path`, readable by this user alone.
fn write_private(path: &Path, text: &str) -> io::Result<()> {
    bragd_core::fs::replace(path, text.as_bytes(), true)
}

// ---------------------------------------------------------------- the Helper

/// How a Helper's run ended.
#[derive(Debug, PartialEq)]
pub enum Ended {
    /// Another Helper runs: this one never started.
    AnotherRuns,
    /// No window for a while.
    Idle,
    /// A window asked it to: a newer version's, or for new settings.
    Asked,
}

/// Be the Helper, in its folder, with the settings in config.toml. Returns when it leaves.
pub fn serve(stay: bool) -> io::Result<Ended> {
    let dir = crate::folder().ok_or_else(|| io::Error::other("no data folder"))?;
    let cfg = bragd_core::config::Config::load().unwrap_or_default();
    let library = crate::default_library(&cfg).ok_or_else(|| io::Error::other("no data folder"))?;
    std::fs::create_dir_all(&dir)?;
    note(format_args!("Helper starts, process {}", std::process::id()));
    let runtime = cfg.engines.runtime.clone();
    let images = Box::new(move || crate::disk::pulled_images(&runtime));
    let jobs = || Jobs::open(&dir, Box::new(crate::net::Web::default()), library, cfg.knowledge.source_priority.clone(), Box::new(crate::battery::on_battery), images);
    let ended = serve_in(OURS, &dir, stay || cfg.helper.start_with_session, LINGER, jobs);
    match &ended {
        Ok(e) => note(format_args!("Helper leaves: {e:?}")),
        Err(e) => note(format_args!("Helper stops: {e}")),
    }
    ended
}

/// `serve` with its version, folder, patience and jobs given, for tests.
pub fn serve_in(ours: Ours, dir: &Path, stays: bool, linger: Duration, jobs: impl FnOnce() -> Arc<Jobs>) -> io::Result<Ended> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    std::fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(0o700))?;
    // A Helper that was asked to go may take a moment to let go.
    let wait = Instant::now();
    let _lock = loop {
        if let Some(l) = Lock::take(dir)? {
            break l;
        }
        if wait.elapsed() > Duration::from_secs(3) {
            return Ok(Ended::AnotherRuns);
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let (listener, addr) = bind(dir)?;
    let token = token()?;
    let addr_file = dir.join("helper.addr");
    write_private(&addr_file, &format!("{addr}\n{token}\n"))?;

    let jobs = jobs();
    let worker = {
        let jobs = jobs.clone();
        std::thread::spawn(move || jobs.work())
    };
    let (clients, quit, last) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicBool::new(false)), Arc::new(Mutex::new(Instant::now())));
    {
        let (jobs, clients, quit, last) = (jobs.clone(), clients.clone(), quit.clone(), last.clone());
        std::thread::spawn(move || {
            let pending = Arc::new(AtomicUsize::new(0));
            for stream in listener.incoming().flatten() {
                if pending.fetch_add(1, Ordering::SeqCst) >= HELLO_PENDING {
                    pending.fetch_sub(1, Ordering::SeqCst);
                    continue;
                }
                let (jobs, token, clients, quit, last, pending) = (jobs.clone(), token.clone(), clients.clone(), quit.clone(), last.clone(), pending.clone());
                std::thread::spawn(move || {
                    let _ = answer(ours, stream, &jobs, &token, stays, Hello { clients: &clients, last: &last, pending: &pending, counted: Counted::Waiting }, &quit);
                });
            }
        });
    }
    while !quit.load(Ordering::SeqCst) {
        let idle = last.lock().unwrap_or_else(|e| e.into_inner()).elapsed();
        // *Start with my session* turned on since this Helper started: it stays, as the one
        // the session would start does.
        if idle_exit(clients.load(Ordering::SeqCst), idle, jobs.busy(), stays, linger) && !bragd_core::config::Config::load().unwrap_or_default().helper.start_with_session {
            break;
        }
        std::thread::sleep(linger.min(Duration::from_millis(200)));
    }
    jobs.stop();
    let _ = worker.join();
    let _ = std::fs::remove_file(addr_file);
    Ok(if quit.load(Ordering::SeqCst) { Ended::Asked } else { Ended::Idle })
}

/// Where a connection stands: waiting for its hello, or a window.
#[derive(PartialEq)]
enum Counted {
    Waiting,
    Window,
}

/// A connection's place in the counts: pending until its token is checked, then a window
/// (`clients`) until it goes, when `last` says so. Only a window keeps the Helper from leaving.
struct Hello<'a> {
    clients: &'a AtomicUsize,
    last: &'a Mutex<Instant>,
    pending: &'a AtomicUsize,
    counted: Counted,
}

impl Hello<'_> {
    /// The token was right: a window from now on.
    fn window(&mut self) {
        if self.counted == Counted::Waiting {
            self.pending.fetch_sub(1, Ordering::SeqCst);
            self.clients.fetch_add(1, Ordering::SeqCst);
            self.counted = Counted::Window;
        }
    }
}

impl Drop for Hello<'_> {
    fn drop(&mut self) {
        match self.counted {
            Counted::Waiting => {
                self.pending.fetch_sub(1, Ordering::SeqCst);
            }
            Counted::Window => {
                self.clients.fetch_sub(1, Ordering::SeqCst);
                *self.last.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
            }
        }
    }
}

/// A stream read up to a deadline over all its reads, not per read: a connection that sends
/// a byte at a time cannot stretch its hello. No deadline once it has said hello.
struct Timed {
    stream: Stream,
    by: Option<Instant>,
}

impl Read for Timed {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(by) = self.by {
            let left = by.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            self.stream.set_read_timeout(Some(left))?;
        }
        self.stream.read(buf)
    }
}

fn answer(ours: Ours, stream: Stream, jobs: &Jobs, token: &str, stays: bool, mut hello: Hello, quit: &AtomicBool) -> io::Result<()> {
    let clients = hello.clients;
    let mut out = stream.try_clone()?;
    let mut from = BufReader::new(Timed { stream, by: Some(Instant::now() + HELLO_WAIT) });
    let mut said_hello = false;
    let mut line = String::new();
    loop {
        line.clear();
        if (&mut from).take(LINE_MAX).read_line(&mut line)? == 0 || !line.ends_with('\n') {
            return Ok(());
        }
        let request: Request = serde_json::from_str(&line).map_err(io::Error::other)?;
        let library = |l: Option<PathBuf>| l.or_else(|| crate::default_library(&bragd_core::config::Config::load().unwrap_or_default())).unwrap_or_default();
        let started = |work| Reply::Started { id: jobs.add(work) };
        let reply = match request {
            Request::Hello { token: theirs, version, protocol } => {
                if !is_token(&theirs, token) {
                    return Ok(());
                }
                said_hello = true;
                hello.window();
                // A window keeps its line open for as long as it runs.
                from.get_mut().by = None;
                out.set_read_timeout(None)?;
                match greet(ours, &version, protocol) {
                    // A Helper on its way out sends newcomers to the next one.
                    _ if quit.load(Ordering::SeqCst) => Reply::Hello { same: false, newer: None },
                    Greet::Leave => {
                        if !quit.fetch_or(true, Ordering::SeqCst) {
                            note(format_args!("a window of the newer version {version} came: leaving for its Helper"));
                        }
                        Reply::Hello { same: false, newer: None }
                    }
                    Greet::Serve => Reply::Hello { same: true, newer: None },
                    Greet::Refuse => {
                        note(format_args!("a window of the older version {version} speaks protocol {protocol}, this Helper {}: it stays", ours.1));
                        Reply::Hello { same: false, newer: Some(ours.0.into()) }
                    }
                }
            }
            _ if !said_hello => return Ok(()),
            Request::Status => Reply::Status(Status {
                version: ours.0.into(),
                pid: std::process::id(),
                clients: clients.load(Ordering::SeqCst),
                stays,
                paused: jobs.paused.load(Ordering::Relaxed),
                active: jobs.active(),
                disk: jobs.crossed(),
            }),
            Request::Jobs => Reply::Jobs { jobs: jobs.list() },
            Request::Start { work } => started(work),
            Request::Cancel { id } => {
                if jobs.cancel(id) {
                    Reply::Done
                } else {
                    Reply::Error { error: format!("no job {id} to cancel") }
                }
            }
            Request::Import { path, library: l, source } => started(Work::Import { path, library: library(l), source: source.unwrap_or_else(|| "file".into()) }),
            Request::DedupeNow { library: l } => started(Work::Dedupe { library: library(l) }),
            Request::FetchNow { source } => started(Work::Fetch { source }),
            Request::AllowDownload { source } => match jobs.allow_download(&source) {
                Ok(id) => Reply::Started { id },
                Err(error) => Reply::Error { error },
            },
            Request::Sources => Reply::Sources { sources: jobs.sources() },
            Request::SetSource { source, options } => match jobs.set_source(&source, options) {
                Ok(()) => Reply::Done,
                Err(error) => Reply::Error { error },
            },
            Request::DiskReport => Reply::Disk { report: jobs.disk_report(&bragd_core::config::Config::load().unwrap_or_default()) },
            Request::SetDiskLimits { limits } => match bragd_core::config::Config::save_disk(&limits) {
                Ok(_) => Reply::Done,
                Err(error) => Reply::Error { error },
            },
            Request::StripDryRun { source } => started(Work::Strip { source, dry_run: true }),
            Request::StripApply { source } => started(Work::Strip { source, dry_run: false }),
            Request::PruneLibrary { library: l, dry_run } => started(Work::Prune { library: library(l), dry_run }),
            Request::Restart => {
                quit.store(true, Ordering::SeqCst);
                Reply::Done
            }
        };
        send(&mut out, &reply)?;
    }
}

fn send(out: &mut Stream, reply: &Reply) -> io::Result<()> {
    let mut text = serde_json::to_string(reply).map_err(io::Error::other)?;
    text.push('\n');
    out.write_all(text.as_bytes())
}

// ---------------------------------------------------------------- the windows' side

/// A window's line to the Helper.
pub struct Conn {
    from: BufReader<Stream>,
    to: Stream,
}

impl Conn {
    /// One request, one reply.
    pub fn ask(&mut self, request: &Request) -> io::Result<Reply> {
        let mut text = serde_json::to_string(request).map_err(io::Error::other)?;
        text.push('\n');
        self.to.write_all(text.as_bytes())?;
        let mut line = String::new();
        if self.from.read_line(&mut line)? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        serde_json::from_str(&line).map_err(io::Error::other)
    }
}

/// What dialling the Helper in `dir` gave.
pub enum Dialled {
    Line(Conn),
    /// A newer Helper this window cannot talk to: its version. It stays; the window goes
    /// without it until it is restarted.
    Newer(String),
    /// None there, or one leaving for a newer window's.
    Nobody,
}

/// Dials the Helper in `dir` and says hello as a window of `ours`.
pub fn dial_in(dir: &Path, ours: Ours) -> Dialled {
    let Ok(text) = std::fs::read_to_string(dir.join("helper.addr")) else { return Dialled::Nobody };
    let mut lines = text.lines();
    let (Some(addr), Some(token)) = (lines.next(), lines.next()) else { return Dialled::Nobody };
    let Ok(stream) = dial(addr) else { return Dialled::Nobody };
    // A busy Helper answers in milliseconds; one silent this long is stuck.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Ok(to) = stream.try_clone() else { return Dialled::Nobody };
    let mut c = Conn { from: BufReader::new(stream), to };
    match c.ask(&Request::Hello { token: token.into(), version: ours.0.into(), protocol: ours.1 }) {
        Ok(Reply::Hello { same: true, .. }) => Dialled::Line(c),
        Ok(Reply::Hello { newer: Some(v), .. }) => Dialled::Newer(v),
        _ => Dialled::Nobody,
    }
}

/// What a window does the `n`th time it finds no Helper.
#[derive(Debug, PartialEq)]
pub enum Step {
    /// Ask systemd or launchd to start the registered one.
    Ask,
    /// Start one, which stays when it is registered.
    Own { stay: bool },
    /// The service manager did not start it: start one that stays.
    Rescue,
    Wait,
}

/// A registered Helper is the service manager's to start (twice: one that was just asked to
/// go may still be leaving), then the window's own; the lock lets only one of them serve.
/// Where nothing starts it again (Windows, autostart), the window does at once.
pub fn step(n: usize, registered: bool, supervised: bool) -> Step {
    match (n, registered && supervised) {
        (0 | 1, true) => Step::Ask,
        (_, true) => Step::Rescue,
        (0, false) => Step::Own { stay: registered },
        _ => Step::Wait,
    }
}

/// When, after the first try, a window that finds no Helper takes its next `step`.
const STEPS: [Duration; 3] = [Duration::ZERO, Duration::from_secs(3), Duration::from_secs(8)];

/// The `bragd-helper` program beside this one.
pub fn program() -> io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(exe.with_file_name(format!("bragd-helper{}", std::env::consts::EXE_SUFFIX)))
}

/// The Helper's line for a window: the one running, or one this window starts (`program`).
pub fn connect_or_start(program: &Path) -> io::Result<Conn> {
    let dir = crate::folder().ok_or_else(|| io::Error::other("no data folder"))?;
    service::refresh(program);
    let (registered, supervised) = (service::installed(), service::Kind::here().supervised());
    let patience = Duration::from_secs(if registered { 15 } else { 5 });
    let (wait, mut tries) = (Instant::now(), 0);
    loop {
        match dial_in(&dir, OURS) {
            Dialled::Line(c) => return Ok(c),
            Dialled::Newer(v) => return Err(io::Error::other(format!("the Helper is the newer version {v}: restart Bragd to use it"))),
            Dialled::Nobody if STEPS.get(tries).is_some_and(|&at| wait.elapsed() >= at) => {
                match step(tries, registered, supervised) {
                    Step::Ask => service::kick(),
                    Step::Own { stay } => launch(program, stay)?,
                    Step::Rescue => launch(program, true)?,
                    Step::Wait => {}
                }
                tries += 1;
            }
            Dialled::Nobody => {}
        }
        if wait.elapsed() > patience {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "no Helper answered"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Starts `exe` as a Helper that outlives the window and its terminal, staying when `stay`,
/// its stderr into `helper.log`. Waited for in a thread, so one that leaves first is no zombie.
pub fn launch(exe: &Path, stay: bool) -> io::Result<()> {
    use std::process::Stdio;
    let mut c = bragd_engines::tools::own(exe);
    if stay {
        c.arg(service::STAY);
    }
    let log = crate::folder().and_then(|d| {
        std::fs::create_dir_all(&d).ok()?;
        let mut o = std::fs::OpenOptions::new();
        o.create(true).append(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
        o.open(d.join("helper.log")).ok()
    });
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(log.map_or_else(Stdio::null, Stdio::from));
    // Out of the window's process group: Ctrl-C in its terminal does not reach the Helper.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut c, 0);
    #[cfg(windows)]
    {
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::os::windows::process::CommandExt::creation_flags(&mut c, DETACHED_PROCESS | CREATE_NO_WINDOW);
    }
    let mut child = c.spawn()?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::tests::jobs_in;
    use crate::net::tests::Recorded;
    use crate::sources::Source;

    const LIMIT: Duration = Duration::from_secs(60);

    fn start(ours: Ours, dir: &Path, linger: Duration) -> std::thread::JoinHandle<io::Result<Ended>> {
        let dir = dir.to_path_buf();
        std::thread::spawn(move || serve_in(ours, &dir, false, linger, || jobs_in(&dir, Recorded::default(), Arc::default())))
    }

    /// A line to the Helper in `dir` as a window of `ours`, once it is up.
    fn line(dir: &Path, ours: Ours) -> Dialled {
        let wait = Instant::now();
        while !dir.join("helper.addr").exists() {
            assert!(wait.elapsed() < LIMIT, "the Helper never came");
            std::thread::sleep(Duration::from_millis(10));
        }
        dial_in(dir, ours)
    }

    #[test]
    fn only_one_helper_holds_the_lock() {
        let d = crate::tests::Dir::new("lock");
        let first = Lock::take(&d.0).unwrap();
        assert!(first.is_some());
        assert!(Lock::take(&d.0).unwrap().is_none(), "a second Helper does not get it");
        drop(first);
        assert!(Lock::take(&d.0).unwrap().is_some(), "free once the first has gone");
    }

    #[test]
    fn a_connection_without_the_token_is_dropped() {
        let d = crate::tests::Dir::new("token");
        let h = start(OURS, &d.0, Duration::from_millis(300));
        let Dialled::Line(mut c) = line(&d.0, OURS) else { panic!("the right token is served") };
        assert!(matches!(c.ask(&Request::Status), Ok(Reply::Status(s)) if s.version == VERSION && s.clients == 1));
        let text = std::fs::read_to_string(d.0.join("helper.addr")).unwrap();
        let addr = text.lines().next().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(d.0.join("helper.addr")).unwrap().permissions().mode() & 0o777, 0o600);
        }
        for first in [
            r#"{"op":"hello","token":"00000000000000000000000000000000","version":"0.1.0","protocol":1}"#,
            r#"{"op":"status"}"#,
        ] {
            let mut s = dial(addr).unwrap();
            s.write_all(format!("{first}\n{{\"op\":\"status\"}}\n").as_bytes()).unwrap();
            let mut got = String::new();
            s.set_read_timeout(Some(LIMIT)).unwrap();
            assert_eq!(s.read_to_string(&mut got).unwrap_or(0), 0, "{first}: answered {got}");
        }
        drop(c);
        assert_eq!(h.join().unwrap().unwrap(), Ended::Idle);
        assert!(!d.0.join("helper.addr").exists());
    }

    /// Two ends of a connection.
    fn pair() -> (Stream, Stream) {
        #[cfg(unix)]
        return Stream::pair().unwrap();
        #[cfg(not(unix))]
        {
            let l = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
            let b = Stream::connect(l.local_addr().unwrap()).unwrap();
            (l.accept().unwrap().0, b)
        }
    }

    /// The hello has one deadline over all its reads: bytes that keep coming do not stretch it.
    #[test]
    fn the_hello_has_one_deadline() {
        let (a, mut b) = pair();
        b.write_all(b"xy").unwrap();
        let mut t = Timed { stream: a, by: Some(Instant::now()) };
        assert_eq!(t.read(&mut [0; 1]).unwrap_err().kind(), io::ErrorKind::TimedOut, "past its deadline, with bytes waiting");
        t.by = None;
        assert_eq!(t.read(&mut [0; 1]).unwrap(), 1, "a window reads without one");
    }

    /// A connection that has not brought the token is no window: it does not count, and does
    /// not keep the Helper from leaving.
    #[test]
    fn only_a_connection_with_the_token_counts() {
        let d = crate::tests::Dir::new("count");
        let h = start(OURS, &d.0, Duration::from_millis(300));
        let Dialled::Line(mut c) = line(&d.0, OURS) else { panic!("the right token is served") };
        let text = std::fs::read_to_string(d.0.join("helper.addr")).unwrap();
        let silent: Vec<Stream> = (0..3).map(|_| dial(text.lines().next().unwrap()).unwrap()).collect();
        // Dialled after the silent ones, so they were taken in first.
        let Dialled::Line(mut c2) = line(&d.0, OURS) else { panic!() };
        drop(c2.ask(&Request::Status));
        assert!(matches!(c.ask(&Request::Status), Ok(Reply::Status(s)) if s.clients == 2), "the two windows only");
        drop((c, c2));
        assert_eq!(h.join().unwrap().unwrap(), Ended::Idle, "left with the silent ones still open");
        drop(silent);
    }

    #[test]
    fn a_newer_window_takes_over_and_an_older_one_never_does() {
        let d = crate::tests::Dir::new("newer");
        let h = start(("1.2.0", 1), &d.0, LIMIT);
        // An older window of the same protocol is served; one of another is refused, and the
        // newer Helper stays.
        assert!(matches!(line(&d.0, ("1.1.9", 1)), Dialled::Line(_)));
        assert!(matches!(line(&d.0, ("1.0.0", 0)), Dialled::Newer(v) if v == "1.2.0"));
        assert!(!h.is_finished());
        // A newer window: the Helper leaves for its own.
        assert!(matches!(line(&d.0, ("1.10.0", 2)), Dialled::Nobody));
        assert_eq!(h.join().unwrap().unwrap(), Ended::Asked);
    }

    #[test]
    fn a_second_helper_leaves_at_once() {
        let d = crate::tests::Dir::new("second");
        let _lock = Lock::take(&d.0).unwrap().unwrap();
        let ended = serve_in(OURS, &d.0, false, LIMIT, || unreachable!("no jobs for a Helper that does not start"));
        assert_eq!(ended.unwrap(), Ended::AnotherRuns);
    }

    #[test]
    fn requests_reach_the_jobs() {
        let d = crate::tests::Dir::new("requests");
        let h = start(OURS, &d.0, Duration::from_millis(300));
        let Dialled::Line(mut c) = line(&d.0, OURS) else { panic!() };
        let pgn = d.0.join("a.pgn");
        std::fs::write(&pgn, crate::jobs::tests::PGN).unwrap();
        let lib = d.0.join("x.bragd");
        assert!(matches!(c.ask(&Request::Import { path: pgn, library: Some(lib.clone()), source: None }).unwrap(), Reply::Started { id: 1 }));
        assert!(matches!(c.ask(&Request::Cancel { id: 99 }).unwrap(), Reply::Error { .. }));
        assert!(matches!(c.ask(&Request::SetSource { source: "nope".into(), options: crate::sources::Twic.about().defaults() }).unwrap(), Reply::Error { .. }));
        let Reply::Sources { sources } = c.ask(&Request::Sources).unwrap() else { panic!() };
        assert_eq!(sources[0].0.id, "twic");
        let wait = Instant::now();
        loop {
            let Reply::Jobs { jobs } = c.ask(&Request::Jobs).unwrap() else { panic!() };
            if jobs[0].status == crate::jobs::Status::Done {
                assert_eq!(jobs[0].done, 2);
                break;
            }
            assert!(wait.elapsed() < LIMIT);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(matches!(c.ask(&Request::Restart).unwrap(), Reply::Done));
        assert_eq!(h.join().unwrap().unwrap(), Ended::Asked);
    }

    #[test]
    fn lifetime_decisions() {
        let ten = LINGER;
        assert!(!idle_exit(1, ten * 2, false, false, ten), "a window is connected");
        assert!(!idle_exit(0, ten - Duration::from_secs(1), false, false, ten));
        assert!(idle_exit(0, ten, false, false, ten));
        assert!(!idle_exit(0, ten * 10, false, true, ten), "start with my session");
        assert!(!idle_exit(0, ten * 10, true, false, ten), "a job runs");
        assert_eq!(greet(("1.2.0", 3), "1.10.0", 4), Greet::Leave);
        assert_eq!(greet(("1.2.0", 3), "1.2.0", 3), Greet::Serve);
        assert_eq!(greet(("1.2.0", 3), "1.1.0", 3), Greet::Serve);
        assert_eq!(greet(("1.2.0", 3), "1.1.0", 2), Greet::Refuse);
        assert_eq!(step(0, false, false), Step::Own { stay: false });
        assert_eq!(step(0, true, false), Step::Own { stay: true });
        assert_eq!(step(1, false, false), Step::Wait);
        assert_eq!((step(0, true, true), step(1, true, true), step(2, true, true)), (Step::Ask, Step::Ask, Step::Rescue));
        assert!(is_token("abc", "abc") && !is_token("abd", "abc") && !is_token("ab", "abc"));
        assert_eq!(token().unwrap().len(), 32);
        assert_ne!(token().unwrap(), token().unwrap());
    }
}
