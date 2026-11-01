//! F1 → Features, the notice area (notices once, hints a few times, the update notice), and what
//! Settings opens outside Bragd: config.toml in the system's editor, a translation issue and
//! the docs in the browser.

use bragd_core::config::Config;
use bragd_core::notices::{self, Notice};
use bragd_core::state::AppState;
use bragd_core::{features, i18n, t, update};
use tauri_plugin_opener::OpenerExt;

use crate::Ctx;

/// F1 → Features, filtered by `query` (case and accents aside).
#[tauri::command]
pub fn feature_list(query: String) -> Vec<features::Entry> {
    features::list(&query)
}

/// The hints shown at start, a few times each (`notices::hint_due`).
const HINTS: [&str; 3] = ["sidebar", "features", "wizard"];

/// Every notice still to show; with `start`, the first hint due, counted as shown once more.
#[tauri::command]
pub fn notice_list(start: bool, ctx: tauri::State<'_, Ctx>) -> Vec<Notice> {
    list(&ctx.cfg(), start)
}

fn list(cfg: &Config, start: bool) -> Vec<Notice> {
    let mut st = AppState::load();
    let mut out = notices::all(cfg, &st);
    if start {
        out.extend(hint(cfg, &mut st, None));
    }
    out
}

/// The hints that apply to the user of `cfg`: the sidebar's Families hint only when it lists
/// Families (one Family's items are listed at once).
fn hints(cfg: &Config) -> Vec<&'static str> {
    let families = matches!(bragd_core::settings::sidebar(cfg), bragd_core::settings::Sidebar::Families { .. });
    HINTS.into_iter().filter(|h| families || *h != "sidebar").collect()
}

/// The next hint due (after `after`), saved as shown once more.
fn hint(cfg: &Config, st: &mut AppState, after: Option<&str>) -> Option<Notice> {
    let h = notices::next_hint(st, &hints(cfg), after)?;
    let _ = st.save();
    Some(Notice { id: format!("hint:{h}"), text: t!(&format!("hint.{h}")), settings: None, copy: None })
}

/// A notice was closed: a notice is not shown again; a hint only this time, and the next hint
/// due comes in its place.
#[tauri::command]
pub fn notice_dismiss(id: String, ctx: tauri::State<'_, Ctx>) -> Option<Notice> {
    let mut st = AppState::load();
    if let Some(h) = id.strip_prefix("hint:") {
        return hint(&ctx.cfg(), &mut st, Some(h));
    }
    notices::dismiss(&mut st, &id);
    let _ = st.save();
    None
}

/// Asks for a newer version when due and the check is on (never with BRAGD_OFFLINE), then
/// every notice again. Starts the day's engine image housekeeping beside it. Up to a few seconds: run off the main thread.
#[tauri::command]
pub async fn update_check(ctx: tauri::State<'_, Ctx>) -> Result<Vec<Notice>, String> {
    let cfg = ctx.cfg();
    let housekept = cfg.clone();
    std::thread::spawn(move || crate::engines::housekeeping(&housekept));
    tauri::async_runtime::spawn_blocking(move || {
        if cfg.updates.check {
            bragd_core::tips::used("update");
        }
        update::check(&cfg);
        list(&cfg, false)
    })
    .await
    .map_err(|e| e.to_string())
}

/// config.toml in the system's editor, made empty first when it is not there yet.
#[tauri::command]
pub fn open_config(app: tauri::AppHandle) -> Result<(), String> {
    let path = Config::path().ok_or_else(|| t!("config.no_folder"))?;
    if !path.exists() {
        bragd_core::fs::replace(&path, b"", true).map_err(|e| e.to_string())?;
    }
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| t!("settings.open_failed", "what" => "config.toml", "error" => e))
}

/// A new GitHub issue for a better translation of the current language, in the browser,
/// naming the text the user typed (`shown`) and its key.
#[tauri::command]
pub fn suggest_translation(shown: String, app: tauri::AppHandle) -> Result<(), String> {
    open(&app, &i18n::suggest_url(i18n::language(), &shown))
}

/// A web page in the browser: only http(s) addresses (the docs, an engine's site, a server).
#[tauri::command]
pub fn open_url(url: String, app: tauri::AppHandle) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("{url}: not a web address"));
    }
    open(&app, &url)
}

pub fn open(app: &tauri::AppHandle, url: &str) -> Result<(), String> {
    app.opener().open_url(url, None::<&str>).map_err(|e| t!("settings.open_failed", "what" => url, "error" => e))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Someone who plays one game has no Families in the sidebar to click: no hint says to.
    #[test]
    fn the_sidebar_hint_is_for_a_sidebar_of_families() {
        assert!(hints(&Config::default()).contains(&"sidebar"));
        let only_go = Config::parse("[families.family]\nchess = false\nxiangqi = false\nshogi = false\ndraughts = false\ntafl = false\nbackgammon = false\nur = false\nsenet = false\n").unwrap();
        assert_eq!(hints(&only_go), ["features", "wizard"]);
    }
}
