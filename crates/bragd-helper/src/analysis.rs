//! Background analysis: a job of positions (`bragd_positions::queue::Job`) put to its Engine,
//! the most frequent first, each result merged into the Position store. Positions the store
//! already answers well enough are skipped, so a job stopped and started again goes on where
//! it was. Several engines run at once, each on one thread, as engine matches play their Games
//! (`bragd_engines::matches::run`). The Helper runs these jobs on a worker of their own
//! (`Jobs::work_analysis`), inside the limits of `[analysis]`.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use bragd_engines::seam::{self, Notation};
use bragd_engines::session::Analysis;
use bragd_engines::{Limit, Session, think};
use bragd_positions::queue::{Job, Per};
use bragd_positions::{Edition, Evaluation, Second, Store};

/// An engine started for a job: its session, how it writes moves, and its Edition.
pub struct Started {
    pub session: Box<dyn Session>,
    pub notation: Notation,
    pub edition: Edition,
}

/// How far a job is: positions done (analysed, or answered already) of all, and an estimate of
/// the seconds left, once one was analysed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
    pub left: Option<u64>,
}

pub struct Run<'a> {
    pub job: &'a Job,
    /// The Position store's file.
    pub store: &'a Path,
    /// Engines at once.
    pub engines: usize,
    /// Starts one engine; called on each engine's own thread.
    pub start: &'a (dyn Fn() -> Result<Started, String> + Sync),
    /// Asked before each position: waits while the limits hold the job back; true to stop.
    pub gate: &'a (dyn Fn() -> bool + Sync),
    pub stop: &'a (dyn Fn() -> bool + Sync),
    pub progress: &'a (dyn Fn(Progress) + Sync),
}

/// Runs the job until every position is answered, `stop` or the gate says so, or an engine
/// fails (its error). The positions done.
pub fn run(r: &Run) -> Result<u64, String> {
    let job = r.job;
    let store = Store::open(r.store).map_err(|e| e.to_string())?;
    let mut todo = vec![];
    for (i, spot) in job.spots.iter().enumerate() {
        if !store.answers(job, spot).map_err(|e| e.to_string())? {
            todo.push(i);
        }
    }
    let total = job.spots.len() as u64;
    let done = AtomicU64::new(total - todo.len() as u64);
    (r.progress)(Progress { done: done.load(Ordering::SeqCst), total, left: None });
    if todo.is_empty() {
        return Ok(total);
    }
    let store = Mutex::new(store);
    let next = AtomicUsize::new(0);
    let analysed = AtomicU64::new(0);
    let error: Mutex<Option<String>> = Mutex::new(None);
    let failed = |e: String| *error.lock().unwrap_or_else(|e| e.into_inner()) = Some(e);
    let begun = Instant::now();
    let limit = match job.per {
        Per::Depth(d) => Limit::Depth(d),
        Per::Millis(ms) => Limit::Time(ms),
    };
    let worker = || {
        let mut engine = match (r.start)() {
            Ok(e) => e,
            Err(e) => return failed(e),
        };
        if !job.accepts(&engine.edition) {
            let min = job.min_version.clone().unwrap_or_default();
            return failed(bragd_core::i18n::tr("analysis.too_old", &[("edition", &engine.edition.version), ("min", &min)]));
        }
        loop {
            if error.lock().unwrap_or_else(|e| e.into_inner()).is_some() || (r.stop)() || (r.gate)() {
                return;
            }
            let Some(&i) = todo.get(next.fetch_add(1, Ordering::SeqCst)) else { return };
            let spot = &job.spots[i];
            let initial = Some(spot.setup.as_str()).filter(|s| !s.trim().is_empty());
            // A position the seam cannot put to this engine (a Game already over) has nothing
            // to analyse: it counts as done.
            if let Ok((p, _)) = seam::position(&job.variant, initial, &spot.moves, engine.notation) {
                // Two lines: the second choice is what tells an only move (`Annotation`'s !!).
                match think(&mut *engine.session, &p, limit, 2, r.stop, &mut |_| {}) {
                    Ok(a) => {
                        if let Some(e) = evaluation(job, spot.key, &engine.edition, &a)
                            && let Err(e) = store.lock().unwrap_or_else(|e| e.into_inner()).put(&e)
                        {
                            return failed(e.to_string());
                        }
                    }
                    Err(_) if (r.stop)() => return,
                    Err(e) => return failed(e.to_string()),
                }
            }
            let n = analysed.fetch_add(1, Ordering::SeqCst) + 1;
            let d = done.fetch_add(1, Ordering::SeqCst) + 1;
            let per = begun.elapsed().as_secs_f64() / n as f64;
            (r.progress)(Progress { done: d, total, left: Some((per * (total - d) as f64).round() as u64) });
        }
    };
    std::thread::scope(|sc| {
        for _ in 0..r.engines.clamp(1, todo.len()) {
            sc.spawn(worker);
        }
    });
    match error.into_inner().unwrap_or_else(|e| e.into_inner()) {
        Some(e) => Err(e),
        None => Ok(done.into_inner()),
    }
}

/// What the store keeps of `a`: its first line's score, depth and moves, and its second line
/// when it gave one. None when the engine gave no score.
fn evaluation(job: &Job, key: u64, edition: &Edition, a: &Analysis) -> Option<Evaluation> {
    let line = a.lines.first()?;
    let second = a.lines.get(1).and_then(|l| Some(Second { score: l.score?.into(), line: l.pv.join(" ") })).filter(|s| !s.line.is_empty());
    Some(Evaluation {
        family: job.family.clone(),
        variant: job.variant.clone(),
        key,
        edition: edition.clone(),
        score: line.score?.into(),
        depth: a.depth,
        line: line.pv.join(" "),
        time: bragd_core::fs::now() as i64,
        second,
    })
}

/// The hour of the day now, local time, 0 to 23.
#[cfg(unix)]
pub fn local_hour() -> u64 {
    let now = bragd_core::fs::now() as libc::time_t;
    // Safety: a plain struct the call fills in.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&now, &mut tm) }.is_null() {
        return bragd_core::fs::now() / 3600 % 24;
    }
    tm.tm_hour as u64
}

#[cfg(windows)]
pub fn local_hour() -> u64 {
    // Safety: a plain struct the call fills in.
    let mut t: windows_sys::Win32::Foundation::SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut t) };
    u64::from(t.wHour)
}

#[cfg(not(any(unix, windows)))]
pub fn local_hour() -> u64 {
    bragd_core::fs::now() / 3600 % 24
}

#[cfg(test)]
mod tests {
    use super::*;
    use bragd_engines::Score;
    use bragd_engines::session::Line;

    #[test]
    fn a_result_is_kept_from_its_first_line() {
        let job = Job::new("chess", "chess", "stockfish", None, Per::Depth(10), vec![], false, "");
        let ed = Edition { engine: "stockfish".into(), version: "Stockfish 17.1".into(), network: None, image: None };
        let a = Analysis { depth: 12, lines: vec![Line { score: Some(Score::Mate(-3)), pv: vec!["e2e4".into(), "e7e5".into()] }, Line { score: Some(Score::Cp(20)), pv: vec!["d2d4".into()] }], ..Default::default() };
        let e = evaluation(&job, 9, &ed, &a).unwrap();
        assert_eq!((e.key, e.depth, e.score, e.line.as_str()), (9, 12, bragd_positions::Score::Mate(-3), "e2e4 e7e5"));
        assert_eq!(e.second, Some(Second { score: bragd_positions::Score::Cp(20), line: "d2d4".into() }));
        assert!(evaluation(&job, 9, &ed, &Analysis::default()).is_none());
    }

    #[test]
    fn the_local_hour_is_an_hour() {
        assert!(local_hour() < 24);
    }
}
