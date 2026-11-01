//! The Tablebases and Opening books items of chess: the Syzygy folder and what it holds, the
//! download of a set (planned first with its size, and run on a thread of this window's own
//! with its progress shown), and the Polyglot books.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use bragd_core::config::Config;
use bragd_engines::fetch::Http;
use bragd_engines::syzygy::{self, Plan, Set};
use bragd_helper::disk;
use bragd_library::chess::polyglot;
use serde::Serialize;

use bragd_core::offline;
use crate::engines::no_folder;
use bragd_core::i18n::tr;

/// A download under way or done.
#[derive(Clone, Default, Serialize)]
pub struct Job {
    set: Option<Set>,
    done: u64,
    total: u64,
    finished: bool,
    error: Option<String>,
}

#[derive(Default)]
pub struct Held {
    job: Arc<Mutex<Job>>,
    stop: Mutex<Option<Arc<AtomicBool>>>,
    /// Plans made, by set, so the size shown is the size fetched.
    plans: Mutex<Vec<Plan>>,
}

/// A set as the page offers it.
#[derive(Serialize)]
pub struct SetView {
    set: Set,
    /// How many of its tables (WDL and DTZ files) are in the folder.
    have: usize,
    /// Its exact size as shown (`1.5 GB`), once planned.
    size: Option<String>,
    /// Downloaded here, or a folder only (7 pieces).
    download: bool,
}

#[derive(Serialize)]
pub struct Page {
    folder: String,
    custom: bool,
    online: bool,
    offline: bool,
    sets: Vec<SetView>,
    job: Job,
    /// The job's bytes so far and in all, as shown: `1.2 GB / 1.5 GB`.
    progress: String,
}

fn pieces_of(set: Set) -> std::ops::RangeInclusive<usize> {
    match set {
        Set::UpTo5 => 3..=5,
        Set::Six => 6..=6,
        Set::Seven => 7..=7,
    }
}

/// How many tables of each piece count `folder` holds.
fn count(folder: &Path, set: Set) -> usize {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| (n.ends_with(".rtbw") || n.ends_with(".rtbz")) && pieces_of(set).contains(&syzygy::pieces(n)))
        .count()
}

pub fn page(cfg: &Config, held: &Held) -> Page {
    let folder = cfg.tablebase_folder().unwrap_or_default();
    let plans = crate::lock(&held.plans);
    let job = crate::lock(&held.job).clone();
    Page {
        custom: !cfg.tablebases.path.trim().is_empty(),
        folder: folder.display().to_string(),
        online: cfg.tablebases.online,
        offline: offline(),
        sets: [Set::UpTo5, Set::Six, Set::Seven].into_iter().map(|set| SetView { set, have: count(&folder, set), size: plans.iter().find(|p| p.set == set).map(|p| disk::bytes(p.total)), download: set != Set::Seven }).collect(),
        progress: format!("{} / {}", disk::bytes(job.done), disk::bytes(job.total)),
        job,
    }
}

/// The size of `set`, asked of the mirror (one small file); nothing is downloaded.
pub fn plan(held: &Held, set: Set) -> Result<u64, String> {
    if offline() {
        return Err(tr("tablebases.offline", &[]));
    }
    let p = syzygy::plan_from(&Http, set)?;
    let total = p.total;
    let mut plans = crate::lock(&held.plans);
    plans.retain(|q| q.set != set);
    plans.push(p);
    Ok(total)
}

/// Downloads the planned `set` into the folder on a thread; the page asks how far it is.
pub fn download(cfg: &Config, held: &Held, set: Set) -> Result<(), String> {
    if offline() {
        return Err(tr("tablebases.offline", &[]));
    }
    let plan = crate::lock(&held.plans).iter().find(|p| p.set == set).cloned().ok_or_else(|| tr("tablebases.plan_first", &[]))?;
    let busy = { let j = crate::lock(&held.job); j.set.is_some() && !j.finished };
    if busy {
        return Err(tr("tablebases.busy", &[]));
    }
    let stop = Arc::new(AtomicBool::new(false));
    *crate::lock(&held.stop) = Some(stop.clone());
    *crate::lock(&held.job) = Job { set: Some(set), total: plan.total, ..Default::default() };
    let (job, folder) = (held.job.clone(), cfg.tablebase_folder().ok_or_else(no_folder)?);
    std::thread::spawn(move || {
        let r = syzygy::download(&plan, &folder, &Http, &mut |done, total| {
            let mut j = crate::lock(&job);
            (j.done, j.total) = (done, total);
            !stop.load(Ordering::Relaxed)
        });
        let mut j = crate::lock(&job);
        j.finished = true;
        j.error = r.err();
    });
    Ok(())
}

pub fn stop(held: &Held) {
    if let Some(s) = crate::lock(&held.stop).as_ref() {
        s.store(true, Ordering::Relaxed);
    }
}

/// An opening book as the Opening books page lists it.
#[derive(Serialize)]
pub struct Book {
    path: String,
    entries: Option<u64>,
    /// Its moves from the start, in SAN.
    first: Vec<String>,
    error: Option<String>,
}

pub fn books(cfg: &Config) -> Vec<Book> {
    cfg.engines
        .books
        .iter()
        .map(|p| match polyglot::summary(Path::new(p)) {
            Ok(s) => Book { path: p.clone(), entries: Some(s.entries), first: s.moves.iter().take(8).map(|m| m.san.clone()).collect(), error: None },
            Err(e) => Book { path: p.clone(), entries: None, first: vec![], error: Some(e.to_string()) },
        })
        .collect()
}

/// Adds the Polyglot book at `path`, once it reads as one.
pub fn add_book(cfg: &Config, path: &str) -> Result<Config, String> {
    let path = path.trim();
    if !polyglot::is_book(Path::new(path)) {
        return Err(tr("books.not_a_book", &[("path", path)]));
    }
    let mut all = cfg.engines.books.clone();
    if !all.iter().any(|b| b == path) {
        all.push(path.to_string());
    }
    Config::save_value(&["engines", "books"], all.iter().map(String::as_str).collect::<toml_edit::Array>().into())
}

pub fn remove_book(cfg: &Config, path: &str) -> Result<Config, String> {
    let all: Vec<&str> = cfg.engines.books.iter().map(String::as_str).filter(|b| *b != path).collect();
    Config::save_value(&["engines", "books"], all.into_iter().collect::<toml_edit::Array>().into())
}
