//! Background analysis and the Position store, as the window asks for them: a Slice queued
//! with *Analyse in background*, the positions of a Game opened queued at low priority, and
//! the store's housekeeping in Disk watch. The work is the Helper's (`bragd_helper::analysis`)
//! and the store's (`bragd_positions`); this module builds the jobs and the texts.

use std::path::Path;

use bragd_core::config::Config;
use bragd_core::t;
use bragd_engines::local;
use bragd_helper::helper::Request;
use bragd_helper::jobs::Work;
use bragd_library::chess::{pgn, polyglot};
use bragd_library::library::Library;
use bragd_positions::queue::{Job, Per, Spot};
use bragd_positions::{Edition, Store};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::Ctx;
use crate::databases::Replay;
use crate::knowledge::Knowledge;

type Res<T> = Result<T, String>;

/// The most positions one Slice job may hold.
const MOST: u32 = 100_000;

/// The job for the chess positions of `r`, a Game just opened, by the Engine its Variant is
/// analysed with, to `[analysis] depth`; None when Settings turn it off, the Game is not chess,
/// or that Engine is not there to run.
// shortcut: chess only, as only chess Games open with their positions (`stored_replay`); the
// other Families join when theirs do.
pub fn opened_job(cfg: &Config, r: &Replay) -> Option<Job> {
    if !cfg.analysis.open_games || r.family != "chess" || r.variant.is_empty() {
        return None;
    }
    let mut spots: Vec<Spot> = vec![];
    for fen in r.steps.iter().filter_map(|s| s.fen.as_deref()) {
        let key = polyglot::key(&pgn::board(fen)?);
        if !spots.iter().any(|s| s.key == key) {
            spots.push(Spot { key, setup: fen.to_string(), moves: vec![], count: 1 });
        }
    }
    let engine = local::default_for(cfg, &r.variant).filter(|e| local::availability(cfg, e).ready())?;
    (!spots.is_empty()).then(|| Job::new("chess", &r.variant, &engine, None, Per::Depth(cfg.analysis.depth.clamp(1, 99) as u32), spots, true, &r.title))
}

/// Queues the positions of the Game `r` just opened, when Settings ask for it. Nothing is said
/// when it cannot be: the Game opens all the same.
pub fn queue_opened(k: &Knowledge, cfg: &Config, r: &Replay) {
    if let Some(job) = opened_job(cfg, r) {
        bragd_core::tips::used("background");
        let _ = k.link.start(&Request::Start { work: Work::Analyse { job } });
    }
}

/// One Engine chosen for a Slice job, with the oldest version that may answer (empty: any).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chosen {
    pub engine: String,
    #[serde(default)]
    pub min_version: String,
}

/// *Analyse in background*: the `top` positions the Games of the Slice `name` reach most often,
/// each to `depth` or for `seconds`, one job per Engine and Variant. The jobs' ids.
#[tauri::command(async)]
pub fn slice_analyse(library: String, name: String, engines: Vec<Chosen>, depth: Option<u32>, seconds: Option<f64>, top: u32, k: State<'_, Knowledge>) -> Res<Vec<u64>> {
    bragd_core::tips::used("background");
    if engines.is_empty() {
        return Err(t!("background.no_engine"));
    }
    let per = match (depth, seconds) {
        (Some(d), _) if d > 0 => Per::Depth(d.min(99)),
        (_, Some(s)) if s > 0.0 => Per::Millis((s * 1000.0).round().min(3_600_000.0) as u32),
        _ => return Err(t!("background.no_limit")),
    };
    let lib = Library::open(Path::new(&library)).map_err(|e| e.to_string())?;
    let ids = lib.slice_games(&name).map_err(|e| e.to_string())?;
    let found = lib.frequent(&ids, top.clamp(1, MOST) as usize).map_err(|e| e.to_string())?;
    let mut variants: Vec<&str> = found.iter().map(|f| f.variant.as_str()).collect();
    variants.sort_unstable();
    variants.dedup();
    if variants.is_empty() {
        return Err(t!("background.no_positions"));
    }
    let mut out = vec![];
    for v in variants {
        let spots: Vec<Spot> = found.iter().filter(|f| f.variant == v).map(|f| Spot { key: f.key, setup: f.fen.clone(), moves: vec![], count: f.count }).collect();
        for c in &engines {
            let job = Job::new("chess", v, &c.engine, Some(c.min_version.clone()), per, spots.clone(), false, &name);
            out.push(k.link.start(&Request::Start { work: Work::Analyse { job } })?);
        }
    }
    Ok(out)
}

/// One Edition's share of the Position store, as Disk watch lists it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditionView {
    pub engine: String,
    pub version: String,
    pub positions: u64,
    pub size: String,
    /// Older than its Engine's newest Edition here.
    pub older: bool,
    /// Its Engine is no longer set up on this machine.
    pub removed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreView {
    pub editions: Vec<EditionView>,
    /// Older Editions and removed Engines, to say what each action takes.
    pub older: String,
    pub removed: String,
}

/// Whether the Engine `key` is no longer set up here: neither installed, nor its image there,
/// nor added by a path that is still there.
fn removed(cfg: &Config, key: &str) -> bool {
    !local::availability(cfg, key).ready()
}

fn store() -> Res<Store> {
    Store::open(&Store::path().ok_or_else(local::no_folder)?).map_err(|e| e.to_string())
}

fn view(cfg: &Config, sizes: Vec<bragd_positions::Size>) -> StoreView {
    let editions: Vec<Edition> = sizes.iter().map(|s| s.edition.clone()).collect();
    let older = |e: &Edition| editions.iter().any(|o| o.newer(e) == Some(std::cmp::Ordering::Greater));
    let mut engines: Vec<(String, bool)> = vec![];
    let mut gone = |key: &str| match engines.iter().find(|(k, _)| k == key) {
        Some((_, r)) => *r,
        None => {
            let r = removed(cfg, key);
            engines.push((key.to_string(), r));
            r
        }
    };
    let (mut old_rows, mut gone_rows) = (0, 0);
    let list = sizes
        .iter()
        .map(|s| {
            let (o, r) = (older(&s.edition), gone(&s.edition.engine));
            old_rows += if o { s.rows } else { 0 };
            gone_rows += if r { s.rows } else { 0 };
            EditionView {
                engine: bragd_engines::known::known(&s.edition.engine).map_or_else(|| s.edition.engine.clone(), |k| k.name.to_string()),
                version: s.edition.version.clone(),
                positions: s.rows,
                size: bragd_core::i18n::bytes(s.bytes),
                older: o,
                removed: r,
            }
        })
        .collect();
    StoreView { editions: list, older: bragd_core::tn!("background.positions", old_rows), removed: bragd_core::tn!("background.positions", gone_rows) }
}

/// The Position store in Disk watch: what each Engine and Edition takes.
#[tauri::command(async)]
pub fn store_report(ctx: State<'_, Ctx>) -> Res<StoreView> {
    Ok(view(&ctx.cfg(), store()?.sizes().map_err(|e| e.to_string())?))
}

/// Prunes the store: `older` Editions, or Engines `removed` from this machine. What went, in words.
#[tauri::command(async)]
pub fn store_prune(what: String, ctx: State<'_, Ctx>) -> Res<String> {
    let cfg = ctx.cfg();
    let mut s = store()?;
    let rows = match what.as_str() {
        "older" => s.prune_older(),
        "removed" => {
            let mut gone: Vec<String> = s.sizes().map_err(|e| e.to_string())?.into_iter().map(|z| z.edition.engine).filter(|e| removed(&cfg, e)).collect();
            gone.dedup();
            s.prune_engines(&gone)
        }
        other => return Err(format!("no pruning called {other}")),
    }
    .map_err(|e| e.to_string())?;
    Ok(bragd_core::tn!("background.pruned", rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::databases::Step;

    fn replay(steps: &[&str]) -> Replay {
        Replay { title: "A – B".into(), family: "chess".into(), variant: "chess".into(), steps: steps.iter().map(|f| Step { fen: Some(f.to_string()), ..Default::default() }).collect(), ..Default::default() }
    }

    /// An added Engine for chess by a path that is there, so it is ready to run.
    fn with_engine() -> Config {
        let exe = std::env::current_exe().unwrap();
        let mut cfg = Config::default();
        cfg.engines.added.insert("mine".into(), "chess".into());
        cfg.engines.paths.insert("mine".into(), exe.display().to_string());
        cfg.engines.default.insert("chess".into(), "mine".into());
        cfg
    }

    #[test]
    fn a_game_opened_queues_its_positions_once_each() {
        let cfg = with_engine();
        let after_e4 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";
        let job = opened_job(&cfg, &replay(&[pgn::START, after_e4, pgn::START])).unwrap();
        assert!(job.low && job.engine == "mine" && job.about == "A – B");
        assert_eq!(job.per, Per::Depth(18));
        assert_eq!(job.spots.len(), 2, "a position twice is one");
        assert_eq!(job.spots[1].key, polyglot::key(&pgn::board(after_e4).unwrap()));
        // Off in Settings, another Family, or no Engine to run: nothing.
        let mut off = cfg.clone();
        off.analysis.open_games = false;
        assert!(opened_job(&off, &replay(&[pgn::START])).is_none());
        assert!(opened_job(&cfg, &Replay { family: "go".into(), ..replay(&[pgn::START]) }).is_none());
    }

    #[test]
    fn the_store_view_marks_older_editions_and_removed_engines() {
        use bragd_positions::Size;
        let cfg = with_engine();
        let ed = |engine: &str, version: &str| Edition { engine: engine.into(), version: version.into(), network: None, image: None };
        let v = view(&cfg, vec![Size { edition: ed("mine", "Mine 1"), rows: 3, bytes: 3000 }, Size { edition: ed("mine", "Mine 2"), rows: 5, bytes: 5000 }, Size { edition: ed("gone", "Gone 1"), rows: 7, bytes: 7000 }]);
        assert_eq!(v.editions.iter().map(|e| (e.older, e.removed)).collect::<Vec<_>>(), [(true, false), (false, false), (false, true)]);
        assert_eq!(v.older, bragd_core::tn!("background.positions", 3));
        assert_eq!(v.removed, bragd_core::tn!("background.positions", 7));
    }
}
