//! The Engines item of each Family: the engines Bragd knows for its Variants and those added
//! by their path, whether each is there to run, the default one per Variant, their options,
//! and an engine's move when it plays a Side in Play.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use bragd_core::config::Config;
use bragd_core::i18n;
use bragd_engines::known::{self, Known, Missing};
use bragd_engines::options::{self, EngineInfo, OptionStore};
use bragd_engines::seam::{self, Notation};
use bragd_engines::session::{Engine, Program, Session, think};
use bragd_engines::{Limit, Protocol, images, network, tools};
use bragd_games::{Family, Variant};
use serde::Serialize;
use serde_json::Value;

/// What the Engines items keep between calls: each engine's options as it last said them,
/// the engine playing a Side, the pulls under way, and the network downloads, by engine.
#[derive(Default)]
pub struct Held {
    info: Mutex<HashMap<String, EngineInfo>>,
    playing: Mutex<Option<(String, Engine)>>,
    pulls: std::sync::Arc<Mutex<HashMap<String, String>>>,
    networks: std::sync::Arc<Mutex<HashMap<String, NetworkJob>>>,
}

/// A network download under way or done.
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkJob {
    done: u64,
    total: u64,
    finished: bool,
    error: Option<String>,
}

/// The evaluation network an engine's image needs, as its card shows it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkView {
    /// Downloaded and checked.
    present: bool,
    /// What one download takes, and where from.
    size: String,
    from: String,
    /// The licence the user agrees to, as the release ships it.
    licence: String,
    job: Option<NetworkJob>,
}

/// Where the networks engine images need are kept: `<data>/bragd/networks`.
pub fn networks_folder() -> PathBuf {
    bragd_core::dirs::data_dir().unwrap_or_default().join("bragd").join("networks")
}

pub fn tr(key: &str, args: &[(&str, &str)]) -> String {
    i18n::tr(key, args)
}

/// Where the user's engine options are kept.
pub fn store_path() -> PathBuf {
    bragd_core::dirs::config_dir().unwrap_or_default().join("bragd").join("engine-options.toml")
}

/// The tablebase folder: the one set, or the one in Bragd's data folder.
pub fn tablebase_folder(cfg: &Config) -> PathBuf {
    let tb = cfg.tablebases.path.trim();
    if tb.is_empty() { bragd_core::dirs::data_dir().unwrap_or_default().join("bragd").join("tablebases") } else { PathBuf::from(tb) }
}

/// `SyzygyPath` for engines: the tablebase folder when it holds tables.
fn syzygy(cfg: &Config) -> Option<String> {
    let f = tablebase_folder(cfg);
    std::fs::read_dir(&f).ok()?.flatten().any(|e| e.path().extension().is_some_and(|x| x == "rtbw")).then(|| f.display().to_string())
}

/// An engine ready to start: its key, program and the protocols to try.
pub struct Chosen {
    pub key: String,
    pub program: Program,
    pub protocols: Vec<Protocol>,
}

/// Every engine for the Variant `key`: the known ones, then those added for its Family.
pub fn engines_for(cfg: &Config, key: &str) -> Vec<String> {
    let family = Variant::from_key(key).map(Variant::family);
    seam::engines(key).iter().map(|k| k.key.to_string()).chain(cfg.engines.added.iter().filter(|(_, f)| Family::from_key(f) == family).map(|(n, _)| n.clone())).collect()
}

/// Where an engine stands on this machine.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Availability {
    Installed { path: String },
    Pulled { image: String },
    /// Runs from its image built on this machine (`tools/dev/images.sh`), as
    /// `BRAGD_DEV_IMAGES=1` allows.
    Built { image: String },
    /// Its image can be pulled: about `size` to download.
    Pull { image: String, size: String },
    /// Not installed, and Bragd's image of it is not published yet.
    NoImage,
    /// Not installed, its image published, but neither podman nor docker is there.
    NoRuntime,
    /// Not installed, and running images is turned off in Settings.
    ContainersOff,
    /// Added by a path that is no longer there.
    Gone { path: String },
}

impl Availability {
    /// There to run now.
    pub fn ready(&self) -> bool {
        matches!(self, Availability::Installed { .. } | Availability::Pulled { .. } | Availability::Built { .. })
    }
}

fn home() -> PathBuf {
    bragd_core::dirs::home_dir().unwrap_or_default()
}

/// The program `k` runs as here: the path set for it, else the one found installed.
fn installed(cfg: &Config, key: &str, k: Option<&Known>) -> Option<PathBuf> {
    cfg.engines.paths.get(key).and_then(|p| known::configured(p, &home())).or_else(|| k.and_then(Known::installed))
}

fn runtime(cfg: &Config) -> Option<tools::Runtime> {
    if cfg.engines.containers { tools::container_runtime(&cfg.engines.runtime) } else { None }
}

/// Where the engine `key` stands. Only an image that is published, or one built here when
/// `BRAGD_DEV_IMAGES=1` allows it, is asked about.
pub fn availability(cfg: &Config, key: &str) -> Availability {
    let k = known::known(key);
    if let Some(p) = installed(cfg, key, k) {
        return Availability::Installed { path: p.display().to_string() };
    }
    let Some(k) = k else { return Availability::Gone { path: cfg.engines.paths.get(key).cloned().unwrap_or_default() } };
    let (local, pinned) = (k.dev_image(known::dev_images()), k.image());
    if local.is_none() && pinned.is_none() {
        return Availability::NoImage;
    }
    if !cfg.engines.containers {
        return Availability::ContainersOff;
    }
    let Some(rt) = runtime(cfg) else { return Availability::NoRuntime };
    let has = |i: &String| !images::pulled(&rt.path, std::slice::from_ref(i)).is_empty();
    match (local.filter(has), pinned) {
        (Some(image), _) => Availability::Built { image },
        (None, None) => Availability::NoImage,
        (None, Some(image)) if has(&image) => Availability::Pulled { image },
        (None, Some(image)) => Availability::Pull { image, size: bragd_helper::disk::bytes(k.image_size) },
    }
}

/// The engine `key` for the Variant `variant`, ready to start, or why it cannot be.
pub fn resolve(cfg: &Config, key: &str, variant: &str) -> Result<Chosen, String> {
    if let Some(k) = known::known(key) {
        let dev = known::dev_images();
        let rt = (k.image().is_some() || k.dev_image(dev).is_some()).then(|| runtime(cfg)).flatten();
        let program = known::choose(k, installed(cfg, key, Some(k)), rt.as_ref(), |rt, i| !images::pulled(&rt.path, &[i.to_string()]).is_empty(), Some(&tablebase_folder(cfg)), &networks_folder(), dev).map_err(|m| missing(cfg, k, &m))?;
        return Ok(Chosen { key: key.into(), program, protocols: vec![k.protocol(variant)] });
    }
    let family = cfg.engines.added.get(key).and_then(|f| Family::from_key(f)).ok_or_else(|| tr("engines.unknown", &[("name", key)]))?;
    let path = installed(cfg, key, None).ok_or_else(|| tr("engines.gone", &[("name", key)]))?;
    Ok(Chosen { key: key.into(), program: Program::Installed { path, args: vec![] }, protocols: seam::protocols(family).to_vec() })
}

fn missing(cfg: &Config, k: &Known, m: &Missing) -> String {
    match m {
        Missing::NotPulled { .. } => tr("engines.state.get", &[("size", &bragd_helper::disk::bytes(k.image_size))]),
        Missing::NoRuntime if !cfg.engines.containers => tr("engines.state.containers_off", &[]),
        Missing::NoRuntime => tr("engines.state.no_runtime", &[]),
        Missing::NoNetwork => tr("engines.network.missing", &[("name", k.name)]),
        Missing::NoImage => format!("{} {}", tr("engines.state.no_image", &[]), install_hint(k)),
    }
}

/// How to install `k` here: its package, and its download page when it has one.
fn install_hint(k: &Known) -> String {
    if k.site.is_empty() { tr("engines.install_hint_package", &[("program", k.programs[0])]) } else { tr("engines.install_hint", &[("program", k.programs[0]), ("site", k.site)]) }
}

/// The engine a Variant is played and analysed with: the one chosen for it, else the first
/// that is there to run, else the first known.
pub fn default_for(cfg: &Config, variant: &str) -> Option<String> {
    let all = engines_for(cfg, variant);
    if let Some(d) = cfg.engines.default.get(variant).filter(|d| all.contains(d)) {
        return Some(d.clone());
    }
    all.iter().find(|k| availability(cfg, k).ready()).or(all.first()).cloned()
}

/// Starts `chosen` for `variant`, with the user's options and the tablebases: the engine,
/// and how it writes moves. Refused when it does not play the Variant.
pub fn start(cfg: &Config, chosen: &Chosen, variant: &str, stop: &dyn Fn() -> bool) -> Result<(Engine, Notation), String> {
    let mut e = Engine::start_any(&chosen.program, &chosen.protocols, Duration::from_secs(30), stop).map_err(|e| tr("engines.no_start", &[("name", &chosen.key), ("why", &e.to_string())]))?;
    if !seam::plays(e.about(), e.protocol(), variant) {
        return Err(tr("engines.not_this_variant", &[("name", &e.about().name)]));
    }
    let store = OptionStore::load(&store_path()).unwrap_or_default();
    e.configure(&store, syzygy(cfg).as_deref()).map_err(|e| e.to_string())?;
    let notation = seam::notation(variant, e.protocol(), &e.about().name);
    Ok((e, notation))
}

/// An engine as the Engines page lists it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineRow {
    key: String,
    name: String,
    added: bool,
    state: Availability,
    /// What to do when it is not there to run.
    hint: String,
    /// Its download page; empty when it has none.
    site: String,
    /// Its pull under way: the runtime's last line, or why it failed.
    pulling: Option<String>,
    variants: Vec<String>,
    /// The network its image needs, when it needs one and is not installed.
    network: Option<NetworkView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantRow {
    key: &'static str,
    name: String,
    engines: Vec<String>,
    /// The engine chosen for it; empty: automatic.
    chosen: String,
    /// The one it gets now.
    current: Option<String>,
    /// Engines play it, but none is ready yet.
    waiting: bool,
}

#[derive(Serialize)]
pub struct Page {
    variants: Vec<VariantRow>,
    engines: Vec<EngineRow>,
}

pub fn page(cfg: &Config, held: &Held, family: Family) -> Page {
    let variants: Vec<VariantRow> = family
        .variants()
        .filter(|v| cfg.variant_on(v.key))
        .map(|v| {
            let chosen = cfg.engines.default.get(v.key).cloned().unwrap_or_default();
            // Only engines there to run are offered, and the one chosen before, whatever it is now.
            let all = engines_for(cfg, v.key);
            let engines: Vec<String> = all.iter().filter(|k| **k == chosen || availability(cfg, k).ready()).cloned().collect();
            VariantRow { key: v.key, name: tr(&format!("variant.{}", v.key), &[]), waiting: engines.is_empty() && !all.is_empty(), engines, chosen, current: default_for(cfg, v.key) }
        })
        .collect();
    let mut keys: Vec<String> = vec![];
    for v in family.variants() {
        for k in engines_for(cfg, v.key) {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
    }
    let pulling = images::pulling();
    let failed = held.pulls.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let downloads = held.networks.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let engines = keys
        .into_iter()
        .map(|key| {
            let k = known::known(&key);
            let state = availability(cfg, &key);
            let hint = match (&state, k) {
                // Running from a checkout with images allowed: the image can be built here.
                (Availability::NoImage, Some(k)) if k.dev_image(known::dev_images()).is_some() => tr("engines.build_here", &[("command", &format!("tools/dev/images.sh {}", k.key))]),
                (Availability::NoImage | Availability::NoRuntime | Availability::ContainersOff | Availability::Pull { .. }, Some(k)) => tr("engines.install_hint_package", &[("program", k.programs[0])]), // its download page is a button
                (Availability::Gone { .. }, _) => tr("engines.gone", &[("name", &key)]),
                _ => String::new(),
            };
            let image = k.and_then(Known::image);
            let variants = family.variants().filter(|v| engines_for(cfg, v.key).contains(&key)).map(|v| tr(&format!("variant.{}", v.key), &[])).collect();
            // Only an image needs the network: an installed engine brings its own, and an image
            // that cannot be had yet needs none.
            let network = network::for_engine(&key).filter(|_| matches!(state, Availability::Pull { .. } | Availability::Pulled { .. } | Availability::Built { .. })).map(|n| NetworkView {
                present: n.path(&networks_folder()).is_file(),
                size: bragd_helper::disk::bytes(n.archive_size),
                from: n.archive.to_string(),
                licence: n.licence.to_string(),
                job: downloads.get(&key).cloned(),
            });
            EngineRow {
                network,
                name: k.map_or_else(|| key.clone(), |k| k.name.to_string()),
                site: k.map(|k| k.site.to_string()).unwrap_or_default(),
                added: k.is_none(),
                pulling: image.as_ref().and_then(|i| pulling.get(i).cloned()).or_else(|| failed.get(&key).cloned()),
                key,
                state,
                hint,
                variants,
            }
        })
        .collect();
    Page { variants, engines }
}

/// Downloads the network the engine `key`'s image needs, on a thread; the page asks how far
/// it is. Pressed under the network's licence, which is the user's consent to it.
pub fn download_network(held: &Held, key: &str) -> Result<(), String> {
    if crate::analyse::offline() {
        return Err(tr("tablebases.offline", &[]));
    }
    let n = network::for_engine(key).ok_or_else(|| tr("engines.unknown", &[("name", key)]))?;
    let jobs = held.networks.clone();
    {
        let mut all = jobs.lock().unwrap_or_else(|e| e.into_inner());
        if all.get(key).is_some_and(|j| !j.finished) {
            return Err(tr("tablebases.busy", &[]));
        }
        all.insert(key.into(), NetworkJob { total: n.archive_size, ..Default::default() });
    }
    let key = key.to_string();
    std::thread::spawn(move || {
        let r = n.download(&networks_folder(), &bragd_engines::fetch::Http, &mut |done, total| {
            if let Some(j) = jobs.lock().unwrap_or_else(|e| e.into_inner()).get_mut(&key) {
                (j.done, j.total) = (done, total);
            }
            true
        });
        if let Some(j) = jobs.lock().unwrap_or_else(|e| e.into_inner()).get_mut(&key) {
            j.finished = true;
            j.error = r.err();
        }
    });
    Ok(())
}

/// Chooses the engine `engine` for `variant` (empty: automatic).
pub fn set_default(variant: &str, engine: &str) -> Result<Config, String> {
    if engine.is_empty() { Config::remove_value(&["engines", "default", variant]) } else { Config::save_value(&["engines", "default", variant], engine.into()) }
}

/// Adds the program at `path` as an engine of `family`, once it has answered in one of the
/// Family's protocols. One that says it is an engine Bragd knows (Stockfish) becomes that
/// engine's program; any other is added under its own name.
pub fn add(family: Family, path: &str) -> Result<Config, String> {
    let p = known::configured(path, &home()).ok_or_else(|| tr("engines.no_file", &[("path", path)]))?;
    let program = Program::Installed { path: p.clone(), args: vec![] };
    let e = Engine::start_any(&program, seam::protocols(family), Duration::from_secs(10), &|| false).map_err(|e| tr("engines.no_start", &[("name", path), ("why", &e.to_string())]))?;
    let name = options::engine_key(&e.about().name);
    drop(e);
    let key: String = name.to_lowercase().split_whitespace().collect::<Vec<_>>().join("-");
    let key = if key.is_empty() { "engine".into() } else { key };
    let path = p.display().to_string();
    if known::known(&key).is_none() {
        Config::save_value(&["engines", "added", &key], family.key().into())?;
    }
    Config::save_value(&["engines", "paths", &key], path.as_str().into())
}

/// Takes an added engine out, or the path set for a known one.
pub fn remove(cfg: &Config, key: &str) -> Result<Config, String> {
    for (variant, _) in cfg.engines.default.iter().filter(|(_, e)| *e == key) {
        Config::remove_value(&["engines", "default", variant])?;
    }
    Config::remove_value(&["engines", "added", key])?;
    Config::remove_value(&["engines", "paths", key])
}

/// Starts the engine `key` for `variant` and lists its options with the values set.
pub fn options_of(cfg: &Config, held: &Held, key: &str, variant: &str) -> Result<EngineInfo, String> {
    let chosen = resolve(cfg, key, variant)?;
    let (e, _) = start(cfg, &chosen, variant, &|| false)?;
    let store = OptionStore::load(&store_path()).unwrap_or_default();
    let info = EngineInfo::new(e.about(), chosen.program.container(), &store);
    held.info.lock().unwrap_or_else(|e| e.into_inner()).insert(key.to_string(), info.clone());
    Ok(info)
}

/// Sets (or with None clears) one option of the engine `key`, listed before by `options_of`.
pub fn set_option(held: &Held, key: &str, name: &str, value: Option<&Value>) -> Result<EngineInfo, String> {
    let mut all = held.info.lock().unwrap_or_else(|e| e.into_inner());
    let info = all.get_mut(key).ok_or_else(|| tr("engines.unknown", &[("name", key)]))?;
    let store = options::set_option(&store_path(), info, name, value)?;
    info.refresh(&store);
    Ok(info.clone())
}

pub fn reset_options(held: &Held, key: &str) -> Result<EngineInfo, String> {
    let mut all = held.info.lock().unwrap_or_else(|e| e.into_inner());
    let info = all.get_mut(key).ok_or_else(|| tr("engines.unknown", &[("name", key)]))?;
    let store = options::reset_options(&store_path(), &info.key)?;
    info.refresh(&store);
    Ok(info.clone())
}

/// Pulls the image of the engine `key` on a thread of its own; its progress shows on the page.
pub fn pull(cfg: &Config, held: &Held, key: &str) -> Result<(), String> {
    let k = known::known(key).ok_or_else(|| tr("engines.unknown", &[("name", key)]))?;
    let image = k.image().ok_or_else(|| tr("engines.state.no_image", &[]))?;
    let rt = runtime(cfg).ok_or_else(|| tr("engines.state.no_runtime", &[]))?;
    let (key, pulls) = (key.to_string(), held.pulls.clone());
    std::thread::spawn(move || {
        let stop = std::sync::atomic::AtomicBool::new(false);
        let r = images::pull(&rt.path, &image, &stop, &|d| {
            std::thread::sleep(d);
            true
        });
        let mut pulls = pulls.lock().unwrap_or_else(|e| e.into_inner());
        match r {
            Ok(()) => pulls.remove(&key),
            Err(e) => pulls.insert(key, e.to_string()),
        };
    });
    Ok(())
}

/// An engine's move in Play.
#[derive(Serialize)]
pub struct Moved {
    pub entry: String,
    pub notation: String,
}

/// The move of the engine `key` (or the Variant's default) in the Game `entries`, thinking
/// `ms` milliseconds, at `skill` (0 to 20, where the engine has a skill level). The engine
/// stays running for the next move.
#[allow(clippy::too_many_arguments, reason = "the Game, the engine and how it plays, as the command takes them")]
pub fn engine_move(cfg: &Config, held: &Held, variant: &str, initial: Option<&str>, entries: &[String], key: Option<&str>, ms: u32, skill: Option<u8>) -> Result<Moved, String> {
    let key = key.filter(|k| !k.is_empty()).map(String::from).or_else(|| default_for(cfg, variant)).ok_or_else(|| tr("engines.none_for", &[]))?;
    let mut playing = held.playing.lock().unwrap_or_else(|e| e.into_inner());
    let id = format!("{key} {variant}");
    if playing.as_ref().is_none_or(|(k, _)| *k != id) {
        *playing = None;
        let chosen = resolve(cfg, &key, variant)?;
        let (e, _) = start(cfg, &chosen, variant, &|| false)?;
        *playing = Some((id, e));
    }
    let (_, e) = playing.as_mut().expect("started above");
    let notation = seam::notation(variant, e.protocol(), &e.about().name);
    if let (Some(s), Some(o)) = (skill, e.about().option("Skill Level").cloned()) {
        let _ = e.set_option(&o.name, &s.to_string());
    }
    let (asked, state) = seam::position(variant, initial, entries, notation)?;
    let a = match think(e, &asked, Limit::Time(ms.clamp(50, 60_000)), 1, &|| false, &mut |_| {}) {
        Ok(a) => a,
        Err(err) => {
            *playing = None;
            return Err(err.to_string());
        }
    };
    let best = a.best.ok_or_else(|| tr("engines.no_move", &[]))?;
    let m = seam::entry(&state, notation, &best).ok_or_else(|| tr("engines.illegal", &[("move", &best)]))?;
    Ok(Moved { entry: m.wire.clone(), notation: m.notation.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engines_for_a_variant_and_where_they_stand() {
        let mut cfg = Config::default();
        assert_eq!(engines_for(&cfg, "xiangqi"), ["fairy-stockfish", "pikafish"]);
        assert!(engines_for(&cfg, "checkers").is_empty());
        cfg.engines.added.insert("my-engine".into(), "chess".into());
        cfg.engines.paths.insert("my-engine".into(), "/nonexistent/my-engine".into());
        assert_eq!(engines_for(&cfg, "seirawan").last().map(String::as_str), Some("my-engine"));
        assert!(!engines_for(&cfg, "shogi").contains(&"my-engine".to_string()));
        assert!(matches!(availability(&cfg, "my-engine"), Availability::Gone { .. }));
        // Unpublished images are never offered, and no runtime is asked about them.
        cfg.engines.paths.insert("pikafish".into(), "/nonexistent/pikafish".into());
        cfg.engines.containers = true;
        if known::known("pikafish").unwrap().installed().is_none() {
            assert_eq!(availability(&cfg, "pikafish"), Availability::NoImage);
            assert!(resolve(&cfg, "pikafish", "xiangqi").is_err());
        }
        cfg.engines.default.insert("xiangqi".into(), "pikafish".into());
        assert_eq!(default_for(&cfg, "xiangqi").as_deref(), Some("pikafish"));
        cfg.engines.default.insert("xiangqi".into(), "stockfish".into());
        assert_ne!(default_for(&cfg, "xiangqi").as_deref(), Some("stockfish"), "Stockfish does not play xiangqi");
    }

    /// A fake UCI engine on a path plays a legal move, given back as the seam's entry.
    #[cfg(unix)]
    #[test]
    fn an_engine_plays_a_side() {
        use std::os::unix::fs::PermissionsExt;
        let d = std::env::temp_dir().join(format!("bragd-gui-engine-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("fake");
        std::fs::write(&p, "#!/bin/sh\nwhile read l; do case \"$l\" in uci) echo 'id name Fake 1'; echo 'option name Skill Level type spin default 20 min 0 max 20'; echo uciok;; go*) echo 'info depth 3 score cp 20 pv e7e5'; echo 'bestmove e7e5';; quit) exit 0;; esac; done\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut cfg = Config::default();
        cfg.engines.added.insert("fake".into(), "chess".into());
        cfg.engines.paths.insert("fake".into(), p.display().to_string());
        let held = Held::default();
        let m = engine_move(&cfg, &held, "chess", None, &["e2e4".into()], Some("fake"), 100, Some(3)).unwrap();
        assert_eq!((m.entry.as_str(), m.notation.as_str()), ("e7e5", "e5"));
        // The same engine refuses a Variant it has no UCI_Variant for.
        assert!(engine_move(&cfg, &held, "seirawan", None, &[], Some("fake"), 100, None).is_err());
        // Its move must be legal: after 1. e4 e5 it is White's turn, and e7e5 is not a move.
        assert!(engine_move(&cfg, &held, "chess", None, &["e2e4".into(), "e7e5".into()], Some("fake"), 100, None).is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }
}
