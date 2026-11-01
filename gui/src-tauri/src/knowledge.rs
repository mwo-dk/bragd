//! The Tauri commands of Knowledge: the Helper and its jobs, Databases (Foreign databases,
//! Libraries, Slices), Sources with their Strip rules, and Disk use. The work is the Helper's
//! (`helper_link`) and `bragd-library`'s (`databases`); this module hands the page what it draws,
//! texts included, and turns what the user typed (sizes, Slice fields) into values.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use bragd_core::config::{Config, DiskConfig};
use bragd_core::t;
use bragd_helper::disk;
use bragd_helper::helper::{Reply, Request};
use bragd_helper::sources::{self, Auto, Options, Route, Schedule};
use bragd_library::chess::Query;
use bragd_library::library::Library;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use crate::{Ctx, files};
use crate::databases::{self, Databases, FileView, LibraryView, OpenFile, Replay, Review, Rows, SliceView};
use crate::helper_link::{self, HelperView, Link};

#[derive(Default)]
pub struct Knowledge {
    pub(crate) link: Link,
    dbs: Databases,
    /// The file Save as… chose that waits for the user's yes; only it may be written over.
    to_confirm: Mutex<Option<PathBuf>>,
}

type Res<T> = Result<T, String>;

/// Reaches the Helper in the background as the window opens, starting it when none runs.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let _ = app.state::<Knowledge>().link.ask(&Request::Status);
    });
}

// ---------------------------------------------------------------- the Helper and its jobs

#[tauri::command(async)]
pub fn helper_status(k: State<'_, Knowledge>) -> Res<HelperView> {
    Ok(helper_link::view(&k.link))
}

/// Tries to reach the Helper again at once, after it could not be reached.
#[tauri::command(async)]
pub fn helper_retry(k: State<'_, Knowledge>) -> Res<HelperView> {
    k.link.retry();
    Ok(helper_link::view(&k.link))
}

#[tauri::command(async)]
pub fn job_cancel(id: u64, k: State<'_, Knowledge>) -> Res<()> {
    k.link.ask(&Request::Cancel { id }).map(drop)
}

// ---------------------------------------------------------------- Foreign databases

/// The extensions of the files Bragd reads in place.
const GAME_FILES: [&str; 13] = ["pgn", "gz", "zst", "bz2", "si4", "si5", "cbh", "cbv", "sgf", "pdn", "kif", "kifu", "csa"];

/// Asks for a Foreign database with the file dialog and opens it; `None` when none was chosen.
#[tauri::command(async)]
pub fn db_pick(app: AppHandle, k: State<'_, Knowledge>, ctx: State<'_, Ctx>) -> Res<Option<FileView>> {
    let ask = files::Ask { title: t!("files.pick_database"), filters: vec![(t!("db.game_files"), GAME_FILES.iter().map(|e| e.to_string()).collect())], ..Default::default() };
    let Some(f) = files::pick_one(&app, &ctx.cfg(), &ask)? else { return Ok(None) };
    bragd_core::tips::used("databases");
    db_open(&f, &k, &ctx.cfg()).map(Some)
}

fn db_open(path: &Path, k: &Knowledge, cfg: &Config) -> Res<FileView> {
    let f = OpenFile::open(path, cfg)?;
    let v = f.view(cfg);
    *crate::lock(&k.dbs.file) = Some(f);
    Ok(v)
}

/// The Foreign database open now, as far as it is read.
#[tauri::command(async)]
pub fn db_current(k: State<'_, Knowledge>, ctx: State<'_, Ctx>) -> Res<Option<FileView>> {
    let cfg = ctx.cfg();
    Ok(crate::lock(&k.dbs.file).as_ref().map(|f| f.view(&cfg)))
}

#[tauri::command(async)]
pub fn db_close(k: State<'_, Knowledge>) -> Res<()> {
    *crate::lock(&k.dbs.file) = None;
    Ok(())
}

#[tauri::command(async)]
pub fn db_rows(query: Query, from: usize, count: usize, k: State<'_, Knowledge>) -> Res<Rows> {
    let mut f = crate::lock(&k.dbs.file);
    Ok(f.as_mut().ok_or_else(|| t!("db.none_open"))?.rows(&query, from, count))
}

#[tauri::command(async)]
pub fn db_game(id: u32, k: State<'_, Knowledge>) -> Res<Replay> {
    let f = crate::lock(&k.dbs.file);
    f.as_ref().ok_or_else(|| t!("db.none_open"))?.game(id)
}

fn open_path(k: &Knowledge) -> Res<PathBuf> {
    let f = crate::lock(&k.dbs.file);
    f.as_ref().map(|f| f.path().to_path_buf()).ok_or_else(|| t!("db.none_open"))
}

/// Imports the open Foreign database into the default Library, as a Helper job: its id.
#[tauri::command(async)]
pub fn db_import(k: State<'_, Knowledge>) -> Res<u64> {
    let path = open_path(&k)?;
    k.link.start(&Request::Import { path, library: None, source: None })
}

/// What Save as… did: saved to `path`, or it waits for the user to confirm writing over a
/// downloaded file there.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    pub path: String,
    pub confirm: bool,
}

fn save_to(from: &Path, to: &Path, confirmed: bool) -> Res<Saved> {
    if to.exists() && databases::downloaded(to) && !confirmed {
        return Ok(Saved { path: to.display().to_string(), confirm: true });
    }
    if from.canonicalize().ok() != to.canonicalize().ok() || !to.exists() {
        // Written beside it, then put in place: a failure leaves the old file whole.
        bragd_core::fs::replace_with(to, false, |f| std::io::copy(&mut std::fs::File::open(from)?, f).map(drop)).map_err(|e| e.to_string())?;
    }
    Ok(Saved { path: to.display().to_string(), confirm: false })
}

/// Save as…: the open Foreign database written to a file the user picks. Writing over a
/// downloaded file waits for `db_save_to` with the user's yes.
#[tauri::command(async)]
pub fn db_save_as(app: AppHandle, k: State<'_, Knowledge>, ctx: State<'_, Ctx>) -> Res<Option<Saved>> {
    let from = open_path(&k)?;
    let name = from.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let ask = files::Ask { title: t!("files.save_database"), save: Some(name), start: from.parent().map(Path::to_path_buf), ..Default::default() };
    let Some(to) = files::pick_one(&app, &ctx.cfg(), &ask)? else { return Ok(None) };
    let saved = save_to(&from, &to, false)?;
    *crate::lock(&k.to_confirm) = saved.confirm.then_some(to);
    Ok(Some(saved))
}

/// Writes the open Foreign database over the file Save as… asked about, the user having said
/// yes. The page names no path, so it cannot have any other file written over.
#[tauri::command(async)]
pub fn db_save_to(k: State<'_, Knowledge>) -> Res<Saved> {
    save_confirmed(&open_path(&k)?, &k.to_confirm)
}

fn save_confirmed(from: &Path, to_confirm: &Mutex<Option<PathBuf>>) -> Res<Saved> {
    let to = crate::lock(to_confirm).take().ok_or_else(|| t!("db.none_open"))?;
    save_to(from, &to, true)
}

// ---------------------------------------------------------------- Libraries and Slices

#[tauri::command(async)]
pub fn libraries_list(ctx: State<'_, Ctx>) -> Res<Vec<LibraryView>> {
    Ok(databases::libraries(&ctx.cfg()))
}

#[tauri::command(async)]
pub fn library_rows(path: String, query: Value, from: usize, count: usize, k: State<'_, Knowledge>) -> Res<Rows> {
    k.dbs.library_rows(Path::new(&path), &databases::query_in(query)?, from, count)
}

#[tauri::command(async)]
pub fn library_game(path: String, id: i64) -> Res<Replay> {
    let lib = Library::open(Path::new(&path)).map_err(|e| e.to_string())?;
    Ok(databases::stored_replay(&lib.stored(id).map_err(|e| e.to_string())?))
}

#[tauri::command(async)]
pub fn library_review(path: String) -> Res<Review> {
    databases::review(Path::new(&path))
}

/// "Dedupe now": the Helper merges the headers of every merged Game again: its job's id.
#[tauri::command(async)]
pub fn library_dedupe(path: String, k: State<'_, Knowledge>) -> Res<u64> {
    k.link.start(&Request::DedupeNow { library: Some(PathBuf::from(path)) })
}

/// Export to Parquet: the Library written to a `.parquet` file the user picks, as a Helper
/// job: its id; None when the dialog was closed.
#[tauri::command(async)]
pub fn library_export(app: AppHandle, path: String, k: State<'_, Knowledge>, ctx: State<'_, Ctx>) -> Res<Option<u64>> {
    bragd_core::tips::used("parquet");
    let library = PathBuf::from(path);
    let name = format!("{}.parquet", library.file_stem().unwrap_or_default().to_string_lossy());
    let ask = files::Ask { title: t!("lib.export_title"), save: Some(name), filters: vec![("Parquet".into(), vec!["parquet".into()])], start: library.parent().map(Path::to_path_buf), ..Default::default() };
    let Some(to) = files::pick_one(&app, &ctx.cfg(), &ask)? else { return Ok(None) };
    k.link.start(&Request::Start { work: bragd_helper::jobs::Work::Export { library, to } }).map(Some)
}

#[tauri::command(async)]
pub fn priority_get(ctx: State<'_, Ctx>) -> Res<Vec<String>> {
    Ok(ctx.cfg().knowledge.source_priority)
}

/// Saves the Source priority to `[knowledge] source_priority` and restarts the Helper, which
/// reads it as it starts; a running job starts again from its beginning, as imports are
/// idempotent.
#[tauri::command(async)]
pub fn priority_save(list: Vec<String>, ctx: State<'_, Ctx>, k: State<'_, Knowledge>) -> Res<Vec<String>> {
    let list: Vec<String> = list.into_iter().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();
    let cfg = bragd_core::settings::save("source_priority", &Value::from(list.clone()))?;
    *ctx.cfg.write().unwrap_or_else(|e| e.into_inner()) = cfg;
    let _ = k.link.ask(&Request::Restart);
    Ok(list)
}

/// Every Slice, by the Game family it is about (`""` for any).
#[tauri::command(async)]
pub fn slices_list(ctx: State<'_, Ctx>) -> Res<BTreeMap<String, Vec<SliceView>>> {
    Ok(databases::all_slices(&ctx.cfg()))
}

#[tauri::command(async)]
pub fn slice_save(library: String, name: String, query: Value) -> Res<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(t!("slice.no_name"));
    }
    let lib = Library::open(Path::new(&library)).map_err(|e| e.to_string())?;
    lib.save_slice(name, &databases::query_in(query)?).map_err(|e| e.to_string())
}

/// "Keep as a separate copy": the new Library's path.
#[tauri::command(async)]
pub fn slice_materialise(library: String, name: String) -> Res<String> {
    databases::materialise(Path::new(&library), &name)
}

/// The key a Slice finds the position `fen` of `family` by, as text.
#[tauri::command(async)]
pub fn position_key(family: String, fen: String) -> Res<String> {
    databases::position_key(&family, &fen)
}

// ---------------------------------------------------------------- Sources

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceView {
    /// A rating list: its players go to the rating lists, not to a Library.
    pub rating_list: bool,
    pub id: String,
    pub name: String,
    pub family: String,
    pub family_name: String,
    /// About how much one fetch downloads.
    pub size: String,
    pub url: String,
    pub fetchable: bool,
    pub custom: bool,
    /// Why it cannot be fetched yet, or empty.
    pub notice: String,
    pub options: Options,
    /// Its disk cap as its field shows it; empty for none.
    pub disk_cap: String,
    /// The most one download may take, as its field shows it; empty for the default.
    pub max_download: String,
    /// The default for that field.
    pub max_download_default: String,
    /// A download over that, waiting for the user's yes; empty when none waits.
    pub too_big: String,
    /// When it was fetched last, and the last error.
    pub last: String,
    pub error: String,
    pub paused: bool,
}

/// A value and its label, for a `<select>` or a row of checkboxes.
#[derive(Serialize)]
pub struct Pick {
    pub value: Value,
    pub label: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcesPage {
    pub sources: Vec<SourceView>,
    /// The Libraries a Route can go to, by name: `main` and every other in the Library folder.
    pub libraries: Vec<String>,
    pub schedules: Vec<Pick>,
    pub speeds: Vec<Pick>,
    pub rating_of: Vec<Pick>,
    pub after_import: Vec<Pick>,
    pub results: Vec<Pick>,
}

fn picks(prefix: &str, values: &[&str]) -> Vec<Pick> {
    values.iter().map(|v| Pick { value: Value::from(*v), label: t!(&format!("{prefix}.{v}")) }).collect()
}

/// When, as the page says it: today, yesterday, N days ago.
fn ago(then: Option<u64>, now: u64) -> String {
    match then.map(|t| now.saturating_sub(t) / 86_400) {
        None => t!("source.never"),
        Some(0) => t!("source.today"),
        Some(d) => bragd_core::tn!("source.days_ago", d),
    }
}

fn sources_of(k: &Knowledge) -> Res<Vec<(sources::About, Options, sources::Memo)>> {
    match k.link.ask(&Request::Sources)? {
        Reply::Sources { sources } => Ok(sources),
        _ => Err(t!("helper.odd_reply")),
    }
}

#[tauri::command(async)]
pub fn sources_list(k: State<'_, Knowledge>, ctx: State<'_, Ctx>) -> Res<SourcesPage> {
    let now = bragd_core::fs::now();
    let sources = sources_of(&k)?
        .into_iter()
        .map(|(a, o, m)| SourceView {
            family_name: if a.family.is_empty() { t!("source.any_family") } else { t!(&format!("family.{}", a.family)) },
            size: if a.size == 0 { t!("source.size_unknown") } else { t!("source.size", "size" => bragd_core::i18n::bytes(a.size)) },
            notice: a.notice.as_deref().map(|n| t!(n, "source" => a.name)).unwrap_or_default(),
            custom: a.id.starts_with(sources::CUSTOM),
            rating_list: a.kind == sources::Kind::RatingList,
            last: ago(m.last_fetch, now),
            error: m.last_error.clone().unwrap_or_default(),
            paused: m.paused,
            disk_cap: o.disk_cap.map(limit_text).unwrap_or_default(),
            max_download: o.max_download.map(limit_text).unwrap_or_default(),
            max_download_default: bragd_core::i18n::bytes(sources::MAX_DOWNLOAD),
            too_big: m.too_big.map(|size| t!("source.too_big", "size" => bragd_core::i18n::bytes(size), "cap" => bragd_core::i18n::bytes(sources::cap(&o, &m)))).unwrap_or_default(),
            id: a.id,
            name: a.name,
            family: a.family,
            url: a.url,
            fetchable: a.fetchable,
            options: o,
        })
        .collect();
    let mut libraries: Vec<String> = databases::library_folder(&ctx.cfg())
        .and_then(|d| std::fs::read_dir(d).ok())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "bragd"))
        .filter_map(|p| Some(p.file_stem()?.to_string_lossy().into_owned()))
        .chain([sources::MAIN.to_string()])
        .collect();
    libraries.sort();
    libraries.dedup();
    Ok(SourcesPage {
        sources,
        libraries,
        schedules: picks("source.schedule", &["daily", "weekly", "monthly", "off"]),
        speeds: picks("strip.speed", &["bullet", "blitz", "rapid", "classical", "correspondence"]),
        rating_of: picks("strip.rating_of", &["both", "either", "average"]),
        after_import: picks("source.after_import", &["keep", "strip", "delete"]),
        results: ["1-0", "0-1", "1/2-1/2", "*"].iter().map(|r| Pick { value: Value::from(*r), label: (*r).into() }).collect(),
    })
}

/// A Source's options as the page edits them, with its disk cap and its largest download as
/// typed when they were in sight; a Route's Strip rule that keeps everything is none.
#[tauri::command(async)]
pub fn source_set(id: String, mut options: Options, disk_cap: Option<String>, max_download: Option<String>, k: State<'_, Knowledge>) -> Res<()> {
    bragd_core::tips::used("helper");
    for r in &mut options.routes {
        r.strip = r.strip.take().filter(|s| !s.is_empty());
        r.library = r.library.trim().to_string();
    }
    if let Some(cap) = disk_cap {
        options.disk_cap = Some(parse_size(&cap)?).filter(|&c| c > 0);
    }
    if let Some(cap) = max_download {
        options.max_download = Some(parse_size(&cap)?).filter(|&c| c > 0);
    }
    k.link.ask(&Request::SetSource { source: id, options }).map(drop)
}

/// A new custom URL Source, on and weekly.
#[tauri::command(async)]
pub fn source_add(name: String, url: String, k: State<'_, Knowledge>) -> Res<()> {
    let name: String = name.trim().chars().filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' ')).collect();
    if name.is_empty() {
        return Err(t!("source.no_name"));
    }
    let options = Options { enabled: true, schedule: Schedule::Weekly, only_unmetered: false, disk_cap: None, max_download: None, url: url.trim().into(), routes: Route::only_main(), after_import: Default::default(), auto: Auto::default() };
    k.link.ask(&Request::SetSource { source: format!("{}{name}", sources::CUSTOM), options }).map(drop)
}

/// A custom URL Source removed, with its downloads; its Games stay in the Libraries.
#[tauri::command(async)]
pub fn source_remove(id: String, k: State<'_, Knowledge>) -> Res<()> {
    k.link.ask(&Request::RemoveSource { source: id }).map(drop)
}

#[tauri::command(async)]
pub fn source_fetch(id: String, k: State<'_, Knowledge>) -> Res<u64> {
    k.link.start(&Request::FetchNow { source: id })
}

/// The user's yes to a download over its Source's cap: fetched now, that once.
#[tauri::command(async)]
pub fn source_allow(id: String, k: State<'_, Knowledge>) -> Res<u64> {
    k.link.start(&Request::AllowDownload { source: id })
}

/// The Source's Strip rule as a job: a preview (`dry_run`) that changes nothing, or applied.
#[tauri::command(async)]
pub fn source_strip(id: String, dry_run: bool, k: State<'_, Knowledge>) -> Res<u64> {
    k.link.start(&if dry_run { Request::StripDryRun { source: id } } else { Request::StripApply { source: id } })
}

// ---------------------------------------------------------------- Disk use

/// `5 GB`, `500MB`, `1.5 TB`, `1200000` (bytes): bytes, in powers of 1000 as `bragd_core::i18n::bytes`
/// writes them. Empty or 0 is no limit.
pub fn parse_size(text: &str) -> Res<u64> {
    // As `i18n::bytes` writes it in any language: a decimal comma, or Persian digits and mark.
    let s: String = text
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| match c {
            ',' | '\u{066B}' => '.',
            '۰'..='۹' => char::from_digit(c as u32 - '۰' as u32, 10).unwrap_or(c),
            c => c,
        })
        .collect();
    if s.is_empty() {
        return Ok(0);
    }
    let split = s.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(s.len());
    let (n, unit) = s.split_at(split);
    let n: f64 = n.parse().map_err(|_| t!("disk.bad_size", "text" => text))?;
    let mult = match unit.trim() {
        "" | "b" => 1u64,
        "k" | "kb" => 1_000,
        "m" | "mb" => 1_000_000,
        "g" | "gb" => 1_000_000_000,
        "t" | "tb" => 1_000_000_000_000,
        _ => return Err(t!("disk.bad_size", "text" => text)),
    };
    Ok((n * mult as f64).round() as u64)
}

/// A limit as the page shows it in its field: empty for none.
fn limit_text(n: u64) -> String {
    if n == 0 { String::new() } else { bragd_core::i18n::bytes(n) }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemView {
    pub id: String,
    pub label: String,
    pub size: String,
    pub path: String,
    pub external: bool,
    pub limit: String,
    pub over: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskView {
    pub items: Vec<ItemView>,
    pub total: String,
    pub total_limit: String,
    pub free_below: String,
    /// Each disk Bragd keeps things on, with its free space.
    pub free: Vec<(String, String)>,
    pub notices: Vec<String>,
    /// The Sources that can be fetched, with what Disk watch may do for each by itself.
    pub sources: Vec<(String, String, Options)>,
}

#[tauri::command(async)]
pub fn disk_report(k: State<'_, Knowledge>) -> Res<DiskView> {
    bragd_core::tips::used("disk");
    let r = match k.link.ask(&Request::DiskReport)? {
        Reply::Disk { report } => report,
        _ => return Err(t!("helper.odd_reply")),
    };
    let sources = sources_of(&k).unwrap_or_default().into_iter().filter(|(a, _, _)| a.fetchable).map(|(a, o, _)| (a.id, a.name, o)).collect();
    Ok(DiskView {
        items: r
            .items
            .iter()
            .map(|i| ItemView {
                label: i.label(),
                size: bragd_core::i18n::bytes(i.bytes),
                path: i.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| t!("disk.runtime")),
                external: i.external,
                limit: i.limit.map(limit_text).unwrap_or_default(),
                over: i.limit.is_some_and(|l| i.bytes > l),
                id: i.id.clone(),
            })
            .collect(),
        total: bragd_core::i18n::bytes(r.total),
        total_limit: limit_text(r.limits.total),
        free_below: limit_text(r.limits.free_below),
        free: r.free.iter().map(|f| (f.path.display().to_string(), f.bytes.map(bragd_core::i18n::bytes).unwrap_or_else(|| t!("disk.unknown")))).collect(),
        notices: r.crossed.iter().map(disk::Crossed::text).collect(),
        sources,
    })
}

/// Disk watch's limits as typed: the total, the free space to keep, and each item's.
#[tauri::command(async)]
pub fn disk_limits_set(total: String, free_below: String, items: BTreeMap<String, String>, k: State<'_, Knowledge>) -> Res<()> {
    let limits = DiskConfig {
        total: parse_size(&total)?,
        free_below: parse_size(&free_below)?,
        items: items.iter().map(|(id, v)| Ok((id.clone(), parse_size(v)?))).collect::<Res<_>>()?,
    };
    k.link.ask(&Request::SetDiskLimits { limits }).map(drop)
}

/// Every Source's Strip rule over the default Library, as a job: a preview, or applied.
#[tauri::command(async)]
pub fn library_prune(dry_run: bool, k: State<'_, Knowledge>) -> Res<u64> {
    k.link.start(&Request::PruneLibrary { library: None, dry_run })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_read_as_typed() {
        assert_eq!(parse_size("5 GB").unwrap(), 5_000_000_000);
        assert_eq!(parse_size("1.5tb").unwrap(), 1_500_000_000_000);
        assert_eq!(parse_size("1,5 GB").unwrap(), 1_500_000_000);
        assert_eq!(parse_size("1200").unwrap(), 1200);
        assert_eq!(parse_size("۱٫۵ GB").unwrap(), 1_500_000_000, "as Persian writes it");
        assert_eq!(parse_size(" ").unwrap(), 0);
        assert!(parse_size("lots").is_err());
        assert!(parse_size("5 parsecs").is_err());
        // What the page shows reads back as the same limit.
        assert_eq!(parse_size(&limit_text(30_000_000_000)).unwrap(), 30_000_000_000);
    }

    #[test]
    fn save_as_waits_before_writing_over_a_download() {
        let dir = bragd_helper::folder().unwrap().join("sources").join("save-test");
        std::fs::create_dir_all(&dir).unwrap();
        let (from, to) = (dir.join("a.pgn"), dir.join("b.pgn"));
        std::fs::write(&from, "new").unwrap();
        std::fs::write(&to, "old").unwrap();
        assert!(save_to(&from, &to, false).unwrap().confirm);
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "old");
        // Only the file Save as… asked about is written over, and only once.
        let to_confirm = Mutex::new(None);
        assert!(save_confirmed(&from, &to_confirm).is_err());
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "old");
        *to_confirm.lock().unwrap() = Some(to.clone());
        assert!(!save_confirmed(&from, &to_confirm).unwrap().confirm);
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "new");
        assert!(save_confirmed(&from, &to_confirm).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
