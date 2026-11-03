//! Ur's solved tables on its Engines page: one a rule set of each Ur Variant that is on, with
//! its size on disk and the memory it takes to make, whether it is here, and the Helper's job
//! that makes it (`Work::UrTable`). Made on this machine: nothing is downloaded.

use bragd_core::config::Config;
use bragd_core::{t, tips};
use bragd_engines::ur_table::{self, Table};
use bragd_games::Family;
use bragd_helper::helper::Request;
use bragd_helper::jobs::{Job, Status, Work};
use serde::Serialize;
use tauri::State;

use crate::Ctx;
use crate::knowledge::Knowledge;

type Res<T> = Result<T, String>;

/// One rule set's table, as the page shows it.
#[derive(Serialize, Debug, PartialEq)]
pub struct TableView {
    variant: String,
    name: String,
    /// On disk, and the memory a solve takes, as shown (`276 MB`).
    size: String,
    memory: String,
    ready: bool,
    /// The job making it, while it is queued or running, and how far, 0 to 1.
    job: Option<u64>,
    progress: Option<f64>,
    error: Option<String>,
}

/// The tables of the Ur Variants that are on; `jobs` from the Helper.
pub fn page(cfg: &Config, jobs: &[Job]) -> Vec<TableView> {
    Family::Ur
        .variants()
        .filter(|v| cfg.variant_on(v.key))
        .filter_map(|v| {
            let rules = bragd_games::ur_rules(v.key)?;
            let mine = |j: &&Job| matches!(&j.work, Work::UrTable { variant } if variant == v.key);
            let live = jobs.iter().filter(mine).find(|j| matches!(j.status, Status::Queued | Status::Running));
            let failed = jobs.iter().rfind(mine).filter(|j| j.status == Status::Failed).and_then(|j| j.error.clone());
            Some(TableView {
                variant: v.key.into(),
                name: t!(&format!("variant.{}", v.key)),
                size: bragd_core::i18n::bytes(ur_table::size(rules)),
                memory: bragd_core::i18n::bytes(ur_table::memory(rules)),
                ready: Table::installed(rules).is_some(),
                job: live.map(|j| j.id),
                progress: live.and_then(|j| j.total.filter(|&t| t > 0).map(|t| j.done as f64 / t as f64)),
                error: failed,
            })
        })
        .collect()
}

#[tauri::command(async)]
pub fn ur_tables(ctx: State<'_, Ctx>, k: State<'_, Knowledge>) -> Res<Vec<TableView>> {
    Ok(page(&ctx.cfg(), &k.link.jobs().unwrap_or_default()))
}

/// Asks the Helper to make the table of `variant`'s rule set.
#[tauri::command(async)]
pub fn ur_table_make(variant: String, k: State<'_, Knowledge>) -> Res<u64> {
    bragd_games::ur_rules(&variant).ok_or_else(|| t!("engines.unknown", "name" => variant))?;
    tips::used("ur_solved");
    k.link.start(&Request::Start { work: Work::UrTable { variant } })
}

/// Deletes the table of `variant`'s rule set.
#[tauri::command(async)]
pub fn ur_table_delete(variant: String) -> Res<()> {
    let rules = bragd_games::ur_rules(&variant).ok_or_else(|| t!("engines.unknown", "name" => variant))?;
    match ur_table::path(rules).map(std::fs::remove_file) {
        Some(Err(e)) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: u64, variant: &str, status: Status) -> Job {
        Job { id, work: Work::UrTable { variant: variant.into() }, status, done: 5, total: Some(10), error: (status == Status::Failed).then(|| "no room".into()), resumed: 0, report: None, lent: None, from: None }
    }

    /// Each Ur Variant that is on has its table's row, with its size, and the job making it.
    #[test]
    fn every_ur_variant_on_has_its_table() {
        let mut cfg = Config::default();
        let rows = page(&cfg, &[job(7, "ur-blitz", Status::Running), job(3, "ur-aseb", Status::Failed)]);
        assert_eq!(rows.iter().map(|r| r.variant.as_str()).collect::<Vec<_>>(), ["ur", "ur-masters", "ur-blitz", "ur-aseb"]);
        let blitz = &rows[2];
        assert_eq!((blitz.job, blitz.progress), (Some(7), Some(0.5)));
        assert_eq!(rows[3].error.as_deref(), Some("no room"));
        assert_eq!(rows[0].job, None);
        assert!(!rows[0].size.is_empty() && !rows[0].memory.is_empty());
        cfg.families.variant.insert("ur-masters".into(), false);
        assert!(page(&cfg, &[]).iter().all(|r| r.variant != "ur-masters"), "a Variant that is off has no row");
    }
}
