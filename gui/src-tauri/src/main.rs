//! Bragd's desktop app: the Tauri commands. Svelte draws what these hand it and sends input
//! back; every rule, setting, text and colour is worked out here or in the crates below.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod background;
mod clubs;
mod leagues;
mod tournaments;
mod help;
mod databases;
mod helper_link;
mod knowledge;
mod matches;
mod model;
mod peers;
mod analyse;
mod engine_commands;
mod engines;
mod files;
mod rules;
mod tablebases;
mod wizard;

use std::collections::HashMap;
use std::sync::RwLock;

use bragd_core::config::Config;
use bragd_core::settings::{self, Area, Sidebar};
use bragd_core::themes::{self, Appearance};
use bragd_core::{i18n, update};
use bragd_games::{Family, GameError, State, Variant};
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
    /// Raced along a track by throws of the dice: backgammon, Ur and senet.
    race: bool,
    /// The file formats Bragd reads for it: "PGN", "SGF"…
    formats: &'static [&'static str],
    /// Those a custom URL Source can fetch: "PGN" for chess, not SCID.
    url_formats: Vec<&'static str>,
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
    /// Listed in its area; otherwise a page of its own draws it (Families, Disk use).
    listed: bool,
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

/// A mutex's guard, even after a thread panicked holding it: the data is still whole.
pub fn lock<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn choice(value: &str, label: String, group: &str) -> Choice {
    Choice {
        value: value.to_string(),
        label,
        group: if group.is_empty() { String::new() } else { i18n::tr(group, &[]) },
    }
}

fn choices(name: &str, cfg: &Config) -> Vec<Choice> {
    match name {
        "theme" => themes::names(cfg)
            .into_iter()
            .map(|(n, g)| {
                let label = i18n::tr(&format!("theme.{n}"), &[]);
                let label = if label.starts_with("theme.") { n.clone() } else { label };
                choice(&n, label, g)
            })
            .collect(),
        "look" => std::iter::once(choice("", i18n::tr("look.theme", &[]), ""))
            .chain(themes::LOOKS.iter().map(|l| choice(l, i18n::tr(&format!("look.{l}"), &[]), "")))
            .collect(),
        "board_theme" => themes::BOARD_THEMES
            .iter()
            .map(|b| choice(b, i18n::tr(&format!("board_theme.{b}"), &[]), ""))
            .collect(),
        "language" => std::iter::once(choice("auto", i18n::tr("settings.language_auto", &[]), ""))
            .chain(i18n::LANGUAGES.iter().map(|l| choice(l.code, l.name.to_string(), l.group)))
            .collect(),
        "file_picker" => picker_choices(files::coxswain().is_some()),
        "tips" => bragd_core::tips::FREQUENCIES.iter().map(|f| choice(f, i18n::tr(&format!("tips.{f}"), &[]), "")).collect(),
        "engines_runtime" => vec![
            choice("auto", i18n::tr("settings.runtime_auto", &[]), ""),
            choice("podman", "podman".into(), ""),
            choice("docker", "docker".into(), ""),
        ],
        _ => vec![],
    }
}

/// The file pickers to choose from. Coxswain's is named only where it is installed; elsewhere
/// the system's dialog is the one choice, and nothing says what is missing.
fn picker_choices(coxswain: bool) -> Vec<Choice> {
    let system = || i18n::tr("settings.file_picker_system", &[]);
    if !coxswain {
        return vec![choice("auto", system(), "")];
    }
    vec![
        choice("auto", i18n::tr("settings.file_picker_auto_found", &[]), ""),
        choice("coxswain", "Coxswain".into(), ""),
        choice("system", system(), ""),
    ]
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
                name: i18n::tr(&format!("family.{}", f.key()), &[]),
                shown: cfg.family_shown(f.key()),
                race: f.race(),
                formats: bragd_library::formats(f.key()),
                url_formats: bragd_library::url_formats(f.key()),
                variants: f
                    .variants()
                    .map(|v| VariantView {
                        key: v.key,
                        name: i18n::tr(&format!("variant.{}", v.key), &[]),
                        on: cfg.variant_on(v.key),
                    })
                    .collect(),
            })
            .collect(),
        areas: settings::areas(cfg).into_iter().map(|a| (a, a.label())).collect(),
        settings: settings::options()
            .map(|o| OptView {
                name: o.name,
                area: o.area,
                listed: o.listed_for(cfg),
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
    let cfg = save_with(&name, &value, register_helper)?;
    if let Some(f) = bragd_core::tips::of_setting(&name) {
        bragd_core::tips::used(f);
    }
    let snap = snapshot(&cfg);
    *ctx.cfg.write().unwrap_or_else(|e| e.into_inner()) = cfg;
    Ok(snap)
}

/// Saves one option. *Start with my session* registers the Helper with the session, or takes
/// the registration away, first (`register`): a registration that failed saves nothing.
fn save_with(name: &str, value: &Value, register: impl FnOnce(bool) -> std::io::Result<()>) -> Result<Config, String> {
    if name == "start_with_session" {
        let on = value.as_bool().ok_or_else(|| format!("{name}: not true or false"))?;
        register(on).map_err(|e| e.to_string())?;
    }
    settings::save(name, value)
}

/// Registers the `bragd-helper` beside this program with the session, or unregisters it.
fn register_helper(on: bool) -> std::io::Result<()> {
    if on { bragd_helper::service::install(&bragd_helper::helper::program()?) } else { bragd_helper::service::uninstall() }
}

/// Turns the Family `family` back on, with its Variants when every one was off.
#[tauri::command]
fn family_turn_on(family: String, ctx: tauri::State<'_, Ctx>) -> Result<Snapshot, String> {
    for (name, value) in settings::turn_on(&ctx.cfg(), &family) {
        // Each switch saved is kept at once, so the window never disagrees with config.toml.
        *ctx.cfg.write().unwrap_or_else(|e| e.into_inner()) = settings::save(&name, &value)?;
    }
    Ok(snapshot(&ctx.cfg()))
}

/// Only the Family `family` shows: the wizard's *Only chess*. Each switch is kept at once.
#[tauri::command]
fn families_only(family: String, ctx: tauri::State<'_, Ctx>) -> Result<Snapshot, String> {
    for (name, value) in settings::only(&ctx.cfg(), &family) {
        *ctx.cfg.write().unwrap_or_else(|e| e.into_inner()) = settings::save(&name, &value)?;
    }
    bragd_core::tips::used("families");
    Ok(snapshot(&ctx.cfg()))
}

/// The Game after `entries`, from `initial` or the Variant's start.
#[tauri::command]
fn game_replay(variant: String, initial: Option<String>, entries: Vec<String>) -> Result<State, GameError> {
    bragd_games::replay(&variant, initial.as_deref(), &entries)
}

#[derive(Serialize)]
struct Started {
    /// The position the Game starts from: a Chess960 array, else none (the Variant's start).
    initial: Option<String>,
    /// The Chess960 array's number, 0–959.
    array: Option<u16>,
    state: State,
}

/// A new Game of `variant`: Chess960 starts from an array picked at random.
#[tauri::command]
fn game_new(variant: String) -> Result<Started, GameError> {
    bragd_core::tips::used("play");
    let array = (bragd_games::variant(&variant)? == Variant::Chess960).then(bragd_core::throw::chess960_array);
    let initial = array.map(bragd_games::chess960_start);
    let state = bragd_games::replay::<&str>(&variant, initial.as_deref(), &[])?;
    Ok(Started { initial, array, state })
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
    let entry = bragd_core::throw::throw(state.variant.family(), &state.throws).ok_or_else(|| GameError::Refused {
        index: entries.len(),
        entry: String::new(),
        reason: "no throw is awaited".into(),
    })?;
    let state = bragd_games::apply(&variant, initial.as_deref(), &entries, &entry)?;
    Ok(Thrown { entry, state })
}

/// The Rules page of a Family: its docs and its teaching positions.
#[tauri::command(async)]
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
        .manage(clubs::Held::default())
        .plugin(tauri_plugin_dialog::init())
        .manage(engines::Held::default())
        .manage(analyse::Held::default())
        .manage(tablebases::Held::default())
        .setup(|app| {
            knowledge::start(app.handle());
            let mut window = app.config().app.windows.first().cloned().ok_or("no window in the config")?;
            if let Some((w, h)) = window_size() {
                (window.width, window.height) = (w, h);
                window.min_width = window.min_width.map(|m| m.min(w));
            }
            let main = tauri::WebviewWindowBuilder::from_config(app.handle(), &window)?.build()?;
            // Ask for focus once, so a compositor that opens new windows out of sight (niri, a
            // scrolling one) brings this one into view. On Wayland GTK presents the window with
            // the XDG_ACTIVATION_TOKEN (or DESKTOP_STARTUP_ID) the launcher gave Bragd.
            let _ = main.set_focus();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            save_setting,
            game_new,
            family_turn_on,
            families_only,
            game_replay,
            game_play,
            game_throw,
            rules_page,
            model::model_status,
            model::model_find,
            model::model_models,
            model::model_key,
            model::comment_view,
            model::comment_translate,
            peers::peers_view,
            peers::peers_apply,
            peers::pair_show,
            peers::pair_hide,
            peers::pair_with,
            peers::peer_unpair,
            peers::peers_look,
            help::feature_list,
            help::notice_list,
            help::notice_dismiss,
            help::update_check,
            help::open_config,
            help::suggest_translation,
            help::open_url,
            files::pick_path,
            clubs::clubs_page,
            clubs::club_open,
            clubs::club_pick,
            clubs::club_create,
            clubs::club_close,
            clubs::club_forget,
            clubs::club_add,
            clubs::club_update,
            clubs::club_remove,
            clubs::club_rating,
            clubs::club_system,
            clubs::club_take_over,
            clubs::rating_search,
            tournaments::tournament_new,
            tournaments::tournament_variants,
            tournaments::tournament_view,
            leagues::league_new,
            leagues::league_view,
            leagues::league_remove,
            leagues::league_add_team,
            leagues::league_pair,
            leagues::league_line_up,
            leagues::league_result,
            tournaments::tournament_remove,
            tournaments::tournament_register,
            tournaments::tournament_register_list,
            tournaments::tournament_defaults,
            tournaments::tournament_status,
            tournaments::tournament_check_in_all,
            tournaments::tournament_pair,
            tournaments::tournament_pair_again,
            tournaments::tournament_check,
            tournaments::tournament_set_pairing,
            tournaments::tournament_result,
            tournaments::tournament_playoff,
            tournaments::tournament_team,
            tournaments::tournament_tiebreaks,
            tournaments::tournament_rate,
            tournaments::tournament_export,
            tournaments::tournament_import,
            tournaments::tournament_show,
            wizard::guide_due,
            wizard::wizard_open,
            wizard::wizard_chosen,
            wizard::wizard_done,
            wizard::wizard_seen,
            wizard::tour_seen,
            wizard::whats_new,
            wizard::tip_next,
            wizard::tip_silence,
            knowledge::helper_status,
            knowledge::helper_retry,
            knowledge::job_cancel,
            matches::matches_page,
            matches::match_start,
            matches::match_book_pick,
            knowledge::db_pick,
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
            knowledge::library_player,
            knowledge::db_player,
            background::slice_analyse,
            background::store_report,
            background::store_prune,
            knowledge::library_players,
            knowledge::library_dedupe,
            knowledge::library_export,
            knowledge::positions_export,
            knowledge::positions_import,
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
            knowledge::source_remove,
            knowledge::source_fetch,
            knowledge::source_allow,
            knowledge::source_strip,
            knowledge::disk_report,
            knowledge::disk_limits_set,
            engine_commands::engines_page,
            engine_commands::engine_network_download,
            engine_commands::engine_default,
            engine_commands::engine_add,
            engine_commands::engine_remove,
            engine_commands::engine_options,
            engine_commands::engine_set_option,
            engine_commands::engine_reset_options,
            engine_commands::engine_pull,
            engine_commands::engine_choices,
            engine_commands::engine_move,
            engine_commands::analysis_start,
            engine_commands::analysis_now,
            engine_commands::analysis_stop,
            engine_commands::review_start,
            engine_commands::review_now,
            engine_commands::review_stop,
            engine_commands::position_known,
            engine_commands::tablebase_probe,
            engine_commands::position_setup,
            engine_commands::tablebases_page,
            engine_commands::tablebase_plan,
            engine_commands::tablebase_download,
            engine_commands::tablebase_stop,
            engine_commands::books_page,
            engine_commands::book_add,
            engine_commands::book_remove,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Bragd");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// *Start with my session* registers the Helper with the session, or takes it away, before
    /// the setting is saved: a registration that failed saves nothing. No other setting does.
    #[test]
    fn start_with_session_is_registered_before_it_is_saved() {
        let mut asked = vec![];
        let failed = save_with("start_with_session", &Value::Bool(true), |on| {
            asked.push(on);
            Err(std::io::Error::other("no session service"))
        });
        assert!(failed.is_err_and(|e| e.contains("no session service")));
        assert!(!Config::load().unwrap_or_default().helper.start_with_session, "a failed registration saves nothing");
        let cfg = save_with("start_with_session", &Value::Bool(true), |on| {
            asked.push(on);
            Ok(())
        })
        .unwrap();
        assert!(cfg.helper.start_with_session);
        let cfg = save_with("start_with_session", &Value::Bool(false), |on| {
            asked.push(on);
            Ok(())
        })
        .unwrap();
        assert!(!cfg.helper.start_with_session);
        save_with("check_updates", &Value::Bool(true), |_| panic!("only the session setting registers")).unwrap();
        assert_eq!(asked, [true, true, false]);
    }

    /// Without Coxswain installed, the file picker offers the system's dialog alone and names
    /// no other program; with it, Coxswain's picker is a choice.
    #[test]
    fn coxswain_is_named_only_where_it_is_installed() {
        let labels = |found| picker_choices(found).into_iter().map(|c| c.label).collect::<Vec<_>>();
        let without = labels(false);
        assert_eq!(without.len(), 1);
        assert!(without.iter().all(|l| !l.contains("Coxswain")), "{without:?}");
        assert!(labels(true).iter().any(|l| l == "Coxswain"));
    }
}
