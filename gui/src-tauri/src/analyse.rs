//! Analyse: an engine's best lines for a position, as they come, in the Variant's notation and
//! scored from the first Side's view; a whole Game reviewed with its moves marked; and for
//! chess the opening's name, the opening books' moves and what the tablebases say.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use bragd_core::config::Config;
use bragd_engines::lichess;
use bragd_engines::protocol::{Info, Limit, Score};
use bragd_engines::review::{Reviewer, Setup};
use bragd_engines::seam::{self, Notation};
use bragd_engines::session::{Session, think};
use bragd_engines::syzygy::{Tablebases, material};
use bragd_games::{State, Variant};
use bragd_library::chess::{openings, polyglot};
use serde::Serialize;

use crate::engines::{self, tr};

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

/// Nothing goes to the network while `BRAGD_OFFLINE` is set (tests and screenshots).
pub fn offline() -> bool {
    std::env::var_os("BRAGD_OFFLINE").is_some()
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
    done: bool,
    error: Option<String>,
}

impl Raw {
    /// Keeps what the engine said of one of its `wanted` lines.
    fn heard(&mut self, i: &Info, wanted: u32) {
        // A line number past the lines asked for is the engine's mistake (or an attack on
        // memory: `multipv 1000000000`), not a line to make room for.
        if i.line > wanted as usize {
            return;
        }
        let at = i.line.max(1) - 1;
        if self.lines.len() <= at {
            self.lines.resize(at + 1, Info::default());
        }
        let keep = self.lines[at].clone();
        self.lines[at] = Info { score: i.score.or(keep.score), pv: if i.pv.is_empty() { keep.pv } else { i.pv.clone() }, ..i.clone() };
    }
}

struct Live {
    asked: Asked,
    raw: Arc<Mutex<Raw>>,
    stop: Arc<AtomicBool>,
}

impl Drop for Live {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

struct Reviewing {
    reviewer: Reviewer,
    entries: usize,
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
    *held.live.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let key = engine.filter(|k| !k.is_empty()).or_else(|| engines::default_for(cfg, variant)).ok_or_else(|| tr("engines.none_for", &[]))?;
    let chosen = engines::resolve(cfg, &key, variant)?;
    let asked = Asked { variant: variant.into(), initial, entries };
    let raw = Arc::new(Mutex::new(Raw { engine: key, ..Default::default() }));
    let stop = Arc::new(AtomicBool::new(false));
    let (r, s, a, cfg) = (raw.clone(), stop.clone(), asked.clone(), cfg.clone());
    let wanted = lines.unwrap_or(LINES).clamp(1, bragd_engines::session::MAX_LINES);
    std::thread::spawn(move || {
        let halt = || s.load(Ordering::Relaxed);
        let result = (|| -> Result<(), String> {
            let (mut e, notation) = engines::start(&cfg, &chosen, &a.variant, &halt)?;
            let (pos, state) = seam::position(&a.variant, a.initial.as_deref(), &a.entries, notation)?;
            {
                let mut raw = r.lock().unwrap_or_else(|e| e.into_inner());
                raw.engine = e.about().name.clone();
                raw.notation = Some(notation);
                raw.second = state.to_move.is_some_and(|s| s.index == 1);
            }
            if state.to_move.is_none() {
                return Ok(());
            }
            // Until stopped, or the engine's own cap (`Limit::cap`).
            let report = &mut |i: &Info| r.lock().unwrap_or_else(|e| e.into_inner()).heard(i, wanted);
            let a = think(&mut e, &pos, Limit::Depth(99), wanted, &halt, report).map_err(|e| e.to_string())?;
            // An engine that reports no lines (GNU Go) still names its move.
            let mut raw = r.lock().unwrap_or_else(|e| e.into_inner());
            if raw.lines.is_empty()
                && let Some(b) = a.best
            {
                raw.lines.push(Info { line: 1, pv: vec![b], ..Default::default() });
            }
            Ok(())
        })();
        let mut raw = r.lock().unwrap_or_else(|e| e.into_inner());
        raw.done = true;
        if let Err(e) = result
            && !halt()
        {
            raw.error = Some(e);
        }
    });
    *held.live.lock().unwrap_or_else(|e| e.into_inner()) = Some(Live { asked, raw, stop });
    Ok(())
}

pub fn stop(held: &Held) {
    if let Some(l) = held.live.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        l.stop.store(true, Ordering::Relaxed);
    }
}

/// A move of a line: its entry and how a Player writes it.
#[derive(Serialize)]
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
}

/// The analysis as far as it has come; None when none was started.
pub fn now(held: &Held) -> Option<Now> {
    // Copied out, so the replays below hold neither lock and the engine's reports never wait.
    let (asked, raw) = {
        let live = held.live.lock().unwrap_or_else(|e| e.into_inner());
        let l = live.as_ref()?;
        let raw = l.raw.lock().unwrap_or_else(|e| e.into_inner()).clone();
        (l.asked.clone(), raw)
    };
    let lines = raw
        .notation
        .map(|n| {
            raw.lines
                .iter()
                .filter(|i| !i.pv.is_empty())
                .map(|i| {
                    let score = i.score.map(|s: Score| if raw.second { s.flipped() } else { s });
                    let pv: Vec<String> = i.pv.iter().take(SHOWN).cloned().collect();
                    let a = &asked;
                    let moves = seam::line(&a.variant, a.initial.as_deref(), &a.entries, n, &pv).into_iter().map(|(entry, notation)| LineMove { entry, notation }).collect();
                    LineView { score: score.map(Score::shown).unwrap_or_default(), depth: i.depth, moves }
                })
                .filter(|v| !v.moves.is_empty())
                .collect()
        })
        .unwrap_or_default();
    Some(Now { running: !raw.done, engine: raw.engine.clone(), lines, error: raw.error.clone() })
}

/// Starts reviewing the whole Game with the Variant's engine (or `engine`).
pub fn review_start(cfg: &Config, held: &Held, variant: &str, initial: Option<&str>, entries: &[String], engine: Option<String>) -> Result<(), String> {
    guard()?;
    *held.review.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let key = engine.filter(|k| !k.is_empty()).or_else(|| engines::default_for(cfg, variant)).ok_or_else(|| tr("engines.none_for", &[]))?;
    let chosen = engines::resolve(cfg, &key, variant)?;
    let protocol = chosen.protocols[0];
    let name = bragd_engines::known::known(&key).map_or(key.as_str(), |k| k.name);
    let notation = seam::notation(variant, protocol, name);
    if notation == Notation::Gnubg {
        return Err(tr("analyse.review_backgammon", &[]));
    }
    let (game, _) = seam::position(variant, initial, entries, notation)?;
    let setup = Setup {
        program: chosen.program,
        protocol,
        options: bragd_engines::options::OptionStore::load(&engines::store_path()?).unwrap_or_default(),
        syzygy: None,
        limit: Limit::Time(300),
        cache: bragd_core::dirs::app_cache().map(|c| c.join("reviews")),
    };
    let reviewer = Reviewer::start(&setup, &game);
    *held.review.lock().unwrap_or_else(|e| e.into_inner()) = Some(Reviewing { reviewer, entries: game.moves.len() });
    Ok(())
}

pub fn review_stop(held: &Held) {
    *held.review.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// A review as the move list shows it: each entry's mark and evaluation.
#[derive(Serialize)]
pub struct ReviewView {
    engine: String,
    done: bool,
    reviewed: usize,
    total: usize,
    marks: Vec<String>,
    evals: Vec<String>,
    error: Option<String>,
}

/// A NAG as the move list writes it.
fn mark(nag: u8) -> &'static str {
    match nag {
        2 => "?",
        4 => "??",
        5 => "!?",
        6 => "?!",
        _ => "",
    }
}

pub fn review_now(held: &Held) -> Option<ReviewView> {
    let r = held.review.lock().unwrap_or_else(|e| e.into_inner());
    let r = r.as_ref()?;
    let rev = r.reviewer.now();
    let mut marks = vec![String::new(); r.entries];
    let mut evals = vec![String::new(); r.entries];
    for p in &rev.plies {
        if p.index < r.entries {
            marks[p.index] = p.mark.map(mark).unwrap_or_default().to_string();
            evals[p.index] = p.eval.clone();
        }
    }
    Some(ReviewView { engine: rev.engine, done: rev.done, reviewed: rev.plies.len(), total: rev.total, marks, evals, error: rev.error })
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
    let tb = Tablebases { folders: cfg.tablebase_folder().into_iter().collect(), online: cfg.tablebases.online && !offline() };
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
        assert_eq!(mark(4), "??");
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
}
