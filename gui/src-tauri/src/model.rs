//! Settings → Language model and a Comment's *Translate* (`bragd-model`). The address, model
//! and consent are options saved like any other; the key goes to the OS keychain from here.
//! With no model ready here, a Peer's model translates through the shared queue (the Helper's
//! `Work::Translate`).

use std::time::{Duration, Instant};

use bragd_core::{i18n, t};
use bragd_helper::helper::{Reply, Request};
use bragd_helper::jobs::{Status as JobStatus, Work};
use bragd_model::{CommentView, Found, Status, Translation, Translations};
use tauri::State;

use crate::Ctx;
use crate::knowledge::Knowledge;

/// How long a translation by a Peer's model is waited for.
const PEER_WAIT: Duration = Duration::from_secs(120);

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

/// Whether a Peer has a language model ready, as it said at the last sync; asked of the Helper
/// only while Peers are on.
fn peer_model(k: &Knowledge, on: bool) -> bool {
    on && matches!(k.link.ask(&Request::Peers { hello: false }), Ok(Reply::Peers { peers }) if peers.peers.iter().any(|p| p.sync.as_ref().is_some_and(|s| s.model && s.error.is_none())))
}

/// Whether to offer *Translate* on `text`, and its translation kept for the reader's language:
/// offered with a model ready here, or a Peer's.
#[tauri::command(async)]
pub fn comment_view(text: String, ctx: State<'_, Ctx>, k: State<'_, Knowledge>) -> CommentView {
    let cfg = ctx.cfg();
    let mut v = bragd_model::comment_view(&cfg.model, kept().as_ref(), &text, i18n::language());
    let foreign = !text.trim().is_empty() && !bragd_model::lang::in_language(text.trim(), i18n::language());
    if foreign && !bragd_model::ready(&cfg.model) {
        v.translatable = peer_model(&k, cfg.peers.on);
    }
    v
}

/// `text` translated into the reader's language, asked of the model only when not kept yet: of
/// this machine's model, else of a Peer's through the shared queue.
#[tauri::command(async)]
pub fn comment_translate(text: String, ctx: State<'_, Ctx>, k: State<'_, Knowledge>) -> Result<Translation, String> {
    let path = Translations::path().ok_or("no data folder")?;
    let store = Translations::open(&path).map_err(|e| e.to_string())?;
    let m = ctx.cfg().model;
    let t = if bragd_model::ready(&m) {
        bragd_model::translate(&m, &store, &text, i18n::language())?
    } else {
        let t = by_peer(&k, &store, &text)?;
        bragd_core::tips::used("sharing");
        t
    };
    bragd_core::tips::used("model");
    Ok(t)
}

/// `text` translated by a Peer's model: queued with the Helper, which lends it to a Peer with a
/// model ready and keeps what comes back. Waits for it, at most `PEER_WAIT`.
fn by_peer(k: &Knowledge, store: &Translations, text: &str) -> Result<Translation, String> {
    let language = i18n::language();
    if let Some(t) = store.get(text.trim(), language).map_err(|e| e.to_string())? {
        return Ok(t);
    }
    let Reply::Started { id } = k.link.ask(&Request::Start { work: Work::Translate { text: text.trim().into(), language: language.into() } })? else {
        return Err(t!("helper.odd_reply"));
    };
    let until = Instant::now() + PEER_WAIT;
    while Instant::now() < until {
        std::thread::sleep(Duration::from_millis(300));
        let Reply::Jobs { jobs } = k.link.ask(&Request::Jobs)? else { return Err(t!("helper.odd_reply")) };
        match jobs.into_iter().find(|j| j.id == id) {
            Some(j) if j.status == JobStatus::Done => return j.report.and_then(|r| r.translation).ok_or_else(|| t!("model.error.no_peer")),
            Some(j) if j.status == JobStatus::Failed => return Err(j.error.unwrap_or_default()),
            Some(j) if j.status == JobStatus::Cancelled => break,
            Some(_) => {}
            None => break,
        }
    }
    let _ = k.link.ask(&Request::Cancel { id });
    Err(t!("model.error.no_peer"))
}
