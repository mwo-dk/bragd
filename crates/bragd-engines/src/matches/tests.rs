use super::*;
use crate::protocol::{About, Best, Event, Info, Position, Score, Start};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// How a fake engine plays.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// The first legal move, scoring the position `cp` from its own view.
    First(i32),
    /// Says `pass` (go).
    Pass,
    /// Names a move that is not legal.
    Illegal,
    /// Says `resign`.
    Resigns,
}

/// An engine in Rust: it reads the position as chess's Wire notation (or go's), plays as its
/// mode says, and names its move at once.
struct Fake {
    about: About,
    variant: String,
    mode: Mode,
    pos: Position,
    out: VecDeque<Event>,
    /// What it was told to think within, move by move.
    told: Arc<Mutex<Vec<Limit>>>,
}

fn fake(variant: &str, mode: Mode) -> Player {
    let notation = if variant.starts_with("go") { Notation::Gtp } else { Notation::Wire };
    Player { session: Box::new(Fake { about: About { name: "Fake".into(), options: vec![] }, variant: variant.into(), mode, pos: Position::default(), out: VecDeque::new(), told: Default::default() }), notation }
}

impl Session for Fake {
    fn about(&self) -> &About {
        &self.about
    }
    fn set_option(&mut self, _: &str, _: &str) -> std::io::Result<()> {
        Ok(())
    }
    fn set_position(&mut self, p: &Position) -> std::io::Result<()> {
        self.pos = p.clone();
        Ok(())
    }
    fn go(&mut self, limit: Limit, _: u32) -> std::io::Result<()> {
        self.told.lock().unwrap().push(limit);
        let initial = match &self.pos.start {
            Start::Setup(s) => Some(s.as_str()),
            Start::Initial => None,
        };
        let mv = match self.mode {
            Mode::First(cp) => {
                let s = bragd_games::replay(&self.variant, initial, &self.pos.moves).unwrap();
                self.out.push_back(Event::Info(Info { line: 1, score: Some(Score::Cp(cp)), ..Default::default() }));
                s.moves.iter().map(|m| m.wire.clone()).find(|w| !talk(w))
            }
            Mode::Pass => Some("pass".into()),
            Mode::Illegal => Some("a1a1".into()),
            Mode::Resigns => Some("resign".into()),
        };
        self.out.push_back(Event::Best(Best { mv, ponder: None }));
        Ok(())
    }
    fn next(&mut self, _: Duration) -> std::io::Result<Option<Event>> {
        Ok(self.out.pop_front())
    }
    fn stop(&mut self) -> std::io::Result<()> {
        Ok(())
    }
    fn quit(&mut self) {}
}

const NO_TABLE: Table = &|_, _| None;
const NEVER: &(dyn Fn() -> bool + Sync) = &|| false;

fn adj(max_moves: u32) -> Adjudication {
    Adjudication { max_moves: Some(max_moves), ..Default::default() }
}

fn one(variant: &str, a: Mode, b: Mode, adjudication: &Adjudication, table: Table) -> Played {
    let mut players = [fake(variant, a), fake(variant, b)];
    play(variant, &Opening::default(), &mut players, Limit::Depth(1), None, adjudication, table, NEVER).unwrap()
}

#[test]
fn schedules_play_each_opening_from_both_sides() {
    let rr = schedule(&Kind::RoundRobin, 3, 2);
    assert_eq!(rr.len(), 3 * 2 * 2);
    for pair in rr.chunks(2) {
        assert_eq!((pair[0].opening, pair[0].first, pair[0].second), (pair[1].opening, pair[1].second, pair[1].first));
    }
    assert_eq!(rr.iter().map(|g| g.opening).max(), Some(5));
    let g = schedule(&Kind::Gauntlet, 4, 1);
    assert_eq!(g.len(), 6);
    assert!(g.iter().all(|g| g.first == 0 || g.second == 0));
    let s = schedule(&Kind::Sprt { elo0: 0.0, elo1: 5.0, alpha: 0.05, beta: 0.05 }, 2, 3);
    assert_eq!(s.iter().map(|g| (g.first, g.second)).collect::<Vec<_>>(), [(0, 1), (1, 0), (0, 1), (1, 0), (0, 1), (1, 0)]);
}

#[test]
fn a_game_too_long_is_a_draw() {
    let p = one("chess", Mode::First(20), Mode::First(20), &adj(5), NO_TABLE);
    assert_eq!((p.ended, p.reason.as_str(), p.entries.len(), p.result.as_str()), (Ended::Draw, "length", 10, "1/2-1/2"));
    assert_eq!(p.history.len(), 10);
}

#[test]
fn an_engine_sure_it_is_lost_resigns() {
    // The first Side sees itself 10 pawns down: after its fourth move it resigns.
    let p = one("chess", Mode::First(-1000), Mode::First(0), &Adjudication { draw_cp: None, ..adj(50) }, NO_TABLE);
    assert_eq!((p.ended, p.reason.as_str(), p.entries.len()), (Ended::Second, "resignation", 7));
    // The second Side, likewise.
    let p = one("chess", Mode::First(0), Mode::First(-1000), &Adjudication { draw_cp: None, ..adj(50) }, NO_TABLE);
    assert_eq!((p.ended, p.entries.len()), (Ended::First, 8));
    // Without resigning, it plays on.
    let p = one("chess", Mode::First(-1000), Mode::First(0), &Adjudication { resign_cp: None, draw_cp: None, ..adj(6) }, NO_TABLE);
    assert_eq!(p.reason, "length");
}

#[test]
fn a_level_stretch_is_a_draw_only_after_its_move() {
    let a = Adjudication { draw_after: 3, draw_moves: 2, ..adj(50) };
    let p = one("chess", Mode::First(5), Mode::First(-5), &a, NO_TABLE);
    assert_eq!((p.ended, p.reason.as_str(), p.entries.len()), (Ended::Draw, "adjudication", 6));
    // Not level enough: no draw.
    let p = one("chess", Mode::First(50), Mode::First(-50), &Adjudication { resign_cp: None, ..a }, NO_TABLE);
    assert_eq!(p.reason, "length");
}

#[test]
fn a_wrong_move_or_none_loses() {
    let p = one("chess", Mode::Illegal, Mode::First(0), &adj(50), NO_TABLE);
    assert_eq!((p.ended, p.reason.as_str(), p.entries.len()), (Ended::Second, "illegal move", 0));
    let p = one("chess", Mode::First(0), Mode::Resigns, &adj(50), NO_TABLE);
    assert_eq!((p.ended, p.reason.as_str(), p.entries.len()), (Ended::First, "resignation", 1));
}

#[test]
fn a_tablebase_ends_the_game_it_knows() {
    let table: Table = &|s, _| (s.history.len() >= 3).then_some(Ended::First);
    let p = one("chess", Mode::First(0), Mode::First(0), &adj(50), table);
    assert_eq!((p.ended, p.reason.as_str(), p.entries.len()), (Ended::First, "tablebase", 3));
    let p = one("chess", Mode::First(0), Mode::First(0), &Adjudication { tablebases: false, ..adj(5) }, table);
    assert_eq!(p.reason, "length");
}

#[test]
fn go_is_counted_after_two_passes() {
    let p = one("go-9", Mode::Pass, Mode::Pass, &adj(50), NO_TABLE);
    // Black passes, White passes, no stones are dead, White agrees: White wins by the komi.
    assert_eq!(p.entries, ["pass", "pass", "dead:", "accept"]);
    assert_eq!((p.ended, p.result.as_str(), p.reason.as_str()), (Ended::Second, "W+7.5", "counted"));
}

#[test]
fn backgammon_dice_are_thrown_for_the_engines() {
    let p = one("backgammon", Mode::Resigns, Mode::Resigns, &adj(50), NO_TABLE);
    // The opening roll is thrown, then whoever is on roll resigns.
    assert_eq!(p.entries.len(), 1);
    assert_eq!(p.reason, "resignation");
    assert!(bragd_games::replay("backgammon", None, &p.entries).unwrap().throws.is_empty());
}

fn setup(kind: Kind, engines: usize, rounds: u32, parallel: u32, adjudication: Adjudication) -> Setup {
    Setup {
        variant: "chess".into(),
        engines: (0..engines).map(|i| format!("e{i}")).collect(),
        kind,
        rounds,
        limit: Limit::Depth(1),
        clock: None,
        parallel,
        book: None,
        book_depth: 8,
        adjudication,
        library: "x".into(),
        seed: 0,
    }
}

#[test]
fn a_round_robin_plays_every_game_in_parallel() {
    let s = setup(Kind::RoundRobin, 3, 1, 2, adj(3));
    let games = schedule(&s.kind, 3, s.rounds);
    let heard = AtomicUsize::new(0);
    let r = Run {
        setup: &s,
        games: &games,
        opening: &|_| Opening::default(),
        start: &|_| Ok(fake("chess", Mode::First(30))),
        table: NO_TABLE,
        stop: NEVER,
        done: &|_, p, _| {
            assert_eq!(p.reason, "length");
            heard.fetch_add(1, Ordering::Relaxed);
        },
    };
    let results = run(&r, unplayed(games.len())).unwrap();
    assert_eq!(results, "======");
    assert_eq!(heard.load(Ordering::Relaxed), 6);
    let st = standings(&games, &results, 3);
    assert!(st.iter().all(|s| (s.points, s.draws) == (2.0, 4)));
    assert!(st[0].elo.unwrap().0.abs() < 1e-9);
    // Started again with everything played: nothing more is played.
    assert_eq!(run(&r, results.clone()).unwrap(), results);
    assert_eq!(heard.load(Ordering::Relaxed), 6);
}

#[test]
fn an_sprt_stops_once_it_decides() {
    let s = setup(Kind::Sprt { elo0: 0.0, elo1: 50.0, alpha: 0.05, beta: 0.05 }, 2, 500, 1, Adjudication { draw_cp: None, ..adj(50) });
    let games = schedule(&s.kind, 2, s.rounds);
    // Engine 1 always sees itself lost, so engine 0 wins every Game.
    let r = Run { setup: &s, games: &games, opening: &|_| Opening::default(), start: &|n| Ok(fake("chess", Mode::First(if n == 0 { 0 } else { -1000 }))), table: NO_TABLE, stop: NEVER, done: &|_, _, _| {} };
    let results = run(&r, unplayed(games.len())).unwrap();
    let played = results.chars().filter(|c| *c != '.').count();
    assert!(played > 0 && played < games.len(), "{played} games");
    let st = sprt_state(&s.kind, &games, &results).unwrap();
    assert_eq!(st.accepted, Some(true));
    assert!(st.llr >= st.upper);
    assert_eq!(standings(&games, &results, 2)[0].engine, 0);
}

#[test]
fn game_pairs_and_the_sprt_state() {
    let kind = Kind::Sprt { elo0: 0.0, elo1: 5.0, alpha: 0.05, beta: 0.05 };
    let games = schedule(&kind, 2, 3);
    // Pair 1: engine 0 wins both. Pair 2: a draw and a loss. Pair 3: half played.
    assert_eq!(pentanomial(&games, "10=11."), [0, 1, 0, 0, 1]);
    let st = sprt_state(&kind, &games, "10=11.").unwrap();
    assert!(st.accepted.is_none() && st.llr > 0.0);
    assert_eq!(sprt_state(&kind, &games, "......").unwrap().llr, 0.0);
    assert!(sprt_state(&Kind::RoundRobin, &games, "......").is_none());
}

#[test]
fn an_engine_that_cannot_start_stops_the_match() {
    let s = setup(Kind::Gauntlet, 3, 1, 2, adj(3));
    let games = schedule(&s.kind, 3, s.rounds);
    let r = Run { setup: &s, games: &games, opening: &|_| Opening::default(), start: &|n| if n == 2 { Err("e2 is not there".into()) } else { Ok(fake("chess", Mode::First(0))) }, table: NO_TABLE, stop: NEVER, done: &|_, _, _| {} };
    assert_eq!(run(&r, unplayed(games.len())), Err("e2 is not there".into()));
}

/// With a clock, each engine is told its own time and the other's, each Side once the first;
/// the Game is played as without one.
#[test]
fn a_clock_tells_each_engine_its_time() {
    let told: [Arc<Mutex<Vec<Limit>>>; 2] = Default::default();
    let mut players = [0, 1].map(|i| {
        let mut f = Fake { about: About::default(), variant: "chess".into(), mode: Mode::First(0), pos: Position::default(), out: VecDeque::new(), told: told[i].clone() };
        f.about.name = "Fake".into();
        Player { session: Box::new(f), notation: Notation::Wire }
    });
    let tc = TimeControl { base_ms: 60_000, inc_ms: 1_000, moves: Some(40) };
    let p = play("chess", &Opening::default(), &mut players, Limit::Depth(1), Some(tc), &adj(3), NO_TABLE, NEVER).unwrap();
    assert_eq!(p.reason, "length");
    let (a, b) = (told[0].lock().unwrap().clone(), told[1].lock().unwrap().clone());
    assert_eq!((a.len(), b.len()), (3, 3));
    assert_eq!(a[0], Limit::Clock { own: 60_000, other: 60_000, inc: 1_000, to_go: Some(40), first: true });
    let Limit::Clock { own, other, to_go, first, .. } = b[0] else { panic!("{:?}", b[0]) };
    // The first Side's move took some milliseconds at most, and earned its second.
    assert_eq!((own, to_go, first), (60_000, Some(40), false));
    assert!((60_000..=61_000).contains(&other), "{other}");
    assert!(matches!(a[2], Limit::Clock { to_go: Some(38), .. }));
}
