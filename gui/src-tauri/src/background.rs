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
use bragd_library::library::{Library, position_keys};
use bragd_library::slice::Frequent;
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

/// A Game just opened, as background analysis takes it: its Family, its Variant
/// (`bragd_games`' key), its title and each of its positions once.
pub struct Opened {
    pub family: String,
    pub variant: String,
    pub title: String,
    pub spots: Vec<Spot>,
}

impl Opened {
    /// A Game of a Foreign database, by the positions its board draws: those its Variant sets
    /// up from their text (chess, xiangqi, shogi). A Game whose board cannot be drawn has none.
    pub fn of_file(r: &Replay) -> Opened {
        let fens: Vec<String> = r.steps.iter().map_while(|s| s.fen.clone()).collect();
        let keys = position_keys(&r.family, &fens);
        let spots = fens.into_iter().zip(keys).filter(|(fen, _)| !r.variant.is_empty() && bragd_games::replay::<&str>(&r.variant, Some(fen), &[]).is_ok()).map(|(setup, key)| Spot { key, setup, moves: vec![], count: 1 }).collect();
        Opened::new(&r.family, &r.variant, &r.title, spots)
    }

    /// The Game `id` of `lib`, played again from its stored moves (`Library::asked`).
    pub fn of_stored(lib: &Library, id: i64, title: &str) -> Res<Opened> {
        let s = lib.stored(id).map_err(|e| e.to_string())?;
        let spots = lib.asked(id, usize::MAX).map_err(|e| e.to_string())?.into_iter().map(|a| Spot { key: a.key, setup: a.setup, moves: a.moves, count: 1 }).collect();
        Ok(Opened::new(&s.family, &s.variant, title, spots))
    }

    fn new(family: &str, variant: &str, title: &str, mut spots: Vec<Spot>) -> Opened {
        let mut seen = std::collections::HashSet::new();
        spots.retain(|s| seen.insert(s.key));
        Opened { family: family.into(), variant: variant.into(), title: title.into(), spots }
    }
}

/// The job for the positions of `o`, a Game just opened, by the Engine its Variant is analysed
/// with, to `[analysis] depth`; None when Settings turn it off, it has no positions an Engine
/// can be asked about, or that Engine is not there to run.
pub fn opened_job(cfg: &Config, o: &Opened) -> Option<Job> {
    if !cfg.analysis.open_games || o.variant.is_empty() || o.spots.is_empty() {
        return None;
    }
    let engine = local::default_for(cfg, &o.variant).filter(|e| local::availability(cfg, e).ready())?;
    Some(Job::new(&o.family, &o.variant, &engine, None, Per::Depth(cfg.analysis.depth.clamp(1, 99) as u32), o.spots.clone(), true, &o.title))
}

/// Queues the positions of the Game `o` just opened, when Settings ask for it. Nothing is said
/// when it cannot be: the Game opens all the same.
pub fn queue_opened(k: &Knowledge, cfg: &Config, o: &Opened) {
    if let Some(job) = opened_job(cfg, o) {
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

/// An Engine *Analyse in background* offers.
#[derive(Serialize)]
pub struct Offered {
    key: String,
    name: String,
}

/// What *Analyse in background* offers for the Variants `variants`: every Engine ready to run
/// that plays one of them, the default ones, and whether more could be set up.
#[derive(Serialize)]
pub struct Offer {
    engines: Vec<Offered>,
    defaults: Vec<String>,
    more: bool,
}

pub fn offer(cfg: &Config, variants: &[String]) -> Offer {
    let mut o = Offer { engines: vec![], defaults: vec![], more: false };
    for v in variants {
        for key in local::engines_for(cfg, v) {
            if !local::availability(cfg, &key).ready() {
                o.more = true;
            } else if !o.engines.iter().any(|e| e.key == key) {
                let name = bragd_engines::known::known(&key).map_or_else(|| key.clone(), |k| k.name.to_string());
                o.engines.push(Offered { key, name });
            }
        }
        if let Some(d) = local::default_for(cfg, v).filter(|d| local::availability(cfg, d).ready() && !o.defaults.contains(d)) {
            o.defaults.push(d);
        }
    }
    o
}

#[tauri::command(async)]
pub fn background_engines(variants: Vec<String>, ctx: State<'_, Ctx>) -> Offer {
    offer(&ctx.cfg(), &variants)
}

/// The jobs for `found`, a Slice's most frequent positions: one per Variant and chosen Engine
/// that plays it, each to `per`.
pub fn slice_jobs(cfg: &Config, name: &str, found: &[Frequent], engines: &[Chosen], per: Per) -> Vec<Job> {
    let mut variants: Vec<(&str, &str)> = found.iter().map(|f| (f.family.as_str(), f.variant.as_str())).collect();
    variants.sort_unstable();
    variants.dedup();
    let mut out = vec![];
    for (family, v) in variants {
        let spots: Vec<Spot> = found.iter().filter(|f| f.variant == v).map(|f| Spot { key: f.key, setup: f.setup.clone(), moves: f.moves.clone(), count: f.count }).collect();
        let plays = local::engines_for(cfg, v);
        for c in engines.iter().filter(|c| plays.contains(&c.engine)) {
            out.push(Job::new(family, v, &c.engine, Some(c.min_version.clone()), per, spots.clone(), false, name));
        }
    }
    out
}

/// *Analyse in background*: the `top` positions the Games of the Slice `name` of `family`
/// (empty: any) reach most often, each to `depth` or for `seconds`, one job per Variant and
/// Engine that plays it. The jobs' ids.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments, reason = "one argument per thing the page sends")]
pub fn slice_analyse(library: String, name: String, family: String, engines: Vec<Chosen>, depth: Option<u32>, seconds: Option<f64>, top: u32, k: State<'_, Knowledge>, ctx: State<'_, Ctx>) -> Res<Vec<u64>> {
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
    let found = lib.frequent(&ids, &family, top.clamp(1, MOST) as usize).map_err(|e| e.to_string())?;
    let jobs = slice_jobs(&ctx.cfg(), &name, &found, &engines, per);
    if jobs.is_empty() {
        return Err(t!("background.no_positions"));
    }
    jobs.into_iter().map(|job| k.link.start(&Request::Start { work: Work::Analyse { job } })).collect()
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
    use bragd_library::chess::{pgn, polyglot};

    fn replay(steps: &[&str]) -> Replay {
        Replay { title: "A – B".into(), family: "chess".into(), variant: "chess".into(), steps: steps.iter().map(|f| Step { fen: Some(f.to_string()), ..Default::default() }).collect(), ..Default::default() }
    }

    /// Added Engines by a path that is there, so they are ready to run: `mine` for chess and
    /// `checkers` for draughts.
    fn with_engine() -> Config {
        let exe = std::env::current_exe().unwrap();
        let mut cfg = Config::default();
        for (key, family) in [("mine", "chess"), ("checkers", "draughts")] {
            cfg.engines.added.insert(key.into(), family.into());
            cfg.engines.paths.insert(key.into(), exe.display().to_string());
        }
        cfg.engines.default.insert("chess".into(), "mine".into());
        cfg.engines.default.insert("draughts".into(), "checkers".into());
        cfg
    }

    #[test]
    fn a_game_opened_queues_its_positions_once_each() {
        let cfg = with_engine();
        let after_e4 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";
        let job = opened_job(&cfg, &Opened::of_file(&replay(&[pgn::START, after_e4, pgn::START]))).unwrap();
        assert!(job.low && job.engine == "mine" && job.about == "A – B");
        assert_eq!(job.per, Per::Depth(18));
        assert_eq!(job.spots.len(), 2, "a position twice is one");
        assert_eq!(job.spots[1].key, polyglot::key(&pgn::board(after_e4).unwrap()));
        // Off in Settings, a board that cannot be drawn, or no Engine to run: nothing.
        let mut off = cfg.clone();
        off.analysis.open_games = false;
        assert!(opened_job(&off, &Opened::of_file(&replay(&[pgn::START]))).is_none());
        assert!(opened_job(&cfg, &Opened::of_file(&Replay { variant: String::new(), ..replay(&[pgn::START]) })).is_none());
    }

    /// A draughts Game opened from a Library is queued as moves from the start, which is how
    /// a draughts Engine is asked; each position once.
    #[test]
    fn a_library_game_of_another_family_is_queued() {
        let d = std::env::temp_dir().join(format!("bragd-bg-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let file = d.join("a.pdn");
        std::fs::write(&file, "[GameType \"20\"]\n\n1. 32-28 19-23 2. 28x19 14x23 *\n").unwrap();
        let mut lib = Library::open(&d.join("l.bragd")).unwrap();
        lib.import(&file, "mine", &|| false, &mut |_| {}).unwrap();
        let o = Opened::of_stored(&lib, 1, "A – B").unwrap();
        let _ = std::fs::remove_dir_all(&d);
        let job = opened_job(&with_engine(), &o).unwrap();
        assert_eq!((job.family.as_str(), job.variant.as_str(), job.engine.as_str()), ("draughts", "draughts", "checkers"));
        assert_eq!(job.spots.len(), 5);
        assert_eq!(job.spots[4].moves, ["32-28", "19-23", "28x19:23", "14x23:19"]);
    }

    /// A Slice's jobs: one per Variant and chosen Engine that plays it.
    #[test]
    fn a_slice_job_goes_only_to_engines_that_play_it() {
        let cfg = with_engine();
        let f = |family: &str, variant: &str, key: u64| Frequent { key, family: family.into(), variant: variant.into(), setup: String::new(), moves: vec![], count: 1 };
        let found = [f("chess", "chess", 1), f("draughts", "draughts", 2), f("chess", "chess", 3), f("go", "go-9", 4)];
        let chosen = |k: &str| Chosen { engine: k.into(), min_version: String::new() };
        let jobs = slice_jobs(&cfg, "S", &found, &[chosen("mine"), chosen("checkers")], Per::Depth(9));
        assert_eq!(jobs.iter().map(|j| (j.variant.as_str(), j.engine.as_str(), j.spots.len())).collect::<Vec<_>>(), [("chess", "mine", 2), ("draughts", "checkers", 1)]);
        let o = offer(&cfg, &["draughts".into()]);
        assert!(o.engines.iter().any(|e| e.key == "checkers") && !o.engines.iter().any(|e| e.key == "mine"));
        assert_eq!(o.defaults, ["checkers"]);
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
