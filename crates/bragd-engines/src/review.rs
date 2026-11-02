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
use bragd_positions::{Annotation, Edition, Evaluation, Second, Store};

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
    /// Each position as the Engine is asked about it, as `keys`, when that is not the Game's
    /// start and the moves to it (backgammon, `spots`); empty: it is. None: not asked about.
    pub spots: Vec<Option<Spot>>,
}

/// A position as the Engine is asked about it, when the moves to it do not say (backgammon: a
/// position before the roll, or a roll to play).
#[derive(Clone, Debug)]
pub struct Spot {
    pub position: Position,
    /// The first Side acts in it.
    pub first: bool,
    /// The move made in it is judged: a backgammon play, not a throw of the dice or the cube.
    pub judged: bool,
}

/// A backgammon Game's positions as GNU Backgammon is asked about them: each before its roll,
/// or its roll to play. A position waiting on the cube is not asked about.
pub fn backgammon_spots(variant: &str, initial: Option<&str>, entries: &[String]) -> Vec<Option<Spot>> {
    (0..=entries.len())
        .map(|i| {
            let (position, state) = crate::seam::position(variant, initial, &entries[..i], crate::seam::Notation::Gnubg).ok()?;
            let judged = position.dice.is_some() && entries.get(i).is_some_and(|e| !matches!(e.as_str(), "double" | "take" | "drop"));
            Some(Spot { position, first: state.to_move?.index == 0, judged })
        })
        .collect()
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
        let total = if setup.spots.is_empty() { game.moves.len() } else { setup.spots.iter().flatten().filter(|s| s.judged).count() };
        let shared = Arc::new(Mutex::new(Review { engine: setup.engine.clone(), total, ..Default::default() }));
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

/// What a review knows of each position: its Evaluation, with the Engine's second choice when
/// it gave one.
struct Known {
    evals: Vec<Option<Evaluation>>,
}

fn run(setup: &Setup, game: &Position, shared: &Mutex<Review>, stop: &AtomicBool) -> std::io::Result<()> {
    let halt = || stop.load(Ordering::Relaxed);
    let n = game.moves.len();
    if setup.keys.len() != n + 1 {
        return Err(std::io::Error::other(format!("{} position keys for {n} moves", setup.keys.len())));
    }
    let spot = |i: usize| setup.spots.get(i).and_then(Option::as_ref);
    // A backgammon Evaluation in the store is the one before the roll: a roll's is not kept.
    let kept = |i: usize| spot(i).is_none_or(|s| s.position.dice.is_none());
    let mut store = setup.store.as_deref().map(Store::open).transpose().map_err(io)?;
    // First what the store has, at once; then the Engine for the rest.
    let mut known = Known { evals: vec![None; n + 1] };
    if let Some(s) = &store {
        for (i, k) in setup.keys.iter().enumerate().filter(|(i, _)| kept(*i)) {
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
        let pos = match setup.spots.is_empty() {
            true => Position { moves: game.moves[..i].to_vec(), ..game.clone() },
            false => match spot(i) {
                Some(s) => s.position.clone(),
                None => continue,
            },
        };
        let a = think(e, &pos, setup.limit, 2, &halt, &mut |_| {})?;
        // `think` turns the scores round by who its position says moves; a Spot knows better.
        let flip = spot(i).is_some_and(|s| s.first != pos.first_to_move());
        let score = |l: usize| a.lines.get(l).and_then(|l| l.score).map(|s| if flip { s.flipped() } else { s }).map(bragd_positions::Score::from);
        // No score at all: no move, so nothing to judge it by.
        let Some(best) = score(0) else { continue };
        let pv = |l: usize| a.lines[l].pv.iter().take(LINE).cloned().collect::<Vec<_>>().join(" ");
        let eval = Evaluation {
            family: setup.family.clone(),
            variant: setup.variant.clone(),
            key: setup.keys[i],
            edition: edition.clone(),
            score: best,
            depth: a.depth,
            line: pv(0),
            time: now(),
            second: score(1).map(|score| Second { score, line: pv(1) }),
        };
        if let Some(s) = store.as_mut().filter(|_| kept(i)) {
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
    let judged = setup.spots.is_empty() || setup.spots.get(i).and_then(Option::as_ref).is_some_and(|s| s.judged);
    let (Some(before), Some(after), true) = (&known.evals[i], &known.evals[i + 1], judged) else { return Ok(()) };
    let mv = &game.moves[i];
    let depth = before.depth.min(after.depth);
    let stored = match &store {
        Some(s) => s.annotation(&setup.family, &setup.variant, setup.keys[i], mv, &setup.engine).map_err(io)?,
        None => None,
    };
    let note = match stored.filter(|a| a.edition == before.edition && a.depth >= depth) {
        Some(a) => a,
        None => {
            let a = made(setup, game, i, before, after);
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
        better: better_line(&note.better, setup.protocol == Protocol::Gnubg),
        motifs: note.motifs,
    };
    let mut r = shared.lock().unwrap_or_else(|e| e.into_inner());
    match r.plies.binary_search_by_key(&i, |p| p.index) {
        Ok(at) => r.plies[at] = ply,
        Err(at) => r.plies.insert(at, ply),
    }
    Ok(())
}

/// An Annotation's better line as moves. A GNU Backgammon play (`gnubg`) is one move with
/// spaces in it (`24/18 13/10`), and its line is that play alone.
pub fn better_line(better: &str, gnubg: bool) -> Vec<String> {
    match gnubg {
        true => Some(better.trim()).filter(|b| !b.is_empty()).map(String::from).into_iter().collect(),
        false => better.split_whitespace().map(String::from).collect(),
    }
}

/// Whether `mv` is the first move of the Engine's `line`: for GNU Backgammon (`gnubg`), the
/// whole line is one play, the same however its parts are ordered or written.
fn played_best(line: &str, mv: &str, gnubg: bool) -> bool {
    match gnubg {
        true => crate::seam::net(line).is_some_and(|l| crate::seam::net(mv).is_some_and(|m| l == m)),
        false => line.split_whitespace().next() == Some(mv),
    }
}

/// Move `i`'s Annotation from the Evaluations before and after it.
fn made(setup: &Setup, game: &Position, i: usize, before: &Evaluation, after: &Evaluation) -> Annotation {
    let mv = &game.moves[i];
    let mover_first = match setup.spots.get(i).and_then(Option::as_ref) {
        Some(s) => s.first,
        None => Position { moves: game.moves[..i].to_vec(), ..game.clone() }.first_to_move(),
    };
    let played_best = played_best(&before.line, mv, setup.protocol == Protocol::Gnubg);
    let gap = before.second.as_ref().filter(|_| played_best).and_then(|s| annotation::gap(before.score, s.score, mover_first));
    let mark = annotation::mark(before.score, after.score, mover_first, gap);
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
            spots: vec![],
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

    /// A backgammon Game reviewed by GNU Backgammon: each play judged by the equity it lost
    /// from the best play of its roll to the position it left before the next roll; the throws
    /// are not judged, and a roll's equity is not kept in the store.
    #[cfg(unix)]
    #[test]
    fn a_backgammon_game_is_reviewed_play_by_play() {
        let d = crate::test_dir("review-backgammon");
        // +0.10 before any roll for the side on roll; a roll's best play +0.30, its second +0.20.
        let p = crate::fake_program(
            &d,
            "gnubg",
            r#"while read l; do case "$l" in "show version") echo '(No game) GNU Backgammon 0.0 fake';; "set prompt"*) echo "(No game) The prompt has been set to \`bragd>'.";; eval) echo 'static: 0.5 0.1 0.0 0.1 0.0 +0.100 +0.1';; hint*) echo '    1. Cubeful 2-ply    24/1 24/2    Eq.:  +0.300'; echo '    2. Cubeful 2-ply    24/3 24/4    Eq.:  +0.200';; "show prompt") echo "bragd>The prompt is set to \`bragd>'.";; quit) exit 0;; esac; done"#,
        );
        let mut entries: Vec<String> = vec![];
        for _ in 0..4 {
            let st = bragd_games::replay("backgammon", None, &entries).unwrap();
            entries.push(st.throws.first().cloned().unwrap_or_else(|| st.moves.iter().find(|m| m.from.is_some()).unwrap().wire.clone()));
        }
        let spots = backgammon_spots("backgammon", None, &entries);
        assert_eq!(spots.iter().map(|s| s.as_ref().map(|s| (s.position.dice.is_some(), s.judged))).collect::<Vec<_>>(), [Some((false, false)), Some((true, true)), Some((false, false)), Some((true, true)), Some((false, false))]);
        let setup = Setup { protocol: Protocol::Gnubg, family: "backgammon".into(), variant: "backgammon".into(), spots, ..setup(&d, &p, &[1, 2, 1, 3, 4]) };
        let game = Position { moves: entries.clone(), ..Position::new("backgammon", &[]) };
        let r = wait(&Reviewer::start(&setup, &game));
        assert_eq!((r.error.as_deref(), r.total), (None, 2));
        assert_eq!(r.plies.iter().map(|p| (p.index, p.mark)).collect::<Vec<_>>(), [(1, Some(Mark::Blunder)), (3, Some(Mark::Blunder))], "0.40 lost: {:?}", r.plies);
        assert_eq!(r.plies[0].better, ["24/1 24/2"]);
        // The store keeps the positions before a roll (keys 1 and 4) only.
        let store = Store::open(&d.join("positions.sqlite")).unwrap();
        let equity = |k: u64| store.shown("backgammon", "backgammon", k, "fake").unwrap().map(|e| e.score);
        assert!(matches!(equity(4), Some(bragd_positions::Score::Equity(e)) if (e - 0.1).abs() < 1e-6), "{:?}", equity(4));
        assert_eq!((equity(2), equity(3)), (None, None), "a roll's equity is not kept");
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// The move played is the Engine's own when it is the first of its line; a backgammon play
    /// is the whole line, however its parts are written.
    #[test]
    fn the_engines_own_move_is_known() {
        assert!(played_best("e7e5 g1f3", "e7e5", false));
        assert!(!played_best("e7e5 g1f3", "g1f3", false));
        assert!(played_best("6/3 3/2", "6/3 3/2", true));
        assert!(played_best("8/5 6/5", "6/5 8/5", true));
        assert!(!played_best("6/3 3/2", "6/3", true));
        assert!(!played_best("24/18 13/10", "24/21 13/10", true));
    }

    /// The Engine's best move earns `!` or `!!` from the second choice kept with the
    /// Evaluation before it, so an Evaluation read from the store earns it as one just made does.
    #[test]
    fn the_second_choice_kept_with_an_evaluation_earns_good_and_brilliant() {
        let d = crate::test_dir("review-second");
        let s = setup(&d, &d.join("none"), &[1, 2]);
        let game = Position::new("chess", &["e2e4"]);
        let ev = |score, line: &str, second: Option<Second>| Evaluation {
            family: "chess".into(),
            variant: "chess".into(),
            key: 1,
            edition: Edition { engine: "fake".into(), version: "Fake 2".into(), network: None, image: None },
            score: bragd_positions::Score::Cp(score),
            depth: 20,
            line: line.into(),
            time: 1,
            second,
        };
        let second = |cp| Some(Second { score: bragd_positions::Score::Cp(cp), line: "d2d4".into() });
        let after = ev(20, "e7e5", None);
        let mark = |before: Evaluation| made(&s, &game, 0, &before, &after).mark;
        assert_eq!(mark(ev(20, "e2e4 e7e5", None)), None, "no second choice known");
        assert_eq!(mark(ev(20, "e2e4 e7e5", second(-130))), Some(Mark::Good));
        assert_eq!(mark(ev(20, "e2e4 e7e5", second(-400))), Some(Mark::Brilliant));
        assert_eq!(mark(ev(20, "d2d4 e7e5", second(-400))), None, "not the Engine's move");
        let _ = std::fs::remove_dir_all(&d);
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
