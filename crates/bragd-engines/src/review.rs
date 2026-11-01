//! A whole Game reviewed by an Engine, through the Position store: each position's Evaluation is
//! read from the store when it has one for the Engine, and the Engine thinks only about the
//! rest, in the background, each result put into the store with its Edition. Each move gets
//! its Annotation from the two Evaluations around it (`bragd_positions::annotation`): its mark,
//! the better line, and for chess its named motifs. Annotations go into the store too, so a
//! position judged once shows at once in every Game that reaches it.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bragd_positions::annotation::{self, Mark, Motif};
use bragd_positions::{Annotation, Edition, Evaluation, Store};

use crate::options::OptionStore;
use crate::protocol::{Limit, Position, Protocol, Score};
use crate::session::{Engine, Program, Session, think};

/// The most moves of a line kept with an Evaluation.
const LINE: usize = 12;

/// One move, reviewed.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Ply {
    /// Its place among the Game's moves, from 0.
    pub index: usize,
    /// The Evaluation after it, from the first Side's view, as shown (`+0.35`, `#3`).
    pub eval: String,
    pub mark: Option<Mark>,
    /// The Engine's line where the move was played, when it would have played another move;
    /// in its notation.
    pub better: Vec<String>,
    pub motifs: Vec<Motif>,
}

/// A Game's review, as far as it has come.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Review {
    /// The Engine's name, as it gave it, or as the store's Edition names it, or its key.
    pub engine: String,
    /// The moves reviewed, by index.
    pub plies: Vec<Ply>,
    /// Of how many moves; done when all are in, or it stopped.
    pub total: usize,
    pub done: bool,
    pub error: Option<String>,
}

/// How a review is run: the Engine (its key, program, protocol and options), the tablebases,
/// the limit per position, the store, and the Game's positions.
#[derive(Clone, Debug)]
pub struct Setup {
    /// The Engine's key (`known::Known::key`): its Evaluations in the store are found by it.
    pub engine: String,
    pub program: Program,
    pub protocol: Protocol,
    pub options: OptionStore,
    pub syzygy: Option<String>,
    pub limit: Limit,
    /// The Position store; None reviews without one.
    pub store: Option<PathBuf>,
    pub family: String,
    pub variant: String,
    /// The key of each position (`bragd_library::library::position_keys`): before the first
    /// move, then after each.
    pub keys: Vec<u64>,
    /// Each position in its Family's notation, as `keys`: chess motifs are read from them.
    pub positions: Vec<String>,
}

/// A Game being reviewed, on a thread of its own; dropped or stopped, the engine stops too.
pub struct Reviewer {
    shared: Arc<Mutex<Review>>,
    stop: Arc<AtomicBool>,
}

impl Drop for Reviewer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Reviewer {
    /// Reviews `game` as `setup` says.
    pub fn start(setup: &Setup, game: &Position) -> Reviewer {
        let shared = Arc::new(Mutex::new(Review { engine: setup.engine.clone(), total: game.moves.len(), ..Default::default() }));
        let stop = Arc::new(AtomicBool::new(false));
        let (s, st, g, setup) = (shared.clone(), stop.clone(), game.clone(), setup.clone());
        std::thread::spawn(move || {
            let r = run(&setup, &g, &s, &st);
            let mut out = s.lock().unwrap_or_else(|e| e.into_inner());
            out.done = true;
            if let Err(e) = r
                && e.kind() != std::io::ErrorKind::Interrupted
            {
                out.error = Some(e.to_string());
            }
        });
        Reviewer { shared, stop }
    }

    /// How far it has come.
    pub fn now(&self) -> Review {
        self.shared.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn io(e: bragd_positions::Error) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

/// What a review knows of each position: its Evaluation, and for one the Engine thought about
/// here, how much worse its second choice was (from the mover's side).
struct Known {
    evals: Vec<Option<Evaluation>>,
    gaps: Vec<Option<f64>>,
}

fn run(setup: &Setup, game: &Position, shared: &Mutex<Review>, stop: &AtomicBool) -> std::io::Result<()> {
    let halt = || stop.load(Ordering::Relaxed);
    let n = game.moves.len();
    if setup.keys.len() != n + 1 {
        return Err(std::io::Error::other(format!("{} position keys for {n} moves", setup.keys.len())));
    }
    let mut store = setup.store.as_deref().map(Store::open).transpose().map_err(io)?;
    // First what the store has, at once; then the Engine for the rest.
    let mut known = Known { evals: vec![None; n + 1], gaps: vec![None; n + 1] };
    if let Some(s) = &store {
        for (i, k) in setup.keys.iter().enumerate() {
            known.evals[i] = s.shown(&setup.family, &setup.variant, *k, &setup.engine).map_err(io)?;
        }
    }
    // Until the Engine starts, if it must, the review is by the Engine the store names.
    if let Some(e) = known.evals.iter().flatten().next() {
        shared.lock().unwrap_or_else(|e| e.into_inner()).engine = e.edition.version.clone();
    }
    for i in 0..n {
        annotate(setup, game, &known, store.as_mut(), shared, i)?;
    }
    let mut engine: Option<(Engine, Edition)> = None;
    for i in 0..=n {
        if known.evals[i].is_some() {
            continue;
        }
        if halt() {
            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "stopped"));
        }
        let (e, edition) = match &mut engine {
            Some(e) => e,
            None => {
                let mut e = Engine::start(&setup.program, setup.protocol, Duration::from_secs(30), &halt)?;
                // Options it does not take: the live analysis says so; the Game is reviewed without them.
                e.configure(&setup.options, setup.syzygy.as_deref())?;
                let edition = crate::edition::of(&setup.engine, e.about(), &setup.program);
                shared.lock().unwrap_or_else(|e| e.into_inner()).engine = e.about().name.clone();
                engine.insert((e, edition))
            }
        };
        let pos = Position { moves: game.moves[..i].to_vec(), ..game.clone() };
        let a = think(e, &pos, setup.limit, 2, &halt, &mut |_| {})?;
        let score = |l: usize| a.lines.get(l).and_then(|l| l.score).map(bragd_positions::Score::from);
        // No score at all: no move, so nothing to judge it by.
        let Some(best) = score(0) else { continue };
        let first = pos.first_to_move();
        known.gaps[i] = score(1).and_then(|second| annotation::gap(best, second, first));
        let eval = Evaluation {
            family: setup.family.clone(),
            variant: setup.variant.clone(),
            key: setup.keys[i],
            edition: edition.clone(),
            score: best,
            depth: a.depth,
            line: a.lines[0].pv.iter().take(LINE).cloned().collect::<Vec<_>>().join(" "),
            time: now(),
        };
        if let Some(s) = &mut store {
            s.put(&eval).map_err(io)?;
        }
        known.evals[i] = Some(eval);
        for m in [i.wrapping_sub(1), i] {
            if m < n {
                annotate(setup, game, &known, store.as_mut(), shared, m)?;
            }
        }
    }
    Ok(())
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Move `i`'s Annotation, once both Evaluations around it are known: the store's, when it has
/// one as deep for the same Edition, else made here and put into the store. Shown in `shared`.
fn annotate(setup: &Setup, game: &Position, known: &Known, store: Option<&mut Store>, shared: &Mutex<Review>, i: usize) -> std::io::Result<()> {
    let (Some(before), Some(after)) = (&known.evals[i], &known.evals[i + 1]) else { return Ok(()) };
    let mv = &game.moves[i];
    let depth = before.depth.min(after.depth);
    let stored = match &store {
        Some(s) => s.annotation(&setup.family, &setup.variant, setup.keys[i], mv, &setup.engine).map_err(io)?,
        None => None,
    };
    let note = match stored.filter(|a| a.edition == before.edition && a.depth >= depth) {
        Some(a) => a,
        None => {
            let a = made(setup, game, known, i, before, after);
            if let Some(s) = store {
                s.put_annotation(&a).map_err(io)?;
            }
            a
        }
    };
    let ply = Ply {
        index: i,
        eval: Score::from(after.score).shown(),
        mark: note.mark,
        better: note.better.split_whitespace().map(String::from).collect(),
        motifs: note.motifs,
    };
    let mut r = shared.lock().unwrap_or_else(|e| e.into_inner());
    match r.plies.binary_search_by_key(&i, |p| p.index) {
        Ok(at) => r.plies[at] = ply,
        Err(at) => r.plies.insert(at, ply),
    }
    Ok(())
}

/// Move `i`'s Annotation from the Evaluations before and after it.
fn made(setup: &Setup, game: &Position, known: &Known, i: usize, before: &Evaluation, after: &Evaluation) -> Annotation {
    let mv = &game.moves[i];
    let mover_first = Position { moves: game.moves[..i].to_vec(), ..game.clone() }.first_to_move();
    let played_best = before.line.split_whitespace().next() == Some(mv.as_str());
    let mark = annotation::mark(before.score, after.score, mover_first, known.gaps[i].filter(|_| played_best));
    let mut motifs = vec![];
    if setup.family == "chess"
        && matches!(setup.variant.as_str(), "chess" | "chess960")
        && let Some(fen) = setup.positions.get(i)
        && let Ok(board) = rules_chess::Position::from_fen(fen)
    {
        let bad = matches!(mark, Some(Mark::Blunder | Mark::Mistake | Mark::Inaccuracy));
        for m in rules_chess::motifs::motifs(&board, mv) {
            match m {
                rules_chess::motifs::Motif::Fork => motifs.push(Motif::Fork),
                rules_chess::motifs::Motif::Pin => motifs.push(Motif::Pin),
                // A piece left where it can be taken is news only when the Engine says it costs.
                rules_chess::motifs::Motif::Hangs if bad => motifs.push(Motif::Hangs),
                rules_chess::motifs::Motif::Hangs => {}
            }
        }
        if !played_best && annotation::missed_mate(before.score, after.score, mover_first) {
            motifs.push(Motif::MissedMate);
        }
    }
    Annotation {
        family: setup.family.clone(),
        variant: setup.variant.clone(),
        key: setup.keys[i],
        mv: mv.clone(),
        edition: before.edition.clone(),
        mark,
        better: if played_best { String::new() } else { before.line.clone() },
        motifs,
        depth: before.depth.min(after.depth),
        time: before.time.max(after.time),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake engine that says +0.20 for the side to move, but +5.00 for Black once White has
    /// played g1f3, and writes a line to `count` for each position it thinks about.
    #[cfg(unix)]
    fn fake(d: &std::path::Path) -> PathBuf {
        let count = d.join("count");
        crate::fake_program(
            d,
            "engine",
            &format!(
                "s=20; while read l; do case \"$l\" in uci) echo 'id name Fake 2'; echo uciok;; isready) echo readyok;; position*g1f3*) s=500;; position*) s=20;; go*) echo go >> '{}'; echo \"info depth 3 multipv 1 score cp $s pv e7e5 g1f3\"; echo 'bestmove e7e5';; quit) exit 0;; esac; done",
                count.display()
            ),
        )
    }

    #[cfg(unix)]
    fn wait(r: &Reviewer) -> Review {
        let t = std::time::Instant::now();
        while !r.now().done && t.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(10));
        }
        r.now()
    }

    fn setup(d: &std::path::Path, p: &std::path::Path, keys: &[u64]) -> Setup {
        Setup {
            engine: "fake".into(),
            program: Program::Installed { path: p.to_path_buf(), args: vec![] },
            protocol: Protocol::Uci,
            options: OptionStore::default(),
            syzygy: None,
            limit: Limit::Time(50),
            store: Some(d.join("positions.sqlite")),
            family: "chess".into(),
            variant: "chess".into(),
            keys: keys.to_vec(),
            positions: vec![],
        }
    }

    #[cfg(unix)]
    fn thought(d: &std::path::Path) -> usize {
        std::fs::read_to_string(d.join("count")).unwrap_or_default().lines().count()
    }

    /// A Game reviewed through the store: marked, kept with its Edition, shown again with the
    /// engine gone, and a second Game thinks only about the position the first had not.
    #[cfg(unix)]
    #[test]
    fn a_game_reviewed_through_the_store() {
        let d = crate::test_dir("review");
        let p = fake(&d);
        let game = Position::new("chess", &["e2e4", "e7e5", "g1f3"]);
        let r = wait(&Reviewer::start(&setup(&d, &p, &[1, 2, 3, 4]), &game));
        assert_eq!((r.engine.as_str(), r.plies.len(), r.error.as_deref()), ("Fake 2", 3, None));
        assert_eq!(thought(&d), 4);
        assert_eq!(r.plies.iter().map(|p| p.eval.as_str()).collect::<Vec<_>>(), ["−0.20", "+0.20", "−5.00"]);
        assert_eq!(r.plies.iter().map(|p| p.mark).collect::<Vec<_>>(), [None, None, Some(Mark::Blunder)]);
        assert!(r.plies[1].better.is_empty(), "e7e5 was the engine's move");
        assert_eq!(r.plies[2].better, ["e7e5", "g1f3"]);

        let store = Store::open(&d.join("positions.sqlite")).unwrap();
        let e = store.shown("chess", "chess", 4, "fake").unwrap().unwrap();
        assert_eq!((e.edition.version.as_str(), e.score, e.depth), ("Fake 2", bragd_positions::Score::Cp(-500), 3));
        let a = store.annotation("chess", "chess", 3, "g1f3", "fake").unwrap().unwrap();
        assert_eq!(a.mark, Some(Mark::Blunder));
        drop(store);

        // Again with the engine gone: all from the store.
        std::fs::remove_file(&p).unwrap();
        let again = wait(&Reviewer::start(&setup(&d, &p, &[1, 2, 3, 4]), &game));
        assert_eq!((again.error.as_deref(), again.plies.clone()), (None, r.plies.clone()));
        assert_eq!(again.engine, "Fake 2", "named by the store's Edition, never started");

        // Another Game through the same positions: one new position to think about.
        let p = fake(&d);
        let other = Position::new("chess", &["e2e4", "e7e5", "d2d4"]);
        let r = wait(&Reviewer::start(&setup(&d, &p, &[1, 2, 3, 5]), &other));
        assert_eq!((r.error.as_deref(), r.plies.len()), (None, 3));
        assert_eq!(thought(&d), 5);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn keys_that_do_not_fit_the_moves_are_an_error() {
        let d = crate::test_dir("review-keys");
        let s = Setup { keys: vec![1], ..setup(&d, &d.join("none"), &[]) };
        let r = Reviewer::start(&s, &Position::new("chess", &["e2e4"]));
        let t = std::time::Instant::now();
        while !r.now().done && t.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(r.now().error.is_some_and(|e| e.contains("1 position keys for 1 moves")));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
