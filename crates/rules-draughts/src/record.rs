//! A game record's board: the position a PDN file is played out on, for the board sizes and
//! rules its `GameType` can name (4×4 to 12×12, men capturing back or not, kings flying or
//! not), with the move taken as the file writes it.
//!
//! This is not [`crate::Position`], whose squares are bits of one `u64` (64 at most) and whose
//! moves come from the rules. A record keeps a cell to the byte, so a 12×12 board fits, and a
//! move is looked for only as far as showing it needs.
//!
//! Any record is read without a panic and without recursion. A capture is looked for along at
//! most `MAX_SEARCH` steps, and all the captures of one game together along at most
//! `MAX_GAME_SEARCH`: past that, a capture is not played (its line stops), however the game
//! was made up. The captures of a position are worked out once (`Captures`).

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

/// Capture paths: each the squares landed on (from first) and the pieces taken.
pub type Paths = Vec<(Vec<usize>, Vec<usize>)>;

/// The most capture paths looked at for one move: a king among many pieces can have a great
/// many, a real move a handful.
pub const MAX_SEARCH: usize = 20_000;
/// The most capture steps of one game with its variations: a real game needs a few thousand.
pub const MAX_GAME_SEARCH: usize = 2_000_000;
/// The positions whose captures are kept, per game.
const MAX_CACHED: usize = 4096;
/// The largest board read (Canadian draughts is 12×12).
pub const MAX_SIZE: u8 = 12;

/// Which draughts: the board and the rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Variant {
    /// The PDN `GameType` number: 20 international, 21 English, …
    pub kind: u8,
    pub width: u8,
    pub height: u8,
    /// Black moves first (English draughts); else White.
    pub black_first: bool,
    /// Men capture backwards too.
    pub men_back: bool,
    /// Kings move and capture any distance along a diagonal.
    pub flying: bool,
}

pub const INTERNATIONAL: Variant = Variant { kind: 20, width: 10, height: 10, black_first: false, men_back: true, flying: true };
pub const ENGLISH: Variant = Variant { kind: 21, width: 8, height: 8, black_first: true, men_back: false, flying: false };

impl Variant {
    /// The dark squares, numbered 1 … from the top left, row by row.
    pub fn squares(&self) -> u32 {
        self.width as u32 * self.height as u32 / 2
    }

    /// Square `n`'s cell (`x + y × width`, rows from the top); the top row's dark squares are
    /// its 2nd, 4th, …, the next row's its 1st, 3rd, …
    pub fn cell(&self, n: u32) -> Option<usize> {
        if n == 0 || n > self.squares() {
            return None;
        }
        let half = self.width as u32 / 2;
        let (y, k) = ((n - 1) / half, (n - 1) % half);
        let x = 2 * k + if y % 2 == 0 { 1 } else { 0 };
        Some((x + y * self.width as u32) as usize)
    }

    /// A square as written: its number (`32`), or its column and row (`c3`, row 1 at the
    /// bottom), on a dark square.
    pub fn parse_square(&self, s: &str) -> Option<usize> {
        let s = s.trim();
        // Bytes are looked at one by one below: anything but ASCII is no square.
        if s.is_empty() || s.len() > 3 || !s.is_ascii() {
            return None;
        }
        if s.bytes().all(|b| b.is_ascii_digit()) {
            return self.cell(s.parse().ok()?);
        }
        let b = s.as_bytes();
        let x = b[0].to_ascii_lowercase().checked_sub(b'a')? as u32;
        let row: u32 = s[1..].parse().ok()?;
        let (w, h) = (self.width as u32, self.height as u32);
        if x >= w || row == 0 || row > h {
            return None;
        }
        let y = h - row;
        ((x + y) % 2 == 1).then_some((x + y * w) as usize)
    }
}

/// A piece on a cell: 0 none, then White's man and king, Black's man and king.
pub const WM: u8 = 1;
pub const WK: u8 = 2;
pub const BM: u8 = 3;
pub const BK: u8 = 4;

pub fn white(p: u8) -> bool {
    p == WM || p == WK
}

pub fn king(p: u8) -> bool {
    p == WK || p == BK
}

/// A position: the pieces, whose move, and the move number.
#[derive(Clone, Debug)]
pub struct Board {
    pub v: Variant,
    pub cells: Vec<u8>,
    pub white: bool,
    pub number: u32,
}

impl Board {
    /// The start: Black's men on the top rows, White's on the bottom ones, two rows between.
    pub fn start(v: Variant) -> Board {
        let mut b = Board { v, cells: vec![0; v.width as usize * v.height as usize], white: !v.black_first, number: 1 };
        let rows = v.height as u32 / 2 - 1;
        let half = v.width as u32 / 2;
        for n in 1..=v.squares() {
            let y = (n - 1) / half;
            if let Some(c) = v.cell(n) {
                if y < rows {
                    b.cells[c] = BM;
                } else if y >= v.height as u32 - rows {
                    b.cells[c] = WM;
                }
            }
        }
        b
    }

    /// A PDN FEN: `W:W31,32,K46:B1-5,K10`: who moves, then each side's squares, a king's with
    /// `K`, runs as `1-5`. None when it names no square.
    pub fn from_fen(v: Variant, fen: &str) -> Option<Board> {
        let mut b = Board { v, cells: vec![0; v.width as usize * v.height as usize], white: !v.black_first, number: 1 };
        let mut any = false;
        let fen = fen.trim().trim_end_matches('.');
        for (i, part) in fen.split(':').enumerate() {
            let part = part.trim();
            let side = part.chars().next().map(|c| c.to_ascii_uppercase());
            if i == 0 && part.len() == 1 {
                b.white = side != Some('B');
                continue;
            }
            let (man, kg) = match side {
                Some('W') => (WM, WK),
                Some('B') => (BM, BK),
                _ => continue,
            };
            for item in part[1..].split(',').take(b.cells.len()) {
                let item = item.trim();
                let (k, sq) = match item.strip_prefix(['K', 'k']) {
                    Some(s) => (true, s),
                    None => (false, item),
                };
                let (a, z) = sq.split_once('-').unwrap_or((sq, sq));
                let (Ok(a), Ok(z)) = (a.trim().parse::<u32>(), z.trim().parse::<u32>()) else {
                    if let Some(c) = v.parse_square(sq) {
                        b.cells[c] = if k { kg } else { man };
                        any = true;
                    }
                    continue;
                };
                for n in a..=z.min(a.saturating_add(v.squares())) {
                    if let Some(c) = v.cell(n) {
                        b.cells[c] = if k { kg } else { man };
                        any = true;
                    }
                }
            }
        }
        any.then_some(b)
    }

    /// The position as the apps draw it: rows from the top, `/` between them, a run of empty
    /// cells as its count, `M` and `K` White's man and king, `m` and `k` Black's; then `w` or
    /// `b` for whose move.
    pub fn fen(&self) -> String {
        let w = self.v.width as usize;
        let mut out = String::new();
        for (y, row) in self.cells.chunks(w).enumerate() {
            if y > 0 {
                out.push('/');
            }
            let mut empty = 0;
            for &p in row {
                if p == 0 {
                    empty += 1;
                    continue;
                }
                if empty > 0 {
                    out.push_str(&empty.to_string());
                    empty = 0;
                }
                out.push(match p {
                    WM => 'M',
                    WK => 'K',
                    BM => 'm',
                    _ => 'k',
                });
            }
            if empty > 0 {
                out.push_str(&empty.to_string());
            }
        }
        out.push_str(if self.white { " w" } else { " b" });
        out
    }

    fn xy(&self, c: usize) -> (i32, i32) {
        let w = self.v.width as usize;
        ((c % w) as i32, (c / w) as i32)
    }

    fn at(&self, x: i32, y: i32) -> Option<usize> {
        let (w, h) = (self.v.width as i32, self.v.height as i32);
        (x >= 0 && y >= 0 && x < w && y < h).then(|| (x + y * w) as usize)
    }

    /// The row a side's men become kings on.
    fn crowned(&self, c: usize, white_side: bool) -> bool {
        let y = self.xy(c).1;
        if white_side { y == 0 } else { y == self.v.height as i32 - 1 }
    }

    /// Every capture the piece on `from` can make, each as the squares it lands on (from
    /// first) and the pieces it takes. Each step (a path taken off or put on the stack) draws
    /// on `budget`: at most `MAX_SEARCH` of them here, and None when the budget runs out.
    pub fn captures(&self, from: usize, budget: &Cell<usize>) -> Option<Vec<(Vec<usize>, Vec<usize>)>> {
        let piece = self.cells[from];
        let me = white(piece);
        let mut out = vec![];
        // (where it is, still a man, landed on, taken)
        let mut stack = vec![(from, !king(piece), vec![from], vec![])];
        let mut looked = 0;
        let mut step = || {
            looked += 1;
            let left = budget.get();
            budget.set(left.saturating_sub(1));
            (left > 0).then_some(looked <= MAX_SEARCH)
        };
        while let Some((at, man, path, taken)) = stack.pop() {
            match step() {
                None => return None,
                Some(false) => break,
                Some(true) => {}
            }
            let free = |c: usize| self.cells[c] == 0 || c == from;
            let foe = |c: usize| self.cells[c] != 0 && c != from && white(self.cells[c]) != me && !taken.contains(&c);
            let (x, y) = self.xy(at);
            let forward = if me { -1 } else { 1 };
            let mut next = vec![];
            for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                if man && !self.v.men_back && dy != forward {
                    continue;
                }
                let long = !man && self.v.flying;
                // Along the diagonal to the first piece (one step only for a man or short king).
                let mut k = 1;
                let mut hit = None;
                while let Some(c) = self.at(x + dx * k, y + dy * k) {
                    if !free(c) {
                        hit = Some((c, k));
                        break;
                    }
                    if !long {
                        break;
                    }
                    k += 1;
                }
                let Some((c, k)) = hit.filter(|&(c, _)| foe(c)) else { continue };
                // The squares beyond it: one, or every free one for a flying king.
                let mut j = k + 1;
                while let Some(land) = self.at(x + dx * j, y + dy * j) {
                    if !free(land) {
                        break;
                    }
                    next.push((land, c));
                    if !long {
                        break;
                    }
                    j += 1;
                }
            }
            if next.is_empty() {
                if !taken.is_empty() {
                    out.push((path, taken));
                }
                continue;
            }
            for (land, c) in next {
                let mut path = path.clone();
                path.push(land);
                let mut taken = taken.clone();
                taken.push(c);
                let crowned = man && self.crowned(land, me);
                // English draughts: a man that reaches the far row is crowned and stops there.
                // Russian draughts: it is crowned and captures on as a king.
                if crowned && self.v.kind == 21 {
                    out.push((path, taken));
                    continue;
                }
                // Every path put on the stack counts too: the stack never outgrows the steps.
                match step() {
                    None => return None,
                    Some(false) => continue,
                    Some(true) => {}
                }
                stack.push((land, man && !(crowned && self.v.kind == 25), path, taken));
            }
        }
        Some(out)
    }

    /// A move as a game file writes it, from the squares it names (the piece's first, where it
    /// lands last, a capture's between) and whether it is written as a capture: None when it
    /// cannot be played. Gives the board and the cells it left and went to.
    pub fn play(&self, squares: &[usize], capture: bool, search: &Captures) -> Option<(Board, usize, usize)> {
        let (&from, &to) = (squares.first()?, squares.last()?);
        if from.max(to) >= self.cells.len() {
            return None;
        }
        let piece = self.cells[from];
        if squares.len() < 2 || piece == 0 || white(piece) != self.white {
            return None;
        }
        let mut b = self.clone();
        let (fx, fy) = self.xy(from);
        let (tx, ty) = self.xy(to);
        let step = (fx - tx).abs() == 1 && (fy - ty).abs() == 1;
        // A capture by the squares it lands on, the one taking the most when several fit.
        let found = if capture || !step {
            search
                .captures(self, from)?
                .iter()
                .filter(|(path, _)| {
                    path.last() == Some(&to) && {
                        let mut it = path.iter();
                        squares[1..squares.len() - 1].iter().all(|s| it.any(|p| p == s))
                    }
                })
                .max_by_key(|(_, taken)| taken.len())
                .cloned()
        } else {
            None
        };
        let mut crowned = false;
        match found {
            Some((path, taken)) => {
                for c in taken {
                    b.cells[c] = 0;
                }
                crowned = self.v.kind == 25 && path.iter().any(|&c| self.crowned(c, self.white));
            }
            // International and English draughts are played by their rules; another variant's
            // capture the rules here do not know takes the pieces of the other side between
            // each square written and the next, when they lie on a diagonal.
            None if capture && !matches!(self.v.kind, 20 | 21) => {
                let mut any = false;
                for w in squares.windows(2) {
                    let ((ax, ay), (zx, zy)) = (self.xy(w[0]), self.xy(w[1]));
                    let (dx, dy) = (zx - ax, zy - ay);
                    if dx.abs() != dy.abs() || dx == 0 {
                        return None;
                    }
                    for k in 1..dx.abs() {
                        let c = self.at(ax + dx.signum() * k, ay + dy.signum() * k)?;
                        if b.cells[c] != 0 && white(b.cells[c]) != self.white {
                            b.cells[c] = 0;
                            any = true;
                        }
                    }
                }
                if !any {
                    return None;
                }
            }
            None if capture => return None,
            None => {}
        }
        if b.cells[to] != 0 && to != from {
            return None;
        }
        b.cells[from] = 0;
        b.cells[to] = if !king(piece) && (crowned || self.crowned(to, self.white)) {
            if self.white { WK } else { BK }
        } else {
            piece
        };
        // The second player's move ends the move number.
        if self.white == self.v.black_first {
            b.number += 1;
        }
        b.white = !self.white;
        Some((b, from, to))
    }
}

/// A position and the cell whose captures are kept.
type CacheKey = (Vec<u8>, usize);

/// The search of one game: what is left of its budget, and the captures of the positions met
/// so far.
pub struct Captures {
    pub budget: Cell<usize>,
    cache: RefCell<HashMap<CacheKey, Rc<Paths>>>,
}

impl Default for Captures {
    fn default() -> Captures {
        Captures::new()
    }
}

impl Captures {
    pub fn new() -> Captures {
        Captures { budget: Cell::new(MAX_GAME_SEARCH), cache: RefCell::default() }
    }

    /// `Board::captures`, once per position and square, shared, not copied. A cached answer
    /// still draws on the budget, a step per path in it, so the paths looked at over a whole
    /// game stay within `MAX_GAME_SEARCH` however often a position comes back.
    pub fn captures(&self, b: &Board, from: usize) -> Option<Rc<Paths>> {
        let key = (b.cells.clone(), from);
        if let Some(c) = self.cache.borrow().get(&key) {
            let cost = c.len().max(1);
            let left = self.budget.get();
            self.budget.set(left.saturating_sub(cost));
            return (left >= cost).then(|| c.clone());
        }
        let found = Rc::new(b.captures(from, &self.budget)?);
        let mut cache = self.cache.borrow_mut();
        if cache.len() < MAX_CACHED {
            cache.insert(key, found.clone());
        }
        Some(found)
    }
}

/// A position's text as its key is made from (`bragd_library::library::position_keys`): PDN's
/// FEN with nothing but the pieces and whose move, `W:W31,32,K46:B1,2` (who moves, then White's
/// squares and Black's, each in rising order, a king's with `K`, no runs). It reads either form
/// Bragd writes: PDN's FEN (`W:W31-35,K46:B1-5`, runs allowed, any case, a trailing `.`) and the
/// picture `Board::fen` draws (`1m1m/…/M1M1 w`). So the same position has one text whoever wrote
/// it and whatever its move number. None for a text that is neither, a square written as `c3`
/// (it needs the board's size), or a piece on a light square of a picture.
pub fn canonical(text: &str) -> Option<String> {
    let (side, white, black) = if text.contains(':') { from_pdn(text)? } else { from_picture(text)? };
    if white.keys().any(|n| black.contains_key(n)) {
        return None;
    }
    let list = |s: &Squares| {
        s.iter().map(|(n, king)| format!("{}{n}", if *king { "K" } else { "" })).collect::<Vec<_>>().join(",")
    };
    Some(format!("{side}:W{}:B{}", list(&white), list(&black)))
}

/// The squares of one side: number to whether it is a king. A square's number is at most `MAX_SQUARE`.
type Squares = BTreeMap<u64, bool>;
const MAX_SQUARE: u64 = 10_000;
/// A picture row's pieces: (cell, is White's, is a king).
type Pieces = Vec<(u64, bool, bool)>;

/// `W:W31,K46:B1-2`, any case and order of the two sides, a trailing `.` allowed.
fn from_pdn(text: &str) -> Option<(char, Squares, Squares)> {
    let text = text.strip_suffix('.').unwrap_or(text);
    let (side, first, second) = match text.split(':').collect::<Vec<_>>().as_slice() {
        [side, first, second] => (*side, *first, *second),
        _ => return None,
    };
    let side = match side.to_ascii_uppercase().as_str() {
        "W" => 'W',
        "B" => 'B',
        _ => return None,
    };
    let (a, b) = (pdn_side(first)?, pdn_side(second)?);
    let (white, black) = match (a.0, b.0) {
        ('W', 'B') => (a.1, b.1),
        ('B', 'W') => (b.1, a.1),
        _ => return None,
    };
    Some((side, white, black))
}

/// One side's part of PDN: its colour letter, then its squares, `K` for a king, `a-b` for a run.
fn pdn_side(part: &str) -> Option<(char, Squares)> {
    let colour = match part.get(..1)?.to_ascii_uppercase().as_str() {
        "W" => 'W',
        "B" => 'B',
        _ => return None,
    };
    let mut squares = Squares::new();
    let list = part.get(1..)?;
    if list.is_empty() {
        return Some((colour, squares));
    }
    for item in list.split(',') {
        let king = item.starts_with(['K', 'k']);
        let item = if king { &item[1..] } else { item };
        let (first, last) = match item.split_once('-') {
            Some((a, b)) => (number(a)?, number(b)?),
            None => (number(item)?, number(item)?),
        };
        if first > last {
            return None;
        }
        for n in first..=last {
            if squares.insert(n, king).is_some() {
                return None;
            }
        }
    }
    Some((colour, squares))
}

/// A square's number as PDN writes it: digits only, from 1 up to `MAX_SQUARE`.
fn number(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok().filter(|n| (1..=MAX_SQUARE).contains(n))
}

/// `1K2/4/4/m3 b`: rows top to bottom, then whose move. The square numbers follow from the width.
fn from_picture(text: &str) -> Option<(char, Squares, Squares)> {
    let mut words = text.split_whitespace();
    let (rows, side) = (words.next()?, words.next()?);
    if words.next().is_some() {
        return None;
    }
    let side = match side {
        "w" => 'W',
        "b" => 'B',
        _ => return None,
    };
    let (mut white, mut black) = (Squares::new(), Squares::new());
    let mut first_width = None;
    for (y, row) in rows.split('/').enumerate() {
        let y = y as u64;
        let (cells, pieces) = picture_row(row)?;
        let width = *first_width.get_or_insert(cells);
        if cells != width || width == 0 || width % 2 == 1 {
            return None;
        }
        for (x, is_white, king) in pieces {
            if (x.checked_add(y)? % 2) == 0 {
                return None;
            }
            let n = y.checked_mul(width / 2)?.checked_add(x / 2)?.checked_add(1)?;
            if n > MAX_SQUARE {
                return None;
            }
            let squares = if is_white { &mut white } else { &mut black };
            squares.insert(n, king);
        }
    }
    Some((side, white, black))
}

/// One picture row: its width in cells, and its pieces as (cell, White's, king's).
fn picture_row(row: &str) -> Option<(u64, Pieces)> {
    let (mut x, mut run) = (0u64, 0u64);
    let mut pieces = Vec::new();
    for c in row.chars() {
        if let Some(d) = c.to_digit(10) {
            run = run.checked_mul(10)?.checked_add(u64::from(d))?;
            if run > MAX_SQUARE {
                return None;
            }
            continue;
        }
        let (is_white, king) = match c {
            'M' => (true, false),
            'K' => (true, true),
            'm' => (false, false),
            'k' => (false, true),
            _ => return None,
        };
        x = x.checked_add(run)?;
        pieces.push((x, is_white, king));
        x = x.checked_add(1)?;
        run = 0;
    }
    Some((x.checked_add(run)?, pieces))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_canonical_text_from_either_form() {
        let pdn = "W:W31,32,K46:B1,2";
        assert_eq!(canonical(pdn).as_deref(), Some(pdn));
        assert_eq!(canonical("w:wk46,32,31:b2,1.").as_deref(), Some(pdn), "any case and order");
        assert_eq!(canonical("W:W31-32,K46:B1-2").as_deref(), Some(pdn), "runs");
        // The start of international draughts drawn as a picture, and as PDN.
        let start = Board::start(INTERNATIONAL);
        let want = "W:W31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50:B1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20";
        assert_eq!(canonical(&start.fen()).as_deref(), Some(want));
        assert_eq!(canonical("W:W31-50:B1-20").as_deref(), Some(want));
        // English draughts: Black to move first.
        let english = Board::start(ENGLISH);
        assert_eq!(canonical(&english.fen()).as_deref(), Some("B:W21,22,23,24,25,26,27,28,29,30,31,32:B1,2,3,4,5,6,7,8,9,10,11,12"));
        // A king in a picture; the side to move kept.
        assert_eq!(canonical("1K2/4/4/m3 b").as_deref(), Some("B:WK1:B7"));
        assert_eq!(canonical("1K2/4/4/m3 w").as_deref(), Some("W:WK1:B7"));
        assert_eq!(canonical("W:W:B5").as_deref(), Some("W:W:B5"), "a side with no pieces");
    }

    #[test]
    fn a_text_that_is_no_position_has_none() {
        assert_eq!(canonical(""), None);
        assert_eq!(canonical("hello"), None);
        assert_eq!(canonical("W:Wc3:B1"), None, "an algebraic square needs the board's size");
        assert_eq!(canonical("K3/4/4/4 w"), None, "a piece on a light square");
        assert_eq!(canonical("1m1/4 w"), None, "rows of different widths");
    }

    #[test]
    fn squares_are_numbered_from_the_top_left() {
        assert_eq!(INTERNATIONAL.cell(1), Some(1));
        assert_eq!(INTERNATIONAL.cell(50), Some(98));
        assert_eq!((INTERNATIONAL.cell(0), INTERNATIONAL.cell(51)), (None, None));
        assert_eq!(ENGLISH.parse_square("a1"), Some(56));
        assert_eq!(ENGLISH.parse_square("b1"), None, "a light square");
        assert_eq!(ENGLISH.parse_square("29"), Some(56));
        assert_eq!(ENGLISH.parse_square("\u{e9}5"), None);
    }

    #[test]
    fn a_fen_sets_pieces_and_the_side_to_move() {
        let b = Board::from_fen(INTERNATIONAL, "B:W37,K46:B1-3,K10").unwrap();
        assert!(!b.white);
        assert_eq!(b.cells.iter().filter(|&&c| c == BM).count(), 3);
        assert_eq!(b.cells[INTERNATIONAL.cell(10).unwrap()], BK);
        assert_eq!(b.cells[INTERNATIONAL.cell(46).unwrap()], WK);
        assert!(Board::from_fen(INTERNATIONAL, "W:W99:B").is_none(), "no square named");
        assert_eq!(Board::start(ENGLISH).fen(), "1m1m1m1m/m1m1m1m1/1m1m1m1m/8/8/M1M1M1M1/1M1M1M1M/M1M1M1M1 b");
    }

    #[test]
    fn a_capture_is_found_by_its_ends_and_takes_what_lies_between() {
        let v = INTERNATIONAL;
        let b = Board::from_fen(v, "W:W37:B32,22,41").unwrap();
        let search = Captures::new();
        let (a, z) = (v.cell(37).unwrap(), v.cell(17).unwrap());
        let (after, from, to) = b.play(&[a, z], true, &search).expect("37x17");
        assert_eq!((from, to), (a, z));
        assert_eq!(after.cells.iter().filter(|&&c| c == BM).count(), 1, "32 and 22 taken");
        assert!(!after.white);
        assert!(b.play(&[z, a], true, &search).is_none(), "no piece there");
        assert!(b.play(&[a], false, &search).is_none(), "one square is no move");
        assert!(b.play(&[a, 10_000], false, &search).is_none(), "off the board");
    }

    #[test]
    fn the_search_is_counted_and_capped() {
        // A flying king among men with gaps has far more capture paths than the cap.
        let v = INTERNATIONAL;
        let blacks: Vec<String> = (7..=44).filter(|n| n % 3 == 0).map(|n| n.to_string()).collect();
        let b = Board::from_fen(v, &format!("W:WK46:B{}", blacks.join(","))).unwrap();
        let from = v.cell(46).unwrap();
        assert!(b.captures(from, &Cell::new(usize::MAX)).unwrap().len() <= MAX_SEARCH);
        assert!(b.captures(from, &Cell::new(0)).is_none(), "no budget, no capture");
        // The game's budget is spent by the search, and a cached answer is charged again.
        let search = Captures::new();
        let first = search.captures(&b, from).unwrap();
        let spent = MAX_GAME_SEARCH - search.budget.get();
        assert!(spent > 0);
        search.captures(&b, from).unwrap();
        assert_eq!(MAX_GAME_SEARCH - search.budget.get(), spent + first.len().max(1));
    }
}
