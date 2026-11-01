//! Settings → Language model and a Comment's *Translate* (`bragd-model`). The address, model
//! and consent are options saved like any other; the key goes to the OS keychain from here.

use bragd_core::i18n;
use bragd_model::{CommentView, Found, Status, Translation, Translations};

use crate::Ctx;

/// The language model as Settings shows it.
#[tauri::command]
pub fn model_status(ctx: tauri::State<'_, Ctx>) -> Status {
    bragd_model::status(&ctx.cfg().model)
}

/// Ollama, Lemonade and LM Studio running on this machine, with their models.
#[tauri::command(async)]
pub fn model_find() -> Vec<Found> {
    bragd_model::client::find()
}

/// The models the server at `url` offers, asked with the key kept for it.
#[tauri::command(async)]
pub fn model_models(url: String) -> Result<Vec<String>, String> {
    bragd_model::client::models(&url, bragd_model::key::get(&url).as_deref())
}

/// Keeps `key` for the address set, in the keychain; an empty one removes it.
#[tauri::command(async)]
pub fn model_key(key: String, ctx: tauri::State<'_, Ctx>) -> Result<Status, String> {
    let m = ctx.cfg().model;
    bragd_model::key::set(&m.url, &key)?;
    Ok(bragd_model::status(&m))
}

/// The translations kept so far; none before the first (the file is made by the first one).
fn kept() -> Option<Translations> {
    Translations::open(&Translations::path().filter(|p| p.exists())?).ok()
}

/// Whether to offer *Translate* on `text`, and its translation kept for the reader's language.
#[tauri::command(async)]
pub fn comment_view(text: String, ctx: tauri::State<'_, Ctx>) -> CommentView {
    bragd_model::comment_view(&ctx.cfg().model, kept().as_ref(), &text, i18n::language())
}

/// `text` translated into the reader's language, asked of the model only when not kept yet.
#[tauri::command(async)]
pub fn comment_translate(text: String, ctx: tauri::State<'_, Ctx>) -> Result<Translation, String> {
    let path = Translations::path().ok_or("no data folder")?;
    let store = Translations::open(&path).map_err(|e| e.to_string())?;
    let t = bragd_model::translate(&ctx.cfg().model, &store, &text, i18n::language())?;
    bragd_core::tips::used("model");
    Ok(t)
}
