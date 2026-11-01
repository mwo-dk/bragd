//! Analyse: an engine's best lines for a position, as they come, in the Variant's notation and
//! scored from the first Side's view; a whole Game reviewed with its moves annotated; and for
//! chess the opening's name, the opening books' moves and what the tablebases say. Both read
//! the Position store first and write what the Engine finds to it, so a position judged once
//! shows at once.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use bragd_core::config::Config;
use bragd_engines::lichess;
use bragd_engines::protocol::{Info, Limit, Score};
use bragd_engines::review::{Ply, Reviewer, Setup};
use bragd_engines::seam::{self, Notation};
use bragd_engines::session::{Session, think};
use bragd_engines::syzygy::{Tablebases, material};
use bragd_games::{State, Variant};
use bragd_library::chess::{openings, polyglot};
use bragd_positions::{Annotation, Evaluation, Mark, Store};
use serde::Serialize;

use crate::engines;
use bragd_core::i18n::tr;

/// A Game being played online, as far as the fair-play guard needs it.
#[derive(Clone, Copy, Debug, Default)]
pub struct LiveGame {
    pub rated: bool,
    /// The other Player is a person, not an Engine.
    pub against_person: bool,
    pub over: bool,
}

/// Fair play: no engine help (Analyse, book moves, tablebases) while a rated online Game
/// against a person is under way in any window.
pub fn locked(live: &[LiveGame]) -> bool {
    live.iter().any(|g| g.rated && g.against_person && !g.over)
}

/// The online Games under way. Online play comes later, so there are none yet.
fn live_games() -> Vec<LiveGame> {
    vec![]
}

fn guard() -> Result<(), String> {
    if locked(&live_games()) { Err(tr("analyse.locked", &[])) } else { Ok(()) }
}

/// The Game asked about.
#[derive(Clone)]
struct Asked {
    variant: String,
    initial: Option<String>,
    entries: Vec<String>,
}

/// What the engine has said so far.
#[derive(Clone, Default)]
struct Raw {
    engine: String,
    /// Its lines by number, as it last gave each: side to move's view, engine notation.
    lines: Vec<Info>,
    notation: Option<Notation>,
    /// The second Side is to move: its scores are turned round.
    second: bool,
    /// Line 1 came from the Position store at this depth: the engine's own replaces it once it
    /// is as deep.
    floor: u32,
    done: bool,
    error: Option<String>,
    /// The last move's Annotation, from the store.
    note: Option<NoteView>,
}

impl Raw {
    /// Keeps what the engine said of one of its `wanted` lines.
    fn heard(&mut self, i: &Info, wanted: u32) {
        // A line number past the lines asked for is the engine's mistake (or an attack on
        // memory: `multipv 1000000000`), not a line to make room for.
        if i.line > wanted as usize {
            return;
        }
        if i.line <= 1 {
            if i.depth.unwrap_or(0) < self.floor {
                return;
            }
            self.floor = 0;
        }
        let at = i.line.max(1) - 1;
        if self.lines.len() <= at {
            self.lines.resize(at + 1, Info::default());
        }
        let keep = self.lines[at].clone();
        self.lines[at] = Info { score: i.score.or(keep.score), pv: if i.pv.is_empty() { keep.pv } else { i.pv.clone() }, ..i.clone() };
    }
}

/// Lines already turned into moves to show, by the engine's moves (at most `SHOWN`): each
/// is replayed from the Game's start, so a poll does that only for a line that changed.
type Shown = Vec<(Vec<String>, Vec<(String, String)>)>;

/// Each of `pvs` as moves to show: from `cache` when the poll before showed it, else by
/// `render`. `cache` then holds these lines only.
fn shown_lines(cache: &mut Shown, pvs: &[Vec<String>], render: impl Fn(&[String]) -> Vec<(String, String)>) -> Vec<Vec<(String, String)>> {
    let mut fresh = Shown::with_capacity(pvs.len());
    let lines = pvs
        .iter()
        .map(|pv| {
            let moves = match cache.iter().find(|(key, _)| key == pv) {
                Some((_, moves)) => moves.clone(),
                None => render(pv),
            };
            fresh.push((pv.clone(), moves.clone()));
            moves
        })
        .collect();
    *cache = fresh;
    lines
}

struct Live {
    asked: Asked,
    raw: Arc<Mutex<Raw>>,
    stop: Arc<AtomicBool>,
    shown: Arc<Mutex<Shown>>,
}

impl Drop for Live {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

struct Reviewing {
    reviewer: Reviewer,
    asked: Asked,
    notation: Notation,
}

#[derive(Default)]
pub struct Held {
    live: Mutex<Option<Live>>,
    review: Mutex<Option<Reviewing>>,
}

/// The most lines asked for.
const LINES: u32 = 3;
/// The most moves of a line shown.
const SHOWN: usize = 10;

/// Starts the engine `engine` (or the Variant's default) on the Game, for `lines` lines;
/// whatever ran before stops.
pub fn start(cfg: &Config, held: &Held, variant: &str, initial: Option<String>, entries: Vec<String>, engine: Option<String>, lines: Option<u32>) -> Result<(), String> {
    guard()?;
    *crate::lock(&held.live) = None;
    let key = engine.filter(|k| !k.is_empty()).or_else(|| engines::default_for(cfg, variant)).ok_or_else(|| tr("engines.none_for", &[]))?;
    let chosen = engines::resolve(cfg, &key, variant)?;
    let asked = Asked { variant: variant.into(), initial, entries };
    let raw = Arc::new(Mutex::new(Raw { engine: key.clone(), ..Default::default() }));
    let stop = Arc::new(AtomicBool::new(false));
    let (r, s, a, cfg, key) = (raw.clone(), stop.clone(), asked.clone(), cfg.clone(), key.clone());
    let wanted = lines.unwrap_or(LINES).clamp(1, bragd_engines::session::MAX_LINES);
    std::thread::spawn(move || {
        let halt = || s.load(Ordering::Relaxed);
        let result = (|| -> Result<(), String> {
            let (mut e, notation) = engines::start(&cfg, &chosen, &a.variant, &halt)?;
            let (pos, state) = seam::position(&a.variant, a.initial.as_deref(), &a.entries, notation)?;
            let second = state.to_move.is_some_and(|s| s.index == 1);
            {
                let mut raw = crate::lock(&r);
                raw.engine = e.about().name.clone();
                raw.notation = Some(notation);
                raw.second = second;
            }
            if state.to_move.is_none() {
                return Ok(());
            }
            let edition = bragd_engines::edition::of(&key, e.about(), &chosen.program);
            let (family, _, keys) = positions(&a.variant, a.initial.as_deref(), &a.entries)?;
            let mut store = Store::path().and_then(|p| Store::open(&p).ok());
            let at = keys[keys.len() - 1];
            if let Some(s) = &store {
                let mut raw = crate::lock(&r);
                if let Ok(Some(ev)) = s.shown(&family, &a.variant, at, &key) {
                    let score = Score::from(ev.score);
                    raw.lines = vec![Info { line: 1, depth: Some(ev.depth), score: Some(if second { score.flipped() } else { score }), pv: ev.line.split_whitespace().map(String::from).collect(), ..Default::default() }];
                    raw.floor = ev.depth.max(1);
                }
                if let (Some(mv), Some(&before)) = (pos.moves.last(), keys.len().checked_sub(2).and_then(|i| keys.get(i)))
                    && let Ok(Some(note)) = s.annotation(&family, &a.variant, before, mv, &key)
                {
                    let n = a.entries.len() - 1;
                    raw.note = note_view(&state, &note, n, &a.variant, a.initial.as_deref(), &a.entries[..n], notation);
                }
            }
            // Until stopped, or the engine's own cap (`Limit::cap`).
            let report = &mut |i: &Info| crate::lock(&r).heard(i, wanted);
            let thought = think(&mut e, &pos, Limit::Depth(99), wanted, &halt, report);
            // The engine's own line 1, however far it got, goes into the store.
            let mut raw = crate::lock(&r);
            if let (Some(s), Some(Info { depth: Some(depth), score: Some(score), pv, .. })) = (&mut store, raw.lines.first().filter(|_| raw.floor == 0))
                && !pv.is_empty()
            {
                let score = if second { score.flipped() } else { *score };
                let ev = Evaluation { family, variant: a.variant.clone(), key: at, edition, score: score.into(), depth: *depth, line: pv.join(" "), time: now_secs() };
                if let Err(e) = s.put(&ev) {
                    raw.error = Some(e.to_string());
                }
            }
            let a = thought.map_err(|e| e.to_string())?;
            // An engine that reports no lines (GNU Go) still names its move.
            if raw.lines.is_empty()
                && let Some(b) = a.best
            {
                raw.lines.push(Info { line: 1, pv: vec![b], ..Default::default() });
            }
            Ok(())
        })();
        let mut raw = crate::lock(&r);
        raw.done = true;
        if let Err(e) = result
            && !halt()
        {
            raw.error = Some(e);
        }
    });
    *crate::lock(&held.live) = Some(Live { asked, raw, stop, shown: Default::default() });
    Ok(())
}

pub fn stop(held: &Held) {
    if let Some(l) = crate::lock(&held.live).as_ref() {
        l.stop.store(true, Ordering::Relaxed);
    }
}

/// A move of a line: its entry and how a Player writes it.
#[derive(Clone, Serialize)]
pub struct LineMove {
    entry: String,
    notation: String,
}

#[derive(Serialize)]
pub struct LineView {
    /// From the first Side's view: `+0.35`, `#3`, `+2.5 (64%)`.
    score: String,
    depth: Option<u32>,
    moves: Vec<LineMove>,
}

#[derive(Serialize)]
pub struct Now {
    running: bool,
    engine: String,
    lines: Vec<LineView>,
    error: Option<String>,
    /// The last move's Annotation, when the store has one by this Engine.
    note: Option<NoteView>,
}

/// The analysis as far as it has come; None when none was started.
pub fn now(held: &Held) -> Option<Now> {
    // Copied out, so the replays below hold neither lock and the engine's reports never wait.
    let (asked, raw, shown) = {
        let live = crate::lock(&held.live);
        let l = live.as_ref()?;
        let raw = crate::lock(&l.raw).clone();
        (l.asked.clone(), raw, l.shown.clone())
    };
    let lines = raw
        .notation
        .map(|n| {
            let heard: Vec<&Info> = raw.lines.iter().filter(|i| !i.pv.is_empty()).collect();
            let pvs: Vec<Vec<String>> = heard.iter().map(|i| i.pv.iter().take(SHOWN).cloned().collect()).collect();
            let a = &asked;
            let moves = shown_lines(&mut crate::lock(&shown), &pvs, |pv| seam::line(&a.variant, a.initial.as_deref(), &a.entries, n, pv));
            heard
                .into_iter()
                .zip(moves)
                .map(|(i, moves)| {
                    let score = i.score.map(|s: Score| if raw.second { s.flipped() } else { s });
                    let moves = moves.into_iter().map(|(entry, notation)| LineMove { entry, notation }).collect();
                    LineView { score: score.map(Score::shown).unwrap_or_default(), depth: i.depth, moves }
                })
                .filter(|v| !v.moves.is_empty())
                .collect()
        })
        .unwrap_or_default();
    Some(Now { running: !raw.done, engine: raw.engine.clone(), lines, error: raw.error.clone(), note: raw.note })
}

/// The Game's Family, its positions in their Family's notation (before the first entry, then
/// after each) and their keys in the Position store.
fn positions(variant: &str, initial: Option<&str>, entries: &[String]) -> Result<(String, Vec<String>, Vec<u64>), String> {
    let family = bragd_games::variant(variant).map_err(|e| e.to_string())?.family().key();
    // shortcut: each prefix replayed from the start, n²/2 moves; replay once with every
    // position kept if a long go Game makes this slow.
    let texts = (0..=entries.len()).map(|i| bragd_games::replay(variant, initial, &entries[..i]).map(|s| s.position)).collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    let keys = bragd_library::library::position_keys(family, &texts);
    Ok((family.to_string(), texts, keys))
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Starts reviewing the whole Game with the Variant's engine (or `engine`).
pub fn review_start(cfg: &Config, held: &Held, variant: &str, initial: Option<&str>, entries: &[String], engine: Option<String>) -> Result<(), String> {
    guard()?;
    bragd_core::tips::used("annotations");
    *crate::lock(&held.review) = None;
    let key = engine.filter(|k| !k.is_empty()).or_else(|| engines::default_for(cfg, variant)).ok_or_else(|| tr("engines.none_for", &[]))?;
    let chosen = engines::resolve(cfg, &key, variant)?;
    let protocol = chosen.protocols[0];
    let name = bragd_engines::known::known(&key).map_or(key.as_str(), |k| k.name);
    let notation = seam::notation(variant, protocol, name);
    if notation == Notation::Gnubg {
        return Err(tr("analyse.review_backgammon", &[]));
    }
    let (game, _) = seam::position(variant, initial, entries, notation)?;
    let (family, texts, keys) = positions(variant, initial, entries)?;
    let setup = Setup {
        engine: key,
        program: chosen.program,
        protocol,
        options: bragd_engines::options::OptionStore::load(&engines::store_path()?).unwrap_or_default(),
        syzygy: None,
        limit: Limit::Time(300),
        store: Store::path(),
        family,
        variant: variant.into(),
        keys,
        positions: texts,
    };
    let reviewer = Reviewer::start(&setup, &game);
    let asked = Asked { variant: variant.into(), initial: initial.map(String::from), entries: entries.to_vec() };
    *crate::lock(&held.review) = Some(Reviewing { reviewer, asked, notation });
    Ok(())
}

pub fn review_stop(held: &Held) {
    *crate::lock(&held.review) = None;
}

/// A move's Annotation, worded: its mark, what it means, and the better line as moves to play.
#[derive(Clone, Serialize)]
pub struct NoteView {
    /// The move's index among the Game's entries, and the move as the move list writes it
    /// (`12. Nf3`).
    index: usize,
    label: String,
    mark: String,
    /// In the reader's language: `Blunder. It leaves a piece hanging. Better was Nf3 Nc6.`
    text: String,
    /// Played from the position before the move.
    better: Vec<LineMove>,
}

/// The Annotation worded from Bragd's own texts; None when it says nothing.
fn worded(mark: Option<Mark>, motifs: &[bragd_positions::Motif], better: &[(String, String)]) -> Option<String> {
    let mut words: Vec<String> = mark.map(|m| tr(&format!("annotation.mark.{}", m.id()), &[])).into_iter().collect();
    words.extend(motifs.iter().map(|m| tr(&format!("annotation.motif.{}", m.id()), &[])));
    let bad = matches!(mark, Some(Mark::Blunder | Mark::Mistake | Mark::Inaccuracy)) || motifs.contains(&bragd_positions::Motif::MissedMate);
    if bad && !better.is_empty() {
        let line: Vec<&str> = better.iter().map(|(_, n)| n.as_str()).collect();
        words.push(tr("annotation.better", &[("line", &line.join(" "))]));
    }
    (!words.is_empty()).then(|| words.join(" "))
}

/// Move `index`'s note, its better line replayed from `entries` (the moves before it); `state`
/// is the whole Game's, for the move's label.
#[allow(clippy::too_many_arguments)]
fn note(state: &State, index: usize, mark: Option<Mark>, motifs: &[bragd_positions::Motif], better: &[String], variant: &str, initial: Option<&str>, entries: &[String], notation: Notation) -> Option<NoteView> {
    let better = if better.is_empty() { vec![] } else { seam::line(variant, initial, entries, notation, &better[..better.len().min(SHOWN)]) };
    let text = worded(mark, motifs, &better)?;
    // A reply has no number in the move list: here it is the move's, as `3… Qg5`.
    let number = match state.numbers.get(index).map_or("", String::as_str) {
        "" if index > 0 => state.numbers[index - 1].replace('.', "…"),
        n => n.to_string(),
    };
    let label = format!("{number} {}", state.history.get(index).map_or("", String::as_str)).trim().to_string();
    Some(NoteView { index, label, mark: mark.map_or("", Mark::sign).into(), text, better: better.into_iter().map(|(entry, notation)| LineMove { entry, notation }).collect() })
}

fn note_view(state: &State, a: &Annotation, index: usize, variant: &str, initial: Option<&str>, entries: &[String], notation: Notation) -> Option<NoteView> {
    let better: Vec<String> = a.better.split_whitespace().map(String::from).collect();
    note(state, index, a.mark, &a.motifs, &better, variant, initial, entries, notation)
}

/// A review as the move list and Analyse show it: each entry's mark and evaluation, and the
/// worded notes of the moves that have one.
#[derive(Serialize)]
pub struct ReviewView {
    engine: String,
    done: bool,
    reviewed: usize,
    total: usize,
    marks: Vec<String>,
    evals: Vec<String>,
    /// Each entry's note in a few words, for its mark in the move list.
    titles: Vec<String>,
    notes: Vec<NoteView>,
    error: Option<String>,
}

pub fn review_now(held: &Held) -> Option<ReviewView> {
    let (rev, asked, notation) = {
        let r = crate::lock(&held.review);
        let r = r.as_ref()?;
        (r.reviewer.now(), r.asked.clone(), r.notation)
    };
    let n = asked.entries.len();
    let state = bragd_games::replay(&asked.variant, asked.initial.as_deref(), &asked.entries).ok()?;
    let mut marks = vec![String::new(); n];
    let mut evals = vec![String::new(); n];
    let mut titles = vec![String::new(); n];
    let mut notes = vec![];
    for Ply { index, eval, mark, better, motifs } in &rev.plies {
        if *index < n {
            marks[*index] = mark.map_or("", Mark::sign).to_string();
            evals[*index] = eval.clone();
            titles[*index] = note(&state, *index, *mark, motifs, &[], &asked.variant, None, &[], notation).map(|n| n.text).unwrap_or_default();
            notes.extend(note(&state, *index, *mark, motifs, better, &asked.variant, asked.initial.as_deref(), &asked.entries[..*index], notation));
        }
    }
    Some(ReviewView { engine: rev.engine, done: rev.done, reviewed: rev.plies.len(), total: rev.total, marks, evals, titles, notes, error: rev.error })
}

/// A book's move for the position.
#[derive(Serialize)]
pub struct BookView {
    entry: String,
    notation: String,
    share: f32,
    book: String,
}

/// What chess knows about a position without an engine.
#[derive(Serialize, Default)]
pub struct Known {
    eco: Option<&'static str>,
    opening: Option<&'static str>,
    book: Vec<BookView>,
}

/// The opening's name and the books' moves, for standard chess.
pub fn known(cfg: &Config, variant: &str, initial: Option<&str>, entries: &[String]) -> Result<Known, String> {
    if variant != "chess" || guard().is_err() {
        return Ok(Known::default());
    }
    let state = bragd_games::replay(variant, initial, entries).map_err(|e| e.to_string())?;
    let (eco, opening) = if initial.is_none_or(|i| i.trim().is_empty()) { openings::by_moves(state.history.iter().map(String::as_str)).unzip() } else { (None, None) };
    let mut book = vec![];
    for path in &cfg.engines.books {
        let name = Path::new(path).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        for m in polyglot::moves_at(Path::new(path), &state.position) {
            if let Some(l) = seam::entry(&state, Notation::Wire, &m.uci) {
                book.push(BookView { entry: l.wire.clone(), notation: l.notation.clone(), share: m.share, book: name.clone() });
            }
        }
    }
    Ok(Known { eco, opening, book })
}

/// What the tablebase says about a chess position of up to seven pieces.
#[derive(Serialize, Default)]
pub struct TableView {
    pieces: usize,
    /// The local tables cover it: the engine reads them.
    local: bool,
    /// The verdict for the side to move, as shown (`White wins`), and its distances.
    verdict: Option<String>,
    dtz: Option<i32>,
    dtm: Option<i32>,
    /// The best moves with their verdicts.
    moves: Vec<(String, String, String)>,
    /// Not covered locally, and the online fallback is off (or Bragd is offline).
    unavailable: bool,
    error: Option<String>,
}

/// The verdict `category` for `side`, as shown.
fn verdict(category: &str, side: &str) -> String {
    tr(&format!("tablebase.{}", category.replace('-', "_")), &[("side", side)])
}

pub fn tablebase(cfg: &Config, variant: &str, initial: Option<&str>, entries: &[String]) -> Result<Option<TableView>, String> {
    if !matches!(variant, "chess" | "chess960") || guard().is_err() {
        return Ok(None);
    }
    let state: State = bragd_games::replay(variant, initial, entries).map_err(|e| e.to_string())?;
    let fen = &state.position;
    let Some((w, b)) = material(fen) else { return Ok(None) };
    let pieces = w.len() + b.len();
    if pieces > 7 || state.to_move.is_none() {
        return Ok(None);
    }
    let side = state.to_move.map(|s| tr(&format!("side.{}", s.name.to_lowercase()), &[])).unwrap_or_default();
    let tb = Tablebases { folders: cfg.tablebase_folder().into_iter().collect(), online: cfg.tablebases.online && !bragd_core::offline() };
    if tb.covers(fen) {
        return Ok(Some(TableView { pieces, local: true, ..Default::default() }));
    }
    if !tb.online {
        return Ok(Some(TableView { pieces, unavailable: true, ..Default::default() }));
    }
    let found = lichess::lookup(&tb, &bragd_engines::fetch::Http, fen);
    Ok(Some(match found {
        Ok(Some(l)) => TableView {
            pieces,
            verdict: Some(verdict(&l.category, &side)),
            dtz: l.dtz,
            dtm: l.dtm,
            moves: l.moves.iter().take(5).filter_map(|m| Some((seam::entry(&state, Notation::Wire, &m.uci)?.wire.clone(), m.san.clone(), tr(&format!("tablebase.after.{}", m.category.replace('-', "_")), &[])))).collect(),
            ..Default::default()
        },
        Ok(None) => TableView { pieces, unavailable: true, ..Default::default() },
        Err(e) => TableView { pieces, error: Some(e), ..Default::default() },
    }))
}

/// A position typed in, for the free board: the Variant's own notation (FEN, SFEN, PDN, a go
/// picture), checked by replaying it.
pub fn setup(variant: &str, text: &str) -> Result<State, String> {
    let _ = Variant::from_key(variant).ok_or_else(|| tr("engines.unknown", &[("name", variant)]))?;
    bragd_games::replay(variant, Some(text.trim()), &[] as &[&str]).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_replayed_only_when_it_changed() {
        let n = std::cell::Cell::new(0);
        let render = |pv: &[String]| {
            n.set(n.get() + 1);
            pv.iter().map(|m| (m.clone(), m.to_uppercase())).collect()
        };
        let pv = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        let mut cache = Shown::new();
        let first = shown_lines(&mut cache, &[pv("e7e5 g1f3"), pv("c7c5")], render);
        assert_eq!((n.get(), first[1].clone()), (2, vec![("c7c5".to_string(), "C7C5".to_string())]));
        assert_eq!(shown_lines(&mut cache, &[pv("e7e5 g1f3"), pv("c7c5")], render), first);
        assert_eq!(n.get(), 2, "nothing changed, nothing replayed");
        let next = shown_lines(&mut cache, &[pv("e7e5 g1f3"), pv("c7c6")], render);
        assert_eq!((n.get(), next[0].clone()), (3, first[0].clone()), "only the changed line");
        assert_eq!(cache.len(), 2, "the cache holds this poll's lines only");
    }

    #[test]
    fn a_line_number_past_those_asked_for_is_ignored() {
        let mut raw = Raw::default();
        raw.heard(&Info { line: 2, pv: vec!["e2e4".into()], ..Default::default() }, 3);
        raw.heard(&Info { line: 1_000_000_000, pv: vec!["d2d4".into()], ..Default::default() }, 3);
        assert_eq!(raw.lines.len(), 2);
        assert_eq!(raw.lines[1].pv, ["e2e4"]);
    }

    #[test]
    fn analysis_is_locked_only_during_a_rated_game_against_a_person() {
        assert!(!locked(&[]));
        let rated = LiveGame { rated: true, against_person: true, over: false };
        assert!(locked(&[rated]));
        assert!(!locked(&[LiveGame { over: true, ..rated }]), "a finished Game");
        assert!(!locked(&[LiveGame { rated: false, ..rated }]), "a casual Game");
        assert!(!locked(&[LiveGame { against_person: false, ..rated }]), "against an Engine");
        assert!(locked(&[LiveGame::default(), rated]), "any window's Game");
        assert!(guard().is_ok(), "no online play yet");
    }

    #[test]
    fn openings_and_tablebases_offline() {
        let cfg = Config::default();
        let k = known(&cfg, "chess", None, &["e2e4".into(), "c7c5".into()]).unwrap();
        assert_eq!(k.opening, Some("Sicilian Defense"));
        assert!(known(&cfg, "shogi", None, &[]).unwrap().opening.is_none());
        // SAFETY: this test binary's only use of BRAGD_OFFLINE.
        unsafe { std::env::set_var("BRAGD_OFFLINE", "1") };
        let t = tablebase(&cfg, "chess", Some("4k3/6KP/8/8/8/8/7p/8 w - - 0 1"), &[]).unwrap().unwrap();
        assert!(t.unavailable && t.pieces == 4 && t.verdict.is_none(), "nothing asked online");
        assert!(tablebase(&cfg, "chess", None, &[]).unwrap().is_none(), "32 pieces");
        assert!(setup("chess", "not a fen").is_err());
    }

    /// A fake engine's lines, shown in SAN from White's view though Black is to move.
    #[cfg(unix)]
    #[test]
    fn lines_in_the_variants_notation() {
        use std::os::unix::fs::PermissionsExt;
        let d = std::env::temp_dir().join(format!("bragd-gui-analyse-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("fake");
        std::fs::write(&p, "#!/bin/sh\nwhile read l; do case \"$l\" in uci) echo 'id name Fake 1'; echo 'option name MultiPV type spin default 1 min 1 max 5'; echo uciok;; go*) echo 'info depth 7 multipv 1 score cp 25 pv e7e5 g1f3'; echo 'info depth 7 multipv 2 score cp -40 pv c7c5'; echo 'bestmove e7e5';; quit) exit 0;; esac; done\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut cfg = Config::default();
        cfg.engines.added.insert("fake".into(), "chess".into());
        cfg.engines.paths.insert("fake".into(), p.display().to_string());
        let held = Held::default();
        start(&cfg, &held, "chess", None, vec!["e2e4".into()], Some("fake".into()), Some(2)).unwrap();
        let mut n = now(&held).unwrap();
        for _ in 0..200 {
            if !n.running {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
            n = now(&held).unwrap();
        }
        assert_eq!(n.error, None);
        assert_eq!(n.engine, "Fake 1");
        assert_eq!(n.lines.len(), 2);
        assert_eq!((n.lines[0].score.as_str(), n.lines[0].depth), ("−0.25", Some(7)));
        let shown: Vec<&str> = n.lines[0].moves.iter().map(|m| m.notation.as_str()).collect();
        assert_eq!(shown, ["e5", "Nf3"]);
        assert_eq!(n.lines[1].score, "+0.40");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_position_reached_another_way_has_the_same_key() {
        let e = |m: &[&str]| m.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let (family, texts, a) = positions("chess", None, &e(&["e2e4", "e7e5", "g1f3"])).unwrap();
        let (_, _, b) = positions("chess", None, &e(&["g1f3", "e7e5", "e2e4"])).unwrap();
        assert_eq!((family.as_str(), texts.len(), a.len()), ("chess", 4, 4));
        assert_eq!(a[3], b[3]);
        assert_ne!(a[1], b[1]);
        assert_eq!(a[0], b[0]);
        let st = bragd_games::replay("chess", None, &e(&["e2e4", "e7e5", "g1f3"])).unwrap();
        let label = |i| note(&st, i, Some(Mark::Mistake), &[], &[], "chess", None, &[], Notation::Wire).unwrap().label;
        assert_eq!((label(0), label(1), label(2)), ("1. e4".to_string(), "1… e5".to_string(), "2. Nf3".to_string()));
    }

    #[test]
    fn an_annotation_is_worded_in_the_readers_language() {
        use bragd_positions::Motif;
        let better = [("g1f3".to_string(), "Nf3".to_string()), ("b8c6".to_string(), "Nc6".to_string())];
        assert_eq!(worded(Some(Mark::Blunder), &[Motif::Hangs], &better).unwrap(), "Blunder. It leaves a piece hanging. Better was Nf3 Nc6.");
        // A good move names no better one, and a move with nothing to say has no note.
        assert_eq!(worded(Some(Mark::Good), &[Motif::Fork], &better).unwrap(), "Good move. It forks two pieces.");
        assert_eq!(worded(None, &[], &better), None);
        assert_eq!(worded(None, &[Motif::MissedMate], &better).unwrap(), "It misses a forced mate. Better was Nf3 Nc6.");
    }

    /// What the store knows shows at once, before the engine is as deep; the engine's own line
    /// goes into the store when it stops.
    #[cfg(unix)]
    #[test]
    fn live_analysis_reads_and_writes_the_store() {
        use std::os::unix::fs::PermissionsExt;
        let d = std::env::temp_dir().join(format!("bragd-gui-analyse-store-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("fake");
        std::fs::write(&p, "#!/bin/sh\nwhile read l; do case \"$l\" in uci) echo 'id name Store 1'; echo uciok;; go*) echo 'info depth 5 multipv 1 score cp 25 pv e7e5 g1f3'; echo 'bestmove e7e5';; quit) exit 0;; esac; done\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut cfg = Config::default();
        cfg.engines.added.insert("storefake".into(), "chess".into());
        cfg.engines.paths.insert("storefake".into(), p.display().to_string());
        let entries = |m: &[&str]| m.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let (_, _, keys) = positions("chess", None, &entries(&["d2d4", "g8f6"])).unwrap();
        let mut store = Store::open(&Store::path().unwrap()).unwrap();
        let edition = bragd_positions::Edition { engine: "storefake".into(), version: "Store 0".into(), network: None, image: None };
        store.put(&Evaluation { family: "chess".into(), variant: "chess".into(), key: keys[2], edition, score: bragd_positions::Score::Cp(-50), depth: 30, line: "c2c4 e7e6".into(), time: 1 }).unwrap();
        let held = Held::default();
        let wait = |held: &Held| {
            let mut n = now(held).unwrap();
            for _ in 0..200 {
                if !n.running {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
                n = now(held).unwrap();
            }
            n
        };
        start(&cfg, &held, "chess", None, entries(&["d2d4", "g8f6"]), Some("storefake".into()), Some(1)).unwrap();
        let n = wait(&held);
        assert_eq!(n.error, None);
        assert_eq!((n.lines[0].score.as_str(), n.lines[0].depth), ("−0.50", Some(30)), "the store's, deeper than the engine got");
        assert_eq!(n.lines[0].moves[0].notation, "c4");
        // Elsewhere, the engine's own line is kept with its Edition.
        start(&cfg, &held, "chess", None, entries(&["e2e4"]), Some("storefake".into()), Some(1)).unwrap();
        let n = wait(&held);
        assert_eq!((n.lines[0].score.as_str(), n.lines[0].depth), ("−0.25", Some(5)));
        let (_, _, keys) = positions("chess", None, &entries(&["e2e4"])).unwrap();
        let kept = store.shown("chess", "chess", keys[1], "storefake").unwrap().unwrap();
        assert_eq!((kept.edition.version.as_str(), kept.score, kept.depth), ("Store 1", bragd_positions::Score::Cp(-25), 5));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
