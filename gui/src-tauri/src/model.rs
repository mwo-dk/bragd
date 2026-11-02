//! Settings → Language model and a Comment's *Translate* (`bragd-model`). The address, model
//! and consent are options saved like any other; the key goes to the OS keychain from here.
//! With no model ready here, a Peer's model translates through the shared queue (the Helper's
//! `Work::Translate`).
//!
//! Commentary on a move always goes through the queue (`Work::Commentary`): the Helper's model
//! lane writes it with this machine's model, or lends it to a Peer's.

use std::time::{Duration, Instant};

use bragd_core::{i18n, t};
use bragd_helper::helper::{Reply, Request};
use bragd_helper::jobs::{Status as JobStatus, Work};
use bragd_model::{CommentView, Found, Status, Translation, Translations};
use serde::Serialize;
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
/// model ready and keeps what comes back.
fn by_peer(k: &Knowledge, store: &Translations, text: &str) -> Result<Translation, String> {
    let language = i18n::language();
    if let Some(t) = store.get(text.trim(), language).map_err(|e| e.to_string())? {
        return Ok(t);
    }
    queued(k, Work::Translate { text: text.trim().into(), language: language.into() }, "model.error.no_peer")
}

/// A model job queued with the Helper and waited for, at most `PEER_WAIT`: what the model wrote,
/// or the text `late` when it did not come in time.
fn queued(k: &Knowledge, work: Work, late: &str) -> Result<Translation, String> {
    let Reply::Started { id } = k.link.ask(&Request::Start { work })? else {
        return Err(t!("helper.odd_reply"));
    };
    let until = Instant::now() + PEER_WAIT;
    while Instant::now() < until {
        std::thread::sleep(Duration::from_millis(300));
        let Reply::Jobs { jobs } = k.link.ask(&Request::Jobs)? else { return Err(t!("helper.odd_reply")) };
        match jobs.into_iter().find(|j| j.id == id) {
            Some(j) if j.status == JobStatus::Done => return j.report.and_then(|r| r.translation).ok_or_else(|| bragd_core::i18n::tr(late, &[])),
            Some(j) if j.status == JobStatus::Failed => return Err(j.error.unwrap_or_default()),
            Some(j) if j.status == JobStatus::Cancelled => break,
            Some(_) => {}
            None => break,
        }
    }
    let _ = k.link.ask(&Request::Cancel { id });
    Err(bragd_core::i18n::tr(late, &[]))
}

/// Commentary as Analyse shows it for the Game's last move: whether a model is ready (here or a
/// Peer's) and the store has facts to explain, why not when it cannot be offered, and the
/// Commentary kept in the reader's language, with its model.
#[derive(Clone, Debug, Default, Serialize)]
pub struct CommentaryView {
    pub model: bool,
    pub offered: bool,
    pub why: Option<String>,
    pub shown: Option<Written>,
}

/// Machine-written prose and the model that wrote it.
#[derive(Clone, Debug, Serialize)]
pub struct Written {
    pub text: String,
    pub model: String,
}

/// The Commentary kept for `f` in the reader's language.
fn kept_commentary(f: &crate::analyse::Facts) -> Result<Option<Written>, String> {
    let Some(path) = bragd_positions::Store::path().filter(|p| p.exists()) else { return Ok(None) };
    let c = bragd_positions::Store::open(&path).and_then(|s| s.commentary(&f.family, &f.variant, f.key, &f.mv, i18n::language())).map_err(|e| e.to_string())?;
    Ok(c.map(|c| Written { text: c.text, model: c.model }))
}

#[tauri::command(async)]
pub fn commentary_view(variant: String, initial: Option<String>, entries: Vec<String>, engine: Option<String>, ctx: State<'_, Ctx>, k: State<'_, Knowledge>) -> Result<CommentaryView, String> {
    let cfg = ctx.cfg();
    let model = bragd_model::ready(&cfg.model) || peer_model(&k, cfg.peers.on);
    let Some(f) = crate::analyse::facts(&cfg, &variant, initial.as_deref(), &entries, engine)? else {
        return Ok(CommentaryView { model, why: Some(t!("commentary.no_facts")), ..Default::default() });
    };
    Ok(CommentaryView { model, offered: model, why: (!model).then(|| t!("commentary.no_model")), shown: kept_commentary(&f)? })
}

/// Commentary on the Game's last move in the reader's language: the one kept, else written now
/// through the shared queue and kept in the Position store.
#[tauri::command(async)]
pub fn commentary_write(variant: String, initial: Option<String>, entries: Vec<String>, engine: Option<String>, ctx: State<'_, Ctx>, k: State<'_, Knowledge>) -> Result<Written, String> {
    let f = crate::analyse::facts(&ctx.cfg(), &variant, initial.as_deref(), &entries, engine)?.ok_or_else(|| t!("commentary.no_facts"))?;
    if let Some(w) = kept_commentary(&f)? {
        return Ok(w);
    }
    let w = queued(&k, Work::Commentary { family: f.family, variant: f.variant, key: f.key, mv: f.mv, language: i18n::language().into(), facts: f.text }, "commentary.timeout")?;
    bragd_core::tips::used("commentary");
    Ok(Written { text: w.text, model: w.model })
}
