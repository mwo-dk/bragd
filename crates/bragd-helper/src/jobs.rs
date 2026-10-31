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
    /// What is new at a Source, into the Library of each of its Routes.
    Fetch { source: String },
    /// A Source's Routes' Strip rules applied to its downloads (`AfterImport`: written again
    /// with only the Games some Route keeps, or deleted) and to the Games it brought to each
    /// Route's Library. `dry_run`: nothing changes; the job's report says what would go.
    Strip { source: String, dry_run: bool },
    /// The Strip rule of every Route to a Library applied to its Games (`Library::prune`).
    Prune { library: PathBuf, dry_run: bool },
}

/// What a Strip or Prune job did, or would do in a dry run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Downloads written again with only the Games kept.
    pub files: Vec<Rewritten>,
    /// Downloads deleted, with their sizes.
    pub removed: Vec<(PathBuf, u64)>,
    /// What went from the Libraries, added up.
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
    /// Missing: empty. Unreadable or broken: moved aside to `state.json.bad` (or
    /// `state.json.<time>.bad` when that is taken) and started empty, so a broken file never
    /// stops the Helper, and saving never writes over it.
    fn load(path: &Path) -> State {
        let read = std::fs::read(path).map_err(|e| e.to_string());
        if read.as_ref().is_err_and(|_| !path.exists()) {
            return State::default();
        }
        read.and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
            .and_then(|v| serde_json::from_value(upgrade(v)).map_err(|e| e.to_string()))
            .unwrap_or_else(|e| {
            let mut bad = path.with_extension("json.bad");
            if bad.exists() {
                bad = path.with_extension(format!("json.{}.bad", bragd_core::fs::now()));
            }
            crate::note(format_args!("{} could not be read ({e}): set aside as {}", path.display(), bad.display()));
            let _ = std::fs::rename(path, bad);
            State::default()
        })
    }
}

/// `state.json` as an older Helper wrote it, made current: a Source's one Strip rule (`strip`)
/// becomes one Route to the main Library with that rule.
fn upgrade(mut v: serde_json::Value) -> serde_json::Value {
    let sources = v.get_mut("sources").and_then(|s| s.as_object_mut());
    for kept in sources.into_iter().flat_map(|s| s.values_mut()) {
        let Some(o) = kept.get_mut("options").and_then(|o| o.as_object_mut()) else { continue };
        if !o.contains_key("routes") {
            let strip = o.remove("strip").unwrap_or(serde_json::Value::Null);
            o.insert("routes".into(), serde_json::json!([{ "strip": strip, "library": sources::MAIN }]));
        }
    }
    v
}

/// A Source's id as a folder name of its own: letters, digits and `-` stay, and every other
/// byte is written `_` and its two hex digits, so no two ids share a folder (`custom:club.a` is
/// `custom_3aclub_2ea`, `custom:club_a` is `custom_3aclub_5fa`).
fn folder_name(id: &str) -> String {
    id.bytes().map(|b| if b.is_ascii_alphanumeric() || b == b'-' { (b as char).to_string() } else { format!("_{b:02x}") }).collect()
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

    /// Sets a Source's options, its Routes among them. A custom URL one is made by naming it
    /// `custom:<name>`, with a URL.
    pub fn set_source(&self, id: &str, options: Options) -> Result<(), String> {
        if id.starts_with(sources::CUSTOM) && !(options.url.starts_with("https://") || options.url.starts_with("http://")) {
            return Err(format!("{id}: a custom URL Source needs an http(s) address"));
        }
        if options.routes.is_empty() {
            return Err(format!("{id}: a Source needs a Route"));
        }
        let mut libraries = HashSet::new();
        for r in &options.routes {
            if !sources::library_name(&r.library) {
                return Err(format!("{id}: {:?} cannot name a Library", r.library));
            }
            if !libraries.insert(r.library.as_str()) {
                return Err(format!("{id}: two Routes go to the Library {}", r.library));
            }
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

    /// Removes a custom URL Source: its options, what its fetches remembered, and its
    /// downloads. The Games it brought stay in the Libraries. A built-in Source cannot be
    /// removed (it is turned off), nor a Source with a job queued or running.
    pub fn remove_source(&self, id: &str) -> Result<(), String> {
        if !id.starts_with(sources::CUSTOM) {
            return Err(format!("{id}: only a custom URL Source can be removed"));
        }
        let mut st = self.lock();
        let busy = st.jobs.iter().any(|j| matches!(j.status, Status::Queued | Status::Running) && matches!(&j.work, Work::Fetch { source } | Work::Strip { source, .. } if source == id));
        if busy {
            return Err(format!("{id}: a job of this Source is under way"));
        }
        if st.sources.remove(id).is_none() {
            return Err(format!("no Source {id}"));
        }
        self.save(&st);
        drop(st);
        let dir = self.source_dir(id);
        match std::fs::remove_dir_all(&dir) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", dir.display())),
            _ => Ok(()),
        }
    }

    /// The user's yes to a download over a Source's cap: its next fetch may download what the
    /// server said, once, and is queued now. The job's id.
    pub fn allow_download(&self, id: &str) -> Result<u64, String> {
        {
            let mut st = self.lock();
            let memo = &mut st.sources.get_mut(id).ok_or_else(|| format!("no Source {id}"))?.memo;
            let size = memo.too_big.take().ok_or_else(|| format!("{id} has no download waiting for a yes"))?;
            memo.allowed = Some(size);
            self.save(&st);
        }
        Ok(self.add(Work::Fetch { source: id.into() }))
    }

    /// Queues a fetch of every Source due at `now` that may go on a connection `metered` and
    /// is not queued already: the ids of the jobs.
    pub fn queue_due(&self, now: u64, metered: Option<bool>) -> Vec<u64> {
        // Offline nothing is due: a fetch could only fail.
        if self.net.offline() {
            return vec![];
        }
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
                let p = self.open_library(library)?.prune(&self.rules_for(library), *dry_run, None, &stop, &mut |n, of| self.progress(id, n, Some(of))).map_err(|e| format!("{}: {e}", library.display()))?;
                let games = p.games;
                self.set_report(id, Report { library: Some(p), ..Default::default() });
                Ok(games)
            }
        }
    }

    /// The Library a Route names: `main` is the main Library, other names sit beside it.
    pub fn library_of(&self, name: &str) -> PathBuf {
        self.library.with_file_name(format!("{name}.bragd"))
    }

    /// The Strip rule of each Source's Route to `library`, by the Source's id.
    fn rules_for(&self, library: &Path) -> BTreeMap<String, StripRule> {
        self.sources()
            .into_iter()
            .filter_map(|(a, o, _)| Some((a.id, o.routes.iter().find(|r| self.library_of(&r.library) == library)?.rule()?.clone())))
            .collect()
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

    /// `Work::Strip`: the Games stripped from downloads and the Libraries, or that would be.
    fn strip(&self, id: u64, source: &str, dry_run: bool) -> Result<u64, String> {
        let (_, o, _) = self.sources().into_iter().find(|(a, _, _)| a.id == source).ok_or_else(|| format!("no Source {source}"))?;
        if !o.routes.iter().any(|r| r.rule().is_some()) {
            return Err(format!("{source} has no Strip rule"));
        }
        // A download keeps what some Route keeps: all of it while one Route keeps everything.
        let rules: Option<Vec<&StripRule>> = o.routes.iter().map(|r| r.rule()).collect();
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
            } else if let Some(rules) = &rules {
                report.files.push(strip::rewrite(f, rules, dry_run, &stop).map_err(|e| format!("{}: {e}", f.display()))?);
            }
            self.progress(id, n as u64 + 1, Some(files.len() as u64));
        }
        let mut pruned = Pruned { dry_run, ..Default::default() };
        for r in o.routes.iter().filter(|r| r.rule().is_some()) {
            let library = self.library_of(&r.library);
            let p = self.open_library(&library)?.prune(&self.rules_for(&library), dry_run, Some(source), &stop, &mut |n, of| self.progress(id, n, Some(of))).map_err(|e| e.to_string())?;
            (pruned.games, pruned.of, pruned.bytes) = (pruned.games + p.games, pruned.of + p.of, pruned.bytes + p.bytes);
            pruned.cancelled |= p.cancelled;
        }
        let games = pruned.games + report.files.iter().map(|f| f.stripped).sum::<u64>();
        report.library = Some(pruned);
        self.set_report(id, report);
        Ok(games)
    }

    /// Where Bragd keeps things, by `cfg`; None when the system names no data or cache folder.
    pub fn places(&self, cfg: &Config) -> Option<disk::Places> {
        Some(disk::Places {
            libraries: self.library.parent().map(Path::to_path_buf).unwrap_or_default(),
            helper: self.dir.clone(),
            sources: self.sources().into_iter().map(|(a, _, _)| (a.id.clone(), self.source_dir(&a.id))).collect(),
            tablebases: cfg.tablebase_folder()?,
            cache: bragd_core::dirs::app_cache()?,
            data: bragd_core::dirs::app_data()?,
        })
    }

    /// What Bragd keeps on disk at `places`, against the limits in `cfg`: nothing when there
    /// are no places.
    fn report(&self, cfg: &Config, images: &[(String, u64)]) -> disk::Report {
        self.places(cfg).map(|p| disk::report(&p, &cfg.disk, images, &disk::free_space)).unwrap_or_default()
    }

    /// What Bragd keeps on disk now, against the limits in `cfg`, with a notice for each
    /// Source Disk watch holds back.
    pub fn disk_report(&self, cfg: &Config) -> disk::Report {
        self.notices(self.report(cfg, &(self.images)()))
    }

    /// `r` with a notice for each Source Disk watch holds back.
    fn notices(&self, mut r: disk::Report) -> disk::Report {
        for (a, o, m) in self.sources() {
            if m.paused {
                r.crossed.push(Crossed { id: format!("disk:paused:{}", a.id), item: Some(format!("downloads:{}", a.id)), key: "notice.disk_fetch_paused".into(), args: vec![("source".into(), a.name.clone())] });
            }
            if let Some(size) = m.too_big {
                let args = vec![("source".into(), a.name), ("size".into(), disk::bytes(size)), ("cap".into(), disk::bytes(sources::cap(&o, &m)))];
                r.crossed.push(Crossed { id: format!("download:too_big:{}", a.id), item: Some(format!("downloads:{}", a.id)), key: "notice.download_too_big".into(), args });
            }
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
        // The runtime is asked about the images once a check, and the folders walked again
        // only when a download was deleted.
        let images = (self.images)();
        let before = self.report(cfg, &images);
        let mut deleted = 0;
        let everyone = before.crossed.iter().any(|c| c.item.is_none());
        for (a, o, m) in self.sources() {
            let item = format!("downloads:{}", a.id);
            let hit = everyone || before.crossed.iter().any(|c| c.item.as_deref() == Some(item.as_str()));
            let strip = hit && o.auto.strip && o.routes.iter().any(|r| r.rule().is_some()) && m.last_auto_strip.is_none_or(|t| now >= t + DAY);
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
                deleted += prune(&self.source_dir(&a.id), cfg.disk.items.get(&item).copied().filter(|&c| c > 0).or(o.disk_cap));
            }
            if strip {
                crate::note(format_args!("a disk limit is crossed: the Strip rule of {} is applied", a.id));
                self.add(Work::Strip { source: a.id, dry_run: false });
            }
        }
        let r = self.notices(if deleted > 0 { self.report(cfg, &images) } else { before });
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
        let cap = sources::cap(&options, &memo);
        // The size the server gave for a download over the cap, for the user to say yes to.
        let mut too_big = None;
        let got = (|| {
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let files = src
                .fetch(&mut Cx { net: self.net.as_ref(), dir: &dir, cap, memo: &mut memo, options: &options, stop: &stop, progress: &mut |b| self.progress(id, b, None) })
                .map_err(|e| {
                    too_big = crate::net::TooBig::of(&e).and_then(|t| t.size);
                    e.to_string()
                })?;
            let mut libraries = options.routes.iter().map(|r| self.open_library(&self.library_of(&r.library))).collect::<Result<Vec<_>, _>>()?;
            let mut read = 0;
            for f in &files {
                let mut routes: Vec<(&mut Library, Option<&StripRule>)> = libraries.iter_mut().zip(&options.routes).map(|(l, r)| (l, r.rule())).collect();
                let all = bragd_library::library::import_routes(f, source, &mut routes, &stop, &mut |_, p| self.progress(id, p.read as u64, None)).map_err(|e| format!("{}: {e}", f.display()))?;
                read += all.first().map_or(0, |p| p.read as u64);
                if stop() {
                    return Err("stopped".to_string());
                }
            }
            // The Games are in their Libraries now: the downloads stay, shrink to what some Route
            // keeps, or go, as the user chose.
            let rules: Option<Vec<&StripRule>> = options.routes.iter().map(|r| r.rule()).collect();
            for f in &files {
                match (options.after_import, &rules) {
                    (AfterImport::Delete, _) => std::fs::remove_file(f).map_err(|e| format!("{}: {e}", f.display()))?,
                    (AfterImport::Strip, Some(rules)) => {
                        strip::rewrite(f, rules, false, &stop).map_err(|e| format!("{}: {e}", f.display()))?;
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
                *kept = Memo { last_fetch: Some(now), last_try: Some(now), last_error: None, too_big: None, allowed: None, ..memo };
            }
            Err(e) => {
                kept.last_error = Some(e.clone());
                kept.too_big = too_big;
            }
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

/// Removes the oldest files in `dir` until they take no more than `cap` bytes: the bytes
/// removed.
fn prune(dir: &Path, cap: Option<u64>) -> u64 {
    let Some(cap) = cap else { return 0 };
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
    let all: u64 = files.iter().map(|f| f.1).sum();
    let mut total = all;
    for (_, len, path) in files {
        if total <= cap {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
    all - total
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
    fn a_broken_state_is_set_aside_and_an_earlier_one_kept() {
        let d = crate::tests::Dir::new("state-bad");
        let path = d.0.join("state.json");
        std::fs::write(&path, "first {").unwrap();
        State::load(&path);
        std::fs::write(&path, "second {").unwrap();
        State::load(&path);
        let mut kept: Vec<String> = std::fs::read_dir(&d.0).unwrap().map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap()).collect();
        kept.sort();
        assert_eq!(kept, ["first {", "second {"], "both kept, neither written over");
        // A file that cannot be read is set aside too, not saved over.
        #[cfg(unix)]
        {
            let d = crate::tests::Dir::new("state-unreadable");
            let path = d.0.join("state.json");
            std::fs::create_dir(&path).unwrap();
            State::load(&path);
            assert!(!path.exists(), "moved aside");
        }
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
        let src = d.0.join("sources/custom_3aclub");
        assert_eq!(std::fs::read_dir(&src).unwrap().count(), 1);
        assert!(src.join("games.zip").exists());
        let memo = jobs.lock().sources["custom:club"].memo.clone();
        assert!(memo.last_fetch.is_some() && memo.last_error.is_none());
        // Fetched again: the same Games, merged into themselves.
        jobs.add(Work::Fetch { source: "custom:club".into() });
        jobs.run_next();
        assert_eq!(count(&lib), 2);
        // Removed: no longer listed, its downloads gone, its Games kept. A built-in one stays.
        let queued = jobs.add(Work::Fetch { source: "custom:club".into() });
        assert!(jobs.remove_source("custom:club").is_err(), "a fetch is queued");
        jobs.cancel(queued);
        jobs.remove_source("custom:club").unwrap();
        assert!(!jobs.sources().iter().any(|(a, _, _)| a.id == "custom:club"));
        assert!(!src.exists());
        assert_eq!(count(&lib), 2);
        assert!(jobs.remove_source("twic").is_err() && jobs.remove_source("custom:club").is_err());
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
        let src = d.0.join("sources/custom_3aclub");
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

    /// Offline (`BRAGD_OFFLINE`), no Source is queued on its schedule.
    #[test]
    fn offline_queues_no_source() {
        let d = crate::tests::Dir::new("offline");
        let jobs = jobs_in(&d.0, Recorded { offline: true, ..Default::default() }, Arc::default());
        let mut o = sources::Twic.about().defaults();
        o.enabled = true;
        jobs.set_source("twic", o).unwrap();
        assert!(jobs.queue_due(1_000_000, Some(false)).is_empty());
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
        bragd_library::stream::file_games(path, &|| false, &mut |_, _, _| true).unwrap().games
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
            o.routes = vec![sources::Route { strip: Some(strong()), library: sources::MAIN.into() }];
            o.after_import = AfterImport::Strip;
        });
        run(&jobs, Work::Fetch { source: "custom:club".into() });
        assert_eq!(count(&d.0.join("lib/main.bragd")), 1, "the weak Game was never stored");
        assert_eq!(games_in(&d.0.join("sources/custom_3aclub/club.zip")), 1, "nor kept in the download");
        // Deleted once imported, when the user says so.
        let d = crate::tests::Dir::new("delete-fetch");
        let jobs = club(&d.0, |o| o.after_import = AfterImport::Delete);
        run(&jobs, Work::Fetch { source: "custom:club".into() });
        assert_eq!(count(&d.0.join("lib/main.bragd")), 2);
        assert_eq!(std::fs::read_dir(d.0.join("sources/custom_3aclub")).unwrap().count(), 0);
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
        let zip = d.0.join("sources/custom_3aclub/club.zip");
        let before = std::fs::read(&zip).unwrap();
        let no_rule = jobs.add(Work::Strip { source: "custom:club".into(), dry_run: true });
        while jobs.run_next() {}
        assert!(jobs.list().iter().any(|j| j.id == no_rule && j.status == Status::Failed), "no rule yet");
        let (a, mut o, _) = jobs.sources().into_iter().find(|(a, _, _)| a.id == "custom:club").unwrap();
        o.routes = vec![sources::Route { strip: Some(strong()), library: sources::MAIN.into() }];
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
        o.routes = vec![sources::Route { strip: Some(strong()), library: sources::MAIN.into() }];
        jobs.set_source(&a.id, o).unwrap();
        let pruned = run(&jobs, Work::Prune { library: d.0.join("lib/main.bragd"), dry_run: false }).report.unwrap().library.unwrap();
        assert_eq!((pruned.games, count(&d.0.join("lib/main.bragd"))), (1, 1));
    }

    /// A disk check builds its report once: the container runtime is asked about the images
    /// once, also when the check deleted a Source's oldest downloads.
    #[test]
    fn a_disk_check_builds_its_report_once() {
        let d = crate::tests::Dir::new("once");
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let a = asked.clone();
        let images = Box::new(move || {
            a.fetch_add(1, Ordering::SeqCst);
            vec![("bragd-stockfish:17.1".to_string(), 1000)]
        });
        let jobs = Jobs::open(&d.0, Box::new(Recorded::default()), d.0.join("lib/main.bragd"), vec![], Box::new(|| false), images);
        let url = "https://example.org/club.zip";
        let mut o = sources::find("custom:club", url).unwrap().about().defaults();
        o.url = url.into();
        o.auto = sources::Auto { strip: false, delete_oldest: true, pause: false };
        jobs.set_source("custom:club", o).unwrap();
        let dir = jobs.source_dir("custom:club");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.pgn"), [b' '; 20]).unwrap();
        let mut cfg = Config::default();
        cfg.tablebases.path = d.0.join("syzygy").display().to_string();
        jobs.check_disk(&cfg, 1_000_000);
        assert_eq!(asked.load(Ordering::SeqCst), 1, "nothing crossed");
        cfg.disk.items.insert("downloads:custom:club".into(), 10);
        let r = jobs.check_disk(&cfg, 1_000_100);
        assert!(!dir.join("a.pgn").exists(), "the download over the limit went");
        assert!(!r.items.iter().any(|i| i.id == "downloads:custom:club"), "and the report says so: {:?}", r.items);
        assert_eq!(asked.load(Ordering::SeqCst), 2, "once a check");
    }

    #[test]
    fn disk_watch_acts_only_as_allowed() {
        let d = crate::tests::Dir::new("watch");
        let jobs = club(&d.0, |o| {
            o.enabled = true;
            o.routes = vec![sources::Route { strip: Some(strong()), library: sources::MAIN.into() }];
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

    /// Every Source has a downloads folder of its own, named by its id, so deleting one Source's
    /// downloads never touches another's.
    #[test]
    fn each_source_has_its_own_downloads_folder() {
        let ids = ["twic", "lichess-db", "custom:club.a", "custom:club_a", "custom:club a", "custom:Club-A", "custom:klubb_ø"];
        let names: Vec<String> = ids.iter().map(|id| folder_name(id)).collect();
        assert_eq!(names[..4], ["twic", "lichess-db", "custom_3aclub_2ea", "custom_3aclub_5fa"]);
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "{names:?}");
        assert!(names.iter().all(|n| n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')), "{names:?}");
        let d = crate::tests::Dir::new("own-folders");
        let jobs = jobs_in(&d.0, Recorded::default(), Arc::new(AtomicBool::new(false)));
        assert_ne!(jobs.source_dir("custom:club.a"), jobs.source_dir("custom:club_a"));
        assert_eq!(jobs.source_dir("twic"), d.0.join("sources").join("twic"));
    }

    /// A download over its Source's cap is not fetched: the size the server gave waits for the
    /// user's yes, shown as a notice; the yes fetches it that once, and the cap applies again.
    #[test]
    fn a_download_over_its_cap_waits_for_a_yes() {
        let d = crate::tests::Dir::new("too-big");
        let url = "https://example.org/club/games.pgn";
        let net = Recorded { files: [(url.to_string(), PGN.as_bytes().to_vec())].into(), sizes: true, ..Default::default() };
        let jobs = jobs_in(&d.0, net, Arc::default());
        let mut o = sources::find("custom:club", url).unwrap().about().defaults();
        o.url = url.into();
        o.max_download = Some(100);
        jobs.set_source("custom:club", o).unwrap();
        assert!(jobs.allow_download("custom:club").is_err(), "nothing waits for a yes yet");
        let id = jobs.add(Work::Fetch { source: "custom:club".into() });
        while jobs.run_next() {}
        let j = jobs.list().into_iter().find(|j| j.id == id).unwrap();
        assert_eq!(j.status, Status::Failed);
        let size = PGN.len() as u64;
        assert_eq!(jobs.lock().sources["custom:club"].memo.too_big, Some(size));
        assert!(!jobs.source_dir("custom:club").join("games.pgn").exists());
        assert!(!d.0.join("lib/main.bragd").exists(), "nothing imported");
        let notice = jobs.disk_report(&Config::default()).crossed.into_iter().find(|c| c.key == "notice.download_too_big").unwrap();
        assert_eq!(notice.id, "download:too_big:custom:club");
        // Yes: fetched, and the yes is used up.
        let id = jobs.allow_download("custom:club").unwrap();
        while jobs.run_next() {}
        assert_eq!(jobs.list().into_iter().find(|j| j.id == id).unwrap().status, Status::Done);
        assert_eq!(count(&d.0.join("lib/main.bragd")), 2);
        let memo = jobs.lock().sources["custom:club"].memo.clone();
        assert_eq!((memo.too_big, memo.allowed), (None, None));
        assert!(!jobs.disk_report(&Config::default()).crossed.iter().any(|c| c.key == "notice.download_too_big"));
        // Without an answer of its own a Source takes the default cap.
        assert_eq!(sources::cap(&sources::Twic.about().defaults(), &Memo::default()), sources::MAX_DOWNLOAD);
        assert_eq!(sources::MAX_DOWNLOAD, 2_000_000_000);
    }

    /// A Route of RATED: `strip` the minimum rating it keeps (none: every Game), into `library`.
    fn route(min: Option<u16>, library: &str) -> sources::Route {
        sources::Route { strip: min.map(|m| StripRule { min_rating: Some(m), ..Default::default() }), library: library.into() }
    }

    /// One download, read once, fans out: a Game lands in every Library whose Route keeps it,
    /// and a Game no Route keeps is stored nowhere. Each Library prunes by its own Route.
    #[test]
    fn routes_send_each_game_to_every_library_that_keeps_it() {
        let d = crate::tests::Dir::new("routes");
        // Strong Games (2000+) into `main`; anything 1000+ into `club`; 3000+ into `top`.
        let jobs = club(&d.0, |o| o.routes = vec![route(Some(2000), "main"), route(Some(1000), "club"), route(Some(3000), "top")]);
        run(&jobs, Work::Fetch { source: "custom:club".into() });
        let main = d.0.join("lib/main.bragd");
        let (club_lib, top) = (jobs.library_of("club"), jobs.library_of("top"));
        assert_eq!(club_lib, d.0.join("lib/club.bragd"));
        assert_eq!((count(&main), count(&club_lib), count(&top)), (1, 2, 0));
        // The strong Game is in two Libraries; the weak one only in `club`.
        let white = |lib: &Path| -> Vec<String> {
            let l = Library::open(lib).unwrap();
            (1..=l.count().unwrap() as i64).map(|id| l.headers(id).unwrap()["White"].as_str().unwrap().to_string()).collect()
        };
        assert_eq!(white(&main), ["Strong"]);
        assert_eq!(white(&club_lib), ["Strong", "Weak"]);
        // No Route keeps anything: nothing is stored anywhere.
        let d2 = crate::tests::Dir::new("routes-none");
        let jobs2 = club(&d2.0, |o| o.routes = vec![route(Some(3000), "main")]);
        run(&jobs2, Work::Fetch { source: "custom:club".into() });
        assert_eq!(count(&d2.0.join("lib/main.bragd")), 0);
        // `club`'s rule raised to 2000: pruning `club` takes the weak Game, `main` is untouched.
        let (a, mut o, _) = jobs.sources().into_iter().find(|(a, _, _)| a.id == "custom:club").unwrap();
        o.routes[1] = route(Some(2000), "club");
        jobs.set_source(&a.id, o).unwrap();
        let pruned = run(&jobs, Work::Prune { library: club_lib.clone(), dry_run: false }).report.unwrap().library.unwrap();
        assert_eq!((pruned.games, count(&club_lib), count(&main)), (1, 1, 1));
        // Routes must name Libraries, each once.
        let (a, mut o, _) = jobs.sources().into_iter().find(|(a, _, _)| a.id == "custom:club").unwrap();
        for bad in [vec![], vec![route(None, "main"), route(Some(1), "main")], vec![route(None, "../x")], vec![route(None, "")]] {
            o.routes = bad.clone();
            assert!(jobs.set_source(&a.id, o.clone()).is_err(), "{bad:?}");
        }
    }

    /// A `state.json` from before Routes: the Source's one Strip rule becomes one Route to the
    /// main Library, and a Source with no rule gets a Route that keeps everything.
    #[test]
    fn an_old_strip_rule_becomes_one_route_to_the_main_library() {
        let d = crate::tests::Dir::new("routes-upgrade");
        let old = serde_json::json!({
            "next_id": 0,
            "jobs": [],
            "sources": {
                "twic": { "options": { "enabled": true, "schedule": "weekly", "strip": { "min_rating": 2200 } }, "memo": {} },
                "custom:club": { "options": { "enabled": false, "schedule": "weekly", "url": "https://example.org/a.pgn" }, "memo": {} }
            }
        });
        std::fs::write(d.0.join("state.json"), old.to_string()).unwrap();
        let jobs = jobs_in(&d.0, Recorded::default(), Arc::default());
        let routes = |id: &str| jobs.sources().into_iter().find(|(a, _, _)| a.id == id).unwrap().1.routes;
        assert_eq!(routes("twic"), [sources::Route { strip: Some(StripRule { min_rating: Some(2200), ..Default::default() }), library: "main".into() }]);
        assert_eq!(routes("custom:club"), [sources::Route::main()]);
        assert!(!d.0.join("state.json.bad").exists());
        // Written back in the new form.
        let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(d.0.join("state.json")).unwrap()).unwrap();
        assert!(saved["sources"]["twic"]["options"].get("strip").is_none());
        assert_eq!(saved["sources"]["twic"]["options"]["routes"][0]["library"], "main");
    }
}
