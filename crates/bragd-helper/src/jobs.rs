//! The Helper's jobs: importing a Foreign database into a Library, merging Duplicates again,
//! exporting a Library to Parquet, and fetching a Source. They run one at a time, in the order
//! asked, and pause while the machine runs on its battery. Every job is kept in `state.json`
//! in the Helper's folder, with the Sources' options and what was remembered of each: a job
//! that was running when the Helper stopped (a crash, a logout, a newer Helper taking over)
//! runs again from the start when the next Helper starts. That is safe because an import is
//! idempotent: a Game already in the Library from the same file merges into itself.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use bragd_library::library::Library;
use serde::{Deserialize, Serialize};

use crate::net::Fetcher;
use crate::sources::{self, About, Cx, Memo, Options};

/// What a job does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Work {
    /// Every Game of the Foreign database at `path` into `library`, as coming from `source`.
    Import { path: PathBuf, library: PathBuf, source: String },
    /// Headers of the Games with several Sources merged again, by the Source priority.
    Dedupe { library: PathBuf },
    Export { library: PathBuf, to: PathBuf },
    /// What is new at a Source, into the default Library.
    Fetch { source: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub work: Work,
    pub status: Status,
    /// How far: Games read, Games merged, rows written, or bytes downloaded.
    pub done: u64,
    /// Out of how many, when that is known.
    pub total: Option<u64>,
    pub error: Option<String>,
    /// How often it was started again after the Helper stopped while it ran.
    #[serde(default)]
    pub resumed: u32,
}

/// What is kept of a Source: the user's options (none until changed) and what the fetches
/// remembered.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceState {
    pub options: Option<Options>,
    pub memo: Memo,
}

/// `state.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub next_id: u64,
    pub jobs: Vec<Job>,
    pub sources: BTreeMap<String, SourceState>,
}

/// Finished jobs kept for the list.
const KEEP: usize = 100;

impl State {
    /// Missing: empty. Unreadable: moved aside to `state.json.bad` and started empty, so a
    /// broken file never stops the Helper and is not lost.
    fn load(path: &Path) -> State {
        let Ok(bytes) = std::fs::read(path) else { return State::default() };
        serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            crate::note(format_args!("{} could not be read ({e}): set aside as .bad", path.display()));
            let _ = std::fs::rename(path, path.with_extension("json.bad"));
            State::default()
        })
    }
}

/// A Source's id as a folder name.
fn folder_name(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect()
}

/// The jobs and the Sources, shared by the connections and the worker.
pub struct Jobs {
    dir: PathBuf,
    state: Mutex<State>,
    wake: Condvar,
    net: Box<dyn Fetcher>,
    library: PathBuf,
    priority: Vec<String>,
    battery: Box<dyn Fn() -> bool + Send + Sync>,
    /// Work waits for the machine to be plugged in.
    pub paused: AtomicBool,
    quit: AtomicBool,
    cancelled: Mutex<HashSet<u64>>,
}

impl Jobs {
    /// The jobs kept in `dir`, the ones that were running queued again. Sources go into
    /// `library`, merged by `priority`.
    pub fn open(dir: &Path, net: Box<dyn Fetcher>, library: PathBuf, priority: Vec<String>, battery: Box<dyn Fn() -> bool + Send + Sync>) -> Arc<Jobs> {
        let mut state = State::load(&dir.join("state.json"));
        for j in state.jobs.iter_mut().filter(|j| j.status == Status::Running) {
            crate::note(format_args!("job {} was stopped while it ran: it starts again", j.id));
            j.status = Status::Queued;
            j.resumed += 1;
        }
        let jobs = Arc::new(Jobs {
            dir: dir.to_path_buf(),
            state: Mutex::new(state),
            wake: Condvar::new(),
            net,
            library,
            priority,
            battery,
            paused: AtomicBool::new(false),
            quit: AtomicBool::new(false),
            cancelled: Mutex::default(),
        });
        jobs.save(&jobs.lock());
        jobs
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn save(&self, st: &State) {
        let text = serde_json::to_vec_pretty(st).unwrap_or_default();
        if let Err(e) = bragd_core::fs::replace(&self.dir.join("state.json"), &text, true) {
            crate::note(format_args!("state.json could not be saved: {e}"));
        }
    }

    /// Queues `work`: its id.
    pub fn add(&self, work: Work) -> u64 {
        let mut st = self.lock();
        st.next_id += 1;
        let id = st.next_id;
        st.jobs.push(Job { id, work, status: Status::Queued, done: 0, total: None, error: None, resumed: 0 });
        // The oldest finished ones go.
        let finished = st.jobs.iter().filter(|j| !matches!(j.status, Status::Queued | Status::Running)).count();
        let mut extra = finished.saturating_sub(KEEP);
        st.jobs.retain(|j| {
            let go = extra > 0 && !matches!(j.status, Status::Queued | Status::Running);
            extra -= usize::from(go);
            !go
        });
        self.save(&st);
        self.wake.notify_all();
        id
    }

    /// Cancels job `id`: a queued one at once, a running one at its next stop. False when there
    /// is no such job still to finish.
    pub fn cancel(&self, id: u64) -> bool {
        let mut st = self.lock();
        let Some(j) = st.jobs.iter_mut().find(|j| j.id == id) else { return false };
        match j.status {
            Status::Queued => j.status = Status::Cancelled,
            Status::Running => {
                self.cancelled.lock().unwrap_or_else(|e| e.into_inner()).insert(id);
            }
            _ => return false,
        }
        self.save(&st);
        true
    }

    pub fn list(&self) -> Vec<Job> {
        self.lock().jobs.clone()
    }

    /// Whether a job runs now and is not paused: the Helper does not leave in the middle.
    pub fn busy(&self) -> bool {
        !self.paused.load(Ordering::Relaxed) && self.lock().jobs.iter().any(|j| j.status == Status::Running)
    }

    /// Every Source, built in or a custom URL, with its options and what was remembered.
    pub fn sources(&self) -> Vec<(About, Options, Memo)> {
        let st = self.lock();
        let custom = st.sources.iter().filter(|(id, _)| id.starts_with(sources::CUSTOM));
        let all = sources::builtin().into_iter().chain(custom.map(|(id, s)| sources::find(id, &s.options.clone().map(|o| o.url).unwrap_or_default()).expect("a custom URL Source")));
        all.map(|s| {
            let a = s.about();
            let kept = st.sources.get(&a.id).cloned().unwrap_or_default();
            let o = kept.options.unwrap_or_else(|| a.defaults());
            (a, o, kept.memo)
        })
        .collect()
    }

    /// Sets a Source's options. A custom URL one is made by naming it `custom:<name>`, with a URL.
    pub fn set_source(&self, id: &str, options: Options) -> Result<(), String> {
        if id.starts_with(sources::CUSTOM) && !(options.url.starts_with("https://") || options.url.starts_with("http://")) {
            return Err(format!("{id}: a custom URL Source needs an http(s) address"));
        }
        if sources::find(id, &options.url).is_none() {
            return Err(format!("no Source {id}"));
        }
        let mut st = self.lock();
        st.sources.entry(id.into()).or_default().options = Some(options);
        self.save(&st);
        self.wake.notify_all();
        Ok(())
    }

    /// Queues a fetch of every Source due at `now` that may go on a connection `metered` and
    /// is not queued already: the ids of the jobs.
    pub fn queue_due(&self, now: u64, metered: Option<bool>) -> Vec<u64> {
        let sources = self.sources();
        let due: Vec<String> = {
            let st = self.lock();
            let queued = |id: &str| st.jobs.iter().any(|j| matches!(&j.work, Work::Fetch { source } if source == id) && matches!(j.status, Status::Queued | Status::Running));
            sources
                .into_iter()
                .filter(|(a, o, m)| a.fetchable && sources::due(o, m, now) && sources::may_fetch(o, metered) && !queued(&a.id))
                .map(|(a, _, _)| a.id)
                .collect()
        };
        due.into_iter().map(|source| self.add(Work::Fetch { source })).collect()
    }

    /// Asked between pieces of work: true to stop (cancelled, or the Helper leaves). Waits here
    /// while the machine runs on its battery.
    fn should_stop(&self, id: u64) -> bool {
        loop {
            if self.quit.load(Ordering::Relaxed) || self.cancelled.lock().unwrap_or_else(|e| e.into_inner()).contains(&id) {
                return true;
            }
            if !(self.battery)() {
                self.paused.store(false, Ordering::Relaxed);
                return false;
            }
            self.paused.store(true, Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(if cfg!(test) { 10 } else { 1000 }));
        }
    }

    fn progress(&self, id: u64, done: u64, total: Option<u64>) {
        if let Some(j) = self.lock().jobs.iter_mut().find(|j| j.id == id) {
            j.done = done;
            j.total = total;
        }
    }

    fn open_library(&self, path: &Path) -> Result<Library, String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let mut lib = Library::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        lib.set_priority(&self.priority.iter().map(String::as_str).collect::<Vec<_>>());
        Ok(lib)
    }

    fn import(&self, id: u64, lib: &mut Library, path: &Path, source: &str) -> Result<u64, String> {
        let stop = || self.should_stop(id);
        let p = lib.import(path, source, &stop, &mut |p| self.progress(id, p.read as u64, None)).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(p.read as u64)
    }

    /// Runs the first queued job: false when there is none.
    pub fn run_next(&self) -> bool {
        let (id, work) = {
            let mut st = self.lock();
            let Some(j) = st.jobs.iter_mut().find(|j| j.status == Status::Queued) else { return false };
            j.status = Status::Running;
            let next = (j.id, j.work.clone());
            self.save(&st);
            next
        };
        let done = self.run(id, &work);
        let quit = self.quit.load(Ordering::Relaxed);
        let cancelled = self.cancelled.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
        let mut st = self.lock();
        if let Some(j) = st.jobs.iter_mut().find(|j| j.id == id) {
            match done {
                // Left as running: the next Helper starts it again.
                _ if quit => {}
                _ if cancelled => j.status = Status::Cancelled,
                Ok(n) => {
                    j.status = Status::Done;
                    j.done = n;
                }
                Err(e) => {
                    crate::note(format_args!("job {id} failed: {e}"));
                    j.status = Status::Failed;
                    j.error = Some(e);
                }
            }
        }
        self.save(&st);
        true
    }

    fn run(&self, id: u64, work: &Work) -> Result<u64, String> {
        match work {
            Work::Import { path, library, source } => {
                if !path.exists() {
                    return Err(format!("{} is not there any more", path.display()));
                }
                self.import(id, &mut self.open_library(library)?, path, source)
            }
            Work::Dedupe { library } => {
                let stop = || self.should_stop(id);
                let n = self.open_library(library)?.merge_again(&stop, &mut |done, all| self.progress(id, done as u64, Some(all as u64))).map_err(|e| e.to_string())?;
                Ok(n as u64)
            }
            Work::Export { library, to } => bragd_library::export::to_parquet(&self.open_library(library)?, to).map_err(|e| format!("{}: {e}", to.display())),
            Work::Fetch { source } => self.fetch(id, source),
        }
    }

    fn fetch(&self, id: u64, source: &str) -> Result<u64, String> {
        let now = bragd_core::fs::now();
        let (options, mut memo) = {
            let mut st = self.lock();
            let kept = st.sources.entry(source.into()).or_default();
            kept.memo.last_try = Some(now);
            let kept = kept.clone();
            self.save(&st);
            (kept.options, kept.memo)
        };
        let src = sources::find(source, &options.as_ref().map(|o| o.url.clone()).unwrap_or_default()).ok_or_else(|| format!("no Source {source}"))?;
        let options = options.unwrap_or_else(|| src.about().defaults());
        let dir = self.dir.join("sources").join(folder_name(source));
        let stop = || self.should_stop(id);
        let got = (|| {
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let files = src
                .fetch(&mut Cx { net: self.net.as_ref(), dir: &dir, memo: &mut memo, options: &options, stop: &stop, progress: &mut |b| self.progress(id, b, None) })
                .map_err(|e| e.to_string())?;
            let mut lib = self.open_library(&self.library)?;
            let mut read = 0;
            for f in &files {
                read += self.import(id, &mut lib, f, source)?;
                if stop() {
                    return Err("stopped".to_string());
                }
            }
            Ok(read)
        })();
        // The unpacked files are in the Library now; the downloads stay, within the cap.
        let _ = std::fs::remove_dir_all(dir.join("unpacked"));
        prune(&dir, options.disk_cap);
        let mut st = self.lock();
        let kept = &mut st.sources.entry(source.into()).or_default().memo;
        match &got {
            Ok(_) => {
                *kept = Memo { last_fetch: Some(now), last_try: Some(now), last_error: None, ..memo };
            }
            Err(e) => kept.last_error = Some(e.clone()),
        }
        self.save(&st);
        got
    }

    /// The worker: runs jobs as they come, and queues the Sources that are due, until `stop`.
    pub fn work(self: Arc<Self>) {
        while !self.quit.load(Ordering::Relaxed) {
            // The system is asked only when a Source cares.
            let metered = if self.sources().iter().any(|(_, o, _)| o.enabled && o.only_unmetered) { crate::net::metered() } else { None };
            self.queue_due(bragd_core::fs::now(), metered);
            if !self.run_next() {
                // Under the lock, so a job added or a stop since is not missed.
                let st = self.lock();
                if !self.quit.load(Ordering::Relaxed) && !st.jobs.iter().any(|j| j.status == Status::Queued) {
                    let _ = self.wake.wait_timeout(st, Duration::from_secs(60));
                }
            }
        }
    }

    /// Stops the worker: a job running is left to start again with the next Helper.
    pub fn stop(&self) {
        let _st = self.lock();
        self.quit.store(true, Ordering::Relaxed);
        self.wake.notify_all();
    }
}

/// Removes the oldest files in `dir` until they take no more than `cap` bytes.
fn prune(dir: &Path, cap: Option<u64>) {
    let Some(cap) = cap else { return };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok().filter(|m| m.is_file())?;
            Some((m.modified().ok()?, m.len(), e.path()))
        })
        .collect();
    files.sort();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    for (_, len, path) in files {
        if total <= cap {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::net::tests::Recorded;
    use crate::sources::Source;
    use crate::sources::tests::zip_of;

    /// Two short games.
    pub const PGN: &str = "[Event \"Fixture\"]\n[White \"A\"]\n[Black \"B\"]\n[Result \"1-0\"]\n\n1. e4 e5 2. Nf3 Nc6 1-0\n\n[Event \"Fixture\"]\n[White \"C\"]\n[Black \"D\"]\n[Result \"0-1\"]\n\n1. d4 d5 2. c4 e6 0-1\n";

    pub fn jobs_in(dir: &Path, net: Recorded, battery: Arc<AtomicBool>) -> Arc<Jobs> {
        Jobs::open(dir, Box::new(net), dir.join("lib/main.bragd"), vec!["twic".into(), "lichess".into()], Box::new(move || battery.load(Ordering::Relaxed)))
    }

    fn count(lib: &Path) -> u64 {
        Library::open(lib).unwrap().count().unwrap()
    }

    #[test]
    fn a_custom_url_is_fetched_unpacked_and_imported_into_the_library() {
        let d = crate::tests::Dir::new("custom");
        let url = "https://example.org/club/games.zip?x=1";
        let net = Recorded { files: [(url.to_string(), zip_of(&[("club.pgn", PGN)]))].into(), ..Default::default() };
        let jobs = jobs_in(&d.0, net, Arc::default());
        let mut o = sources::find("custom:club", url).unwrap().about().defaults();
        assert!(jobs.set_source("custom:club", o.clone()).is_err(), "no address");
        o.url = url.into();
        o.enabled = true;
        jobs.set_source("custom:club", o).unwrap();
        assert!(jobs.sources().iter().any(|(a, o, _)| a.id == "custom:club" && o.enabled));
        let id = jobs.add(Work::Fetch { source: "custom:club".into() });
        assert!(jobs.run_next());
        let job = jobs.list().into_iter().find(|j| j.id == id).unwrap();
        assert_eq!((job.status, job.error.as_deref()), (Status::Done, None));
        let lib = d.0.join("lib/main.bragd");
        assert_eq!(count(&lib), 2);
        let l = Library::open(&lib).unwrap();
        assert_eq!(l.sources(1).unwrap()[0].0, "custom:club");
        // The zip stays, its unpacked games go; the fetch is remembered.
        let src = d.0.join("sources/custom_club");
        assert!(src.join("games.zip").exists() && !src.join("unpacked").exists());
        let memo = jobs.lock().sources["custom:club"].memo.clone();
        assert!(memo.last_fetch.is_some() && memo.last_error.is_none());
        // Fetched again: the same Games, merged into themselves.
        jobs.add(Work::Fetch { source: "custom:club".into() });
        jobs.run_next();
        assert_eq!(count(&lib), 2);
    }

    #[test]
    fn a_job_running_when_the_helper_stopped_runs_again() {
        let d = crate::tests::Dir::new("resume");
        let pgn = d.0.join("a.pgn");
        std::fs::write(&pgn, PGN).unwrap();
        let lib = d.0.join("lib/main.bragd");
        let job = |id, path: &Path| Job { id, work: Work::Import { path: path.into(), library: lib.clone(), source: "file".into() }, status: Status::Running, done: 1, total: None, error: None, resumed: 0 };
        let st = State { next_id: 2, jobs: vec![job(1, &pgn), job(2, &d.0.join("gone.pgn"))], sources: BTreeMap::new() };
        std::fs::write(d.0.join("state.json"), serde_json::to_vec(&st).unwrap()).unwrap();
        let jobs = jobs_in(&d.0, Recorded::default(), Arc::default());
        assert!(jobs.list().iter().all(|j| j.status == Status::Queued && j.resumed == 1));
        while jobs.run_next() {}
        let list = jobs.list();
        assert_eq!((list[0].status, list[0].done), (Status::Done, 2));
        assert_eq!(count(&lib), 2);
        // One whose file has gone is reported, not lost.
        assert_eq!(list[1].status, Status::Failed);
        assert!(list[1].error.as_deref().unwrap().contains("gone.pgn"));
        // And all of it is in state.json for the next Helper.
        assert_eq!(State::load(&d.0.join("state.json")).jobs, list);
    }

    #[test]
    fn due_sources_are_queued_once() {
        let d = crate::tests::Dir::new("due");
        let jobs = jobs_in(&d.0, Recorded::default(), Arc::default());
        assert!(jobs.queue_due(1_000_000, None).is_empty(), "nothing is on");
        for id in ["twic", "ogs"] {
            let mut o = sources::find(id, "").unwrap().about().defaults();
            o.enabled = true;
            o.only_unmetered = true;
            jobs.set_source(id, o).unwrap();
        }
        assert!(jobs.queue_due(1_000_000, Some(true)).is_empty(), "metered");
        assert_eq!(jobs.queue_due(1_000_000, Some(false)).len(), 1, "TWIC, not OGS, which is not fetched yet");
        assert!(jobs.queue_due(1_000_000, None).is_empty(), "already queued");
        assert!(jobs.set_source("nope", sources::Twic.about().defaults()).is_err());
    }

    #[test]
    fn cancel_and_battery() {
        let d = crate::tests::Dir::new("cancel");
        let pgn = d.0.join("a.pgn");
        std::fs::write(&pgn, PGN).unwrap();
        let battery = Arc::new(AtomicBool::new(true));
        let jobs = jobs_in(&d.0, Recorded::default(), battery.clone());
        let lib = d.0.join("lib/main.bragd");
        let a = jobs.add(Work::Import { path: pgn.clone(), library: lib.clone(), source: "file".into() });
        let b = jobs.add(Work::Dedupe { library: lib.clone() });
        assert!(jobs.cancel(b));
        // On the battery the import waits, paused.
        let worker = {
            let jobs = jobs.clone();
            std::thread::spawn(move || while jobs.run_next() {})
        };
        let wait = std::time::Instant::now();
        while !jobs.paused.load(Ordering::Relaxed) {
            assert!(wait.elapsed() < Duration::from_secs(60), "never paused");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!jobs.busy(), "paused work does not keep the Helper");
        assert_eq!(count(&lib), 0);
        battery.store(false, Ordering::Relaxed);
        worker.join().unwrap();
        let list = jobs.list();
        assert_eq!(list.iter().map(|j| (j.id, j.status)).collect::<Vec<_>>(), [(a, Status::Done), (b, Status::Cancelled)]);
        assert_eq!(count(&lib), 2);
        assert!(!jobs.cancel(a), "finished");
    }

    #[test]
    fn downloads_are_pruned_to_the_cap() {
        let d = crate::tests::Dir::new("prune");
        for (i, n) in ["old", "mid", "new"].iter().enumerate() {
            let p = d.0.join(n);
            std::fs::write(&p, [0u8; 10]).unwrap();
            let t = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + i as u64);
            std::fs::File::options().write(true).open(&p).unwrap().set_modified(t).unwrap();
        }
        prune(&d.0, Some(20));
        assert!(!d.0.join("old").exists() && d.0.join("mid").exists() && d.0.join("new").exists());
        prune(&d.0, None);
        assert!(d.0.join("mid").exists());
    }
}
