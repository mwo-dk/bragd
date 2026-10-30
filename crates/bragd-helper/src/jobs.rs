//! The Helper's jobs: importing a Foreign database into a Library, merging Duplicates again,
//! exporting a Library to Parquet, fetching a Source, and applying Strip rules to downloads and
//! Libraries. Between jobs, and once an hour, Disk watch checks the limits (`check_disk`). They run one at a time, in the order
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

use bragd_core::config::Config;
use bragd_library::library::{Library, Pruned};
use bragd_library::strip::{self, Rewritten, StripRule};
use serde::{Deserialize, Serialize};

use crate::disk::{self, Crossed};
use crate::net::Fetcher;
use crate::sources::{self, About, AfterImport, Cx, Memo, Options};

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
    /// A Source's Strip rule applied to its downloads (`AfterImport`: written again with only
    /// the Games kept, or deleted) and to the Games it brought to the default Library.
    /// `dry_run`: nothing changes; the job's report says what would go.
    Strip { source: String, dry_run: bool },
    /// Every Source's Strip rule applied to a Library's Games (`Library::prune`).
    Prune { library: PathBuf, dry_run: bool },
}

/// What a Strip or Prune job did, or would do in a dry run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Downloads written again with only the Games kept.
    pub files: Vec<Rewritten>,
    /// Downloads deleted, with their sizes.
    pub removed: Vec<(PathBuf, u64)>,
    /// What went from the Library.
    pub library: Option<Pruned>,
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
    /// What a Strip or Prune job did, once it is done.
    #[serde(default)]
    pub report: Option<Report>,
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
    /// The engine images Bragd pulled, with their sizes (`disk::pulled_images`).
    images: Box<dyn Fn() -> Vec<(String, u64)> + Send + Sync>,
    /// The limits crossed at the last check.
    crossed: Mutex<Vec<Crossed>>,
    /// Work waits for the machine to be plugged in.
    pub paused: AtomicBool,
    quit: AtomicBool,
    cancelled: Mutex<HashSet<u64>>,
}

impl Jobs {
    /// The jobs kept in `dir`, the ones that were running queued again. Sources go into
    /// `library`, merged by `priority`. `images`: the engine images Bragd pulled, for Disk watch.
    pub fn open(dir: &Path, net: Box<dyn Fetcher>, library: PathBuf, priority: Vec<String>, battery: Box<dyn Fn() -> bool + Send + Sync>, images: Box<dyn Fn() -> Vec<(String, u64)> + Send + Sync>) -> Arc<Jobs> {
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
            images,
            crossed: Mutex::default(),
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
        st.jobs.push(Job { id, work, status: Status::Queued, done: 0, total: None, error: None, resumed: 0, report: None });
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

    /// How many jobs are queued or running.
    pub fn active(&self) -> usize {
        self.lock().jobs.iter().filter(|j| matches!(j.status, Status::Queued | Status::Running)).count()
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

    /// Sets a Source's options, its Strip rule among them. A custom URL one is made by naming
    /// it `custom:<name>`, with a URL.
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
                .filter(|(a, o, m)| a.fetchable && !m.paused && sources::due(o, m, now) && sources::may_fetch(o, metered) && !queued(&a.id))
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

    fn import(&self, id: u64, lib: &mut Library, path: &Path, source: &str, rule: Option<&StripRule>) -> Result<u64, String> {
        let stop = || self.should_stop(id);
        let p = lib.import_with(path, source, rule, &stop, &mut |p| self.progress(id, p.read as u64, None)).map_err(|e| format!("{}: {e}", path.display()))?;
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
                self.import(id, &mut self.open_library(library)?, path, source, None)
            }
            Work::Dedupe { library } => {
                let stop = || self.should_stop(id);
                let n = self.open_library(library)?.merge_again(&stop, &mut |done, all| self.progress(id, done as u64, Some(all as u64))).map_err(|e| e.to_string())?;
                Ok(n as u64)
            }
            Work::Export { library, to } => bragd_library::export::to_parquet(&self.open_library(library)?, to).map_err(|e| format!("{}: {e}", to.display())),
            Work::Fetch { source } => self.fetch(id, source),
            Work::Strip { source, dry_run } => self.strip(id, source, *dry_run),
            Work::Prune { library, dry_run } => {
                let stop = || self.should_stop(id);
                let p = self.open_library(library)?.prune(&self.rules(), *dry_run, None, &stop, &mut |n, of| self.progress(id, n, Some(of))).map_err(|e| format!("{}: {e}", library.display()))?;
                let games = p.games;
                self.set_report(id, Report { library: Some(p), ..Default::default() });
                Ok(games)
            }
        }
    }

    /// Every Source's Strip rule, by its id.
    fn rules(&self) -> BTreeMap<String, StripRule> {
        self.sources().into_iter().filter_map(|(a, o, _)| Some((a.id, o.strip?))).collect()
    }

    fn set_report(&self, id: u64, report: Report) {
        if let Some(j) = self.lock().jobs.iter_mut().find(|j| j.id == id) {
            j.report = Some(report);
        }
    }

    /// The folder of a Source's downloads.
    pub fn source_dir(&self, source: &str) -> PathBuf {
        self.dir.join("sources").join(folder_name(source))
    }

    /// `Work::Strip`: the Games stripped from downloads and the Library, or that would be.
    fn strip(&self, id: u64, source: &str, dry_run: bool) -> Result<u64, String> {
        let (_, o, _) = self.sources().into_iter().find(|(a, _, _)| a.id == source).ok_or_else(|| format!("no Source {source}"))?;
        let rule = o.strip.clone().filter(|r| !r.is_empty()).ok_or_else(|| format!("{source} has no Strip rule"))?;
        let stop = || self.should_stop(id);
        let mut files: Vec<PathBuf> = std::fs::read_dir(self.source_dir(source))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && bragd_library::stream::streams(p) && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
            .collect();
        files.sort();
        let mut report = Report::default();
        for (n, f) in files.iter().enumerate() {
            if o.after_import == AfterImport::Delete {
                let len = std::fs::metadata(f).map_or(0, |m| m.len());
                if !dry_run {
                    std::fs::remove_file(f).map_err(|e| format!("{}: {e}", f.display()))?;
                }
                report.removed.push((f.clone(), len));
            } else {
                report.files.push(strip::rewrite(f, &rule, dry_run, &stop).map_err(|e| format!("{}: {e}", f.display()))?);
            }
            self.progress(id, n as u64 + 1, Some(files.len() as u64));
        }
        let pruned = self.open_library(&self.library)?.prune(&self.rules(), dry_run, Some(source), &stop, &mut |n, of| self.progress(id, n, Some(of))).map_err(|e| e.to_string())?;
        let games = pruned.games + report.files.iter().map(|f| f.stripped).sum::<u64>();
        report.library = Some(pruned);
        self.set_report(id, report);
        Ok(games)
    }

    /// Where Bragd keeps things, by `cfg`.
    pub fn places(&self, cfg: &Config) -> disk::Places {
        let data = bragd_core::dirs::data_dir().unwrap_or_default().join("bragd");
        let tb = cfg.tablebases.path.trim();
        disk::Places {
            libraries: self.library.parent().map(Path::to_path_buf).unwrap_or_default(),
            helper: self.dir.clone(),
            sources: self.sources().into_iter().map(|(a, _, _)| (a.id.clone(), self.source_dir(&a.id))).collect(),
            tablebases: if tb.is_empty() { data.join("tablebases") } else { PathBuf::from(tb) },
            cache: bragd_core::dirs::cache_dir().unwrap_or_default().join("bragd"),
            data,
        }
    }

    /// What Bragd keeps on disk now, against the limits in `cfg`, with a notice for each
    /// Source Disk watch holds back.
    pub fn disk_report(&self, cfg: &Config) -> disk::Report {
        let mut r = disk::report(&self.places(cfg), &cfg.disk, &(self.images)(), &disk::free_space);
        for (a, _, _) in self.sources().into_iter().filter(|(_, _, m)| m.paused) {
            r.crossed.push(Crossed { id: format!("disk:paused:{}", a.id), item: Some(format!("downloads:{}", a.id)), key: "notice.disk_fetch_paused".into(), args: vec![("source".into(), a.name)] });
        }
        r
    }

    /// The limits crossed at the last check (`check_disk`).
    pub fn crossed(&self) -> Vec<Crossed> {
        self.crossed.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Checks the disk against the limits in `cfg` at `now` (Unix seconds). For each Source a
    /// crossed limit counts against (its downloads' own, or the total or the free space, which
    /// count against all), it does what the user allowed (`sources::Auto`): deletes its oldest
    /// downloads beyond its limit, holds its fetches back until the disk is within the limits
    /// again, and queues its Strip rule, at most once a day. The report after that.
    pub fn check_disk(&self, cfg: &Config, now: u64) -> disk::Report {
        const DAY: u64 = 86_400;
        let before = disk::report(&self.places(cfg), &cfg.disk, &(self.images)(), &disk::free_space);
        let everyone = before.crossed.iter().any(|c| c.item.is_none());
        for (a, o, m) in self.sources() {
            let item = format!("downloads:{}", a.id);
            let hit = everyone || before.crossed.iter().any(|c| c.item.as_deref() == Some(item.as_str()));
            let strip = hit && o.auto.strip && o.strip.as_ref().is_some_and(|r| !r.is_empty()) && m.last_auto_strip.is_none_or(|t| now >= t + DAY);
            let paused = hit && o.auto.pause;
            if paused != m.paused || strip {
                let mut st = self.lock();
                let memo = &mut st.sources.entry(a.id.clone()).or_default().memo;
                memo.paused = paused;
                if strip {
                    memo.last_auto_strip = Some(now);
                }
                self.save(&st);
            }
            if hit && o.auto.delete_oldest {
                prune(&self.source_dir(&a.id), cfg.disk.items.get(&item).copied().filter(|&c| c > 0).or(o.disk_cap));
            }
            if strip {
                crate::note(format_args!("a disk limit is crossed: the Strip rule of {} is applied", a.id));
                self.add(Work::Strip { source: a.id, dry_run: false });
            }
        }
        let r = self.disk_report(cfg);
        *self.crossed.lock().unwrap_or_else(|e| e.into_inner()) = r.crossed.clone();
        r
    }

    fn fetch(&self, id: u64, source: &str) -> Result<u64, String> {
        let now = bragd_core::fs::now();
        let kept = self.lock().sources.get(source).cloned().unwrap_or_default();
        let (options, mut memo) = (kept.options, kept.memo);
        // Found before anything is stored: a mistyped id leaves no entry behind for good.
        let src = (!source.starts_with(sources::CUSTOM) || options.is_some())
            .then(|| sources::find(source, &options.as_ref().map(|o| o.url.clone()).unwrap_or_default()))
            .flatten()
            .ok_or_else(|| format!("no Source {source}"))?;
        {
            let mut st = self.lock();
            st.sources.entry(source.into()).or_default().memo.last_try = Some(now);
            self.save(&st);
        }
        memo.last_try = Some(now);
        let options = options.unwrap_or_else(|| src.about().defaults());
        let dir = self.source_dir(source);
        let stop = || self.should_stop(id);
        let got = (|| {
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let files = src
                .fetch(&mut Cx { net: self.net.as_ref(), dir: &dir, memo: &mut memo, options: &options, stop: &stop, progress: &mut |b| self.progress(id, b, None) })
                .map_err(|e| e.to_string())?;
            let mut lib = self.open_library(&self.library)?;
            let rule = options.strip.as_ref().filter(|r| !r.is_empty());
            let mut read = 0;
            for f in &files {
                read += self.import(id, &mut lib, f, source, rule)?;
                if stop() {
                    return Err("stopped".to_string());
                }
            }
            // The Games are in the Library now: the downloads stay, shrink or go, as the user chose.
            for f in &files {
                match (options.after_import, rule) {
                    (AfterImport::Delete, _) => std::fs::remove_file(f).map_err(|e| format!("{}: {e}", f.display()))?,
                    (AfterImport::Strip, Some(rule)) => {
                        strip::rewrite(f, rule, false, &stop).map_err(|e| format!("{}: {e}", f.display()))?;
                    }
                    _ => {}
                }
            }
            Ok(read)
        })();
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
        const CHECK_EVERY: u64 = 3600;
        let mut checked = 0;
        while !self.quit.load(Ordering::Relaxed) {
            // The system is asked only when a Source cares.
            let metered = if self.sources().iter().any(|(_, o, _)| o.enabled && o.only_unmetered) { crate::net::metered() } else { None };
            self.queue_due(bragd_core::fs::now(), metered);
            let ran = self.run_next();
            let now = bragd_core::fs::now();
            // After each job, and once an hour; with the limits as config.toml has them now.
            if ran || now >= checked + CHECK_EVERY {
                checked = now;
                self.check_disk(&Config::load().unwrap_or_default(), now);
            }
            if !ran {
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
        Jobs::open(dir, Box::new(net), dir.join("lib/main.bragd"), vec!["twic".into(), "lichess".into()], Box::new(move || battery.load(Ordering::Relaxed)), Box::new(|| vec![("bragd-stockfish:17.1".into(), 1000)]))
    }

    fn count(lib: &Path) -> u64 {
        Library::open(lib).unwrap().count().unwrap()
    }

    #[test]
    fn fetching_an_unknown_source_stores_nothing() {
        let d = crate::tests::Dir::new("unknown-source");
        let jobs = jobs_in(&d.0, Recorded::default(), Arc::default());
        let id = jobs.add(Work::Fetch { source: "custom:typo".into() });
        assert!(jobs.run_next());
        assert_eq!(jobs.list().into_iter().find(|j| j.id == id).unwrap().status, Status::Failed);
        assert!(!jobs.lock().sources.contains_key("custom:typo"));
        assert!(!jobs.sources().iter().any(|(a, _, _)| a.id == "custom:typo"));
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
        // The zip stays as it came, and nothing was unpacked beside it; the fetch is remembered.
        let src = d.0.join("sources/custom_club");
        assert_eq!(std::fs::read_dir(&src).unwrap().count(), 1);
        assert!(src.join("games.zip").exists());
        let memo = jobs.lock().sources["custom:club"].memo.clone();
        assert!(memo.last_fetch.is_some() && memo.last_error.is_none());
        // Fetched again: the same Games, merged into themselves.
        jobs.add(Work::Fetch { source: "custom:club".into() });
        jobs.run_next();
        assert_eq!(count(&lib), 2);
    }

    #[test]
    fn a_custom_url_left_half_fetched_is_fetched_whole_next_time() {
        let d = crate::tests::Dir::new("custom-part");
        let url = "https://example.org/club/games.pgn";
        let net = Recorded { files: [(url.to_string(), PGN.as_bytes().to_vec())].into(), ranges: true, ..Default::default() };
        let jobs = jobs_in(&d.0, net, Arc::default());
        let mut o = sources::find("custom:club", url).unwrap().about().defaults();
        o.url = url.into();
        jobs.set_source("custom:club", o).unwrap();
        // What an older version of the file left when its fetch stopped.
        let src = d.0.join("sources/custom_club");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("games.pgn.part"), "[Event \"Old version, longer than the head\"]\n").unwrap();
        run(&jobs, Work::Fetch { source: "custom:club".into() });
        assert_eq!(std::fs::read_to_string(src.join("games.pgn")).unwrap(), PGN);
    }

    #[test]
    fn a_job_running_when_the_helper_stopped_runs_again() {
        let d = crate::tests::Dir::new("resume");
        let pgn = d.0.join("a.pgn");
        std::fs::write(&pgn, PGN).unwrap();
        let lib = d.0.join("lib/main.bragd");
        let job = |id, path: &Path| Job { id, work: Work::Import { path: path.into(), library: lib.clone(), source: "file".into() }, status: Status::Running, done: 1, total: None, error: None, resumed: 0, report: None };
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

    /// A strong Game (both 2500) and a weak one (both 1200).
    const RATED: &str = "[White \"Strong\"]\n[WhiteElo \"2500\"]\n[BlackElo \"2500\"]\n[Result \"1-0\"]\n\n1. e4 e5 2. Nf3 Nc6 1-0\n\n[White \"Weak\"]\n[WhiteElo \"1200\"]\n[BlackElo \"1200\"]\n[Result \"0-1\"]\n\n1. d4 d5 2. c4 e6 0-1\n";

    fn strong() -> StripRule {
        StripRule { min_rating: Some(2000), ..Default::default() }
    }

    fn games_in(path: &Path) -> u64 {
        bragd_library::stream::file_games(path, &mut |_, _, _| true).unwrap().games
    }

    /// A custom URL Source of RATED, set up with its options changed by `set`.
    fn club(d: &Path, set: impl FnOnce(&mut Options)) -> Arc<Jobs> {
        let url = "https://example.org/club.zip";
        let net = Recorded { files: [(url.to_string(), zip_of(&[("club.pgn", RATED)]))].into(), ..Default::default() };
        let jobs = jobs_in(d, net, Arc::default());
        let mut o = sources::find("custom:club", url).unwrap().about().defaults();
        o.url = url.into();
        set(&mut o);
        jobs.set_source("custom:club", o).unwrap();
        jobs
    }

    fn run(jobs: &Jobs, work: Work) -> Job {
        let id = jobs.add(work);
        while jobs.run_next() {}
        let j = jobs.list().into_iter().find(|j| j.id == id).unwrap();
        assert_eq!((j.status, j.error.as_deref()), (Status::Done, None), "{:?}", j.work);
        j
    }

    #[test]
    fn a_fetch_strips_as_it_imports_and_shrinks_its_download() {
        let d = crate::tests::Dir::new("strip-fetch");
        let jobs = club(&d.0, |o| {
            o.strip = Some(strong());
            o.after_import = AfterImport::Strip;
        });
        run(&jobs, Work::Fetch { source: "custom:club".into() });
        assert_eq!(count(&d.0.join("lib/main.bragd")), 1, "the weak Game was never stored");
        assert_eq!(games_in(&d.0.join("sources/custom_club/club.zip")), 1, "nor kept in the download");
        // Deleted once imported, when the user says so.
        let d = crate::tests::Dir::new("delete-fetch");
        let jobs = club(&d.0, |o| o.after_import = AfterImport::Delete);
        run(&jobs, Work::Fetch { source: "custom:club".into() });
        assert_eq!(count(&d.0.join("lib/main.bragd")), 2);
        assert_eq!(std::fs::read_dir(d.0.join("sources/custom_club")).unwrap().count(), 0);
    }

    #[test]
    fn a_strip_rule_is_tried_first_then_applied_and_spares_what_another_source_wants() {
        let d = crate::tests::Dir::new("strip-job");
        let jobs = club(&d.0, |_| {});
        run(&jobs, Work::Fetch { source: "custom:club".into() });
        // A file of the user's own has the weak Game too.
        let mine = d.0.join("mine.pgn");
        std::fs::write(&mine, RATED.split("\n\n[White").nth(1).map(|g| format!("[White{g}")).unwrap()).unwrap();
        let lib = d.0.join("lib/main.bragd");
        run(&jobs, Work::Import { path: mine, library: lib.clone(), source: "file".into() });
        assert_eq!(count(&lib), 2);
        let zip = d.0.join("sources/custom_club/club.zip");
        let before = std::fs::read(&zip).unwrap();
        let no_rule = jobs.add(Work::Strip { source: "custom:club".into(), dry_run: true });
        while jobs.run_next() {}
        assert!(jobs.list().iter().any(|j| j.id == no_rule && j.status == Status::Failed), "no rule yet");
        let (a, mut o, _) = jobs.sources().into_iter().find(|(a, _, _)| a.id == "custom:club").unwrap();
        o.strip = Some(strong());
        jobs.set_source(&a.id, o).unwrap();
        let dry = run(&jobs, Work::Strip { source: "custom:club".into(), dry_run: true }).report.unwrap();
        assert_eq!((dry.files[0].kept, dry.files[0].stripped), (1, 1));
        assert_eq!(dry.library.as_ref().map(|p| (p.games, p.dry_run)), Some((0, true)), "the weak Game is the user's own too");
        assert_eq!((std::fs::read(&zip).unwrap(), count(&lib)), (before, 2), "a dry run changes nothing");
        let done = run(&jobs, Work::Strip { source: "custom:club".into(), dry_run: false });
        assert_eq!(done.done, 1);
        assert_eq!((games_in(&zip), count(&lib)), (1, 2));
        let pruned = run(&jobs, Work::Prune { library: lib.clone(), dry_run: false }).report.unwrap().library.unwrap();
        assert_eq!(pruned.games, 0, "the file without a rule still wants it");
        // The Source's own weak Game, which nobody else brought, goes.
        let d = crate::tests::Dir::new("strip-job-alone");
        let jobs = club(&d.0, |_| {});
        run(&jobs, Work::Fetch { source: "custom:club".into() });
        let (a, mut o, _) = jobs.sources().into_iter().find(|(a, _, _)| a.id == "custom:club").unwrap();
        o.strip = Some(strong());
        jobs.set_source(&a.id, o).unwrap();
        let pruned = run(&jobs, Work::Prune { library: d.0.join("lib/main.bragd"), dry_run: false }).report.unwrap().library.unwrap();
        assert_eq!((pruned.games, count(&d.0.join("lib/main.bragd"))), (1, 1));
    }

    #[test]
    fn disk_watch_acts_only_as_allowed() {
        let d = crate::tests::Dir::new("watch");
        let jobs = club(&d.0, |o| {
            o.enabled = true;
            o.strip = Some(strong());
            o.auto = sources::Auto { strip: true, delete_oldest: true, pause: true };
        });
        let dir = jobs.source_dir("custom:club");
        std::fs::create_dir_all(&dir).unwrap();
        for (i, n) in ["old.pgn", "new.pgn"].iter().enumerate() {
            let p = dir.join(n);
            std::fs::write(&p, [b' '; 10]).unwrap();
            std::fs::File::options().write(true).open(&p).unwrap().set_modified(std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + i as u64)).unwrap();
        }
        let mut cfg = Config::default();
        cfg.tablebases.path = d.0.join("syzygy").display().to_string();
        cfg.disk.items.insert("downloads:custom:club".into(), 15);
        let r = jobs.check_disk(&cfg, 1_000_000);
        assert!(r.items.iter().any(|i| i.id == "image:bragd-stockfish:17.1" && i.bytes == 1000));
        assert!(!dir.join("old.pgn").exists() && dir.join("new.pgn").exists(), "the oldest download went");
        assert!(jobs.sources().iter().any(|(a, _, m)| a.id == "custom:club" && m.paused));
        assert!(jobs.queue_due(1_000_000, None).is_empty(), "held back");
        assert!(jobs.crossed().iter().any(|c| c.key == "notice.disk_fetch_paused"));
        let strips = || jobs.list().iter().filter(|j| matches!(j.work, Work::Strip { .. })).count();
        assert_eq!(strips(), 1, "its Strip rule is queued");
        // Within the limit again: fetched again, and no second Strip.
        let r = jobs.check_disk(&cfg, 1_000_100);
        assert!(r.crossed.is_empty(), "{:?}", r.crossed);
        assert!(jobs.sources().iter().any(|(a, _, m)| a.id == "custom:club" && !m.paused));
        assert_eq!(jobs.queue_due(1_000_100, None).len(), 1);
        // Crossed again within the day: no second Strip; a day on, one more.
        std::fs::write(dir.join("big.pgn"), [b' '; 20]).unwrap();
        cfg.disk.items.insert("downloads:custom:club".into(), 1);
        jobs.check_disk(&cfg, 1_000_200);
        assert_eq!(strips(), 1);
        std::fs::write(dir.join("big.pgn"), [b' '; 20]).unwrap();
        jobs.check_disk(&cfg, 1_000_000 + 86_400);
        assert_eq!(strips(), 2);
        // Nothing allowed: a notice, and nothing done.
        let d = crate::tests::Dir::new("watch-off");
        let jobs = club(&d.0, |o| o.enabled = true);
        let dir = jobs.source_dir("custom:club");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.pgn"), [b' '; 20]).unwrap();
        let r = jobs.check_disk(&cfg, 1_000_000);
        assert_eq!(r.crossed.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(), ["notice.disk_item_over"]);
        assert!(dir.join("a.pgn").exists() && jobs.queue_due(1_000_000, None).len() == 1 && jobs.list().len() == 1);
    }
}
