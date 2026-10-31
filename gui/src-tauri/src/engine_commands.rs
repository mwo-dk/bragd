//! The Tauri commands of Engines, Analyse, Tablebases and Opening books. Those that start an
//! engine or go to the network run off the main thread (`async`), so the window never waits.

use bragd_core::config::Config;
use bragd_engines::options::EngineInfo;
use bragd_engines::syzygy::Set;
use bragd_games::{Family, State};
use serde::Serialize;
use serde_json::Value;
use tauri::State as Managed;

use crate::engines::{self, tr};
use crate::{Ctx, analyse, tablebases};

type Res<T> = Result<T, String>;

fn family(key: &str) -> Res<Family> {
    Family::from_key(key).ok_or_else(|| tr("engines.unknown", &[("name", key)]))
}

/// The settings now, after a change saved by `saved`.
fn keep(ctx: &Ctx, saved: Res<Config>) -> Res<Config> {
    let cfg = saved?;
    *ctx.cfg.write().unwrap_or_else(|e| e.into_inner()) = cfg.clone();
    Ok(cfg)
}

#[tauri::command(async)]
pub fn engines_page(family_key: String, ctx: Managed<'_, Ctx>, held: Managed<'_, engines::Held>) -> Res<engines::Page> {
    Ok(engines::page(&ctx.cfg(), &held, family(&family_key)?))
}

#[tauri::command]
pub fn engine_network_download(key: String, held: Managed<'_, engines::Held>) -> Res<()> {
    engines::download_network(&held, &key)
}

#[tauri::command]
pub fn engine_default(variant: String, engine: String, ctx: Managed<'_, Ctx>) -> Res<()> {
    keep(&ctx, engines::set_default(&variant, &engine)).map(drop)
}

#[tauri::command(async)]
pub fn engine_add(family_key: String, path: String, ctx: Managed<'_, Ctx>) -> Res<()> {
    keep(&ctx, engines::add(family(&family_key)?, &path)).map(drop)
}

#[tauri::command]
pub fn engine_remove(key: String, ctx: Managed<'_, Ctx>) -> Res<()> {
    keep(&ctx, engines::remove(&ctx.cfg(), &key)).map(drop)
}

#[tauri::command(async)]
pub fn engine_options(key: String, variant: String, ctx: Managed<'_, Ctx>, held: Managed<'_, engines::Held>) -> Res<EngineInfo> {
    engines::options_of(&ctx.cfg(), &held, &key, &variant)
}

#[tauri::command]
pub fn engine_set_option(key: String, name: String, value: Option<Value>, held: Managed<'_, engines::Held>) -> Res<EngineInfo> {
    engines::set_option(&held, &key, &name, value.as_ref())
}

#[tauri::command]
pub fn engine_reset_options(key: String, held: Managed<'_, engines::Held>) -> Res<EngineInfo> {
    engines::reset_options(&held, &key)
}

#[tauri::command]
pub fn engine_pull(key: String, ctx: Managed<'_, Ctx>, held: Managed<'_, engines::Held>) -> Res<()> {
    engines::pull(&ctx.cfg(), &held, &key)
}

/// An engine as Play and Analyse offer it.
#[derive(Serialize)]
pub struct Choice {
    key: String,
    name: String,
    ready: bool,
}

#[derive(Serialize)]
pub struct Choices {
    engines: Vec<Choice>,
    default: Option<String>,
    /// The Variant's two Sides, named in the current language.
    sides: Vec<String>,
}

#[tauri::command(async)]
pub fn engine_choices(variant: String, ctx: Managed<'_, Ctx>) -> Choices {
    let cfg = ctx.cfg();
    let engines = engines::engines_for(&cfg, &variant)
        .into_iter()
        .map(|key| {
            let ready = matches!(engines::availability(&cfg, &key), engines::Availability::Installed { .. } | engines::Availability::Pulled { .. });
            let name = bragd_engines::known::known(&key).map_or_else(|| key.clone(), |k| k.name.to_string());
            Choice { key, name, ready }
        })
        .collect();
    let sides = bragd_games::Variant::from_key(&variant).map(|v| v.info().sides.iter().map(|s| tr(&format!("side.{}", s.to_lowercase()), &[])).collect()).unwrap_or_default();
    Choices { engines, default: engines::default_for(&cfg, &variant), sides }
}

#[tauri::command(async)]
#[allow(clippy::too_many_arguments, reason = "one argument per thing the page sends")]
pub fn engine_move(variant: String, initial: Option<String>, entries: Vec<String>, engine: Option<String>, ms: u32, skill: Option<u8>, ctx: Managed<'_, Ctx>, held: Managed<'_, engines::Held>) -> Res<engines::Moved> {
    engines::engine_move(&ctx.cfg(), &held, &variant, initial.as_deref(), &entries, engine.as_deref(), ms, skill)
}

#[tauri::command(async)]
pub fn analysis_start(variant: String, initial: Option<String>, entries: Vec<String>, engine: Option<String>, lines: Option<u32>, ctx: Managed<'_, Ctx>, held: Managed<'_, analyse::Held>) -> Res<()> {
    analyse::start(&ctx.cfg(), &held, &variant, initial, entries, engine, lines)
}

#[tauri::command(async)]
pub fn analysis_now(held: Managed<'_, analyse::Held>) -> Option<analyse::Now> {
    analyse::now(&held)
}

#[tauri::command]
pub fn analysis_stop(held: Managed<'_, analyse::Held>) {
    analyse::stop(&held)
}

#[tauri::command(async)]
pub fn review_start(variant: String, initial: Option<String>, entries: Vec<String>, engine: Option<String>, ctx: Managed<'_, Ctx>, held: Managed<'_, analyse::Held>) -> Res<()> {
    analyse::review_start(&ctx.cfg(), &held, &variant, initial.as_deref(), &entries, engine)
}

#[tauri::command(async)]
pub fn review_now(held: Managed<'_, analyse::Held>) -> Option<analyse::ReviewView> {
    analyse::review_now(&held)
}

#[tauri::command]
pub fn review_stop(held: Managed<'_, analyse::Held>) {
    analyse::review_stop(&held)
}

#[tauri::command(async)]
pub fn position_known(variant: String, initial: Option<String>, entries: Vec<String>, ctx: Managed<'_, Ctx>) -> Res<analyse::Known> {
    analyse::known(&ctx.cfg(), &variant, initial.as_deref(), &entries)
}

#[tauri::command(async)]
pub fn tablebase_probe(variant: String, initial: Option<String>, entries: Vec<String>, ctx: Managed<'_, Ctx>) -> Res<Option<analyse::TableView>> {
    analyse::tablebase(&ctx.cfg(), &variant, initial.as_deref(), &entries)
}

#[tauri::command]
pub fn position_setup(variant: String, text: String) -> Res<State> {
    analyse::setup(&variant, &text)
}

#[tauri::command(async)]
pub fn tablebases_page(ctx: Managed<'_, Ctx>, held: Managed<'_, tablebases::Held>) -> tablebases::Page {
    tablebases::page(&ctx.cfg(), &held)
}

#[tauri::command(async)]
pub fn tablebase_plan(set: Set, held: Managed<'_, tablebases::Held>) -> Res<u64> {
    tablebases::plan(&held, set)
}

#[tauri::command]
pub fn tablebase_download(set: Set, ctx: Managed<'_, Ctx>, held: Managed<'_, tablebases::Held>) -> Res<()> {
    tablebases::download(&ctx.cfg(), &held, set)
}

#[tauri::command]
pub fn tablebase_stop(held: Managed<'_, tablebases::Held>) {
    tablebases::stop(&held)
}

#[tauri::command(async)]
pub fn books_page(ctx: Managed<'_, Ctx>) -> Vec<tablebases::Book> {
    tablebases::books(&ctx.cfg())
}

#[tauri::command]
pub fn book_add(path: String, ctx: Managed<'_, Ctx>) -> Res<()> {
    keep(&ctx, tablebases::add_book(&ctx.cfg(), &path)).map(drop)
}

#[tauri::command]
pub fn book_remove(path: String, ctx: Managed<'_, Ctx>) -> Res<()> {
    keep(&ctx, tablebases::remove_book(&ctx.cfg(), &path)).map(drop)
}
