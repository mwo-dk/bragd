//! Databases: a Foreign database read in place (`bragd_library::chess::Table`, or a go
//! `Collection`), the Libraries in the Library folder, their Slices, and a Game of either laid
//! out for replay. The page asks for the rows it shows (a window of a long list) and for one
//! Game; the open file stays open here while the window runs, so leaving the page keeps it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use bragd_core::config::Config;
use bragd_core::t;
use bragd_helper::disk;
use bragd_library::chess::{self, Format, Query, Table, pgn};
use bragd_library::go::Collection;
use bragd_library::library::{Library, Stored, text_key};
use bragd_library::slice;
use serde::Serialize;
use serde_json::Value;

/// A file open as a Foreign database, and the last list asked of it.
pub struct OpenFile {
    path: PathBuf,
    reader: Reader,
    /// The query, how many games were read when it was answered, and its answer.
    last: Option<(Query, usize, Vec<u32>)>,
}

enum Reader {
    Table(Table),
    Go(Collection),
}

#[derive(Default)]
pub struct Databases {
    pub file: Mutex<Option<OpenFile>>,
    /// The last list asked of a Library: its file, the query, and the Games' ids.
    last: Mutex<Option<(PathBuf, slice::Query, Vec<i64>)>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The open file as the page shows it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileView {
    pub path: String,
    pub name: String,
    /// The Game family its Games belong to.
    pub family: String,
    pub family_name: String,
    /// Downloaded (a Source's download, or in the Downloads folder): writing over it is
    /// discouraged.
    pub downloaded: bool,
    pub games: usize,
    pub done: bool,
    /// How far the reading has come, 0 to 1.
    pub read: f64,
    /// A line on how far: `1,204 Games read`, `stopped at the limit`, an error.
    pub status: String,
}

/// A Game as a list shows it.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RowView {
    /// What the page sends back to open it: the index in the file, or the Library's id.
    pub id: i64,
    /// Its place in the file or the Library, from 1.
    pub n: i64,
    pub white: String,
    pub black: String,
    pub white_elo: String,
    pub black_elo: String,
    pub result: String,
    pub date: String,
    pub event: String,
    pub eco: String,
    pub opening: String,
    pub moves: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rows {
    pub total: usize,
    pub rows: Vec<RowView>,
}

/// A Game laid out for replay: a step for the start and one for each move of the main line.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Replay {
    pub title: String,
    pub family: String,
    /// The Variant (`bragd-games`' key) whose board draws each step's position; empty when its
    /// positions cannot be drawn, and only the moves are listed.
    pub variant: String,
    pub headers: Vec<(String, String)>,
    pub steps: Vec<Step>,
    pub result: String,
    /// The first move that could not be played.
    pub error: String,
    /// ChessBase's evaluation profile as a line to draw (`Evp`).
    pub evp: Option<Evp>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    /// `12. Nf3`, `12… Nf6`; empty for the start.
    pub label: String,
    /// The position after it, in the Family's notation.
    pub fen: Option<String>,
    pub comment: String,
    /// Where it is on the evaluation graph, from 0 to `Evp::width`.
    pub evp_x: Option<f64>,
}

/// An evaluation profile as an SVG polyline in a `width` × `height` box, White's advantage up;
/// the middle line is equality.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evp {
    pub width: f64,
    pub height: f64,
    pub points: String,
}

const EVP_W: f64 = 240.0;
const EVP_H: f64 = 48.0;

pub fn family_of(format: Format) -> &'static str {
    match format {
        Format::Pdn => "draughts",
        Format::Xiangqi => "xiangqi",
        Format::Kif | Format::Csa => "shogi",
        Format::Sgf => "go",
        _ => "chess",
    }
}

/// Whether `path` was downloaded: a Source's download, or a file in the Downloads folder.
pub fn downloaded(path: &Path) -> bool {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let helper = bragd_helper::folder();
    let downloads = bragd_core::dirs::home_dir().map(|h| h.join("Downloads"));
    [helper, downloads].into_iter().flatten().any(|d| path.starts_with(d.canonicalize().unwrap_or(d)))
}

fn file_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

impl OpenFile {
    pub fn open(path: &Path) -> Result<OpenFile, String> {
        let reader = if bragd_library::go::is_sgf(path) {
            Reader::Go(Collection::open(path).map_err(|e| e.to_string())?)
        } else {
            Reader::Table(Table::open(path).map_err(|e| e.to_string())?)
        };
        Ok(OpenFile { path: path.to_path_buf(), reader, last: None })
    }

    fn family(&self) -> &'static str {
        match &self.reader {
            Reader::Table(t) => family_of(t.format),
            Reader::Go(_) => "go",
        }
    }

    pub fn view(&self) -> FileView {
        let p = match &self.reader {
            Reader::Table(t) => t.progress(),
            Reader::Go(c) => c.progress(),
        };
        let status = if let Some(e) = &p.error {
            t!("db.read_error", "why" => e)
        } else if p.capped {
            t!("db.capped", "games" => p.games)
        } else if p.done {
            bragd_core::tn!("db.games", p.games)
        } else {
            t!("db.reading", "games" => p.games)
        };
        let family = self.family();
        FileView {
            path: self.path.display().to_string(),
            name: file_name(&self.path),
            family: family.into(),
            family_name: t!(&format!("family.{family}")),
            downloaded: downloaded(&self.path),
            games: p.games,
            done: p.done,
            read: if p.done || p.size == 0 { 1.0 } else { p.read as f64 / p.size as f64 },
            status,
        }
    }

    /// The rows `from`.. of the games `q` lets through, at most `count` of them.
    pub fn rows(&mut self, q: &Query, from: usize, count: usize) -> Rows {
        match &self.reader {
            Reader::Table(t) => {
                let games = t.progress().games;
                if !matches!(&self.last, Some((lq, n, _)) if lq == q && *n == games) {
                    self.last = Some((q.clone(), games, t.query(q)));
                }
                let ids = self.last.as_ref().map(|l| &l.2[..]).unwrap_or_default();
                let rows = ids.iter().skip(from).take(count).filter_map(|&i| t.row(i)).map(|r| RowView {
                    id: r.n as i64 - 1,
                    n: r.n as i64,
                    white: r.white,
                    black: r.black,
                    white_elo: r.white_elo.map(|e| e.to_string()).unwrap_or_default(),
                    black_elo: r.black_elo.map(|e| e.to_string()).unwrap_or_default(),
                    result: r.result,
                    date: r.date,
                    event: r.event,
                    eco: r.eco,
                    opening: r.opening,
                    moves: r.moves,
                });
                Rows { total: ids.len(), rows: rows.collect() }
            }
            Reader::Go(c) => {
                // shortcut: a go file is filtered by player only and not sorted; enough for the
                // archives of a few thousand games read so far.
                let player = q.player.trim().to_lowercase();
                let all: Vec<_> = c.games(0, usize::MAX).into_iter().filter(|g| player.is_empty() || g.white.to_lowercase().contains(&player) || g.black.to_lowercase().contains(&player)).collect();
                let rows = all.iter().skip(from).take(count).map(|g| RowView {
                    id: g.n as i64 - 1,
                    n: g.n as i64,
                    white: g.white.clone(),
                    black: g.black.clone(),
                    white_elo: g.white_rank.clone(),
                    black_elo: g.black_rank.clone(),
                    result: g.result.clone(),
                    date: g.date.clone(),
                    event: g.event.clone(),
                    eco: String::new(),
                    opening: g.name.clone(),
                    moves: g.moves,
                });
                Rows { total: all.len(), rows: rows.collect() }
            }
        }
    }

    /// Game `i` (from 0) for replay.
    pub fn game(&self, i: u32) -> Result<Replay, String> {
        match &self.reader {
            Reader::Table(t) => Ok(replay(&t.game(i).map_err(|e| e.to_string())?, family_of(t.format))),
            Reader::Go(c) => {
                // shortcut: go Games are listed with their headers only; replay them on the go
                // board once the go reader hands over positions bragd-games can draw.
                let g = c.info(i as usize).ok_or_else(|| t!("db.no_game"))?;
                Ok(Replay {
                    title: format!("{} – {}", g.black, g.white),
                    family: "go".into(),
                    headers: vec![("Black".into(), g.black), ("White".into(), g.white), ("Result".into(), g.result.clone()), ("Date".into(), g.date), ("Event".into(), g.event)],
                    result: g.result,
                    error: t!("db.no_board"),
                    ..Default::default()
                })
            }
        }
    }
}

/// The Variant whose board draws a Game of `family` with `variant_tag`.
fn board_variant(family: &str, variant_tag: Option<&str>) -> &'static str {
    let tag: String = variant_tag.unwrap_or("").chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect();
    match family {
        "chess" if tag.contains("960") || tag.contains("fischer") => "chess960",
        "chess" if tag.is_empty() || tag == "standard" || tag == "fromposition" => "chess",
        "shogi" => "shogi",
        "xiangqi" => "xiangqi",
        _ => "",
    }
}

fn title(tags: &[(String, String)]) -> String {
    let tag = |k: &str| tags.iter().find(|(n, _)| n.eq_ignore_ascii_case(k)).map(|(_, v)| v.trim()).filter(|v| !v.is_empty() && *v != "?").unwrap_or("?");
    format!("{} – {}", tag("White"), tag("Black"))
}

fn comment(before: &str, after: &str) -> String {
    [before.trim(), after.trim()].iter().filter(|c| !c.is_empty()).copied().collect::<Vec<_>>().join(" ")
}

/// `g`'s main line laid out for replay, with its comments and evaluation profile.
pub fn replay(g: &pgn::Game, family: &str) -> Replay {
    let line = g.main_line();
    let profile = g.eval_profile.as_ref().filter(|p| p.centipawns.len() > 1);
    // The ply each point stands for: the profile's start, then one a ply.
    let x_of = |ply: usize| profile.and_then(|p| ply.checked_sub(p.start_ply as usize)).filter(|&i| profile.is_some_and(|p| i < p.centipawns.len())).map(|i| i as f64 * EVP_W / (profile.map_or(1, |p| p.centipawns.len()) - 1) as f64);
    let mut steps = vec![Step { label: String::new(), fen: Some(g.start.clone()), comment: g.comment.trim().to_string(), evp_x: x_of(0) }];
    for (ply, &i) in line.iter().enumerate() {
        let n = &g.nodes[i as usize];
        let label = if n.white { format!("{}. {}", n.number, n.san) } else { format!("{}… {}", n.number, n.san) };
        let nags = chess::nags(&n.nags).0;
        steps.push(Step { label: format!("{label}{nags}"), fen: n.fen.clone(), comment: comment(&n.before, &n.after), evp_x: x_of(ply + 1) });
    }
    let variant = board_variant(family, g.tag("Variant"));
    // A board only when the start replays there; else the moves alone.
    let variant = if !variant.is_empty() && bragd_games::replay::<&str>(variant, Some(&g.start), &[]).is_ok() { variant } else { "" };
    let evp = profile.map(|p| {
        let n = p.centipawns.len();
        let points = p
            .centipawns
            .iter()
            .enumerate()
            .map(|(i, &cp)| {
                let v = cp.clamp(-pgn::EVAL_CAP, pgn::EVAL_CAP) as f64 / pgn::EVAL_CAP as f64;
                format!("{:.1},{:.1}", i as f64 * EVP_W / (n - 1) as f64, EVP_H / 2.0 * (1.0 - v))
            })
            .collect::<Vec<_>>()
            .join(" ");
        Evp { width: EVP_W, height: EVP_H, points }
    });
    Replay {
        title: title(&g.tags),
        family: family.into(),
        variant: variant.into(),
        headers: g.tags.clone(),
        steps,
        result: g.result.clone(),
        error: g.error.clone().map(|e| t!("db.unplayable", "move" => e)).unwrap_or_default(),
        evp,
    }
}

/// A Game from a Library, laid out for replay. Chess Games are read again as PGN, so each step
/// has its position; the other Families list their moves.
pub fn stored_replay(s: &Stored) -> Replay {
    let headers: Vec<(String, String)> = s.headers.iter().map(|(k, v)| (k.clone(), v.as_str().map_or_else(|| v.to_string(), String::from))).collect();
    if s.family == "chess" {
        let mut text: String = headers.iter().filter(|(k, _)| !k.eq_ignore_ascii_case("FEN")).map(|(k, v)| format!("[{k} \"{}\"]\n", v.replace('"', "'"))).collect();
        if let Some(fen) = &s.start {
            text.push_str(&format!("[FEN \"{fen}\"]\n[SetUp \"1\"]\n"));
        }
        text.push('\n');
        for (i, m) in s.moves.iter().enumerate() {
            if i % 2 == 0 {
                text.push_str(&format!("{}. ", i / 2 + 1));
            }
            text.push_str(m);
            text.push(' ');
        }
        text.push_str(&s.result);
        let mut r = replay(&pgn::parse(&text), "chess");
        if s.variant != "standard" {
            r.variant = board_variant("chess", Some(&s.variant)).into();
        }
        return r;
    }
    let mut steps = vec![Step::default()];
    steps.extend(s.moves.iter().enumerate().map(|(i, m)| Step { label: format!("{}. {m}", i + 1), ..Default::default() }));
    Replay { title: title(&headers), family: s.family.clone(), headers, steps, result: s.result.clone(), error: t!("db.no_board"), ..Default::default() }
}

// ---------------------------------------------------------------- Libraries

/// The folder Libraries are kept in.
pub fn library_folder(cfg: &Config) -> Option<PathBuf> {
    bragd_helper::default_library(cfg)?.parent().map(Path::to_path_buf)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryView {
    pub path: String,
    pub name: String,
    pub games: u64,
    pub size: String,
    /// Games merged from more than one Source (at most `REVIEW`).
    pub duplicates: usize,
    pub near_duplicates: usize,
    pub slices: Vec<SliceView>,
    pub error: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliceView {
    pub library: String,
    pub name: String,
    /// Its query, with the position key as text (JavaScript numbers lose a u64).
    pub query: Value,
    pub materialised: bool,
    pub path: String,
    /// The Game family it is about; empty for any.
    pub family: String,
}

/// The most Duplicates and Near-duplicates the review lists.
const REVIEW: usize = 500;

/// Every `*.bragd` file in the Library folder, by name.
pub fn libraries(cfg: &Config) -> Vec<LibraryView> {
    let Some(dir) = library_folder(cfg) else { return vec![] };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "bragd")).collect();
    files.sort();
    files
        .iter()
        .map(|p| {
            let mut v = LibraryView { path: p.display().to_string(), name: file_name(p), games: 0, size: disk::bytes(disk::size(p)), duplicates: 0, near_duplicates: 0, slices: vec![], error: String::new() };
            match Library::open(p) {
                Ok(lib) => {
                    v.games = lib.count().unwrap_or(0);
                    v.size = disk::bytes(lib.size());
                    v.duplicates = lib.merged(REVIEW).map_or(0, |m| m.len());
                    v.near_duplicates = lib.near_duplicate_count().map_or(0, |n| n as usize);
                    v.slices = lib.slices().unwrap_or_default().into_iter().map(|s| slice_view(p, s)).collect();
                }
                Err(e) => v.error = e.to_string(),
            }
            v
        })
        .collect()
}

pub fn slice_view(library: &Path, s: slice::Slice) -> SliceView {
    SliceView {
        library: library.display().to_string(),
        family: s.query.family.clone().unwrap_or_default(),
        query: query_out(&s.query),
        name: s.name,
        materialised: s.materialised,
        path: s.path.map(|p| p.display().to_string()).unwrap_or_default(),
    }
}

/// A Slice's query for the page: the position key as text.
pub fn query_out(q: &slice::Query) -> Value {
    let mut v = serde_json::to_value(q).unwrap_or_default();
    v["position"] = q.position.map_or(Value::Null, |p| Value::String(p.to_string()));
    v
}

/// A Slice's query from the page: empty texts are no condition.
pub fn query_in(mut v: Value) -> Result<slice::Query, String> {
    let position = v.get("position").and_then(|p| p.as_str().map(String::from).or_else(|| p.as_u64().map(|n| n.to_string())));
    v["position"] = Value::Null;
    if let Some(o) = v.as_object_mut() {
        o.retain(|_, x| !matches!(x, Value::String(s) if s.trim().is_empty()));
    }
    let mut q: slice::Query = serde_json::from_value(v).map_err(|e| e.to_string())?;
    q.players.retain(|p| !p.trim().is_empty());
    q.position = position.filter(|p| !p.is_empty()).map(|p| p.parse().map_err(|_| t!("db.bad_position"))).transpose()?;
    Ok(q)
}

/// The key a Library finds a position by (`slice::Query::position`): Polyglot for chess, the
/// hash of its notation for the other Families the readers give positions for.
pub fn position_key(family: &str, fen: &str) -> Result<String, String> {
    if family == "chess" {
        let b = pgn::board(fen).ok_or_else(|| t!("db.bad_position"))?;
        return Ok(chess::polyglot::key(&b).to_string());
    }
    Ok(text_key(fen).to_string())
}

fn row_of(lib: &Library, id: i64) -> RowView {
    let h = lib.headers(id).unwrap_or_default();
    let get = |k: &str| h.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let s = lib.stored(id).ok();
    RowView {
        id,
        n: id,
        white: get("White"),
        black: get("Black"),
        white_elo: get("WhiteElo"),
        black_elo: get("BlackElo"),
        result: s.as_ref().map(|s| s.result.clone()).filter(|r| !r.is_empty()).unwrap_or_else(|| get("Result")),
        date: get("Date"),
        event: get("Event"),
        eco: get("ECO"),
        opening: get("Opening"),
        moves: s.map_or(0, |s| s.moves.len().div_ceil(2) as u32),
    }
}

impl Databases {
    /// The Games of the Library at `path` that `q` lets through, rows `from`.., at most `count`.
    pub fn library_rows(&self, path: &Path, q: &slice::Query, from: usize, count: usize) -> Result<Rows, String> {
        let lib = Library::open(path).map_err(|e| e.to_string())?;
        let mut last = lock(&self.last);
        // Asked again from the first row: the Library may have grown.
        if from == 0 || !matches!(&*last, Some((p, lq, _)) if p == path && lq == q) {
            *last = Some((path.to_path_buf(), q.clone(), lib.find(q).map_err(|e| e.to_string())?));
        }
        let ids = last.as_ref().map(|l| &l.2[..]).unwrap_or_default();
        Ok(Rows { total: ids.len(), rows: ids.iter().skip(from).take(count).map(|&id| row_of(&lib, id)).collect() })
    }
}

/// The Duplicates and Near-duplicates of a Library, for review.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    /// Games merged from several Sources, with their Sources.
    pub duplicates: Vec<(RowView, Vec<String>)>,
    /// The shorter Game, then the longer one.
    pub near: Vec<(RowView, RowView)>,
}

pub fn review(path: &Path) -> Result<Review, String> {
    let lib = Library::open(path).map_err(|e| e.to_string())?;
    let e = |e: bragd_library::library::Error| e.to_string();
    let duplicates = lib
        .merged(REVIEW)
        .map_err(e)?
        .into_iter()
        .map(|id| (row_of(&lib, id), lib.sources(id).unwrap_or_default().into_iter().map(|(s, r, _)| format!("{s}: {}", r.rsplit(['/', '\\']).next().unwrap_or(&r))).collect()))
        .collect();
    let near = lib.near_duplicates(REVIEW).map_err(e)?.into_iter().map(|(a, b)| (row_of(&lib, a), row_of(&lib, b))).collect();
    Ok(Review { duplicates, near })
}

/// Keeps a Slice as a separate copy: `<name>.bragd` beside its Library.
pub fn materialise(library: &Path, name: &str) -> Result<String, String> {
    let lib = Library::open(library).map_err(|e| e.to_string())?;
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | ' ') { c } else { '_' }).collect();
    let out = library.with_file_name(format!("{}.bragd", safe.trim()));
    lib.materialise(name, &out).map_err(|e| e.to_string())?;
    Ok(out.display().to_string())
}

/// Every Slice of every Library, for the sidebar.
pub fn all_slices(cfg: &Config) -> BTreeMap<String, Vec<SliceView>> {
    let mut out: BTreeMap<String, Vec<SliceView>> = BTreeMap::new();
    for l in libraries(cfg) {
        for s in l.slices {
            out.entry(s.family.clone()).or_default().push(s);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_game_is_laid_out_with_its_comments_and_profile() {
        let g = pgn::parse("[White \"A\"]\n[Black \"B\"]\n\n{[%evp 0,20,-40,1500] Start} 1. e4 {Good} e5 2. Nf3 *");
        let r = replay(&g, "chess");
        assert_eq!(r.title, "A – B");
        assert_eq!(r.variant, "chess");
        assert_eq!(r.steps.len(), 4);
        assert_eq!(r.steps[1].label, "1. e4");
        assert_eq!(r.steps[1].comment, "Good");
        assert_eq!(r.steps[2].label, "1… e5");
        assert!(r.steps[3].fen.as_deref().unwrap().starts_with("rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2"));
        let evp = r.evp.unwrap();
        // From the start position, a point a ply: the last capped at the top.
        assert_eq!(evp.points.split(' ').count(), 3);
        assert!(evp.points.ends_with(",0.0"), "{}", evp.points);
        assert_eq!((r.steps[0].evp_x, r.steps[2].evp_x, r.steps[3].evp_x), (Some(0.0), Some(EVP_W), None));
    }

    #[test]
    fn a_stored_chess_game_replays_on_the_board() {
        let s = Stored { id: 1, family: "chess".into(), variant: "standard".into(), moves: vec!["e4".into(), "e5".into(), "Qh5".into()], result: "*".into(), ..Default::default() };
        let r = stored_replay(&s);
        assert_eq!((r.variant.as_str(), r.steps.len()), ("chess", 4));
        assert_eq!(r.steps[3].label, "2. Qh5");
        let d = Stored { family: "draughts".into(), moves: vec!["32-28".into()], ..s };
        let r = stored_replay(&d);
        assert_eq!((r.variant.as_str(), r.steps.len()), ("", 2));
    }

    #[test]
    fn a_slice_query_keeps_its_position_whole() {
        let q = slice::Query { players: vec!["Morphy".into()], position: Some(u64::MAX - 1), ..Default::default() };
        let mut v = query_out(&q);
        assert_eq!(v["position"], Value::String((u64::MAX - 1).to_string()));
        v["eco_from"] = Value::String(" ".into());
        assert_eq!(query_in(v).unwrap(), q);
        assert!(position_key("chess", "not a fen").is_err());
        assert_eq!(position_key("chess", pgn::START).unwrap(), "5060803636482931868");
    }

    #[test]
    fn downloads_are_told_apart() {
        let helper = bragd_helper::folder().unwrap().join("sources").join("twic").join("twic1600g.zip");
        assert!(downloaded(&helper));
        assert!(!downloaded(Path::new("/nowhere/games.pgn")));
    }
}
