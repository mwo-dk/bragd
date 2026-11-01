//! The engines on this machine, as config.toml sets them up: every engine for a Variant, where
//! each one stands (installed, its image pulled or built here, or why not), the one a Variant is
//! played with, and an engine started with the user's options and the tablebases. The app and
//! the Helper both start engines through here.

use std::path::PathBuf;
use std::time::Duration;

use bragd_core::config::Config;
use bragd_core::i18n;
use bragd_core::state::AppState;
use bragd_games::{Family, Variant};
use serde::Serialize;

use crate::known::{self, Known, Missing};
use crate::options::OptionStore;
use crate::seam::{self, Notation};
use crate::session::{Engine, Program, Session};
use crate::{Protocol, images, tools};

/// Where the networks engine images need are kept: `<data>/bragd/networks`.
pub fn networks_folder() -> Result<PathBuf, String> {
    Ok(bragd_core::dirs::app_data().ok_or_else(no_folder)?.join("networks"))
}

/// The error when the system names no folder for Bragd's own files.
pub fn no_folder() -> String {
    i18n::tr("app.no_folder", &[])
}

/// Where the user's engine options are kept.
pub fn store_path() -> Result<PathBuf, String> {
    Ok(bragd_core::dirs::app_config().ok_or_else(no_folder)?.join("engine-options.toml"))
}

/// `SyzygyPath` for engines: the tablebase folder when it holds tables.
pub fn syzygy(cfg: &Config) -> Option<String> {
    let f = cfg.tablebase_folder()?;
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

pub fn home() -> Option<PathBuf> {
    bragd_core::dirs::home_dir()
}

/// The program `k` runs as here: the path set for it, else the one found installed.
pub fn installed(cfg: &Config, key: &str, k: Option<&Known>) -> Option<PathBuf> {
    cfg.engines.paths.get(key).and_then(|p| known::configured(p, &home()?)).or_else(|| k.and_then(Known::installed))
}

pub fn runtime(cfg: &Config) -> Option<tools::Runtime> {
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
        (None, Some(image)) => Availability::Pull { image, size: bragd_core::i18n::bytes(k.image_size) },
    }
}

/// The engine `key` for the Variant `variant`, ready to start, or why it cannot be.
pub fn resolve(cfg: &Config, key: &str, variant: &str) -> Result<Chosen, String> {
    if let Some(k) = known::known(key) {
        let dev = known::dev_images();
        let rt = (k.image().is_some() || k.dev_image(dev).is_some()).then(|| runtime(cfg)).flatten();
        let program = known::choose(k, installed(cfg, key, Some(k)), rt.as_ref(), |rt, i| !images::pulled(&rt.path, &[i.to_string()]).is_empty(), cfg.tablebase_folder().as_deref(), &networks_folder()?, dev).map_err(|m| missing(cfg, k, &m))?;
        return Ok(Chosen { key: key.into(), program, protocols: vec![k.protocol(variant)] });
    }
    let family = cfg.engines.added.get(key).and_then(|f| Family::from_key(f)).ok_or_else(|| i18n::tr("engines.unknown", &[("name", key)]))?;
    let path = installed(cfg, key, None).ok_or_else(|| i18n::tr("engines.gone", &[("name", key)]))?;
    Ok(Chosen { key: key.into(), program: Program::Installed { path, args: vec![] }, protocols: seam::protocols(family).to_vec() })
}

fn missing(cfg: &Config, k: &Known, m: &Missing) -> String {
    match m {
        Missing::NotPulled { .. } => i18n::tr("engines.state.get", &[("size", &bragd_core::i18n::bytes(k.image_size))]),
        Missing::NoRuntime if !cfg.engines.containers => i18n::tr("engines.state.containers_off", &[]),
        Missing::NoRuntime => i18n::tr("engines.state.no_runtime", &[]),
        Missing::NoNetwork => i18n::tr("engines.network.missing", &[("name", k.name)]),
        Missing::NoImage => format!("{} {}", i18n::tr("engines.state.no_image", &[]), install_hint(k)),
    }
}

/// How to install `k` here: its package, and its download page when it has one.
fn install_hint(k: &Known) -> String {
    if k.site.is_empty() { i18n::tr("engines.install_hint_package", &[("program", k.programs[0])]) } else { i18n::tr("engines.install_hint", &[("program", k.programs[0]), ("site", k.site)]) }
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
    let mut e = Engine::start_any(&chosen.program, &chosen.protocols, Duration::from_secs(30), stop).map_err(|e| i18n::tr("engines.no_start", &[("name", &chosen.key), ("why", &e.to_string())]))?;
    used(&chosen.program);
    if !seam::plays(e.about(), e.protocol(), variant) {
        return Err(i18n::tr("engines.not_this_variant", &[("name", &e.about().name)]));
    }
    let store = OptionStore::load(&store_path()?).unwrap_or_default();
    e.configure(&store, syzygy(cfg).as_deref()).map_err(|e| e.to_string())?;
    let notation = seam::notation(variant, e.protocol(), &e.about().name);
    Ok((e, notation))
}

/// An engine just started in `program`: when that is one of Bragd's pinned images, it was used
/// now, so `housekeeping` keeps it for another 90 days. Images built here are never recorded.
pub fn used(program: &Program) {
    if let Program::Container { image, .. } = program
        && image.contains("@sha256:")
    {
        let mut st = AppState::load();
        st.images_used.insert(image.clone(), bragd_core::fs::now());
        let _ = st.save();
    }
}
