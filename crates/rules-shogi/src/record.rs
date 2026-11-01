//! A game record's board: the position a KIF or CSA file is played out on, with the pieces in
//! hand, taken as the file writes it.
//!
//! This is not [`crate::Position`]. A record may promote any piece (`+G`, `+K`), hold a hand of
//! anything, lack a king or have two, and is never asked whether a move is legal, so it keeps
//! pieces as a kind letter and two flags. Reading a file must give the same SFEN the
//! file always gave, because a Library keys its positions by it.

/// A piece: its kind (`P L N S G B R K`), Sente's (else Gote's), promoted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub kind: char,
    pub sente: bool,
    pub promoted: bool,
}

pub const KINDS: [char; 8] = ['R', 'B', 'G', 'S', 'N', 'L', 'P', 'K'];

/// A position: 81 cells row by row from the top (Gote's side), file 9 on the left; the pieces
/// in hand, counted per kind (`KINDS` order, the king left out); whose move.
#[derive(Clone, Debug, PartialEq)]
pub struct Board {
    pub cells: [Option<Piece>; 81],
    pub hands: [[u8; 7]; 2],
    pub sente: bool,
}

/// The start position, in SFEN.
pub const START: &str = crate::position::START;

/// A square from its file (1 to 9, right to left as Sente sees it) and rank (1 to 9 from the
/// top).
pub fn cell(file: u32, rank: u32) -> Option<usize> {
    ((1..=9).contains(&file) && (1..=9).contains(&rank)).then(|| (9 - file as usize) + (rank as usize - 1) * 9)
}

impl Board {
    pub fn from_sfen(sfen: &str) -> Option<Board> {
        let mut f = sfen.split_whitespace();
        let rows: Vec<&str> = f.next()?.split('/').collect();
        if rows.len() != 9 {
            return None;
        }
        let mut b = Board { cells: [None; 81], hands: [[0; 7]; 2], sente: true };
        for (y, row) in rows.iter().enumerate() {
            let (mut x, mut plus) = (0usize, false);
            for c in row.chars() {
                if c == '+' {
                    plus = true;
                    continue;
                }
                if let Some(d) = c.to_digit(10) {
                    x += d as usize;
                } else {
                    let kind = c.to_ascii_uppercase();
                    if !KINDS.contains(&kind) || x >= 9 {
                        return None;
                    }
                    b.cells[x + y * 9] = Some(Piece { kind, sente: c.is_ascii_uppercase(), promoted: plus });
                    x += 1;
                }
                plus = false;
                if x > 9 {
                    return None;
                }
            }
        }
        b.sente = f.next() != Some("w");
        let mut n = 0u32;
        for c in f.next().unwrap_or("-").chars() {
            if let Some(d) = c.to_digit(10) {
                n = (n * 10 + d).min(99);
            } else if let Some(k) = KINDS[..7].iter().position(|&k| k == c.to_ascii_uppercase()) {
                let h = &mut b.hands[(!c.is_ascii_uppercase()) as usize][k];
                *h = h.saturating_add(n.max(1) as u8).min(18);
                n = 0;
            }
        }
        Some(b)
    }

    /// The position in SFEN, as the apps draw it (`+` before a promoted piece).
    pub fn sfen(&self) -> String {
        let mut out = String::new();
        for y in 0..9 {
            if y > 0 {
                out.push('/');
            }
            let mut empty = 0;
            for x in 0..9 {
                match self.cells[x + y * 9] {
                    None => empty += 1,
                    Some(p) => {
                        if empty > 0 {
                            out.push_str(&empty.to_string());
                            empty = 0;
                        }
                        if p.promoted {
                            out.push('+');
                        }
                        out.push(if p.sente { p.kind } else { p.kind.to_ascii_lowercase() });
                    }
                }
            }
            if empty > 0 {
                out.push_str(&empty.to_string());
            }
        }
        out.push_str(if self.sente { " b " } else { " w " });
        let mut hands = String::new();
        for (side, h) in self.hands.iter().enumerate() {
            for (k, &n) in h.iter().enumerate() {
                if n > 1 {
                    hands.push_str(&n.to_string());
                }
                if n > 0 {
                    hands.push(if side == 0 { KINDS[k] } else { KINDS[k].to_ascii_lowercase() });
                }
            }
        }
        out.push_str(if hands.is_empty() { "-" } else { &hands });
        out
    }

    /// A move as the file writes it: from a square (None: a drop of `kind` from the hand) to one, promoting or not.
    pub fn play(&self, from: Option<usize>, to: usize, kind: char, promote: bool) -> Option<Board> {
        let mut b = self.clone();
        let side = (!self.sente) as usize;
        let piece = match from {
            Some(f) => {
                let p = b.cells[f].take()?;
                if p.sente != self.sente {
                    return None;
                }
                Piece { promoted: p.promoted || promote, ..p }
            }
            None => {
                let k = KINDS[..7].iter().position(|&k| k == kind)?;
                let h = &mut b.hands[side][k];
                *h = h.checked_sub(1)?;
                Piece { kind, sente: self.sente, promoted: false }
            }
        };
        if let Some(taken) = b.cells[to] {
            if taken.sente == self.sente || from.is_none() {
                return None;
            }
            if let Some(k) = KINDS[..7].iter().position(|&k| k == taken.kind) {
                b.hands[side][k] = b.hands[side][k].saturating_add(1);
            }
        }
        b.cells[to] = Some(piece);
        b.sente = !self.sente;
        Some(b)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_position_round_trips_in_the_dialect_a_library_keys_by() {
        assert_eq!(Board::from_sfen(START).unwrap().sfen(), START.trim_end_matches(" 1"));
    }

    #[test]
    fn a_record_may_promote_any_piece_and_hold_more_than_a_game_could() {
        let b = Board::from_sfen("+G7+k/9/9/9/9/9/9/9/9 w 99P3r - extra").unwrap();
        assert_eq!(b.cells[0], Some(Piece { kind: 'G', sente: true, promoted: true }));
        assert_eq!(b.hands[0][6], 18, "a hand is cut at eighteen");
        assert_eq!(b.hands[1][0], 3);
        assert_eq!(b.sfen(), "+G7+k/9/9/9/9/9/9/9/9 w 18P3r");
        for bad in ["", "9/9", "9/9/9/9/9/9/9/9/55", "9/9/9/9/9/9/9/9/x8", "9/9/9/9/9/9/9/9/P9"] {
            assert!(Board::from_sfen(bad).is_none(), "read {bad:?}");
        }
    }

    #[test]
    fn a_move_as_written_takes_into_hand_and_drops_from_it() {
        let b = Board::from_sfen("9/9/9/9/9/9/b8/B8/9 b - 1").unwrap();
        let (from, to) = (cell(9, 8).unwrap(), cell(9, 7).unwrap());
        let taken = b.play(Some(from), to, 'B', true).expect("a capture");
        assert_eq!(taken.sfen(), "9/9/9/9/9/9/+B8/9/9 w B");
        let dropped = taken.play(None, cell(5, 5).unwrap(), 'B', false);
        assert!(dropped.is_none(), "Gote holds nothing to drop");
        assert!(b.play(Some(to), from, 'B', false).is_none(), "Gote's piece on Sente's turn");
        let two = Board::from_sfen("9/9/9/9/9/9/9/BB7/9 b - 1").unwrap();
        assert!(two.play(Some(from), cell(8, 8).unwrap(), 'B', false).is_none(), "onto its own side");
        let hand = Board::from_sfen("9/9/9/9/9/9/9/9/9 b 2P 1").unwrap();
        let once = hand.play(None, cell(5, 5).unwrap(), 'P', false).expect("a drop");
        assert_eq!(once.sfen(), "9/9/9/9/4P4/9/9/9/9 w P");
        assert!(once.play(None, cell(5, 5).unwrap(), 'P', false).is_none(), "Gote has no pawn");
    }
}
