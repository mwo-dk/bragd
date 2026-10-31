//! Bragd's engine images: run sealed, pulled with progress and retried on passing network
//! faults, kept up to date, and removed when unused for 90 days.
//!
//! Every image is pinned by its digest (`name:tag@sha256:…`): a pull fetches exactly that
//! build, never whatever the tag points at that day, and an unpinned image is never pulled. A
//! new version of Bragd brings new pins; a round of `housekeeping` then pulls the new pin of each
//! image whose old pin an engine ran in, removes the old copy (`superseded`, `move_pins`), and
//! removes the images no engine used for 90 days.
//!
//! Only Bragd's own images are ever touched: they are removed by their pin, never with
//! `image prune`, which would take the user's other untagged images too.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::tools::{self, Runtime};

const DAY: u64 = 24 * 3600;
/// An image no engine used for this many days goes.
pub const UNUSED_DAYS: u64 = 90;

/// The user every engine container runs as: `nobody`, owning nothing.
pub const USER: &str = "65534:65534";

/// The options that hold every engine container: a read-only root with a small `/tmp` in
/// memory, bounded memory and processes, no capabilities, no gaining of privileges. Given once,
/// as docker refuses a second `--tmpfs /tmp`.
pub const TIGHT: &[&str] = &["--read-only", "--tmpfs", "/tmp:rw,size=256m", "--memory=3g", "--pids-limit=256", "--cap-drop=ALL", "--security-opt", "no-new-privileges"];

/// `<rt> run` of an engine image, talking over stdin and stdout: removed when it ends, never
/// pulled by the run itself, no network, no SELinux relabelling of the user's folders, named
/// `name` so it can be killed, held by `TIGHT`, as `USER`, and each of `mounts` (a host folder
/// and where it appears, such as the tablebases at `/syzygy`) read-only. `HOME=/tmp`, as the root
/// is read-only, and `LC_ALL=C`, so numbers are written with a full stop.
pub fn sealed(rt: &Runtime, name: &str, image: &str, mounts: &[(PathBuf, &str)]) -> Command {
    let mut c = tools::command(&rt.path);
    c.args(["run", "--rm", "-i", "--pull=never", "--network=none", "--security-opt", "label=disable", "--name", name]);
    c.args(tight(&rt.path));
    c.args(["--user", USER, "-e", "HOME=/tmp", "-e", "LC_ALL=C"]);
    for (host, at) in mounts {
        c.arg("--mount").arg(bind(host, at, true));
    }
    c.arg(image);
    c
}

/// `--mount` of the folder `host` at `at`: not `-v`, which splits at every `:`. The source is
/// one quoted CSV field, as podman and docker read it, so a `,` or `"` in the path stays in it.
pub fn bind(host: &Path, at: &str, ro: bool) -> String {
    let src = host.display().to_string().replace('"', "\"\"");
    format!("type=bind,\"source={src}\",target={at}{}", if ro { ",readonly" } else { "" })
}

/// What `bounds_work` found, by runtime and image.
static BOUNDS: Mutex<BTreeMap<(PathBuf, String), bool>> = Mutex::new(BTreeMap::new());

/// `TIGHT`, less its memory and process bounds where a probe (`bounds_work`) found that `rt`
/// cannot keep them; all of it while nothing says so.
pub fn tight(rt: &Path) -> Vec<&'static str> {
    bounded(!BOUNDS.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|((r, _), ok)| r == rt && !ok))
}

/// `TIGHT`, with its memory and process bounds only when `bounds`.
pub fn bounded(bounds: bool) -> Vec<&'static str> {
    TIGHT.iter().copied().filter(|a| bounds || !(a.starts_with("--memory") || a.starts_with("--pids-limit"))).collect()
}

/// Whether the runtime at `rt` can bound a container's memory and processes: a rootless podman
/// whose cgroup controllers are not delegated cannot. Found by a container of `image` that only
/// runs `true` with both bounds. It fails closed: only a probe the runtime refused for want of
/// those controllers turns the bounds off; any other failure keeps them on and asks again.
pub fn bounds_work(rt: &Path, image: &str) -> bool {
    let key = (rt.to_path_buf(), image.to_string());
    if let Some(k) = BOUNDS.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return *k;
    }
    let mut c = tools::command(rt);
    c.args(["run", "--rm", "--pull=never", "--network=none", "--memory=64m", "--pids-limit=16", "--entrypoint", "true", image]);
    let said = tools::capture(c, Duration::from_secs(60), 4096).ok().and_then(|o| bounds_said(o.status.success(), &String::from_utf8_lossy(&o.stderr)));
    match said {
        Some(ok) => {
            BOUNDS.lock().unwrap_or_else(|e| e.into_inner()).insert(key, ok);
            ok
        }
        None => true,
    }
}

/// What a probe of `bounds_work` says: it ran (they work), it was refused for want of the
/// controllers (they do not), or it failed some other way (not known: None).
fn bounds_said(ran: bool, stderr: &str) -> Option<bool> {
    if ran {
        return Some(true);
    }
    let e = stderr.to_lowercase();
    (["cgroup", "memory.max", "pids.max", "controller"].iter().any(|w| e.contains(w)) && !e.contains("image not known") && !e.contains("no such image")).then_some(false)
}

/// The last progress line of every pull under way, by image.
static PULLING: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

/// The latest progress line of every pull under way, by image, for the app to show.
pub fn pulling() -> BTreeMap<String, String> {
    PULLING.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// A pull waiting to try again after a passing fault: the fault, and when it tries again (Unix seconds).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Waiting {
    pub error: PullError,
    pub at: u64,
}

/// The pulls waiting to try again, by image.
static WAITING: Mutex<BTreeMap<String, Waiting>> = Mutex::new(BTreeMap::new());

/// The pulls waiting to try again, by image, for the app to show.
pub fn waiting() -> BTreeMap<String, Waiting> {
    WAITING.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Why a pull failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Fault {
    /// The network or the registry had a moment: worth trying again.
    Network,
    Registry,
    /// It will fail again: the image is gone, the disk is full, it is not pinned.
    Lasting,
    Stopped,
}

/// A pull that failed: what kind of fault, after how many tries, and the runtime's last words.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PullError {
    pub image: String,
    pub fault: Fault,
    pub tries: u32,
    pub detail: String,
}

impl std::fmt::Display for PullError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let what = short(&self.image);
        let tries = if self.tries > 1 { format!(" Tried {} times.", self.tries) } else { String::new() };
        match self.fault {
            Fault::Network => write!(f, "{what} couldn't be downloaded: no network.{tries}"),
            Fault::Registry => write!(f, "{what} couldn't be downloaded: the registry is busy.{tries}"),
            Fault::Stopped => write!(f, "{what}: download stopped."),
            Fault::Lasting => write!(f, "{what} couldn't be downloaded: {}", self.detail),
        }
    }
}

/// What kind of fault the runtime's words `said` describe. A lasting one is looked for first:
/// an image that is gone stays gone, however the network is.
pub fn fault(said: &str) -> Fault {
    let s = said.to_lowercase();
    let any = |words: &[&str]| words.iter().any(|w| s.contains(w));
    if any(&["manifest unknown", "not found", "unauthorized", "denied", "no space left", "invalid reference", "not pinned"]) {
        Fault::Lasting
    } else if any(&["toomanyrequests", "too many requests", " 429", " 500", " 502", " 503", " 504", "bad gateway", "service unavailable", "gateway timeout", "internal server error"]) {
        Fault::Registry
    } else if any(&["name resolution", "no such host", "lookup ", "dial tcp", "connection refused", "connection reset", "i/o timeout", "timed out", "timeout", "tls handshake", "network is unreachable", "unexpected eof", "no route to host"]) {
        Fault::Network
    } else {
        Fault::Lasting
    }
}

/// The waits between tries of one pull when a fault passes: a few seconds, then longer.
pub const BACKOFF: &[Duration] = &[Duration::from_secs(5), Duration::from_secs(30), Duration::from_secs(120)];

/// When to try again after a pull failed `rounds` times in a row on a passing fault: in five
/// minutes, then fifteen, then every hour.
pub fn next_try(rounds: u32) -> Duration {
    match rounds {
        0 | 1 => Duration::from_secs(5 * 60),
        2 => Duration::from_secs(15 * 60),
        _ => Duration::from_secs(3600),
    }
}

/// Pull `image`, streaming its progress into `pulling()`. On a passing network or registry fault
/// it waits (`BACKOFF`, then `next_try`, through `sleep`, which says false when stopped while
/// waiting) and tries again, until it works, a fault lasts, or it is stopped; while it waits,
/// `waiting()` names it. `stop` ends it: the pull is killed, and the runtime keeps what it fetched.
pub fn pull(rt: &Path, image: &str, stop: &AtomicBool, sleep: &dyn Fn(Duration) -> bool) -> Result<(), PullError> {
    let mut tries = 0;
    loop {
        tries += 1;
        match pull_once(rt, image, stop) {
            Ok(()) => return Ok(()),
            Err(mut e) => {
                e.tries = tries;
                if !matches!(e.fault, Fault::Network | Fault::Registry) {
                    PULLING.lock().unwrap_or_else(|e| e.into_inner()).remove(image);
                    return Err(e);
                }
                let wait = BACKOFF.get(tries as usize - 1).copied().unwrap_or_else(|| next_try(tries - BACKOFF.len() as u32));
                let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                PULLING.lock().unwrap_or_else(|e| e.into_inner()).remove(image);
                WAITING.lock().unwrap_or_else(|e| e.into_inner()).insert(image.to_string(), Waiting { error: e.clone(), at: now + wait.as_secs() });
                let go_on = sleep(wait) && !stop.load(Ordering::Relaxed);
                WAITING.lock().unwrap_or_else(|e| e.into_inner()).remove(image);
                if !go_on {
                    return Err(PullError { fault: Fault::Stopped, ..e });
                }
            }
        }
    }
}

/// One try of `pull`.
fn pull_once(rt: &Path, image: &str, stop: &AtomicBool) -> Result<(), PullError> {
    let fail = |fault, detail: String| PullError { image: image.to_string(), fault, tries: 1, detail };
    if !image.contains("@sha256:") {
        return Err(fail(Fault::Lasting, "it is not pinned by its digest.".into()));
    }
    PULLING.lock().unwrap_or_else(|e| e.into_inner()).insert(image.to_string(), String::new());
    let result = (|| {
        let mut child = tools::command(rt).args(["pull", image]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| fail(Fault::Lasting, e.to_string()))?;
        // docker reports on stdout, podman on stderr; errors come on stderr.
        let (out, err) = (child.stdout.take(), child.stderr.take());
        let (a, b) = (image.to_string(), image.to_string());
        let t = std::thread::spawn(move || out.map(|o| follow(o, &a)).unwrap_or_default());
        let e = std::thread::spawn(move || err.map(|e| follow(e, &b)).unwrap_or_default());
        let status = loop {
            if stop.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(fail(Fault::Stopped, String::new()));
            }
            if let Some(s) = child.try_wait().map_err(|e| fail(Fault::Lasting, e.to_string()))? {
                break s;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let _ = t.join();
        let last = e.join().unwrap_or_default();
        if status.success() { Ok(()) } else { Err(fail(fault(&last), last)) }
    })();
    PULLING.lock().unwrap_or_else(|e| e.into_inner()).remove(image);
    result
}

/// Keeps the latest line of `r` (progress bars redraw with `\r`) in `PULLING` for `image`;
/// returns the last one.
fn follow(mut r: impl Read, image: &str) -> String {
    let (mut buf, mut acc, mut last) = ([0u8; 4096], String::new(), String::new());
    loop {
        let n = match r.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        acc.push_str(&String::from_utf8_lossy(&buf[..n]));
        let Some(i) = acc.rfind(['\n', '\r']) else { continue };
        if let Some(line) = acc[..i].rsplit(['\n', '\r']).map(str::trim).find(|l| !l.is_empty()) {
            last = line.to_string();
            PULLING.lock().unwrap_or_else(|e| e.into_inner()).insert(image.to_string(), last.clone());
        }
        acc.drain(..=i);
    }
    if let Some(rest) = Some(acc.trim()).filter(|l| !l.is_empty()) {
        last = rest.to_string();
    }
    last
}

/// An image's name as texts show it: without the digest it is pinned to.
pub fn short(image: &str) -> &str {
    image.split('@').next().unwrap_or(image)
}

/// When each of Bragd's pinned images was last used, and when `housekeeping` last ran: kept by
/// the caller with the app's state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ImageState {
    pub used: BTreeMap<String, u64>,
    pub checked: u64,
}

impl ImageState {
    /// An engine just ran in `image`.
    pub fn used(&mut self, image: &str, now: u64) {
        self.used.insert(image.to_string(), now);
    }

    /// Is a round of `housekeeping` due at `now`: once a day.
    pub fn due(&self, now: u64) -> bool {
        now.saturating_sub(self.checked) >= DAY
    }

    /// Of `pulled`, the images no engine used for `UNUSED_DAYS`. One with no use recorded counts
    /// as used now, so nothing is removed the first time it is looked at.
    pub fn stale(&mut self, pulled: &[String], now: u64) -> Vec<String> {
        pulled
            .iter()
            .filter(|i| {
                let last = *self.used.entry(i.to_string()).or_insert(now);
                now.saturating_sub(last) >= UNUSED_DAYS * DAY
            })
            .cloned()
            .collect()
    }

    /// Copies of Bragd's images an engine ran in that are pinned no more, each with the pin
    /// that took its place (`current`): the same name and tag under another digest, or the
    /// bare tag. Nothing else is ever part of it.
    pub fn superseded(&self, current: &[String]) -> Vec<(String, String)> {
        self.used
            .keys()
            .filter(|old| !current.contains(old))
            .filter_map(|old| current.iter().find(|new| short(new) == short(old)).map(|new| (old.clone(), new.clone())))
            .collect()
    }
}

/// Moves each pin of `pairs` (`superseded`): when the runtime has the old copy, the new pin is
/// pulled and the old copy removed by its name. The old images done with are returned, to be
/// forgotten. A pull that fails keeps the old copy for the next round.
pub fn move_pins(rt: &Path, pairs: &[(String, String)], pull: impl Fn(&str) -> Result<(), PullError>) -> Vec<String> {
    pairs.iter().filter(|(old, new)| image_size(rt, old).is_none() || (pull(new).is_ok() && remove(rt, old).is_ok())).map(|(old, _)| old.clone()).collect()
}

/// Of `images`, those the runtime `rt` has, each once.
pub fn pulled(rt: &Path, images: &[String]) -> Vec<String> {
    let mut v: Vec<String> = images.iter().filter(|i| !i.is_empty()).cloned().collect();
    v.sort();
    v.dedup();
    v.retain(|i| image_size(rt, i).is_some());
    v
}

/// The bytes of the image named exactly `image`, when the runtime `rt` has it.
pub fn image_size(rt: &Path, image: &str) -> Option<u64> {
    let out = tools::command(rt).args(["image", "inspect", "--format", "{{.Size}}", image]).stdin(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().parse().ok()).flatten()
}

/// `<rt> image rm what`, a name or an id; the runtime's last error line when it refuses.
pub fn remove(rt: &Path, what: &str) -> Result<(), String> {
    let out = tools::command(rt).args(["image", "rm", what]).stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&out.stderr).lines().rev().find(|l| !l.trim().is_empty()).unwrap_or_default().to_string())
}

/// How many bytes the runtime's images take on disk together, shared layers counted once
/// (`system df`): podman says it in bytes, docker in rounded units ("1.2GB").
pub fn storage(rt: &Path) -> Option<u64> {
    let out = tools::command(rt).args(["system", "df", "--format", "json"]).stdin(Stdio::null()).output().ok()?;
    out.status.success().then(|| images_size(&String::from_utf8_lossy(&out.stdout))).flatten()
}

/// The images' row of `system df --format json`: podman's array, or docker's one object a line.
fn images_size(said: &str) -> Option<u64> {
    let rows: Vec<serde_json::Value> = serde_json::from_str(said).ok().or_else(|| said.lines().map(|l| serde_json::from_str(l).ok()).collect())?;
    let row = rows.iter().find(|r| r["Type"] == "Images")?;
    row["RawSize"].as_u64().or_else(|| units(row["Size"].as_str()?))
}

/// "1.234GB", "512.5kB", "0B": docker's sizes, in powers of 1000.
fn units(s: &str) -> Option<u64> {
    let at = s.find(|c: char| c.is_ascii_alphabetic())?;
    let n: f64 = s[..at].trim().parse().ok()?;
    let times = match s[at..].trim().to_ascii_uppercase().as_str() {
        "B" => 1e0,
        "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        _ => return None,
    };
    Some((n * times) as u64)
}

/// Removes `image`: the bytes that freed, which are only the layers no other image uses.
/// `None` when the runtime would not say.
pub fn remove_freeing(rt: &Path, image: &str) -> Result<Option<u64>, String> {
    let before = storage(rt);
    remove(rt, image)?;
    Ok(before.zip(storage(rt)).map(|(b, a)| b.saturating_sub(a)))
}

/// A round of housekeeping at `now`: each image an engine ran in whose pin a new Bragd moved on
/// is pulled at its new pin and the old copy removed (`superseded`, `move_pins`); then each of
/// Bragd's images the runtime has that no engine used for `UNUSED_DAYS` is removed. Only images
/// of `current` (today's pins) and of `st` are looked at, never the user's others. What is gone
/// is forgotten.
pub fn housekeeping(rt: &Path, st: &mut ImageState, current: &[String], now: u64, pull: impl Fn(&str) -> Result<(), PullError>) {
    for old in move_pins(rt, &st.superseded(current), pull) {
        st.used.remove(&old);
    }
    let ours: Vec<String> = current.iter().chain(st.used.keys()).cloned().collect();
    for old in st.stale(&pulled(rt, &ours), now) {
        if remove(rt, &old).is_ok() {
            st.used.remove(&old);
        }
    }
    st.checked = now;
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIN: &str = "ghcr.io/mwo-dk/bragd-stockfish:17.1@sha256:0000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn images_size_from_podman_and_docker() {
        let podman = r#"[{"Type":"Images","Total":6,"Active":1,"RawSize":9024242441,"Size":"9.024GB"},{"Type":"Containers","Total":1,"Active":1,"RawSize":13146,"Size":"13.15kB"}]"#;
        assert_eq!(images_size(podman), Some(9_024_242_441));
        let docker = "{\"Active\":\"1\",\"Size\":\"1.234GB\",\"TotalCount\":\"6\",\"Type\":\"Images\"}\n{\"Size\":\"0B\",\"Type\":\"Containers\"}\n";
        assert_eq!(images_size(docker), Some(1_234_000_000));
        assert_eq!(units("512.5kB"), Some(512_500));
        assert_eq!(images_size("nonsense"), None);
    }

    #[test]
    fn housekeeping_comes_once_a_day() {
        let mut st = ImageState::default();
        let now = 100 * DAY;
        assert!(st.due(now));
        st.checked = now - DAY + 1;
        assert!(!st.due(now));
        st.checked = now - DAY;
        assert!(st.due(now));
    }

    #[test]
    fn images_unused_for_90_days_are_stale_and_one_never_seen_starts_now() {
        let mut st = ImageState::default();
        let now = 1000 * DAY;
        st.used("a", now - 91 * DAY);
        st.used("b", now - 10 * DAY);
        let pulled = ["a", "b", "c"].map(String::from);
        assert_eq!(st.stale(&pulled, now), ["a"]);
        assert_eq!(st.used.get("c"), Some(&now), "first seen: counted as used now");
        assert_eq!(st.stale(&pulled, now + 89 * DAY), ["a", "b"]);
        assert_eq!(st.stale(&pulled, now + 90 * DAY), ["a", "b", "c"]);
    }

    #[test]
    fn only_our_older_pins_are_superseded() {
        let tag = short(PIN).to_string();
        let mut st = ImageState::default();
        for i in [format!("{tag}@sha256:old"), tag.clone(), PIN.to_string(), "ghcr.io/me/stockfish:17.1".into(), "docker.io/library/alpine:3".into()] {
            st.used(&i, 1);
        }
        let mut got = st.superseded(&[PIN.to_string()]);
        got.sort();
        assert_eq!(got, [(tag.clone(), PIN.to_string()), (format!("{tag}@sha256:old"), PIN.to_string())]);
    }

    #[test]
    fn faults_that_pass_and_faults_that_last() {
        assert_eq!(fault("Error: initializing source docker://ghcr.io/x: pinging container registry ghcr.io: Get \"https://ghcr.io/v2/\": dial tcp: lookup ghcr.io: Temporary failure in name resolution"), Fault::Network);
        assert_eq!(fault("read tcp 10.0.0.2:5555->140.82.112.33:443: read: connection reset by peer"), Fault::Network);
        assert_eq!(fault("Error response from daemon: toomanyrequests: retry later"), Fault::Registry);
        assert_eq!(fault("received unexpected HTTP status: 503 Service Unavailable"), Fault::Registry);
        assert_eq!(fault("Error: reading manifest 17.1 in ghcr.io/mwo-dk/bragd-stockfish: manifest unknown"), Fault::Lasting);
        assert_eq!(fault("writing blob: no space left on device"), Fault::Lasting);
        assert_eq!(fault("something odd"), Fault::Lasting);
        let e = PullError { image: PIN.into(), fault: Fault::Network, tries: 4, detail: String::new() };
        assert_eq!(e.to_string(), "ghcr.io/mwo-dk/bragd-stockfish:17.1 couldn't be downloaded: no network. Tried 4 times.");
        assert_eq!((next_try(1), next_try(2), next_try(9)), (Duration::from_secs(300), Duration::from_secs(900), Duration::from_secs(3600)));
    }

    #[test]
    fn an_unpinned_image_is_never_pulled() {
        let e = pull(Path::new("/nonexistent/bragd-test-runtime"), "ghcr.io/mwo-dk/bragd-stockfish:latest", &AtomicBool::new(false), &|_| true).unwrap_err();
        assert_eq!((e.fault, e.tries), (Fault::Lasting, 1));
    }

    #[test]
    fn follow_keeps_last_complete_line() {
        assert_eq!(follow("one\ntwo\rthree".as_bytes(), "t"), "three");
        assert_eq!(follow("one\ntwo\r".as_bytes(), "t2"), "two");
    }

    #[test]
    fn engine_containers_are_sealed() {
        let rt = Runtime { name: "docker", path: PathBuf::from("/nonexistent/docker") };
        let c = sealed(&rt, "bragd-stockfish-1-0", PIN, &[(PathBuf::from("/home/me/syzygy, 3-4-5"), "/syzygy")]);
        let args: Vec<String> = c.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        for want in ["--rm", "--pull=never", "--network=none", "--read-only", "--cap-drop=ALL", "no-new-privileges", "--memory=3g", "--pids-limit=256", USER, "HOME=/tmp", "type=bind,\"source=/home/me/syzygy, 3-4-5\",target=/syzygy,readonly"] {
            assert!(args.iter().any(|a| a == want), "{want}: {args:?}");
        }
        assert_eq!(args.last().map(String::as_str), Some(PIN), "the image last: what follows would be its arguments");
        for one in ["--tmpfs", "--read-only", "--cap-drop=ALL"] {
            assert_eq!(args.iter().filter(|a| *a == one).count(), 1, "{one}");
        }
        assert!(!args.iter().any(|a| a.starts_with("type=bind") && !a.ends_with(",readonly")), "nothing writable");
    }

    #[test]
    fn bounds_stay_on_unless_the_runtime_lacks_them() {
        assert_eq!(bounds_said(true, ""), Some(true));
        assert_eq!(bounds_said(false, "Error: crun: opening file `memory.max` for writing: No such file or directory"), Some(false));
        assert_eq!(bounds_said(false, "Error: ghcr.io/x: image not known"), None, "not pulled yet: unknown, bounds stay on");
        assert_eq!(bounds_said(false, "Error: cannot connect to Podman"), None);
        assert_eq!(bounded(false), ["--read-only", "--tmpfs", "/tmp:rw,size=256m", "--cap-drop=ALL", "--security-opt", "no-new-privileges"]);
    }

    /// A fake runtime: `pull` fails with a DNS error the first two times (a counter file), then
    /// works; a pull of an image named `gone` fails for good.
    #[cfg(unix)]
    #[test]
    fn a_pull_tries_again_on_a_passing_fault() {
        let d = crate::test_dir("pull-retry");
        let rt = crate::fake_program(
            &d,
            "podman",
            &format!(
                "case \"$1\" in pull)\n  case \"$2\" in *gone*) echo 'Error: manifest unknown' >&2; exit 125;; esac\n  n=$(cat {0}/n 2>/dev/null || echo 0); echo $((n+1)) > {0}/n\n  echo 'Copying blob 1234 [=====>    ] 12MB / 60MB' >&2\n  if [ $n -lt 2 ]; then echo 'Error: dial tcp: lookup ghcr.io: Temporary failure in name resolution' >&2; exit 125; fi\n  echo 'Writing manifest to image destination' >&2;;\nesac\n",
                d.display()
            ),
        );
        let waits = std::cell::RefCell::new(vec![]);
        let sleep = |w: Duration| {
            waits.borrow_mut().push(w);
            true
        };
        pull(&rt, PIN, &AtomicBool::new(false), &sleep).unwrap();
        assert_eq!(*waits.borrow(), BACKOFF[..2], "two waits, the second longer");
        assert_eq!(std::fs::read_to_string(d.join("n")).unwrap().trim(), "3");
        assert!(!pulling().contains_key(PIN));
        // A lasting fault is not tried again.
        waits.borrow_mut().clear();
        let gone = PIN.replace("stockfish", "gone");
        let e = pull(&rt, &gone, &AtomicBool::new(false), &sleep).unwrap_err();
        assert_eq!((e.fault, e.tries, waits.borrow().len()), (Fault::Lasting, 1, 0));
        assert!(e.to_string().ends_with("Error: manifest unknown"), "{e}");
        // Always offline: it keeps waiting, longer each time, until stopped; while it waits,
        // `waiting()` says why and when it tries again.
        std::fs::write(d.join("n"), "-100").unwrap();
        let start = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let seen = std::cell::RefCell::new(vec![]);
        let six = |w: Duration| {
            let now = waiting().get(PIN).map(|x| (x.error.fault, x.error.tries, x.at));
            seen.borrow_mut().push((w, now));
            seen.borrow().len() < 6
        };
        let e = pull(&rt, PIN, &AtomicBool::new(false), &six).unwrap_err();
        assert_eq!(e.fault, Fault::Stopped);
        let seen = seen.into_inner();
        assert_eq!(seen.iter().map(|s| s.0).collect::<Vec<_>>(), [BACKOFF[0], BACKOFF[1], BACKOFF[2], next_try(1), next_try(2), next_try(3)]);
        for (i, (w, now)) in seen.iter().enumerate() {
            let (fault, tries, at) = now.unwrap_or_else(|| panic!("wait {i} not shown"));
            assert_eq!((fault, tries), (Fault::Network, i as u32 + 1));
            assert!(at >= start + w.as_secs() && at <= start + w.as_secs() + 60, "wait {i}: {at}");
        }
        assert!(!waiting().contains_key(PIN), "no longer waiting once stopped");
        // Stopped while waiting.
        std::fs::write(d.join("n"), "0").unwrap();
        let e = pull(&rt, PIN, &AtomicBool::new(false), &|_| false).unwrap_err();
        assert_eq!(e.fault, Fault::Stopped);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// A fake runtime that has the images named in `has` and writes what it removes to `rm.log`.
    #[cfg(unix)]
    #[test]
    fn move_pins_pulls_the_new_and_removes_the_old() {
        let d = crate::test_dir("pins");
        let rt = crate::fake_program(&d, "podman", &format!("case \"$1 $2\" in\n'image inspect') grep -qxF \"$5\" {0}/has && echo 1;;\n'image rm') echo \"$3\" >> {0}/rm.log;;\nesac\n", d.display()));
        std::fs::write(d.join("has"), "a:1@sha256:old\nb:1\n").unwrap();
        let pairs = [("a:1@sha256:old", "a:1@sha256:new"), ("b:1", "b:1@sha256:new"), ("c:1@sha256:gone", "c:1@sha256:new")].map(|(o, n)| (o.to_string(), n.to_string()));
        let pulled = std::cell::RefCell::new(vec![]);
        let done = move_pins(&rt, &pairs, |i| {
            pulled.borrow_mut().push(i.to_string());
            if i.starts_with("b:") { Err(PullError { image: i.into(), fault: Fault::Network, tries: 1, detail: String::new() }) } else { Ok(()) }
        });
        assert_eq!(*pulled.borrow(), ["a:1@sha256:new", "b:1@sha256:new"], "nothing pulled for a copy not there");
        assert_eq!(done, ["a:1@sha256:old", "c:1@sha256:gone"], "a failed pull keeps the old copy");
        assert_eq!(std::fs::read_to_string(d.join("rm.log")).unwrap(), "a:1@sha256:old\n");
        assert_eq!(super::pulled(&rt, &["b:1".into(), "x:1".into(), "b:1".into()]), ["b:1"]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Housekeeping moves an old pin on, removes Bragd's images unused for 90 days, and touches
    /// nothing else the runtime has.
    #[cfg(unix)]
    #[test]
    fn housekeeping_moves_pins_and_removes_what_went_unused() {
        let d = crate::test_dir("housekeeping");
        let rt = crate::fake_program(&d, "podman", &format!("echo \"$*\" >> {0}/log\ncase \"$1 $2\" in\n'image inspect') grep -qxF \"$5\" {0}/has && echo 1;;\n'image rm') grep -vxF \"$3\" {0}/has > {0}/t; mv {0}/t {0}/has;;\nesac\n", d.display()));
        let (old, new, idle, mine) = ("a:1@sha256:old", "a:1@sha256:new", "b:1@sha256:b", "docker.io/library/alpine:3");
        std::fs::write(d.join("has"), format!("{old}\n{idle}\n{mine}\n")).unwrap();
        let now = 1000 * DAY;
        let mut st = ImageState::default();
        st.used(old, now - DAY);
        st.used(idle, now - 91 * DAY);
        let current = [new.to_string(), idle.to_string()];
        housekeeping(&rt, &mut st, &current, now, |i| {
            let has = std::fs::read_to_string(d.join("has")).unwrap();
            std::fs::write(d.join("has"), format!("{has}{i}\n")).unwrap();
            Ok(())
        });
        assert_eq!(std::fs::read_to_string(d.join("has")).unwrap(), format!("{mine}\n{new}\n"), "the old pin and the idle image gone, the user's own kept");
        assert_eq!(st.used.keys().collect::<Vec<_>>(), [new], "what is gone is forgotten; the new pin starts now");
        assert_eq!((st.used[new], st.checked), (now, now));
        assert!(!std::fs::read_to_string(d.join("log")).unwrap().contains("prune"));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
