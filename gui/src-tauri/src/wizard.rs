//! The install wizard and What's new (`bragd_core::wizard`, `bragd_core::tour`): what each step
//! shows, found here (engines, Sources, free space) and decided in bragd-core, and what Done
//! starts: the chosen Sources turned on in the Helper, and the Syzygy 3–5 download.


use bragd_core::config::Config;
use bragd_core::state::AppState;
use bragd_core::{t, tips, tour, wizard};
use bragd_engines::known::{self, KNOWN};
use bragd_games::Family;
use bragd_helper::helper::{self as line, Reply, Request};
use bragd_helper::{disk, sources};
use serde::Serialize;

use crate::Ctx;

/// What the app shows at start: the wizard on a first start, else the tour cards newer than
/// the version last seen (none most starts).
#[derive(Serialize)]
pub struct Due {
    wizard: bool,
    cards: Vec<tour::View>,
}

#[tauri::command]
pub fn guide_due(ctx: tauri::State<'_, Ctx>) -> Due {
    let st = AppState::load();
    Due { wizard: st.guide_due(), cards: tour::cards_since(&st.seen_version, &ctx.cfg()).iter().map(tour::view).collect() }
}

/// Changes the state as `f` says and saves it, after taking what another window wrote.
fn with_state(f: impl FnOnce(&mut AppState)) {
    let mut st = AppState::load();
    f(&mut st);
    if let Err(e) = st.save() {
        eprintln!("bragd: state.json: {e}");
    }
}

/// The wizard was closed in any way: not shown by itself again.
#[tauri::command]
pub fn wizard_seen() {
    with_state(wizard::seen);
}

/// The tour cards were closed: shown again only for a newer version.
#[tauri::command]
pub fn tour_seen() {
    with_state(tour::seen);
}

/// What's new in the sidebar: every version, newest first, with its cards.
#[tauri::command]
pub fn whats_new(ctx: tauri::State<'_, Ctx>) -> Vec<(&'static str, Vec<tour::View>)> {
    tour::by_version(&ctx.cfg())
}

/// The Tip due now, if any, counted as shown. `busy`: a live Game or the wizard is on screen.
#[tauri::command]
pub fn tip_next(busy: bool, ctx: tauri::State<'_, Ctx>) -> Option<tips::Tip> {
    let cfg = ctx.cfg();
    let mut st = AppState::load();
    let f = tips::next(&cfg, &st, bragd_core::fs::now(), busy)?;
    tips::shown(&mut st, f, bragd_core::fs::now());
    let _ = st.save();
    Some(tips::tip(f))
}

/// *Don't suggest this again* for the Tip `id`.
#[tauri::command]
pub fn tip_silence(id: String) {
    with_state(|st| tips::silence(st, &id));
}

/// An engine as the Engines step lists it.
#[derive(Serialize)]
struct EngineView {
    name: &'static str,
    /// Where it is installed, or none.
    installed: Option<String>,
    /// Its sealed image is published, so it can be pulled.
    image: bool,
    /// Its download page; empty: install it from the system's packages, as `program`.
    site: &'static str,
    program: &'static str,
}

#[derive(Serialize)]
struct FamilyEngines {
    name: String,
    engines: Vec<EngineView>,
}

/// Everything the wizard draws that is not in the snapshot.
#[derive(Serialize)]
pub struct Open {
    steps: Vec<wizard::StepView>,
    themes: Vec<wizard::Swatch>,
    boards: Vec<wizard::Swatch>,
    engines: Vec<FamilyEngines>,
    /// The sites the Online step names: those that play a Family that is on.
    servers: Vec<&'static str>,
    /// The servers Bragd plays on for the Families that are on: the step offers their sign-in.
    online: Vec<crate::online::ServerView>,
    offers: Vec<wizard::Offer>,
    /// "Coming later: …": the Sources that cannot be fetched yet, in one line.
    coming: Option<String>,
    ticked: Vec<String>,
}

/// The Helper's Sources, as the wizard weighs them.
fn helper_sources() -> Vec<wizard::Source> {
    let tags = bragd_core::i18n::locale_tags();
    sources::builtin()
        .iter()
        .map(|s| {
            let a = s.about();
            let list = bragd_clubs::lists::LISTS.iter().find(|l| l.id == a.id);
            wizard::Source {
                list: list.is_some(),
                more: list.is_some_and(|l| !l.suggested(&tags)),
                schedule: serde_json::to_value(a.schedule).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default(),
                name: list.map_or(a.name, |l| l.title()),
                id: a.id,
                family: a.family,
                size: a.size,
                fetchable: a.fetchable,
            }
        })
        .collect()
}

/// The engines of each Family that is on, each looked for here the way Play looks: the path set
/// in Settings first, then the PATH. A Family without outside engines has none: Bragd's own Bot
/// plays it.
fn engines(cfg: &Config) -> Vec<FamilyEngines> {
    Family::ALL
        .iter()
        .filter(|f| cfg.family_shown(f.key()))
        .map(|f| FamilyEngines {
            name: t!(&format!("family.{}", f.key())),
            engines: KNOWN
                .iter()
                // The engines' own Variant keys differ from the catalogue's (S5): the Family decides.
                .filter(|_| f.has_engine())
                .filter(|k| k.variants.iter().any(|v| known::family(v).and_then(|kf| serde_json::to_value(kf).ok()).is_some_and(|kf| kf == f.key())))
                .map(|k| EngineView { name: k.name, installed: crate::engines::installed(cfg, k.key, Some(k)).map(|p| p.display().to_string()), image: k.image().is_some(), site: k.site, program: k.programs[0] })
                .collect(),
        })
        .collect()
}

/// Where each Family can be played online in the browser only. Lichess, Lishogi and OGS are
/// played in Bragd; pychess.org has no API a client may play through (docs/online/pychess.md).
const SERVERS: &[(&str, &[&str])] = &[("https://www.pychess.org/", &["chess", "xiangqi", "shogi"])];

fn servers(cfg: &Config) -> Vec<&'static str> {
    SERVERS.iter().filter(|(_, fams)| fams.iter().any(|f| cfg.family_shown(f))).map(|(url, _)| *url).collect()
}

#[tauri::command(async)]
pub fn wizard_open(ctx: tauri::State<'_, Ctx>) -> Open {
    let cfg = ctx.cfg();
    let srcs = helper_sources();
    let offers = wizard::offers(&srcs, &cfg, bragd_core::i18n::bytes);
    Open {
        steps: wizard::steps(&cfg),
        themes: wizard::theme_swatches(),
        boards: wizard::board_swatches(&cfg),
        engines: engines(&cfg),
        servers: servers(&cfg),
        online: crate::online::offered(&cfg),
        ticked: wizard::ticked(&offers),
        coming: wizard::coming(&srcs, &cfg),
        offers,
    }
}

#[derive(Serialize)]
pub struct Chosen {
    totals: wizard::Totals,
    /// What Done does, a line each.
    summary: Vec<String>,
}

/// The size of what is ticked against the free space, and what Done would do.
#[tauri::command]
pub fn wizard_chosen(chosen: Vec<String>, ctx: tauri::State<'_, Ctx>) -> Chosen {
    let cfg = ctx.cfg();
    let srcs = helper_sources();
    let free = bragd_core::dirs::app_data().and_then(|d| disk::free_space(&d));
    Chosen {
        totals: wizard::totals(&srcs, &chosen, free, bragd_core::i18n::bytes),
        summary: wizard::summary(&wizard::plan(&srcs, &chosen, &cfg), &srcs, bragd_core::i18n::bytes),
    }
}

/// Done: turns the chosen Sources on in the Helper, keeping any options already set for them,
/// and starts the Syzygy download. What could not be started, a line each.
#[tauri::command]
pub async fn wizard_done(chosen: Vec<String>, ctx: tauri::State<'_, Ctx>) -> Result<Vec<String>, String> {
    let cfg = ctx.cfg();
    with_state(|st| {
        wizard::seen(st);
        tips::count(st, "wizard");
    });
    tauri::async_runtime::spawn_blocking(move || done(&cfg, &chosen)).await.map_err(|e| e.to_string())
}

fn done(cfg: &Config, chosen: &[String]) -> Vec<String> {
    let srcs = helper_sources();
    let plan = wizard::plan(&srcs, chosen, cfg);
    let mut failed = vec![];
    let offline = bragd_core::offline();
    if !plan.sources.is_empty()
        && let Err(e) = turn_on(&plan.sources, offline)
    {
        failed.push(t!("wizard.failed", "what" => names(&srcs, &plan.sources), "error" => e));
    }
    if plan.syzygy {
        if offline {
            failed.push(t!("wizard.failed", "what" => t!("wizard.syzygy"), "error" => t!("common.offline")));
        } else {
            let folder = cfg.tablebase_folder();
            // TODO(tablebases): progress and errors belong on the Tablebases page; until it
            // shows them, the download runs here and reports only to stderr.
            if let Some(folder) = folder {
                std::thread::spawn(move || {
                    let http = bragd_engines::fetch::Http;
                    let r = bragd_engines::syzygy::plan_from(&http, bragd_engines::syzygy::Set::UpTo5)
                        .and_then(|p| bragd_engines::syzygy::download(&p, &folder, &http, &mut |_, _| true));
                    if let Err(e) = r {
                        eprintln!("bragd: Syzygy 3–5: {e}");
                    }
                });
            }
        }
    }
    failed
}

/// The Sources `ids` by their names: "The Week in Chess, Lichess broadcasts".
fn names(srcs: &[wizard::Source], ids: &[String]) -> String {
    ids.iter().map(|id| srcs.iter().find(|s| &s.id == id).map_or(id.as_str(), |s| s.name.as_str())).collect::<Vec<_>>().join(", ")
}

/// Each Source turned on through the Helper, with the options it has kept so far.
fn turn_on(ids: &[String], offline: bool) -> Result<(), String> {
    if offline {
        return Err(t!("common.offline"));
    }
    let mut c = line::program().and_then(|p| line::connect_or_start(&p)).map_err(|e| e.to_string())?;
    let Ok(Reply::Sources { sources }) = c.ask(&Request::Sources) else {
        return Err("the Helper did not list its Sources".into());
    };
    for id in ids {
        let Some((_, mut options, _)) = sources.iter().find(|(a, ..)| &a.id == id).cloned() else { continue };
        options.enabled = true;
        match c.ask(&Request::SetSource { source: id.clone(), options }) {
            Ok(Reply::Done) => {}
            Ok(Reply::Error { error }) => return Err(error),
            Ok(other) => return Err(format!("{other:?}")),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An engine whose path is set in Settings is installed, even when the PATH lacks it.
    #[test]
    fn the_wizard_sees_an_engine_set_by_path() {
        let me = std::env::current_exe().unwrap();
        let mut cfg = Config::default();
        cfg.engines.paths.insert("stockfish".into(), me.display().to_string());
        let seen = engines(&cfg).into_iter().flat_map(|f| f.engines).find(|e| e.program == "stockfish").unwrap();
        assert_eq!(seen.installed.as_deref(), Some(me.display().to_string().as_str()));
    }

    /// Someone who plays one game reads only of the sites and engines for it.
    #[test]
    fn the_wizard_names_only_what_plays_the_families_that_are_on() {
        let only = |keep: &str| {
            let mut cfg = Config::default();
            for f in Family::ALL.iter().filter(|f| f.key() != keep) {
                cfg.families.family.insert(f.key().into(), false);
            }
            cfg
        };
        assert!(servers(&only("go")).is_empty(), "OGS is played in Bragd");
        assert_eq!(servers(&only("chess")), ["https://www.pychess.org/"], "Lichess is played in Bragd");
        assert_eq!(servers(&only("xiangqi")), ["https://www.pychess.org/"]);
        let online = |f| crate::online::offered(&only(f)).iter().map(|s| s.id).collect::<Vec<_>>();
        assert_eq!((online("go"), online("shogi"), online("chess")), (vec!["ogs"], vec!["lishogi"], vec!["lichess"]));
        assert!(online("tafl").is_empty());
        assert!(servers(&only("tafl")).is_empty());
        let tafl = engines(&only("tafl"));
        assert_eq!(tafl.len(), 1, "tafl is listed, played by Bragd's own Bot");
        assert!(tafl[0].engines.is_empty());
        assert_eq!(servers(&Config::default()).len(), SERVERS.len());
    }

    /// A Source that could not start is named as users know it, not by its id.
    #[test]
    fn what_failed_is_named_as_users_know_it() {
        assert_eq!(names(&helper_sources(), &["twic".into(), "custom:x".into()]), "The Week in Chess, custom:x");
    }
}
