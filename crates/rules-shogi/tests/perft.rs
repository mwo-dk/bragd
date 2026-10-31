//! The published perft corpus, imported at last.
//!
//! `tests/rules.rs` opens by saying a published corpus exists and that it is
//! not it. This is it. Sources: the shogi-l "Shogi Perft Numbers" thread and
//! the TalkChess "Shogi Perft numbers" table, which agree on the starting
//! position through depth six.

use rules_shogi::Position;
use rules_shogi::perft::perft;

/// The TalkChess drops position, and a number the source gets wrong.
///
/// The published table (Siyam v0.44) says 86 at depth one, alongside a caveat
/// that a pawn dropped to give immediate mate is excluded. The two halves of
/// that claim disagree, and the arithmetic settles it: Black has 17 board
/// moves (nine for the lance on 2h, counting promotion choices at 2c and 2b
/// and the forced one on 2a; six for the silver on 1c; two for the king), and
/// 76 empty squares minus the seven on the back rank leave 69 pawn drops —
/// of which P*1b is mate against the cornered king on 1a, with 1b guarded by
/// the silver and 2b covered by it, so uchifuzume forbids exactly one.
/// 17 + 68 = **85**. The published 86 includes the drop-mate its own caveat
/// says to exclude, which is the cross-engine disagreement the thread itself
/// reports. The count asserted here is the derived one, not the copied one.
#[test]
fn the_drops_position_counts_its_drops_and_refuses_the_mate() {
    let position =
        Position::from_sfen("7lk/9/8S/9/9/9/9/7L1/8K b P 1").expect("the corpus position parses");
    assert_eq!(perft(&position, 1), 85);
}

#[test]
fn the_starting_position_matches_the_published_table() {
    let start = Position::starting();
    assert_eq!(perft(&start, 1), 30);
    assert_eq!(perft(&start, 2), 900);
    assert_eq!(perft(&start, 3), 25_470);
    assert_eq!(perft(&start, 4), 719_731);
}

#[test]
#[ignore = "deep perft; CI runs these in release like the chess ones"]
fn the_starting_position_matches_the_published_table_deep() {
    let start = Position::starting();
    assert_eq!(perft(&start, 5), 19_861_490);
    assert_eq!(perft(&start, 6), 547_581_517);
}
