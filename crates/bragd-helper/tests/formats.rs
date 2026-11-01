//! The Helper's `state.json`: today's form is the one committed for its version, and every
//! committed version reads with today's code and keeps its jobs and Sources
//! (`docs/reference/file-formats.md`).

use std::path::PathBuf;

use bragd_core::format;
use bragd_helper::jobs::{FORMAT, State, Status, Work};

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bragd-helper-formats-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The jobs and Sources every fixture holds, as JSON a user's Helper could have written.
const SAMPLE: &str = r#"{
  "next_id": 3,
  "jobs": [
    { "id": 1, "work": { "kind": "fetch", "source": "twic" }, "status": "done", "done": 2048, "total": 2048, "error": null, "resumed": 1, "report": null },
    { "id": 2, "work": { "kind": "import", "path": "/home/anna/games.pgn", "library": "/home/anna/main.bragd", "source": "mine" }, "status": "failed", "done": 10, "total": null, "error": "no such file", "resumed": 0, "report": null }
  ],
  "sources": {
    "twic": {
      "options": { "enabled": true, "schedule": "weekly", "only_unmetered": true, "disk_cap": 1000000000, "max_download": null, "url": "", "routes": [{ "strip": { "min_rating": 2200 }, "library": "main" }], "after_import": "strip" },
      "memo": { "last_fetch": 1800000000, "last_try": 1800000000, "last_issue": 1600 }
    }
  }
}"#;

/// `state.json` read as the Helper reads it.
fn load(text: &str, name: &str) -> (State, bool) {
    let p = dir(name).join("state.json");
    std::fs::write(&p, text).unwrap();
    let st = State::load(&p);
    assert!(p.exists(), "{name}: set aside as broken");
    st
}

#[test]
fn the_current_format_is_the_one_committed() {
    let (st, keep) = load(SAMPLE, "current");
    assert!(keep);
    let text = String::from_utf8(st.to_json()).unwrap();
    format::snapshot("helper-state", FORMAT.version(), "state.json", &text);
    format::snapshot("helper-state", FORMAT.version(), "shape.txt", &format::shape(&serde_json::from_str(&text).unwrap()));
}

#[test]
fn every_committed_version_keeps_its_jobs_and_sources() {
    let all = format::versions("helper-state");
    assert_eq!(all.last().map(|v| v.0), Some(FORMAT.version()), "the current version has no fixture");
    for (v, fixture) in all {
        let (st, keep) = load(&std::fs::read_to_string(fixture.join("state.json")).unwrap(), &format!("v{v}"));
        assert!(keep, "v{v}");
        assert_eq!(st.next_id, 3, "v{v}");
        let jobs: Vec<_> = st.jobs.iter().map(|j| (j.id, j.status, j.done, j.total, j.error.clone(), j.resumed)).collect();
        assert_eq!(jobs, [(1, Status::Done, 2048, Some(2048), None, 1), (2, Status::Failed, 10, None, Some("no such file".into()), 0)], "v{v}");
        assert_eq!(st.jobs[0].work, Work::Fetch { source: "twic".into() }, "v{v}");
        let twic = &st.sources["twic"];
        let o = twic.options.as_ref().unwrap();
        assert!(o.enabled && o.only_unmetered, "v{v}");
        assert_eq!((o.disk_cap, o.routes[0].library.as_str(), o.routes[0].strip.as_ref().and_then(|s| s.min_rating)), (Some(1_000_000_000), "main", Some(2200)), "v{v}");
        assert_eq!((twic.memo.last_fetch, twic.memo.last_issue), (Some(1_800_000_000), Some(1600)), "v{v}");
    }
}

#[test]
fn a_state_from_a_newer_bragd_is_left_as_it_is() {
    let newer = format!(r#"{{"format":{},"next_id":9}}"#, FORMAT.version() + 1);
    let (st, keep) = load(&newer, "newer");
    assert!(!keep);
    assert_eq!(st.next_id, 0);
}
