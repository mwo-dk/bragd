//! A Zobrist key for a go position: the stones, the side to move and the ko point.
//!
//! **This key is stored.** Libraries keep it per position and the Position store is keyed by
//! it, so the seed and the way a feature is numbered must never change. Each feature (a stone
//! of one colour on one point, White to move, the ko point, the board's size) has a 64-bit
//! number from SplitMix64, and a position's key is all of its features XORed together. A
//! stone placed or taken off is therefore one XOR, and the same position reached by different
//! move orders has the same key.
//!
//! Points are numbered as a game record numbers them: column from the left, row **from the
//! top**, on boards up to 52 by 52 (`record::Record`), so a record replayed and a rules
//! [`Position`](crate::Position) give the same key.

/// Fixed for good: stored keys depend on it.
const SEED: u64 = 0x6f67_5a6f_6272_6973;

/// The widest board a record allows; points are numbered `row * STRIDE + column`.
const STRIDE: u64 = 64;

/// SplitMix64's output for step `n` from `SEED`.
const fn feature(n: u64) -> u64 {
    let mut z = SEED.wrapping_add(n.wrapping_add(1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

// Feature numbers: stones from 0, then the ko point, White to move, and the size.
const BLACK: u64 = 0;
const WHITE: u64 = STRIDE * STRIDE;
const KO: u64 = 2 * STRIDE * STRIDE;
const WHITE_TO_MOVE: u64 = 3 * STRIDE * STRIDE;
const SIZE: u64 = WHITE_TO_MOVE + 1;

fn point(x: u8, y: u8) -> u64 {
    y as u64 * STRIDE + x as u64
}

/// The key of a position. `stones`: row by row from the top, 0 empty, 1 black, 2 white,
/// `width * height` of them. `ko`: the point the ko rule forbids to the side to move.
pub fn key(width: u8, height: u8, stones: &[u8], black_next: bool, ko: Option<(u8, u8)>) -> u64 {
    let mut k = feature(SIZE + ((width as u64) << 8 | height as u64));
    for (i, &s) in stones.iter().enumerate().take(width as usize * height as usize) {
        let at = point((i % width as usize) as u8, (i / width as usize) as u8);
        match s {
            1 => k ^= feature(BLACK + at),
            2 => k ^= feature(WHITE + at),
            _ => {}
        }
    }
    if !black_next {
        k ^= feature(WHITE_TO_MOVE);
    }
    if let Some((x, y)) = ko {
        k ^= feature(KO + point(x, y));
    }
    k
}

/// The ko point after a move, found from the boards before and after it (as `key` takes
/// them), for records that keep no ko of their own. It is the one stone taken, when exactly
/// one was taken and the stone that took it stands alone with that point as its only liberty.
pub fn ko_after(width: u8, height: u8, before: &[u8], after: &[u8]) -> Option<(u8, u8)> {
    let (w, h) = (width as usize, height as usize);
    let n = w * h;
    if before.len() < n || after.len() < n {
        return None;
    }
    let mut placed = None;
    let mut taken = None;
    for i in 0..n {
        match (before[i], after[i]) {
            (0, s) if s != 0 => {
                if placed.replace(i).is_some() {
                    return None;
                }
            }
            (s, 0) if s != 0 => {
                if taken.replace(i).is_some() {
                    return None;
                }
            }
            _ => {}
        }
    }
    let (p, t) = (placed?, taken?);
    if before[t] == after[p] {
        return None;
    }
    let (x, y) = (p % w, p / w);
    let near = [(x > 0).then(|| p - 1), (x + 1 < w).then(|| p + 1), (y > 0).then(|| p - w), (y + 1 < h).then(|| p + w)];
    let mut liberties = 0;
    for q in near.into_iter().flatten() {
        match after[q] {
            0 => liberties += 1,
            s if s == after[p] => return None,
            _ => {}
        }
    }
    (liberties == 1 && near.contains(&Some(t))).then_some(((t % w) as u8, (t / w) as u8))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::Record;
    use crate::{Colour, Move, Point, Position};

    /// Plays `moves` (record coordinates; `None` passes) from an empty board: the key after each.
    fn keys(size: u8, moves: &[Option<(u8, u8)>]) -> Vec<u64> {
        let mut r = Record::new(size, size);
        let mut black = true;
        let mut out = vec![];
        for m in moves {
            let before = r.stones.clone();
            let ko = m.and_then(|(x, y)| {
                r.play(black, x, y);
                ko_after(size, size, &before, &r.stones)
            });
            black = !black;
            out.push(key(size, size, &r.stones, black, ko));
        }
        out
    }

    /// The ko shape of position.rs's tests: White's d3 has one liberty, Black to take it at c3.
    const KO: [&str; 9] = [".........", ".........", ".........", ".........", ".........", "..OX.....", ".O.OX....", "..OX.....", "........."];

    fn stones(rows: &[&str]) -> Vec<u8> {
        rows.concat().bytes().map(|c| match c {
            b'X' => 1,
            b'O' => 2,
            _ => 0,
        }).collect()
    }

    #[test]
    fn a_transposition_has_one_key() {
        let a = keys(9, &[Some((2, 2)), Some((6, 6)), Some((2, 6)), Some((6, 2))]);
        let b = keys(9, &[Some((2, 6)), Some((6, 2)), Some((2, 2)), Some((6, 6))]);
        assert_eq!(a[3], b[3]);
        assert_ne!(a[1], b[1]);
    }

    #[test]
    fn the_side_to_move_the_size_and_the_ko_are_part_of_it() {
        let empty = [0u8; 81];
        let plain = key(9, 9, &empty, true, None);
        assert_ne!(plain, key(9, 9, &empty, false, None));
        assert_ne!(plain, key(9, 9, &empty, true, Some((4, 4))));
        assert_ne!(key(9, 9, &empty, true, Some((4, 4))), key(9, 9, &empty, true, Some((4, 5))));
        assert_ne!(plain, key(13, 13, &[0; 169], true, None));
        assert_ne!(key(9, 13, &[0; 117], true, None), key(13, 9, &[0; 117], true, None));
        // A pass changes only the side to move.
        let k = keys(9, &[Some((4, 4)), None]);
        let mut one = [0u8; 81];
        one[4 * 9 + 4] = 1;
        assert_eq!(k[1], key(9, 9, &one, true, None));
    }

    #[test]
    fn stored_keys_never_change() {
        // Libraries and the Position store keep these: a change here orphans every stored go key.
        assert_eq!(key(19, 19, &[0; 361], true, None), STORED_EMPTY_19);
        let mut one = [0u8; 361];
        one[3 * 19 + 15] = 1;
        assert_eq!(key(19, 19, &one, false, None), STORED_Q16);
    }

    const STORED_EMPTY_19: u64 = 7596884340368290333;
    const STORED_Q16: u64 = 8170862696035161924;

    #[test]
    fn a_ko_is_found_from_the_boards_alone() {
        let before = stones(&KO);
        let mut r = Record::new(9, 9);
        r.stones = before.clone();
        r.play(true, 2, 6);
        assert_eq!(ko_after(9, 9, &before, &r.stones), Some((3, 6)));
        // A stone that takes nothing, or takes two, or is not alone, makes no ko.
        assert_eq!(ko_after(9, 9, &before, &before), None);
        let mut quiet = Record::new(9, 9);
        quiet.stones = before.clone();
        quiet.play(true, 7, 7);
        assert_eq!(ko_after(9, 9, &before, &quiet.stones), None);
        let mut joined = stones(&KO);
        joined[7 * 9 + 2] = 1; // c2 black: the taking stone is no longer alone
        let mut r = Record::new(9, 9);
        r.stones = joined.clone();
        r.play(true, 2, 6);
        assert_eq!(ko_after(9, 9, &joined, &r.stones), None);
    }

    #[test]
    fn a_rules_position_keys_as_its_record_does() {
        let start = Position::from_picture(&KO, Colour::Black);
        let taken = start.after(Move::Play(Point::parse("c3").unwrap())).unwrap();
        assert_eq!(taken.ko(), Some(Point::parse("d3").unwrap()));
        let before = stones(&KO);
        let mut r = Record::new(9, 9);
        r.stones = before.clone();
        r.play(true, 2, 6);
        assert_eq!(taken.zobrist(), key(9, 9, &r.stones, false, ko_after(9, 9, &before, &r.stones)));
        assert_eq!(start.zobrist(), key(9, 9, &before, true, None));
        assert_ne!(taken.zobrist(), start.zobrist());
    }
}
