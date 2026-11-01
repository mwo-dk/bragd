//! The Clubs page: the Club files known here, the one open, its members, ratings and audit
//! log, and players found in the rating lists. Everything is decided in `bragd_clubs`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use bragd_clubs::club::{self, Club, Details};
use bragd_clubs::lists;
use bragd_clubs::rating::System;
use bragd_core::state::AppState;
use bragd_core::t;
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{Ctx, files};

type Res<T> = Result<T, String>;

/// The Club open on the Clubs page; it holds its lock while open.
#[derive(Default)]
pub struct Held(pub(crate) Mutex<Option<Club>>);

/// A Club file known here.
#[derive(Serialize)]
pub struct Known {
    path: PathBuf,
    /// The file name without its extension.
    name: String,
    there: bool,
}

/// A rating list and what was last imported of it.
#[derive(Serialize)]
pub struct ListView {
    id: &'static str,
    name: String,
    /// Players imported; none before the first fetch.
    players: Option<u64>,
    at: Option<u64>,
}

#[derive(Serialize)]
pub struct Page {
    clubs: Vec<Known>,
    open: Option<club::View>,
    lists: Vec<ListView>,
}

fn known() -> Vec<Known> {
    AppState::load()
        .clubs
        .into_iter()
        .map(|p| Known { name: p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), there: p.is_file(), path: p })
        .collect()
}

fn remember(path: &Path) {
    let mut st = AppState::load();
    if !st.clubs.iter().any(|p| p == path) {
        st.clubs.push(path.into());
        let _ = st.save();
    }
}

fn rating_lists() -> Vec<ListView> {
    let imported = lists::path().and_then(|p| lists::lists(&p).ok()).unwrap_or_default();
    let tags = bragd_core::i18n::locale_tags();
    // FIDE's, the user's own federation's and any already fetched; the rest are under More lists
    // on chess's Sources page.
    lists::LISTS
        .iter()
        .filter_map(|l| {
            let i = imported.iter().find(|i| i.list == l.id);
            (i.is_some() || l.suggested(&tags)).then(|| ListView { id: l.id, name: l.title(), players: i.map(|i| i.players), at: i.map(|i| i.at) })
        })
        .collect()
}

fn view(held: &Held) -> Res<Option<club::View>> {
    crate::lock(&held.0).as_ref().map(|c| c.view().map_err(|e| e.to_string())).transpose()
}

#[tauri::command(async)]
pub fn clubs_page(held: State<'_, Held>) -> Res<Page> {
    Ok(Page { clubs: known(), open: view(&held)?, lists: rating_lists() })
}

fn open_at(path: &Path, held: &Held) -> Res<club::View> {
    // The Club open before lets go of its lock first.
    *crate::lock(&held.0) = None;
    let c = Club::open(path).map_err(|e| e.to_string())?;
    let v = c.view().map_err(|e| e.to_string())?;
    *crate::lock(&held.0) = Some(c);
    remember(path);
    bragd_core::tips::used("clubs");
    Ok(v)
}

#[tauri::command(async)]
pub fn club_open(path: PathBuf, held: State<'_, Held>) -> Res<club::View> {
    open_at(&path, &held)
}

/// Asks for a Club file with the file dialog; `None` when none was chosen.
#[tauri::command(async)]
pub fn club_pick(app: AppHandle, held: State<'_, Held>, ctx: State<'_, Ctx>) -> Res<Option<club::View>> {
    let ask = files::Ask { title: t!("clubs.pick"), filters: vec![(t!("clubs.files"), vec![club::EXT.into()])], ..Default::default() };
    let Some(f) = files::pick_one(&app, &ctx.cfg(), &ask)? else { return Ok(None) };
    open_at(&f, &held).map(Some)
}

/// Where a new Club named `name` is kept: `clubs/<name>.bragd-club` in the data folder.
fn new_path(name: &str) -> Res<PathBuf> {
    let file: String = name.trim().chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | ' ') { c } else { '_' }).collect();
    let dir = bragd_core::dirs::app_data().ok_or("no data folder")?.join("clubs");
    Ok(dir.join(format!("{}.{}", file.trim(), club::EXT)))
}

#[tauri::command(async)]
pub fn club_create(name: String, system: System, director: String, held: State<'_, Held>) -> Res<club::View> {
    let path = new_path(&name)?;
    *crate::lock(&held.0) = None;
    let c = Club::create(&path, &name, system, &director).map_err(|e| e.to_string())?;
    let v = c.view().map_err(|e| e.to_string())?;
    *crate::lock(&held.0) = Some(c);
    remember(&path);
    bragd_core::tips::used("clubs");
    Ok(v)
}

#[tauri::command(async)]
pub fn club_close(held: State<'_, Held>) {
    *crate::lock(&held.0) = None;
}

/// Takes a Club off the list here; the file stays where it is.
#[tauri::command(async)]
pub fn club_forget(path: PathBuf, held: State<'_, Held>) -> Res<Page> {
    if crate::lock(&held.0).as_ref().is_some_and(|c| c.path() == path) {
        *crate::lock(&held.0) = None;
    }
    let mut st = AppState::load();
    st.clubs.retain(|p| p != &path);
    st.save().map_err(|e| e.to_string())?;
    clubs_page(held)
}

/// Runs `f` on the open Club, then gives its view.
fn with(held: &Held, f: impl FnOnce(&mut Club) -> club::Result<()>) -> Res<club::View> {
    let mut g = crate::lock(&held.0);
    let c = g.as_mut().ok_or_else(|| t!("clubs.none_open"))?;
    f(c).map_err(|e| e.to_string())?;
    c.view().map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn club_add(by: i64, details: Details, rating: Option<f64>, held: State<'_, Held>) -> Res<club::View> {
    with(&held, |c| c.add(by, &details, rating).map(drop))
}

#[tauri::command(async)]
pub fn club_update(by: i64, id: i64, details: Details, held: State<'_, Held>) -> Res<club::View> {
    with(&held, |c| c.update(by, id, &details))
}

#[tauri::command(async)]
pub fn club_remove(by: i64, id: i64, held: State<'_, Held>) -> Res<club::View> {
    with(&held, |c| c.remove(by, id))
}

#[tauri::command(async)]
pub fn club_rating(by: i64, id: i64, rating: f64, why: String, held: State<'_, Held>) -> Res<club::View> {
    with(&held, |c| c.set_rating(by, id, rating, &why))
}

#[tauri::command(async)]
pub fn club_system(by: i64, system: System, why: String, held: State<'_, Held>) -> Res<club::View> {
    with(&held, |c| c.set_system(by, system, &why))
}

#[tauri::command(async)]
pub fn club_take_over(by: i64, held: State<'_, Held>) -> Res<club::View> {
    with(&held, |c| c.take_over(by))
}

/// A player found in a rating list, with the list's name, and whether it is FIDE's (its ID is
/// then a FIDE ID).
#[derive(Serialize)]
pub struct Found {
    list: String,
    fide: bool,
    #[serde(flatten)]
    player: lists::Player,
}

/// Players in the rating lists whose name holds `query`, or whose ID it is.
#[tauri::command(async)]
pub fn rating_search(query: String) -> Res<Vec<Found>> {
    let Some(db) = lists::path() else { return Ok(vec![]) };
    Ok(lists::search(&db, &query)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|f| {
            let list = lists::LISTS.iter().find(|l| l.id == f.list);
            Found { list: list.map_or("?".into(), |l| l.title()), fide: list.is_some_and(|l| l.region.is_empty()), player: f.player }
        })
        .collect())
}
