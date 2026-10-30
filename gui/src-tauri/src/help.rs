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

/// Every notice still to show; with `start`, the hints due, counted as shown once more.
#[tauri::command]
pub fn notice_list(start: bool, ctx: tauri::State<'_, Ctx>) -> Vec<Notice> {
    list(&ctx.cfg(), start)
}

fn list(cfg: &Config, start: bool) -> Vec<Notice> {
    let mut st = AppState::load();
    let mut out = notices::all(cfg, &st);
    if start {
        let due: Vec<&str> = HINTS.into_iter().filter(|h| notices::hint_due(&mut st, h)).collect();
        if !due.is_empty() {
            let _ = st.save();
        }
        out.extend(due.into_iter().map(|h| Notice { id: format!("hint:{h}"), text: t!(&format!("hint.{h}")), settings: None, copy: None }));
    }
    out
}

/// A notice was closed: a notice is not shown again; a hint only this time.
#[tauri::command]
pub fn notice_dismiss(id: String) {
    if id.starts_with("hint:") {
        return;
    }
    let mut st = AppState::load();
    notices::dismiss(&mut st, &id);
    let _ = st.save();
}

/// Asks for a newer version when due and the check is on (never with BRAGD_OFFLINE), then
/// every notice again. Up to a few seconds: run off the main thread.
#[tauri::command]
pub async fn update_check(ctx: tauri::State<'_, Ctx>) -> Result<Vec<Notice>, String> {
    let cfg = ctx.cfg();
    tauri::async_runtime::spawn_blocking(move || {
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

fn open(app: &tauri::AppHandle, url: &str) -> Result<(), String> {
    app.opener().open_url(url, None::<&str>).map_err(|e| t!("settings.open_failed", "what" => url, "error" => e))
}
