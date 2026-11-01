//! The window's line to the Helper (`bragd_helper::helper`): dialled, and the Helper started
//! when none runs, from the `bragd-helper` program beside this one (`helper::program`). Every
//! request goes over the one line; a line that broke (the Helper left, or restarted for new
//! settings) is dialled again once. Also here: the Helper's status and its jobs as the sidebar
//! footer and the Jobs page draw them, every text made here.

use std::sync::Mutex;

use crate::lock;
use std::time::{Duration, Instant};

use bragd_core::t;
use bragd_helper::disk;
use bragd_helper::helper::{self, Conn, Reply, Request};
use bragd_helper::jobs::{Job, Report, Status, Work};
use serde::Serialize;

/// How long a window waits before it tries again to reach a Helper that did not answer.
const RETRY: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct Link {
    conn: Mutex<Option<Conn>>,
    /// The last failure to reach it, and when.
    failed: Mutex<Option<(Instant, String)>>,
}

impl Link {
    /// One request, one reply; the Helper's own refusals come back as errors.
    pub fn ask(&self, request: &Request) -> Result<Reply, String> {
        let mut conn = lock(&self.conn);
        for _ in 0..2 {
            if conn.is_none() {
                if let Some((at, e)) = &*lock(&self.failed)
                    && at.elapsed() < RETRY
                {
                    return Err(e.clone());
                }
                match helper::program().and_then(|p| helper::connect_or_start(&p)) {
                    Ok(c) => {
                        *conn = Some(c);
                        *lock(&self.failed) = None;
                    }
                    Err(e) => {
                        let e = t!("helper.unreachable", "why" => e);
                        *lock(&self.failed) = Some((Instant::now(), e.clone()));
                        return Err(e);
                    }
                }
            }
            match conn.as_mut().map(|c| c.ask(request)) {
                Some(Ok(Reply::Error { error })) => return Err(error),
                Some(Ok(reply)) => return Ok(reply),
                // The line broke: dial again, once.
                _ => *conn = None,
            }
        }
        Err(t!("helper.lost"))
    }

    /// Forgets the last failure, so the next request dials at once.
    pub fn retry(&self) {
        *lock(&self.failed) = None;
    }

    /// Starts a job by `request`: its id.
    pub fn start(&self, request: &Request) -> Result<u64, String> {
        match self.ask(request)? {
            Reply::Started { id } => Ok(id),
            _ => Err(t!("helper.odd_reply")),
        }
    }

    pub fn jobs(&self) -> Result<Vec<Job>, String> {
        match self.ask(&Request::Jobs)? {
            Reply::Jobs { jobs } => Ok(jobs),
            _ => Err(t!("helper.odd_reply")),
        }
    }
}

/// The Helper as the sidebar footer and the Jobs page show it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelperView {
    pub on: bool,
    /// Why it could not be reached; empty when it was.
    pub error: String,
    /// One line for the footer: idle, N jobs, paused on the battery, or out of reach.
    pub line: String,
    pub paused: bool,
    pub active: usize,
    /// How far the running job is, 0 to 1, when that is known.
    pub progress: Option<f64>,
    /// Newest first.
    pub jobs: Vec<JobView>,
    /// The disk limits crossed, as texts.
    pub notices: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: u64,
    pub what: String,
    pub status: Status,
    pub status_text: String,
    pub progress: Option<f64>,
    /// `1,234 of 5,000`, or `1,234` when the total is not known.
    pub count: String,
    pub error: String,
    /// What a Strip or Prune job did or would do.
    pub report: String,
    /// Queued or running: it can be cancelled.
    pub live: bool,
}

fn file_name(p: &std::path::Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// What a job does, in words.
pub fn what(work: &Work) -> String {
    match work {
        Work::Import { path, library, .. } => t!("job.import", "file" => file_name(path), "library" => file_name(library)),
        Work::Dedupe { library } => t!("job.dedupe", "library" => file_name(library)),
        Work::Export { library, .. } => t!("job.export", "library" => file_name(library)),
        Work::Fetch { source } => t!("job.fetch", "source" => source),
        Work::Strip { source, dry_run: true } => t!("job.strip_preview", "source" => source),
        Work::Strip { source, .. } => t!("job.strip", "source" => source),
        Work::Prune { library, dry_run: true } => t!("job.prune_preview", "library" => file_name(library)),
        Work::Prune { library, .. } => t!("job.prune", "library" => file_name(library)),
    }
}

/// What a Strip or Prune job did, or would do, in words.
pub fn report(r: &Report) -> String {
    let mut out = vec![];
    if !r.files.is_empty() {
        let (kept, stripped) = r.files.iter().fold((0, 0), |(k, s), f| (k + f.kept, s + f.stripped));
        let before: u64 = r.files.iter().map(|f| f.bytes_before).sum();
        let after: u64 = r.files.iter().map(|f| f.bytes_after).sum();
        out.push(t!("job.report_files", "files" => r.files.len(), "kept" => kept, "stripped" => stripped, "before" => disk::bytes(before), "after" => disk::bytes(after)));
    }
    if !r.removed.is_empty() {
        let bytes: u64 = r.removed.iter().map(|(_, b)| b).sum();
        out.push(t!("job.report_removed", "files" => r.removed.len(), "bytes" => disk::bytes(bytes)));
    }
    if let Some(l) = &r.library {
        out.push(t!("job.report_library", "games" => l.games, "of" => l.of, "bytes" => disk::bytes(l.bytes)));
    }
    if out.is_empty() {
        out.push(t!("job.report_nothing"));
    }
    out.join(" ")
}

fn fraction(j: &Job) -> Option<f64> {
    j.total.filter(|&t| t > 0).map(|t| (j.done as f64 / t as f64).clamp(0.0, 1.0))
}

pub fn job_view(j: &Job) -> JobView {
    let status_text = t!(&format!("job.status.{}", serde_json::to_value(j.status).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()));
    JobView {
        id: j.id,
        what: what(&j.work),
        status: j.status,
        status_text,
        progress: fraction(j),
        count: match j.total {
            Some(t) => t!("job.count_of", "done" => j.done, "total" => t),
            None => t!("job.count", "done" => j.done),
        },
        error: j.error.clone().unwrap_or_default(),
        report: j.report.as_ref().map(report).unwrap_or_default(),
        live: matches!(j.status, Status::Queued | Status::Running),
    }
}

/// The Helper's status and jobs; out of reach is a view too, with the reason.
pub fn view(link: &Link) -> HelperView {
    let status = match link.ask(&Request::Status) {
        Ok(Reply::Status(s)) => s,
        Ok(_) => helper::Status::default(),
        Err(error) => {
            return HelperView { on: false, line: t!("helper.off"), error, paused: false, active: 0, progress: None, jobs: vec![], notices: vec![] };
        }
    };
    let jobs = link.jobs().unwrap_or_default();
    let progress = jobs.iter().find(|j| j.status == Status::Running).and_then(fraction);
    let line = if status.paused {
        t!("helper.paused")
    } else if status.active > 0 {
        bragd_core::tn!("helper.active", status.active)
    } else {
        t!("helper.idle")
    };
    HelperView {
        on: true,
        error: String::new(),
        line,
        paused: status.paused,
        active: status.active,
        progress,
        jobs: jobs.iter().rev().map(job_view).collect(),
        notices: status.disk.iter().map(disk::Crossed::text).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bragd_library::library::Pruned;
    use bragd_library::strip::Rewritten;

    #[test]
    fn a_strip_report_says_what_goes() {
        let r = Report {
            files: vec![Rewritten { kept: 3, stripped: 7, bytes_before: 10_000, bytes_after: 3_000, ..Default::default() }],
            removed: vec![],
            library: Some(Pruned { games: 7, of: 10, bytes: 2_000_000, ..Default::default() }),
        };
        let text = report(&r);
        assert!(text.contains('7') && text.contains("10") && text.contains("2.0 MB") && text.contains("3.0 kB"), "{text}");
        assert_eq!(report(&Report::default()), t!("job.report_nothing"));
    }

    #[test]
    fn a_job_shows_its_progress() {
        let j = Job { id: 1, work: Work::Fetch { source: "twic".into() }, status: Status::Running, done: 25, total: Some(100), error: None, resumed: 0, report: None };
        let v = job_view(&j);
        assert_eq!(v.progress, Some(0.25));
        assert!(v.live && v.what.contains("twic"));
        assert!(!job_view(&Job { status: Status::Done, ..j }).live);
    }
}
