//! Janggi's correctness gate.
//!
//! **The perft numbers below are self-generated and are not verified against
//! any external corpus.** That has to be said plainly, because a perft table is
//! normally a claim about the rules and this one is not. Xiangqi's table in
//! `perft.rs` is worth gating on because two independent lineages produced the
//! same numbers from the same position; nobody has published one for janggi
//! that this engine could be checked against, and a number this engine produced
//! agrees with this engine by construction.
//!
//! So they are **regression baselines**: they say the move generator has not
//! changed since the day it was written, and they say nothing whatever about
//! whether it was right that day. If a rule below is later found to be wrong,
//! these numbers move, and that is the correct outcome rather than a failure to
//! be worked around.
//!
//! One relation in the table is *not* self-referential and is asserted as a
//! rule rather than a number: **the passes at one depth equal the nodes minus
//! the checks at the depth above.** Every position that is not in check offers
//! exactly one pass and every position that is in check offers none, so the two
//! columns have to agree, and they are produced by two entirely different parts
//! of the engine — the null move and the attack generator. A bug in either
//! breaks it.
//!
//! What carries the rest of the weight is the property tests, which state rules
//! rather than counts and check them over every position reached by thousands
//! of plies of random play:
//!
//! - no legal move leaves your own general attacked, the pass included;
//! - a cannon never moves without exactly one screen, and never over or onto
//!   another cannon;
//! - the elephant is stopped by either of its two intermediate points;
//! - a diagonal move is always along a line drawn on a palace.

use rules_xiangqi::{Move, Point, Position, Role, Ruleset, Side};

fn janggi(fen: &str) -> Position {
    Position::from_fen_with(Ruleset::Janggi, fen).expect("a valid position")
}

/// The tactically dense position the second table is taken from.
///
/// Chosen for what it exercises rather than for how it arose: cannons with
/// screens on both sides, a chariot on the f3 corner of Red's palace so the
/// palace lines are live, soldiers past the halfway line in both directions,
/// and both generals on the e-file behind enough material that bikjang is two
/// captures away.
const MIDDLEGAME: &str =
    "r1ba1ab2/4k4/1n1rc4/2p3p2/4p2c1/1C2P4/2P3P2/4CR1N1/3A1A3/R1B1K1B2 w - - 0 1";

#[derive(Default, PartialEq, Eq, Debug)]
struct Counts {
    nodes: u64,
    captures: u64,
    /// Counted in its own column rather than folded into the nodes, because
    /// the pass is the one move janggi has and no other game here does. A
    /// table that hid it would hide the rule.
    passes: u64,
    checks: u64,
}

fn perft(position: &Position, depth: u32, counts: &mut Counts) {
    for mv in position.legal_moves() {
        // A pass lands on the general's own point, so the ordinary test for a
        // capture would count every pass as one.
        let capture = !mv.is_pass() && position.piece_at(mv.to).is_some();
        let after = position.make_move(mv);
        if depth == 1 {
            counts.nodes += 1;
            if capture {
                counts.captures += 1;
            }
            if mv.is_pass() {
                counts.passes += 1;
            }
            if after.is_in_check(after.to_move()) {
                counts.checks += 1;
            }
        } else {
            perft(&after, depth - 1, counts);
        }
    }
}

fn count(position: &Position, depth: u32) -> Counts {
    let mut counts = Counts::default();
    perft(position, depth, &mut counts);
    counts
}

/// Nodes, captures, passes, checks. **Self-generated; see the module note.**
const START_COUNTS: &[(u64, u64, u64, u64)] = &[
    (32, 0, 1, 0),
    (1_024, 0, 32, 0),
    (33_348, 77, 1_024, 14),
    (1_085_579, 3_431, 33_334, 782),
    (36_380_045, 235_404, 1_084_797, 79_827),
];

/// **Self-generated; see the module note.**
const MIDDLEGAME_COUNTS: &[(u64, u64, u64, u64)] = &[
    (53, 4, 1, 4),
    (2_351, 197, 49, 52),
    (115_179, 8_787, 2_299, 7_389),
    (5_040_987, 417_147, 107_790, 120_374),
    (236_670_057, 18_343_793, 4_920_613, 13_375_325),
];

fn check(position: &Position, table: &[(u64, u64, u64, u64)], upto: usize, name: &str) {
    let mut previous: Option<Counts> = None;
    for (index, &(nodes, captures, passes, checks)) in table.iter().take(upto).enumerate() {
        let depth = index as u32 + 1;
        let counted = count(position, depth);
        assert_eq!(
            counted,
            Counts {
                nodes,
                captures,
                passes,
                checks
            },
            "{name} at depth {depth}"
        );
        // The one relation here that is a rule rather than a recorded number:
        // exactly the positions that are not in check offer a pass.
        if let Some(above) = previous {
            assert_eq!(
                counted.passes,
                above.nodes - above.checks,
                "{name}: passes at depth {depth} against checks at depth {}",
                depth - 1
            );
        }
        previous = Some(counted);
    }
}

#[test]
fn the_janggi_opening_array_is_what_the_rules_say() {
    let start = Position::janggi();
    assert_eq!(start.side(Side::Red).count(), 16);
    assert_eq!(start.pieces(Side::Red, Role::Soldier).count(), 5);
    assert_eq!(start.pieces(Side::Black, Role::Cannon).count(), 2);
    assert_eq!(start.general(Side::Red), Point::parse("e2"));
    assert_eq!(
        start
            .legal_moves()
            .iter()
            .filter(|mv| start.piece_at(mv.from).map(|p| p.role) == Some(Role::Cannon))
            .count(),
        0,
        "neither cannon can move at all: every line out of b3 and h3 is either \
         empty, ends at the piece it would leap, or is screened by the other cannon"
    );
}

#[test]
fn start_shallow() {
    check(&Position::janggi(), START_COUNTS, 3, "the opening position");
}

#[test]
fn middlegame_shallow() {
    check(&janggi(MIDDLEGAME), MIDDLEGAME_COUNTS, 3, "the middlegame");
}

#[test]
#[ignore = "minutes: run with --release -- --ignored"]
fn start_deep() {
    check(&Position::janggi(), START_COUNTS, 5, "the opening position");
}

#[test]
#[ignore = "minutes: run with --release -- --ignored"]
fn middlegame_deep() {
    check(&janggi(MIDDLEGAME), MIDDLEGAME_COUNTS, 5, "the middlegame");
}

/// Every position a few thousand plies of random janggi passes through.
///
/// The properties below are about *all* positions, and a handful of composed
/// ones would only ever test the cases their author thought of. Random play
/// reaches endgames, palaces stripped of guards, and cannons on the same line,
/// which is where these rules are easiest to get wrong.
fn playout(seed: u64, games: usize) -> Vec<Position> {
    let mut state = seed;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut seen = Vec::new();
    for _ in 0..games {
        let mut position = Position::janggi();
        for _ in 0..160 {
            seen.push(position);
            let moves = position.legal_moves();
            if moves.is_empty() {
                break;
            }
            position = position.make_move(moves[(next() % moves.len() as u64) as usize]);
        }
    }
    seen
}

/// The points strictly between two points on one line, diagonal or orthogonal.
fn between(from: Point, to: Point) -> Vec<Point> {
    let df = to.file() as i16 - from.file() as i16;
    let dr = to.rank() as i16 - from.rank() as i16;
    let steps = df.abs().max(dr.abs());
    (1..steps)
        .map(|step| {
            from.offset(df.signum() * step, dr.signum() * step)
                .expect("a point on the line")
        })
        .collect()
}

#[test]
fn no_legal_move_leaves_your_own_general_attacked() {
    // The pass included, which is the case worth stating: a pass changes
    // nothing, so a player in check who passed would still be in check, and
    // the rule that you may not pass out of check needs no code of its own.
    for position in playout(0x1234_5678_9ABC_DEF1, 12) {
        let mover = position.to_move();
        let in_check = position.is_in_check(mover);
        let mut passes = 0;
        for mv in position.legal_moves() {
            let after = position.make_move(mv);
            assert!(
                !after.is_in_check(mover),
                "{mv} leaves the general in check"
            );
            if mv.is_pass() {
                passes += 1;
            }
        }
        assert_eq!(
            passes,
            usize::from(!in_check),
            "exactly one pass, and only when not in check"
        );
    }
}

#[test]
fn a_cannon_moves_over_exactly_one_screen_and_never_over_or_onto_a_cannon() {
    // Three rules in one sweep, because they are three ways of describing one
    // move and a position that satisfies two of them is the interesting one.
    for position in playout(0x0F1E_2D3C_4B5A_6978, 12) {
        let cannons = position.role(Role::Cannon);
        // Pseudo-legal, because the screen rule is geometry: a cannon move
        // thrown out for leaving its own general in check still had to be a
        // legal cannon move first, and those are the ones a bug hides in.
        for mv in position.pseudo_legal_moves() {
            if position.piece_at(mv.from).map(|p| p.role) != Some(Role::Cannon) {
                continue;
            }
            let path = between(mv.from, mv.to);
            let screens: Vec<Point> = path
                .iter()
                .copied()
                .filter(|&point| position.piece_at(point).is_some())
                .collect();
            assert_eq!(screens.len(), 1, "{mv} leapt {} pieces", screens.len());
            assert!(
                !cannons.contains(screens[0]),
                "{mv} used a cannon on {} as its screen",
                screens[0]
            );
            assert!(!cannons.contains(mv.to), "{mv} took a cannon");
        }
    }
}

#[test]
fn an_elephant_is_stopped_by_either_of_its_two_intermediate_points() {
    // Asserted both ways round. That a legal move has both points clear is the
    // easy half; that a move whose points are *not* clear is absent is the half
    // that catches an implementation checking only the first one.
    for position in playout(0x2B3C_4D5E_6F70_8192, 12) {
        let side = position.to_move();
        let reachable = position.pseudo_legal_moves();
        for from in position.pieces(side, Role::Elephant) {
            for (df, dr) in [
                (2, 3),
                (2, -3),
                (-2, 3),
                (-2, -3),
                (3, 2),
                (3, -2),
                (-3, 2),
                (-3, -2),
            ] {
                let Some(to) = from.offset(df, dr) else {
                    continue;
                };
                if position.side(side).contains(to) {
                    continue;
                }
                // The orthogonal step is along whichever axis it travels three
                // points on; the two diagonals then follow it outward.
                let orthogonal = if dr.abs() == 3 {
                    (0, dr.signum())
                } else {
                    (df.signum(), 0)
                };
                let leg = from
                    .offset(orthogonal.0, orthogonal.1)
                    .expect("one step short of a point that exists");
                let knee = leg
                    .offset(df.signum(), dr.signum())
                    .expect("two steps short of a point that exists");
                let clear = position.piece_at(leg).is_none() && position.piece_at(knee).is_none();
                assert_eq!(
                    reachable.contains(&Move::new(from, to)),
                    clear,
                    "{from}{to}: leg {leg} and knee {knee} in\n{position:?}"
                );
            }
        }
    }
}

#[test]
fn a_diagonal_move_is_always_along_a_line_drawn_on_a_palace() {
    // The complement of the palace-diagonal tests in `position.rs`, which say
    // the lines exist. This one says nothing else does: no piece anywhere on
    // the board ever moves diagonally except along a drawn line, and the
    // elephant — whose move is diagonal in part but never diagonal end to end
    // — is not an exception because its net displacement never has equal
    // components.
    for position in playout(0x7766_5544_3322_1100, 12) {
        for mv in position.pseudo_legal_moves() {
            let df = mv.to.file() as i16 - mv.from.file() as i16;
            let dr = mv.to.rank() as i16 - mv.from.rank() as i16;
            if df == 0 || dr == 0 || df.abs() != dr.abs() {
                continue;
            }
            let mut at = mv.from;
            for _ in 0..df.abs() {
                at = at
                    .palace_diagonal(df.signum(), dr.signum())
                    .unwrap_or_else(|| panic!("{mv} leaves the drawn lines at {at}"));
            }
            assert_eq!(at, mv.to);
        }
    }
}
