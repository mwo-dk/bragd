//! An engine running, in whatever protocol it speaks: started installed or as a sealed image,
//! given a position, asked to think within a limit while it reports, stopped and quit. It is
//! killed and waited for when dropped: no engine is left running.

use serde::Serialize;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use crate::images;
use crate::network::CONTAINER_NETWORK;
use crate::options::OptionStore;
use crate::protocol::{About, Best, Dialect, Event, Info, Limit, Position, Protocol, Score, Step};
use crate::tools::{self, Runtime};

/// Where an engine runs.
#[derive(Clone, Debug, PartialEq)]
pub enum Program {
    /// A program installed here, with the arguments it needs (`gtp`, `--mode gtp`, `hub`).
    Installed { path: PathBuf, args: Vec<String> },
    /// A pinned image, run sealed (`images::sealed`) by the runtime `rt`, its tablebase folder
    /// (when there is one) read-only at `/syzygy`, and the evaluation network it needs (when it
    /// needs one, `network`) read-only at `network::CONTAINER_NETWORK`.
    Container { rt: Runtime, image: String, args: Vec<String>, syzygy: Option<PathBuf>, network: Option<PathBuf> },
}

/// Where a container sees the tablebases.
pub const CONTAINER_SYZYGY: &str = "/syzygy";

impl Program {
    /// What tells one engine from another, for the cache of reviewed games.
    pub fn key(&self) -> String {
        match self {
            Program::Installed { path, .. } => path.to_string_lossy().into_owned(),
            Program::Container { image, .. } => image.clone(),
        }
    }

    pub fn container(&self) -> bool {
        matches!(self, Program::Container { .. })
    }

    /// The command that starts it, and for a container its runtime and its name, to stop it by.
    fn command(&self) -> (std::process::Command, Option<(PathBuf, String)>) {
        match self {
            Program::Installed { path, args } => {
                let mut c = tools::command(path);
                c.args(args).env("LC_ALL", "C");
                (c, None)
            }
            Program::Container { rt, image, args, syzygy, network } => {
                static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                let base = image.rsplit('/').next().unwrap_or("engine").split([':', '@']).next().unwrap_or("engine");
                let name = format!("{base}-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
                let mounts: Vec<(PathBuf, &str)> = syzygy.iter().map(|s| (s.clone(), CONTAINER_SYZYGY)).chain(network.iter().map(|n| (n.clone(), CONTAINER_NETWORK))).collect();
                // Probed once per runtime and image, so `sealed` leaves out bounds it cannot keep.
                images::bounds_work(&rt.path, image);
                let mut c = images::sealed(rt, &name, image, &mounts);
                c.args(args);
                (c, Some((rt.path.clone(), name)))
            }
        }
    }
}

/// An engine to ask about positions, whatever its protocol.
pub trait Session {
    /// Its name and options, as it said when it started.
    fn about(&self) -> &About;
    fn set_option(&mut self, name: &str, value: &str) -> std::io::Result<()>;
    fn set_position(&mut self, p: &Position) -> std::io::Result<()>;
    /// Starts thinking about the position set; its reports come from `next`.
    fn go(&mut self, limit: Limit, lines: u32) -> std::io::Result<()>;
    /// What it said next, waiting at most `wait`; None when it said nothing in that time.
    fn next(&mut self, wait: Duration) -> std::io::Result<Option<Event>>;
    /// Tells it to stop thinking and name its move (where the protocol can).
    fn stop(&mut self) -> std::io::Result<()>;
    fn quit(&mut self);
}

fn failed(kind: std::io::ErrorKind) -> std::io::Error {
    std::io::Error::new(kind, "The engine stopped answering.")
}

/// An engine process speaking a dialect over its standard input and output.
pub struct Engine {
    child: Child,
    stdin: ChildStdin,
    /// Its lines, read on a thread of their own, so a silent engine never blocks the asker.
    out: Receiver<std::io::Result<String>>,
    dialect: Box<dyn Dialect>,
    about: About,
    /// Events read but not yet handed out (one line can say several things).
    queued: VecDeque<Event>,
    /// A container's runtime and name: stopped by it too, as killing `podman run` leaves the
    /// container running.
    container: Option<(PathBuf, String)>,
    /// It runs in a container given its network at `CONTAINER_NETWORK`.
    network: bool,
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.quit();
        // A moment to quit by itself, then killed.
        for _ in 0..20 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some((rt, name)) = &self.container {
            let _ = tools::command(rt).args(["kill", name]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
        }
    }
}

impl Engine {
    /// Starts `program` and waits until it is ready in `protocol`, reading its name and
    /// options; given up on after `wait`, or as soon as `stop` is true.
    pub fn start(program: &Program, protocol: Protocol, wait: Duration, stop: &dyn Fn() -> bool) -> std::io::Result<Engine> {
        let (mut cmd, container) = program.command();
        let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| failed(std::io::ErrorKind::BrokenPipe))?;
        let stdout = child.stdout.take().ok_or_else(|| failed(std::io::ErrorKind::BrokenPipe))?;
        // Bounded, so an engine that talks while nobody listens waits instead of filling memory.
        let (tx, out) = std::sync::mpsc::sync_channel(4096);
        // Ends when the engine's output closes, at the latest when Drop kills it.
        std::thread::spawn(move || read_lines(BufReader::new(stdout), |l| tx.send(l).is_ok()));
        let dialect = protocol.dialect();
        let network = matches!(program, Program::Container { network: Some(_), .. });
        let mut e = Engine { child, stdin, out, dialect, about: About::default(), queued: VecDeque::new(), container, network };
        let hello = e.dialect.hello();
        e.send(&hello)?;
        let deadline = Instant::now() + wait;
        loop {
            let line = e.line(deadline, stop)?;
            match e.dialect.greeted(&line, &mut e.about) {
                Step::More => {}
                Step::Reply(text) => e.send(&text)?,
                Step::Ready(text) => {
                    e.send(&text)?;
                    return Ok(e);
                }
            }
        }
    }

    /// Starts `program` in the first of `protocols` it answers, each given `wait`: an engine
    /// chosen by its path, whose protocol is not known (UCI, else UCCI, for xiangqi).
    pub fn start_any(program: &Program, protocols: &[Protocol], wait: Duration, stop: &dyn Fn() -> bool) -> std::io::Result<Engine> {
        let mut last = failed(std::io::ErrorKind::InvalidInput);
        for &p in protocols {
            match Engine::start(program, p, wait, stop) {
                Ok(e) => return Ok(e),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => return Err(e),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    pub fn protocol(&self) -> Protocol {
        self.dialect.protocol()
    }

    /// Sends the options the user set for this engine (`OptionStore::to_send`), the tablebase
    /// folder to an engine that takes one, and its network to an image given one (`EvalFile`).
    /// The options not sent come back with why.
    pub fn configure(&mut self, store: &OptionStore, syzygy: Option<&str>) -> std::io::Result<Vec<(String, String)>> {
        let (send, ignored) = store.to_send(&self.about, self.container.is_some());
        let mut text: String = send.iter().map(|(n, v)| self.dialect.set_option(n, v)).collect();
        if let (Some(path), Some(o)) = (syzygy.filter(|p| !p.is_empty()), self.about.option("SyzygyPath")) {
            let path = if self.container.is_some() { CONTAINER_SYZYGY } else { path };
            text.push_str(&self.dialect.set_option(&o.name, path));
        }
        if self.network
            && let Some(o) = self.about.option("EvalFile")
        {
            text.push_str(&self.dialect.set_option(&o.name, CONTAINER_NETWORK));
        }
        self.send(&text)?;
        Ok(ignored)
    }

    fn send(&mut self, text: &str) -> std::io::Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        self.stdin.write_all(text.as_bytes())?;
        self.stdin.flush()
    }

    /// The next line it writes; Err when it ended, `stop` turned true or `deadline` passed.
    fn line(&self, deadline: Instant, stop: &dyn Fn() -> bool) -> std::io::Result<String> {
        loop {
            if stop() {
                return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "stopped"));
            }
            let wait = deadline.saturating_duration_since(Instant::now());
            if wait.is_zero() {
                return Err(failed(std::io::ErrorKind::TimedOut));
            }
            match self.out.recv_timeout(wait.min(Duration::from_millis(100))) {
                Ok(l) => return l,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(failed(std::io::ErrorKind::UnexpectedEof)),
            }
        }
    }
}

impl Session for Engine {
    fn about(&self) -> &About {
        &self.about
    }

    fn set_option(&mut self, name: &str, value: &str) -> std::io::Result<()> {
        let text = self.dialect.set_option(name, value);
        self.send(&text)
    }

    fn set_position(&mut self, p: &Position) -> std::io::Result<()> {
        self.queued.clear();
        let text = self.dialect.position(&self.about, p);
        self.send(&text)
    }

    fn go(&mut self, limit: Limit, lines: u32) -> std::io::Result<()> {
        let text = self.dialect.go(&self.about, limit, lines);
        self.send(&text)
    }

    fn next(&mut self, wait: Duration) -> std::io::Result<Option<Event>> {
        let deadline = Instant::now() + wait;
        loop {
            if let Some(e) = self.queued.pop_front() {
                return Ok(Some(e));
            }
            let line = match self.line(deadline, &|| false) {
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => return Ok(None),
                r => r?,
            };
            let events = self.dialect.read(&line);
            self.queued.extend(events);
        }
    }

    fn stop(&mut self) -> std::io::Result<()> {
        let text = self.dialect.stop();
        self.send(&text)
    }

    fn quit(&mut self) {
        let text = self.dialect.quit();
        let _ = self.send(&text);
    }
}

/// What an engine says about a position: its best lines, the best first, each scored from the
/// first Side's view (White, Red; Black in shogi and go), and the move it settled on.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Analysis {
    pub engine: String,
    pub depth: u32,
    pub lines: Vec<Line>,
    pub best: Option<String>,
}

/// One of its lines, in the engine's notation.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Line {
    pub score: Option<Score>,
    pub pv: Vec<String>,
}

/// The most lines asked for.
pub const MAX_LINES: u32 = 5;

/// Asks `s` about `p` until `limit`, for `lines` lines, handing each report to `report` as it
/// comes. One that has not answered by `Limit::cap` is told to stop, and given up on 30 s after
/// that. `stop` is asked between its reports: true ends it, and the engine is of no more use.
pub fn think(s: &mut dyn Session, p: &Position, limit: Limit, lines: u32, stop: &dyn Fn() -> bool, report: &mut dyn FnMut(&Info)) -> std::io::Result<Analysis> {
    let wanted = lines.clamp(1, MAX_LINES) as usize;
    s.set_position(p)?;
    s.go(limit, wanted as u32)?;
    let first = p.first_to_move();
    let mut a = Analysis { engine: s.about().name.clone(), ..Default::default() };
    let mut slots: Vec<Line> = vec![Line::default(); wanted];
    let mut deadline = Instant::now() + limit.cap();
    let mut told = false;
    loop {
        if stop() {
            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "stopped"));
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            if told {
                return Err(failed(std::io::ErrorKind::TimedOut));
            }
            // Past its cap: told to stop, which makes it name its move.
            told = true;
            s.stop()?;
            deadline = Instant::now() + Duration::from_secs(30);
            continue;
        }
        match s.next(left.min(Duration::from_millis(100)))? {
            None => {}
            Some(Event::Info(i)) => {
                report(&i);
                let Some(slot) = slots.get_mut(i.line.wrapping_sub(1)) else { continue };
                if i.line == 1
                    && let Some(d) = i.depth
                {
                    a.depth = d;
                }
                if let Some(sc) = i.score {
                    slot.score = Some(if first { sc } else { sc.flipped() });
                }
                if !i.pv.is_empty() {
                    slot.pv = i.pv;
                }
            }
            Some(Event::Best(Best { mv, .. })) => {
                // An engine that gave no line still names its move.
                if slots[0].pv.is_empty()
                    && let Some(m) = &mv
                {
                    slots[0].pv = vec![m.clone()];
                }
                a.best = mv;
                a.lines = slots.into_iter().filter(|l| !l.pv.is_empty() || l.score.is_some()).collect();
                return Ok(a);
            }
            Some(Event::Refused(why)) => return Err(std::io::Error::other(format!("The engine refused: {why}"))),
        }
    }
}

/// The longest line kept from an engine; the rest of a longer one is dropped.
const LINE_MAX: u64 = 64 * 1024;

/// Hands each line `r` gives to `send` (false stops), cut at `LINE_MAX` bytes, so an engine that
/// never writes a newline cannot make Bragd hold all it writes.
fn read_lines(mut r: impl BufRead, mut send: impl FnMut(std::io::Result<String>) -> bool) {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let line = match std::io::Read::take(&mut r, LINE_MAX).read_until(b'\n', &mut buf) {
            Ok(0) => return,
            Ok(_) => {
                if buf.last() != Some(&b'\n') && buf.len() as u64 == LINE_MAX && r.skip_until(b'\n').is_err() {
                    return;
                }
                while matches!(buf.last(), Some(b'\n' | b'\r')) {
                    buf.pop();
                }
                Ok(String::from_utf8_lossy(&buf).into_owned())
            }
            Err(e) => Err(e),
        };
        let failed = line.is_err();
        if !send(line) || failed {
            return;
        }
    }
}

#[cfg(test)]
mod line_tests {
    use super::*;

    #[test]
    fn a_line_without_end_is_cut() {
        let mut text = vec![b'x'; 1 << 20];
        text.extend_from_slice(b"\r\nuciok\nlast");
        let mut got = Vec::new();
        read_lines(&text[..], |l| {
            got.push(l.unwrap());
            true
        });
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].len() as u64, LINE_MAX);
        assert_eq!(&got[1..], ["uciok", "last"]);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn installed(path: PathBuf) -> Program {
        Program::Installed { path, args: vec![] }
    }

    /// A UCI engine of a few lines: what it says is read as Stockfish's is, Black's score turned
    /// to White's side, and what it is sent is written down.
    #[test]
    fn a_uci_engine_asked() {
        let d = crate::test_dir("uci");
        let log = d.join("sent");
        let p = crate::fake_program(
            &d,
            "engine",
            &format!(
                "while read l; do echo \"$l\" >> '{}'; case \"$l\" in uci) echo 'id name Fake 1'; echo 'option name MultiPV type spin default 1 min 1 max 500'; echo 'option name SyzygyPath type string default <empty>'; echo 'option name Threads type spin default 1 min 1 max 512'; echo uciok;; isready) echo readyok;; go*) echo 'info depth 5 score cp 40 lowerbound pv a7a6'; echo 'info depth 9 seldepth 12 multipv 1 score cp 30 nodes 99 pv e7e5 g1f3 b8c6'; echo 'info depth 9 multipv 2 score mate -4 pv c7c5'; echo 'bestmove e7e5 ponder g1f3';; quit) exit 0;; esac; done",
                log.display()
            ),
        );
        let mut e = Engine::start(&installed(p.clone()), Protocol::Uci, Duration::from_secs(10), &|| false).unwrap();
        assert_eq!(e.about().name, "Fake 1");
        let mut store = OptionStore::default();
        store.0.insert("Fake".into(), toml::from_str("Threads = 1\nContempt = 3").unwrap());
        let ignored = e.configure(&store, Some("/tables/syzygy")).unwrap();
        assert_eq!(ignored.len(), 1);
        let pos = Position::new("chess", &["e2e4"]);
        let mut reports = 0;
        let a = think(&mut e, &pos, Limit::Time(100), 3, &|| false, &mut |_| reports += 1).unwrap();
        assert_eq!((a.engine.as_str(), a.depth, a.lines.len(), a.best.as_deref(), reports), ("Fake 1", 9, 2, Some("e7e5"), 2));
        assert_eq!(a.lines[0], Line { score: Some(Score::Cp(-30)), pv: vec!["e7e5".into(), "g1f3".into(), "b8c6".into()] });
        // Black to move and mating: plus for Black is minus from White's side.
        assert_eq!(a.lines[1].score, Some(Score::Mate(4)));
        drop(e);
        let sent = std::fs::read_to_string(&log).unwrap();
        for l in ["uci", "setoption name Threads value 1", "setoption name SyzygyPath value /tables/syzygy", "setoption name MultiPV value 3", "position startpos moves e2e4", "go movetime 100", "quit"] {
            assert!(sent.lines().any(|s| s == l), "{l} not in {sent}");
        }
        assert!(!sent.contains("Contempt"), "{sent}");
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// An engine that answers `go` only when told to stop is stopped at its cap; one that never
    /// answers is given up on when asked to stop; one that is not UCI is tried as UCCI.
    #[test]
    fn engines_that_hang_or_speak_otherwise() {
        let d = crate::test_dir("uci-slow");
        let slow = crate::fake_program(&d, "slow", "while read l; do case \"$l\" in uci) echo uciok;; stop) echo 'bestmove e2e4';; quit) exit 0;; esac; done");
        let mut e = Engine::start(&installed(slow), Protocol::Uci, Duration::from_secs(10), &|| false).unwrap();
        let a = think(&mut e, &Position::new("chess", &[]), Limit::Time(1), 1, &|| false, &mut |_| {}).unwrap();
        assert_eq!(a.best.as_deref(), Some("e2e4"));
        let silent = crate::fake_program(&d, "silent", "exec sleep 1000");
        let t = Instant::now();
        let soon = || t.elapsed() > Duration::from_millis(300);
        assert!(Engine::start(&installed(silent), Protocol::Uci, Duration::from_secs(30), &soon).is_err());
        assert!(t.elapsed() < Duration::from_secs(10), "given up on when asked");
        let ucci = crate::fake_program(&d, "ucci", "while read l; do case \"$l\" in ucci) echo 'id name Old Xiangqi'; echo ucciok;; quit) exit 0;; esac; done");
        let e = Engine::start_any(&installed(ucci), &[Protocol::Uci, Protocol::Ucci], Duration::from_secs(3), &|| false).unwrap();
        assert_eq!((e.protocol(), e.about().name.as_str()), (Protocol::Ucci, "Old Xiangqi"));
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// A GTP engine through the whole session: acks swallowed, its move read.
    #[test]
    fn a_gtp_engine_asked() {
        let d = crate::test_dir("gtp");
        let p = crate::fake_program(&d, "gnugo", "while read l; do case \"$l\" in name) printf '= GNU Go\\n\\n';; version) printf '= 3.8\\n\\n';; genmove*) printf '= C3\\n\\n';; quit) printf '=\\n\\n'; exit 0;; *) printf '=\\n\\n';; esac; done");
        let mut e = Engine::start(&installed(p), Protocol::Gtp, Duration::from_secs(10), &|| false).unwrap();
        assert_eq!(e.about().name, "GNU Go 3.8");
        let a = think(&mut e, &Position::new("go-9", &["E5"]), Limit::Time(1000), 1, &|| false, &mut |_| {}).unwrap();
        assert_eq!(a.best.as_deref(), Some("C3"));
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// An image given its network gets it read-only, and is told where it is (`EvalFile`); an
    /// engine without one is told nothing of it.
    #[test]
    fn an_image_is_given_its_network_read_only() {
        let rt = Runtime { name: "podman", path: PathBuf::from("/nonexistent/podman") };
        let net = PathBuf::from("/data/bragd/networks/pikafish/pikafish.nnue");
        let p = Program::Container { rt, image: "ghcr.io/x/bragd-pikafish:1@sha256:3".into(), args: vec![], syzygy: None, network: Some(net) };
        let (c, _) = p.command();
        let args: Vec<String> = c.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(args.contains(&format!("type=bind,\"source=/data/bragd/networks/pikafish/pikafish.nnue\",target={CONTAINER_NETWORK},readonly")), "{args:?}");
        let d = crate::test_dir("evalfile");
        let log = d.join("sent");
        let engine = crate::fake_program(
            &d,
            "engine",
            &format!("while read l; do echo \"$l\" >> '{}'; case \"$l\" in uci) echo 'id name Pikafish 2026-09-06'; echo 'option name EvalFile type string default pikafish.nnue'; echo uciok;; isready) echo readyok;; quit) exit 0;; esac; done", log.display()),
        );
        for network in [false, true] {
            let mut e = Engine::start(&installed(engine.clone()), Protocol::Uci, Duration::from_secs(10), &|| false).unwrap();
            // As `start` sets it for a container given a network.
            e.network = network;
            e.configure(&OptionStore::default(), None).unwrap();
            drop(e);
            let sent = std::fs::read_to_string(&log).unwrap();
            assert_eq!(sent.contains(&format!("setoption name EvalFile value {CONTAINER_NETWORK}")), network, "{sent}");
            std::fs::remove_file(&log).unwrap();
        }
        std::fs::remove_dir_all(&d).unwrap();
    }
}
