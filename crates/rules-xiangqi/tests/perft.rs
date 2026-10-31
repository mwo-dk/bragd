//! Perft — the correctness gate for xiangqi.
//!
//! Two independent lineages agree on the opening-position counts: a 2009
//! TalkChess post and a later independent verification described as "computed
//! and verified by two different chess apps". That agreement is what makes a
//! table worth gating on; a single source is a mirror.
//!
//! The **breakdowns matter more here than in chess**. A bare node count hides
//! which rule is wrong, and xiangqi has three rules that nothing else in
//! Bragd has: the cannon's screen, the horse's hobbled leg, and the two generals
//! refusing to face each other down an open file. Counting captures and checks
//! separately localises a failure to one of them — a cannon bug moves the
//! capture count, and a facing-generals bug moves the check count.
//!
//! What this does **not** gate is the repetition rules, which is where xiangqi
//! is genuinely hard. Perpetual check and perpetual chase are adjudicated over
//! a whole sequence rather than a position, and the WXF's own conformance
//! material for them is 110 worked diagrams rather than a node count. Those are
//! not implemented yet, and this file cannot tell you so.

use rules_xiangqi::{Position, Role};

#[derive(Default, PartialEq, Eq, Debug)]
struct Counts {
    nodes: u64,
    captures: u64,
    checks: u64,
}

fn perft(position: &Position, depth: u32, counts: &mut Counts) {
    for mv in position.legal_moves() {
        let capture = position.piece_at(mv.to).is_some();
        let after = position.make_move(mv);
        if depth == 1 {
            counts.nodes += 1;
            if capture {
                counts.captures += 1;
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

/// The opening position: nodes, captures, checks.
const START_COUNTS: &[(u64, u64, u64)] = &[
    (44, 2, 0),
    (1_920, 72, 6),
    (79_666, 3_159, 384),
    (3_290_240, 115_365, 19_380),
    (133_312_995, 4_917_734, 953_251),
];

/// The "xiangqi Kiwipete" — a tactically dense middlegame with cannons in play,
/// which is where the screen rule is actually exercised.
const KIWIPETE: &str = "r1ba1a3/4kn3/2n1b4/pNp1p1p1p/4c4/6P2/P1P2R2P/1CcC5/9/2BAKAB2 w - - 0 1";

const KIWIPETE_COUNTS: &[(u64, u64, u64)] = &[
    (38, 1, 1),
    (1_128, 10, 12),
    (43_929, 2_105, 1_190),
    (1_339_047, 31_409, 21_299),
    (53_112_976, 3_262_495, 1_496_697),
];

fn check(position: &Position, table: &[(u64, u64, u64)], upto: usize, name: &str) {
    for (index, &(nodes, captures, checks)) in table.iter().take(upto).enumerate() {
        let depth = index as u32 + 1;
        assert_eq!(
            count(position, depth),
            Counts {
                nodes,
                captures,
                checks
            },
            "{name} at depth {depth}"
        );
    }
}

#[test]
fn the_opening_array_is_what_the_rules_say() {
    let start = Position::start();
    assert_eq!(start.side(rules_xiangqi::Side::Red).count(), 16);
    assert_eq!(start.pieces(rules_xiangqi::Side::Red, Role::Soldier).count(), 5);
    assert_eq!(start.pieces(rules_xiangqi::Side::Black, Role::Cannon).count(), 2);
}

#[test]
fn start_shallow() {
    check(&Position::start(), START_COUNTS, 3, "the opening position");
}

#[test]
fn kiwipete_shallow() {
    let position = Position::from_fen(KIWIPETE).expect("a valid position");
    check(&position, KIWIPETE_COUNTS, 3, "kiwipete");
}

#[test]
#[ignore = "minutes: run with --release -- --ignored"]
fn start_deep() {
    check(&Position::start(), START_COUNTS, 5, "the opening position");
}

#[test]
#[ignore = "minutes: run with --release -- --ignored"]
fn kiwipete_deep() {
    let position = Position::from_fen(KIWIPETE).expect("a valid position");
    check(&position, KIWIPETE_COUNTS, 5, "kiwipete");
}
