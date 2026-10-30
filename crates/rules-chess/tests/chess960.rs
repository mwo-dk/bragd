//! Chess960 perft — the correctness gate for the second variant.
//!
//! `chess960.epd` is the published 960-position suite: every one of the 960
//! starting arrangements, advanced eight plies so that castling is reachable
//! inside the perft depth, with node counts to depth 6. It is the same file
//! that ships as `src/perft/fischer.epd` in Ethereal and as the Chess
//! Programming Wiki's Chess960 table, and the copy here was checked
//! byte-for-byte against three independently obtained copies before it landed.
//!
//! Why it earns its place beside the six standard positions: Chess960 castling
//! is not one rule with a wider board, it is a family of shapes that standard
//! chess cannot express — the king that does not move, the rook already on its
//! destination, the pair that swap squares, the queenside castle whose king
//! travels *east*, and the back-rank x-ray that only exists because a castling
//! rook can stand on b1. Every one of those appears somewhere in these 960
//! positions, and none of them appears in the standard six.
//!
//! Two cheap invariants come free with a file this size, and both are checked
//! below: the totals. Summed over all 960 positions the counts are 25,943 at
//! depth 1 and 19,890,658 at depth 3. A single transposed digit anywhere in the
//! file changes them, so they are a checksum on the corpus as well as on the
//! engine.
//!
//! Depth 1–3 runs on every `cargo test`. Depth 4 and beyond are `#[ignore]`d
//! for the same reason as the standard suite: worth minutes, not seconds.
//!
//! The castling field in this file is **Shredder-FEN** — `HFhf` rather than
//! `KQkq` — which is what the published corpus uses and therefore what the
//! parser has to accept. That is not incidental: it means this file cannot be
//! loaded at all until the FEN work is done, so it is the first thing to fail
//! and the last thing to pass.

use rules_chess::{Position, perft::perft};

/// One line of the EPD: a FEN, then `;D1 n ;D2 n …`.
struct Row {
    fen: String,
    counts: Vec<u64>,
}

fn corpus() -> Vec<Row> {
    include_str!("chess960.epd")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut parts = line.split(';');
            let fen = parts.next().expect("a line has a FEN").trim().to_string();
            let counts = parts
                .map(|part| {
                    part.split_whitespace()
                        .nth(1)
                        .expect("a depth field is `Dn count`")
                        .parse()
                        .expect("a count is a number")
                })
                .collect();
            Row { fen, counts }
        })
        .collect()
}

fn run(rows: &[Row], depth: usize) {
    let mut failures = Vec::new();
    for row in rows {
        let position = match Position::from_fen(&row.fen) {
            Ok(position) => position,
            Err(problem) => {
                failures.push(format!("{}: {problem}", row.fen));
                continue;
            }
        };
        let Some(&expected) = row.counts.get(depth - 1) else {
            continue;
        };
        let got = perft(&position, depth as u32);
        if got != expected {
            // The FEN is in the message on purpose: it is what you paste into
            // `perft::divide` to find the move whose subtree disagrees.
            failures.push(format!(
                "{}: depth {depth} gave {got}, want {expected}",
                row.fen
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} positions wrong at depth {depth}:\n{}",
        failures.len(),
        rows.len(),
        // Enough to see the shape of the failure without printing 960 lines.
        failures
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_corpus_is_intact() {
    // A checksum on the file, not on the engine. If someone re-fetches the
    // corpus and it differs, this says so before 960 perft failures do.
    let rows = corpus();
    assert_eq!(rows.len(), 960, "the corpus is 960 positions");
    let total = |depth: usize| -> u64 { rows.iter().filter_map(|r| r.counts.get(depth - 1)).sum() };
    assert_eq!(total(1), 25_943, "sum of depth-1 counts");
    assert_eq!(total(3), 19_890_658, "sum of depth-3 counts");
}

#[test]
fn perft_shallow() {
    let rows = corpus();
    for depth in 1..=3 {
        run(&rows, depth);
    }
}

#[test]
#[ignore = "minutes: run with --release -- --ignored"]
fn perft_deep() {
    let rows = corpus();
    run(&rows, 4);
}

#[test]
fn the_standard_start_is_one_of_the_960() {
    // Chess960 arrangement 518 is standard chess, and expressing it in
    // Shredder-FEN is the sharpest test there is that the general path
    // reproduces the special case: same board, same counts, castling field
    // spelled the other way.
    let shredder = Position::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w AHah - 0 1")
        .expect("the standard start in Shredder-FEN");
    assert_eq!(perft(&shredder, 4), 197_281);
    assert_eq!(
        shredder,
        Position::starting(),
        "the same position, however its castling field is spelled"
    );
}
