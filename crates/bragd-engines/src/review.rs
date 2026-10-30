//! A whole Game reviewed by an engine: each position of its moves evaluated in the background,
//! the moves that lose marked `?!`, `?` or `??` by how much they lose (and `!?` for the only
//! move that holds), kept in a cache per Game, and written out with the marks, the evaluations
//! and the engine's better lines. Marks need centipawns, so only Families whose engines score
//! in them (chess, xiangqi, shogi, draughts) get marks; go and backgammon get evaluations.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::options::OptionStore;
use crate::protocol::{Limit, MATE_CP, Position, Protocol};
use crate::session::{Engine, Program, Session, think};

/// What a lost advantage is worth, in centipawns from the mover's side: at least this much is
/// `?!` (an inaccuracy), `?` (a mistake), `??` (a blunder). Evaluations count up to `CAP` either
/// way, so a move in a won position is not a blunder for winning a little less.
pub const INACCURACY: i32 = 50;
pub const MISTAKE: i32 = 100;
pub const BLUNDER: i32 = 300;
pub const CAP: i32 = MATE_CP;
/// `!?`: the engine's best move, played, where its second choice is this much worse, in a
/// position not already won.
pub const ONLY_MOVE: i32 = 300;

/// One move, evaluated.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ply {
    /// Its place among the Game's moves, from 0.
    pub index: usize,
    /// The evaluation after it, from the first Side's view, and as shown (`+0.35`, `#3`).
    pub cp: i32,
    pub eval: String,
    /// Its mark as a NAG: 6 `?!`, 2 `?`, 4 `??`, 5 `!?`.
    pub mark: Option<u8>,
    /// What the engine would have played instead, in its notation; empty when it was played.
    pub best: Vec<String>,
}

/// A Game's review, as far as it has come.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Review {
    pub engine: String,
    /// The start position's evaluation, then each move's.
    pub start: Option<i32>,
    pub plies: Vec<Ply>,
    /// Of how many moves; done when all are in, or it stopped.
    pub total: usize,
    pub done: bool,
    pub error: Option<String>,
}

/// The mark of a move that took the evaluation from `before` to `after` (first Side's view), by
/// the first Side (`first`) or the second; `only`: it is the engine's best move, and by how much
/// its second choice is worse.
pub fn mark(before: i32, after: i32, first: bool, only: Option<i32>) -> Option<u8> {
    let side = |v: i32| if first { v } else { -v };
    let lost = side(before).clamp(-CAP, CAP) - side(after).clamp(-CAP, CAP);
    if lost >= BLUNDER {
        Some(4)
    } else if lost >= MISTAKE {
        Some(2)
    } else if lost >= INACCURACY {
        Some(6)
    } else if only.is_some_and(|gap| gap >= ONLY_MOVE) && side(before) < MISTAKE * 3 {
        Some(5)
    } else {
        None
    }
}

/// The evaluation as shown: `+0.35`, `−1.20`, `#3`, `#−2`.
pub fn shown(v: i32) -> String {
    if v.abs() > CAP {
        let m = CAP + 100 - v.abs();
        return if v > 0 { format!("#{m}") } else { format!("#−{m}") };
    }
    let p = v.abs() as f64 / 100.0;
    if v < 0 { format!("−{p:.2}") } else { format!("+{p:.2}") }
}

/// The evaluation as PGN's `[%eval]` writes it: `0.35`, `-1.20`, `#3`, `#-2`.
fn pgn_eval(v: i32) -> String {
    shown(v).replace('−', "-").replace('+', "")
}

/// The first Side's share of the Game, 0 to 1, for the bar beside the board and the graph.
pub fn share(v: i32) -> f64 {
    1.0 / (1.0 + (-(v.clamp(-CAP, CAP)) as f64 / 250.0).exp())
}

/// How a review is run: the engine, its protocol and options, the tablebases, the limit per
/// position, and where reviews are cached.
#[derive(Clone, Debug)]
pub struct Setup {
    pub program: Program,
    pub protocol: Protocol,
    pub options: OptionStore,
    pub syzygy: Option<String>,
    pub limit: Limit,
    pub cache: Option<PathBuf>,
}

/// Where the review of `game` by `engine` with `settings` is kept: by a hash of all three, so
/// another engine, limit or option is another review.
fn cached(dir: &Path, engine: &str, settings: &str, game: &Position) -> PathBuf {
    // FNV-1a: stable across builds, unlike the standard hasher.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |s: &str| {
        for b in s.bytes().chain([0]) {
            h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
    };
    feed(engine);
    feed(settings);
    feed(&game.variant);
    feed(&format!("{:?}", game.start));
    for m in &game.moves {
        feed(m);
    }
    dir.join(format!("{h:016x}.json"))
}

/// How many reviews the cache keeps: the oldest go past it (a few KB each).
const KEPT: usize = 1000;

/// The newest `most` reviews in `dir` kept, the rest removed.
fn keep_newest(dir: &Path, most: usize) {
    let mut all: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    if all.len() > most {
        all.sort();
        for (_, p) in &all[..all.len() - most] {
            let _ = std::fs::remove_file(p);
        }
    }
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
    /// Reviews `game` as `setup` says. A Game reviewed before (same engine, settings and
    /// moves) comes from the cache at once.
    pub fn start(setup: &Setup, game: &Position) -> Reviewer {
        let settings = format!("{:?} {:?} {:?}", setup.limit, setup.options, setup.syzygy);
        let file = setup.cache.as_ref().map(|d| cached(d, &setup.program.key(), &settings, game));
        let shared = Arc::new(Mutex::new(Review { total: game.moves.len(), ..Default::default() }));
        let stop = Arc::new(AtomicBool::new(false));
        if let Some(r) = file.as_ref().and_then(|f| std::fs::read(f).ok()).and_then(|b| serde_json::from_slice::<Review>(&b).ok())
            && r.done
            && r.error.is_none()
        {
            *shared.lock().unwrap_or_else(|e| e.into_inner()) = r;
            return Reviewer { shared, stop };
        }
        let (s, st, g, setup) = (shared.clone(), stop.clone(), game.clone(), setup.clone());
        std::thread::spawn(move || {
            let r = run(&setup, &g, &s, &st);
            let mut out = s.lock().unwrap_or_else(|e| e.into_inner());
            out.done = true;
            if let Err(e) = r {
                if e.kind() != std::io::ErrorKind::Interrupted {
                    out.error = Some(e.to_string());
                }
            } else if let Some(f) = file
                && std::fs::create_dir_all(f.parent().unwrap_or(Path::new("."))).is_ok()
                && let Ok(json) = serde_json::to_vec(&*out)
            {
                let _ = std::fs::write(&f, json);
                if let Some(dir) = f.parent() {
                    keep_newest(dir, KEPT);
                }
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

fn run(setup: &Setup, game: &Position, shared: &Mutex<Review>, stop: &AtomicBool) -> std::io::Result<()> {
    let halt = || stop.load(Ordering::Relaxed);
    let mut engine = Engine::start(&setup.program, setup.protocol, Duration::from_secs(30), &halt)?;
    // Options it does not take: the live analysis says so; the Game is reviewed without them.
    engine.configure(&setup.options, setup.syzygy.as_deref())?;
    shared.lock().unwrap_or_else(|e| e.into_inner()).engine = engine.about().name.clone();
    // The position before each move: its score, the engine's line, and how much worse its
    // second choice is (from the mover's side).
    let mut before: Option<(i32, Vec<String>, Option<i32>)> = None;
    for i in 0..=game.moves.len() {
        let pos = Position { moves: game.moves[..i].to_vec(), ..game.clone() };
        let a = think(&mut engine, &pos, setup.limit, 2, &halt, &mut |_| {})?;
        let first = pos.first_to_move();
        let score = |n: usize| a.lines.get(n).and_then(|l| l.score?.cp());
        // No score at all: no move, and no mate said: a draw.
        let v = score(0).unwrap_or(0);
        let gap = score(0).zip(score(1)).map(|(x, y)| if first { x - y } else { y - x });
        let line: Vec<String> = a.lines.first().map(|l| l.pv.iter().take(6).cloned().collect()).unwrap_or_default();
        let mut r = shared.lock().unwrap_or_else(|e| e.into_inner());
        match before.take() {
            None => r.start = Some(v),
            Some((prev, best, only)) => {
                let played = &game.moves[i - 1];
                let played_best = best.first() == Some(played);
                r.plies.push(Ply {
                    index: i - 1,
                    cp: v,
                    eval: shown(v),
                    mark: mark(prev, v, !first, only.filter(|_| played_best)),
                    best: if played_best { vec![] } else { best },
                });
            }
        }
        before = Some((v, line, gap));
    }
    Ok(())
}

/// The Game written out with its review, in PGN's way: `tags` (`Annotator` the engine), then
/// `moves` as shown (SAN for chess), numbered from `number` with the first Side moving first
/// when `first`; on each reviewed move its mark (a NAG), its evaluation as `[%eval 0.35]`, and
/// for a marked move the engine's line as a variation, as `line` writes it (its moves as shown,
/// numbered), from the move's index and the engine's moves.
pub fn annotated(tags: &[(String, String)], review: &Review, moves: &[String], number: u32, first: bool, line: impl Fn(usize, &[String]) -> Option<String>) -> String {
    let esc = |v: &str| v.replace('\\', "\\\\").replace('"', "\\\"");
    let mut out = String::new();
    for (k, v) in tags.iter().filter(|(k, _)| !k.eq_ignore_ascii_case("Annotator")) {
        out.push_str(&format!("[{k} \"{}\"]\n", esc(v)));
    }
    out.push_str(&format!("[Annotator \"{}\"]\n\n", esc(&review.engine)));
    let result = tags.iter().find(|(k, _)| k.eq_ignore_ascii_case("Result")).map_or("*", |(_, v)| v.as_str());
    let mut words: Vec<String> = vec![];
    let (mut n, mut mover_first, mut after_note) = (number, first, true);
    for (i, mv) in moves.iter().enumerate() {
        if mover_first {
            words.push(format!("{n}."));
        } else if after_note {
            words.push(format!("{n}..."));
        }
        words.push(mv.clone());
        after_note = false;
        if let Some(p) = review.plies.iter().find(|p| p.index == i) {
            words.extend(p.mark.map(|m| format!("${m}")));
            words.push(format!("{{[%eval {}]}}", pgn_eval(p.cp)));
            if p.mark.is_some_and(|m| m != 5)
                && !p.best.is_empty()
                && let Some(text) = line(i, &p.best)
            {
                words.push(format!("({text})"));
            }
            after_note = true;
        }
        if !mover_first {
            n += 1;
        }
        mover_first = !mover_first;
    }
    words.push(result.to_string());
    let mut text = String::new();
    for w in words {
        if !text.is_empty() && text.len() + 1 + w.len() > 79 {
            out.push_str(&text);
            out.push('\n');
            text.clear();
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&w);
    }
    out.push_str(&text);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Score;

    #[test]
    fn marks_by_what_is_lost() {
        // White from +0.30 to −0.10: 40 lost, nothing; to −0.30: 60, ?!; to −1.00: ?; to −3.00: ??.
        assert_eq!(mark(30, -10, true, None), None);
        assert_eq!(mark(30, -30, true, None), Some(6));
        assert_eq!(mark(30, -100, true, None), Some(2));
        assert_eq!(mark(30, -300, true, None), Some(4));
        // Black's side: White's score going up is Black losing.
        assert_eq!(mark(-30, 300, false, None), Some(4));
        // Winning a little less in a won position is no blunder.
        assert_eq!(mark(2000, 1000, true, None), None);
        // The only move that holds, not in a position already won.
        assert_eq!(mark(0, 0, true, Some(400)), Some(5));
        assert_eq!(mark(0, 0, true, Some(200)), None);
        assert_eq!(mark(900, 900, true, Some(400)), None);
        assert_eq!((shown(35), shown(-120), shown(Score::Mate(3).cp().unwrap()), shown(Score::Mate(-2).cp().unwrap())), ("+0.35".into(), "−1.20".into(), "#3".into(), "#−2".into()));
        assert!(share(300) > 0.7 && share(-300) < 0.3 && (share(0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn the_review_cache_keeps_its_newest() {
        let d = crate::test_dir("review-kept");
        for (i, days) in [(0, 3), (1, 2), (2, 1)] {
            let f = d.join(format!("{i}.json"));
            std::fs::write(&f, "{}").unwrap();
            let t = std::time::SystemTime::now() - Duration::from_secs(days * 86400);
            std::fs::File::options().write(true).open(&f).unwrap().set_modified(t).unwrap();
        }
        keep_newest(&d, 2);
        assert!(!d.join("0.json").exists());
        assert!(d.join("1.json").exists() && d.join("2.json").exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn an_annotated_game() {
        let r = Review {
            engine: "Fake \"1\"".into(),
            start: Some(30),
            plies: vec![
                Ply { index: 0, cp: 30, eval: "+0.30".into(), mark: None, best: vec![] },
                Ply { index: 5, cp: Score::Mate(5).cp().unwrap(), eval: "#5".into(), mark: Some(4), best: vec!["g7g6".into(), "h5f3".into()] },
            ],
            total: 7,
            done: true,
            error: None,
        };
        let sans: Vec<String> = ["e4", "e5", "Qh5", "Nc6", "Bc4", "Nf6", "Qxf7#"].map(String::from).to_vec();
        let tags = [("White", "A"), ("Annotator", "Old"), ("Result", "1-0")].map(|(a, b)| (a.to_string(), b.to_string()));
        let text = annotated(&tags, &r, &sans, 1, true, |i, best| (i == 5 && best[0] == "g7g6").then(|| "3... g6 4. Qf3".to_string()));
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(text.contains("[Annotator \"Fake \\\"1\\\"\"]") && !text.contains("Old"), "{text}");
        assert!(flat.contains("1. e4 {[%eval 0.30]} 1... e5 2. Qh5 Nc6 3. Bc4 Nf6 $4 {[%eval #5]} (3... g6 4. Qf3) 4. Qxf7# 1-0"), "{text}");
    }

    /// The whole Game by a fake engine that says +0.20 for the side to move: every position the
    /// same for White, so no marks; Black's best move played; then from the cache.
    #[cfg(unix)]
    #[test]
    fn a_game_reviewed_and_cached() {
        let d = crate::test_dir("review");
        let p = crate::fake_program(&d, "engine", "while read l; do case \"$l\" in uci) echo 'id name Fake 2'; echo uciok;; isready) echo readyok;; go*) echo 'info depth 3 multipv 1 score cp 20 pv e7e5'; echo 'bestmove e7e5';; quit) exit 0;; esac; done");
        let setup = Setup { program: Program::Installed { path: p.clone(), args: vec![] }, protocol: Protocol::Uci, options: OptionStore::default(), syzygy: None, limit: Limit::Time(50), cache: Some(d.join("cache")) };
        let game = Position::new("chess", &["e2e4", "e7e5", "g1f3"]);
        let wait = |r: &Reviewer| {
            let t = std::time::Instant::now();
            while !r.now().done && t.elapsed() < Duration::from_secs(20) {
                std::thread::sleep(Duration::from_millis(10));
            }
            r.now()
        };
        let r = wait(&Reviewer::start(&setup, &game));
        assert_eq!((r.engine.as_str(), r.plies.len(), r.error.as_deref(), r.start), ("Fake 2", 3, None, Some(20)));
        assert_eq!(r.plies.iter().map(|p| p.cp).collect::<Vec<_>>(), [-20, 20, -20]);
        assert!(r.plies.iter().all(|p| p.mark.is_none()), "{r:?}");
        assert!(r.plies[1].best.is_empty(), "e7e5 was the engine's move");
        assert_eq!(r.plies[0].best, ["e7e5"]);
        // From the cache, even with the engine gone; another limit is another review.
        std::fs::remove_file(&p).unwrap();
        let again = Reviewer::start(&setup, &game).now();
        assert!(again.done && again.plies.len() == 3, "{again:?}");
        let other = Setup { limit: Limit::Depth(5), ..setup.clone() };
        assert_ne!(cached(Path::new("/c"), "x", &format!("{:?}", other.limit), &game), cached(Path::new("/c"), "x", &format!("{:?}", setup.limit), &game));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
