//! Bragd's desktop app: the Tauri commands. Svelte draws what these hand it and sends input
//! back; every rule, setting, text and colour is worked out here or in the crates below.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod help;
mod databases;
mod helper_link;
mod knowledge;
mod rules;
mod throw;
mod wizard;

use std::collections::HashMap;
use std::sync::RwLock;

use bragd_core::config::Config;
use bragd_core::settings::{self, Area, Sidebar};
use bragd_core::themes::{self, Appearance};
use bragd_core::{i18n, update};
use bragd_games::{Family, GameError, State};
use serde::Serialize;
use serde_json::Value;

pub(crate) struct Ctx {
    cfg: RwLock<Config>,
}

impl Ctx {
    pub(crate) fn cfg(&self) -> Config {
        self.cfg.read().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// Everything the page needs to draw itself: texts, colours, the sidebar and Settings.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    version: &'static str,
    language: &'static str,
    rtl: bool,
    strings: HashMap<String, Value>,
    appearance: Appearance,
    sidebar: Sidebar,
    families: Vec<FamilyView>,
    /// The areas of Settings, in order, with their names.
    areas: Vec<(Area, String)>,
    settings: Vec<OptView>,
}

/// A Game family with its Variants, named in the current language, and whether each is on.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FamilyView {
    id: &'static str,
    name: String,
    shown: bool,
    variants: Vec<VariantView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VariantView {
    key: &'static str,
    name: String,
    on: bool,
}

/// One option as Settings draws it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OptView {
    name: &'static str,
    area: Area,
    label: String,
    hint: String,
    value: Value,
    /// The values it can take, when it is a choice: a list for a `<select>`.
    choices: Vec<Choice>,
}

#[derive(Serialize)]
struct Choice {
    value: String,
    label: String,
    /// The heading it is listed under, or empty.
    group: String,
}

fn tr(key: &str) -> String {
    i18n::tr(key, &[])
}

fn choice(value: &str, label: String, group: &str) -> Choice {
    Choice {
        value: value.to_string(),
        label,
        group: if group.is_empty() { String::new() } else { tr(group) },
    }
}

fn choices(name: &str, cfg: &Config) -> Vec<Choice> {
    match name {
        "theme" => themes::names(cfg)
            .into_iter()
            .map(|(n, g)| {
                let label = tr(&format!("theme.{n}"));
                let label = if label.starts_with("theme.") { n.clone() } else { label };
                choice(&n, label, g)
            })
            .collect(),
        "look" => std::iter::once(choice("", tr("look.theme"), ""))
            .chain(themes::LOOKS.iter().map(|l| choice(l, tr(&format!("look.{l}")), "")))
            .collect(),
        "board_theme" => themes::BOARD_THEMES
            .iter()
            .map(|b| choice(b, tr(&format!("board_theme.{b}")), ""))
            .collect(),
        "language" => std::iter::once(choice("auto", tr("settings.language_auto"), ""))
            .chain(i18n::LANGUAGES.iter().map(|l| choice(l.code, l.name.to_string(), l.group)))
            .collect(),
        "engines_runtime" => vec![
            choice("auto", tr("settings.runtime_auto"), ""),
            choice("podman", "podman".into(), ""),
            choice("docker", "docker".into(), ""),
        ],
        _ => vec![],
    }
}

fn snapshot(cfg: &Config) -> Snapshot {
    let language = i18n::resolve(&cfg.language);
    i18n::set_language(language);
    let values = settings::values(cfg);
    Snapshot {
        version: update::VERSION,
        language,
        rtl: i18n::is_rtl(language),
        strings: i18n::catalogue(language),
        appearance: themes::appearance(cfg),
        sidebar: settings::sidebar(cfg),
        families: Family::ALL
            .iter()
            .map(|f| FamilyView {
                id: f.key(),
                name: tr(&format!("family.{}", f.key())),
                shown: cfg.family_shown(f.key()),
                variants: f
                    .variants()
                    .map(|v| VariantView {
                        key: v.key,
                        name: tr(&format!("variant.{}", v.key)),
                        on: cfg.variant_on(v.key),
                    })
                    .collect(),
            })
            .collect(),
        areas: Area::ALL.iter().map(|a| (*a, a.label())).collect(),
        settings: settings::options()
            .map(|o| OptView {
                name: o.name,
                area: o.area,
                label: o.label(),
                hint: o.hint(),
                value: values.get(o.name).cloned().unwrap_or(Value::Null),
                choices: choices(o.name, cfg),
            })
            .collect(),
    }
}

#[tauri::command]
fn get_snapshot(ctx: tauri::State<'_, Ctx>) -> Snapshot {
    snapshot(&ctx.cfg())
}

/// Saves one option to config.toml; the page redraws from the new snapshot.
#[tauri::command]
fn save_setting(name: String, value: Value, ctx: tauri::State<'_, Ctx>) -> Result<Snapshot, String> {
    let cfg = settings::save(&name, &value)?;
    let snap = snapshot(&cfg);
    *ctx.cfg.write().unwrap_or_else(|e| e.into_inner()) = cfg;
    Ok(snap)
}

/// The Game after `entries`, from `initial` or the Variant's start.
#[tauri::command]
fn game_replay(variant: String, initial: Option<String>, entries: Vec<String>) -> Result<State, GameError> {
    bragd_games::replay(&variant, initial.as_deref(), &entries)
}

/// The Game after one more entry, or why the rules refuse it.
#[tauri::command]
fn game_play(variant: String, initial: Option<String>, entries: Vec<String>, entry: String) -> Result<State, GameError> {
    bragd_games::apply(&variant, initial.as_deref(), &entries, &entry)
}

#[derive(Serialize)]
struct Thrown {
    entry: String,
    state: State,
}

/// Throws the dice or sticks for the side to act, and plays the throw.
#[tauri::command]
fn game_throw(variant: String, initial: Option<String>, entries: Vec<String>) -> Result<Thrown, GameError> {
    let state = bragd_games::replay(&variant, initial.as_deref(), &entries)?;
    let entry = throw::throw(state.variant.family(), &state.throws).ok_or_else(|| GameError::Refused {
        index: entries.len(),
        entry: String::new(),
        reason: "no throw is awaited".into(),
    })?;
    let state = bragd_games::apply(&variant, initial.as_deref(), &entries, &entry)?;
    Ok(Thrown { entry, state })
}

/// The Rules page of a Family: its docs and its teaching positions.
#[tauri::command]
fn rules_page(family: String) -> Result<rules::Page, String> {
    let family = Family::from_key(&family).ok_or_else(|| format!("no Family {family}"))?;
    Ok(rules::page(family))
}

/// `BRAGD_WINDOW_SIZE=WxH`: the window's size, for screenshots at a given width.
fn window_size() -> Option<(f64, f64)> {
    let v = std::env::var("BRAGD_WINDOW_SIZE").ok()?;
    let (w, h) = v.split_once(['x', 'X'])?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

fn main() {
    if let Some("--version" | "-V") = std::env::args().nth(1).as_deref() {
        return println!("bragd-gui {}", update::VERSION);
    }
    // A mistake in config.toml costs only its area; a file that cannot be read leaves defaults.
    let cfg = Config::load().unwrap_or_else(|e| {
        eprintln!("bragd: {e}; using defaults");
        Config::default()
    });
    i18n::set_language(i18n::resolve(&cfg.language));
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Ctx { cfg: RwLock::new(cfg) })
        .manage(knowledge::Knowledge::default())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            knowledge::start(app.handle());
            let mut window = app.config().app.windows.first().cloned().ok_or("no window in the config")?;
            if let Some((w, h)) = window_size() {
                (window.width, window.height) = (w, h);
                window.min_width = window.min_width.map(|m| m.min(w));
            }
            tauri::WebviewWindowBuilder::from_config(app.handle(), &window)?.build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            save_setting,
            game_replay,
            game_play,
            game_throw,
            rules_page,
            help::feature_list,
            help::notice_list,
            help::notice_dismiss,
            help::update_check,
            help::open_config,
            help::suggest_translation,
            help::open_url,
            wizard::guide_due,
            wizard::wizard_open,
            wizard::wizard_chosen,
            wizard::wizard_done,
            wizard::wizard_seen,
            wizard::tour_seen,
            wizard::whats_new,
            knowledge::helper_status,
            knowledge::helper_retry,
            knowledge::job_cancel,
            knowledge::db_pick,
            knowledge::db_open,
            knowledge::db_current,
            knowledge::db_close,
            knowledge::db_rows,
            knowledge::db_game,
            knowledge::db_import,
            knowledge::db_save_as,
            knowledge::db_save_to,
            knowledge::libraries_list,
            knowledge::library_rows,
            knowledge::library_game,
            knowledge::library_review,
            knowledge::library_dedupe,
            knowledge::library_prune,
            knowledge::priority_get,
            knowledge::priority_save,
            knowledge::slices_list,
            knowledge::slice_save,
            knowledge::slice_materialise,
            knowledge::position_key,
            knowledge::sources_list,
            knowledge::source_set,
            knowledge::source_add,
            knowledge::source_fetch,
            knowledge::source_strip,
            knowledge::disk_report,
            knowledge::disk_limits_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Bragd");
}
