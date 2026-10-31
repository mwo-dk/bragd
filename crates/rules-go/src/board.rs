//! The board: points, what stands on each of them, and the chains they form.
//!
//! **There is no bitboard here, and that is the first thing Go breaks.** Every
//! other engine in Bragd answered a bigger board with a wider integer —
//! eighty-one squares of shogi in a `u128`, a hundred and twenty-one of tafl in
//! the same — and three hundred and sixty-one points is where that answer runs
//! out. There is no `u384`. Six `u64`s stitched together would buy the bit
//! tricks back, at the price of writing the board's geometry twice and having
//! two places to get an edge wrong; and the operations Go actually wants —
//! *which stones is this one connected to, and how many empty points touch
//! them* — are a flood fill whichever way the occupancy is stored. So a board
//! is an array with one entry per point, and the fill is written once.
//!
//! The array is always three hundred and sixty-one entries long, whatever size
//! is being played, with the size kept as a field beside it. A 9×9 board
//! therefore carries two hundred and eighty bytes it never reads. That is the
//! price of a `Board` being a fixed-size value that clones without allocating,
//! and it is the right way round: move generation builds one board per
//! candidate point, so a heap allocation per candidate is a cost that would
//! show, and idle bytes on the stack are not.

use core::fmt;

/// The largest board Go is played on.
pub const MAX_SIZE: u8 = 19;

/// How many points that is. The size of every board's array.
pub const MAX_POINTS: usize = (MAX_SIZE as usize) * (MAX_SIZE as usize);

/// The three sizes played. Nine is a teaching board and a fast game, thirteen
/// is the middle one, nineteen is the game. They are the same rules throughout,
/// which is why size is a parameter here and not three engines.
pub const SIZES: [u8; 3] = [9, 13, 19];

/// Whether Bragd offers a board this size.
///
/// **The one place the list is read**, and it exists because it was being read
/// three ways. [`Ruleset::on`](crate::ruleset::Ruleset::on) compared against
/// `SIZES[0]`, `SIZES[1]` and `SIZES[2]` by hand, because it is a `const fn` and
/// `[u8]::contains` is not one; the seam in `bragd-games` used `contains`; and a fourth
/// spelling sat in Bragd's catalogue keys. Three spellings of a three-element
/// list is not a size problem, it is that the two in this crate *disagreed about
/// what happens to a size nobody offers* — one panics and one returns an error —
/// so the difference between a dead module and a refused game came down to which
/// of them a caller happened to reach first.
///
/// The disagreement stays, because it is right: a `const fn` cannot return an
/// error and a boundary must not abort. What is gone is the second reading of
/// the list, which is what would have let the two drift apart about *which*
/// sizes rather than only about how to refuse one.
///
/// A `while` loop rather than `contains` because this has to be callable from a
/// `const` context, which is the whole reason the hand-written comparison
/// existed.
pub const fn plays(size: u8) -> bool {
    let mut at = 0;
    while at < SIZES.len() {
        if SIZES[at] == size {
            return true;
        }
        at += 1;
    }
    false
}

/// A player, and the colour of the stones they place.
///
/// Black plays first — the reverse of chess, and the reason komi goes to White.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Colour {
    Black,
    White,
}

impl Colour {
    pub const fn other(self) -> Colour {
        match self {
            Colour::Black => Colour::White,
            Colour::White => Colour::Black,
        }
    }

    /// Where this colour sits in a two-element array. Black is zero.
    pub const fn index(self) -> usize {
        match self {
            Colour::Black => 0,
            Colour::White => 1,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Colour::Black => "Black",
            Colour::White => "White",
        }
    }
}

/// The column letters, and **`i` is missing from them.**
///
/// Go notation skips `i` so that it cannot be confused with `j` — or, on a
/// board diagram, with a `1`. A 19×19 board runs `a` to `t` with a hole in the
/// middle, which means code computing a letter as `b'a' + column` is correct
/// for the first eight columns and wrong for the remaining eleven. It is the
/// classic off-by-one in this game, it is silent, and it is wrong in the half
/// of the board people play most of their moves in.
const COLUMN_LETTERS: &[u8; MAX_SIZE as usize] = b"abcdefghjklmnopqrst";

/// A point on the board, as a column and a row.
///
/// Held as the two coordinates rather than as an index, because a point's name
/// is its column letter and its row number and nothing else — so a `Point` can
/// print itself without being told what size board it came from. Which board it
/// is *on* is a separate question, and [`Board::contains`] answers it.
///
/// Row zero is the bottom row, written `1`. Column zero is the left, written
/// `a`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point {
    column: u8,
    row: u8,
}

impl Point {
    /// # Panics
    /// If either coordinate is past the largest board there is.
    pub const fn at(column: u8, row: u8) -> Point {
        assert!(
            column < MAX_SIZE && row < MAX_SIZE,
            "not a point on any board"
        );
        Point { column, row }
    }

    pub const fn new(column: u8, row: u8) -> Option<Point> {
        if column < MAX_SIZE && row < MAX_SIZE {
            Some(Point { column, row })
        } else {
            None
        }
    }

    pub const fn column(self) -> u8 {
        self.column
    }

    pub const fn row(self) -> u8 {
        self.row
    }

    /// The point as it is written and spoken: `d4`, `q16`.
    pub fn name(self) -> String {
        let letter = COLUMN_LETTERS[self.column as usize] as char;
        format!("{letter}{}", self.row + 1)
    }

    /// Reads `d4`, and `D4`. The inverse of [`Point::name`], and the other of
    /// the two places that know `i` is not a column.
    pub fn parse(text: &str) -> Option<Point> {
        let (letter, digits) = text.as_bytes().split_first()?;
        let column = COLUMN_LETTERS
            .iter()
            .position(|&c| c == letter.to_ascii_lowercase())? as u8;

        // Parsed by hand rather than through `str::parse`, which accepts a
        // leading `+` and leading zeros. `d04` is not a point; it is a typo,
        // and a wire format that quietly accepts two spellings of one move is a
        // wire format that will one day be asked which one it stored.
        if digits.is_empty() || digits[0] == b'0' || !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }
        let mut row: u16 = 0;
        for digit in digits {
            row = row.checked_mul(10)?.checked_add((digit - b'0') as u16)?;
        }
        if row < 1 || row > MAX_SIZE as u16 {
            return None;
        }
        Point::new(column, row as u8 - 1)
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

impl fmt::Debug for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

/// The points orthogonally next to a point, and only the ones on the board.
///
/// Diagonals are not neighbours in Go. Stones connect and breathe along the
/// lines only, which is why a diagonal shape is a *shape* and not a group.
pub struct Neighbours {
    around: [Option<Point>; 4],
    next: usize,
}

impl Iterator for Neighbours {
    type Item = Point;
    fn next(&mut self) -> Option<Point> {
        while self.next < 4 {
            let point = self.around[self.next];
            self.next += 1;
            if point.is_some() {
                return point;
            }
        }
        None
    }
}

/// Every point of a board, bottom row first.
pub struct Points {
    size: u8,
    next: u16,
}

impl Iterator for Points {
    type Item = Point;
    fn next(&mut self) -> Option<Point> {
        let size = self.size as u16;
        if self.next >= size * size {
            return None;
        }
        let index = self.next;
        self.next += 1;
        Some(Point::at((index % size) as u8, (index / size) as u8))
    }
}

/// A chain: every stone of one colour connected to a given one, and how many
/// empty points touch them.
///
/// The stones are held in a fixed array rather than a `Vec` because this is the
/// hot path — a chain is walked for every neighbour of every candidate move —
/// and the whole reason the board is a fixed-size value is undone if finding a
/// group allocates. The array is the size of the largest possible chain, which
/// is the whole board.
pub struct Chain {
    colour: Colour,
    stones: [Point; MAX_POINTS],
    len: u16,
    liberties: u16,
}

impl Chain {
    pub fn colour(&self) -> Colour {
        self.colour
    }

    pub fn stones(&self) -> &[Point] {
        &self.stones[..self.len as usize]
    }

    /// How many distinct empty points touch the chain. **Distinct** is the
    /// whole difficulty: two stones of a chain often share a liberty, and
    /// counting it twice is how a dead group looks alive.
    pub fn liberties(&self) -> u16 {
        self.liberties
    }

    fn push(&mut self, point: Point) {
        self.stones[self.len as usize] = point;
        self.len += 1;
    }
}

impl fmt::Debug for Chain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {:?} with {} liberties",
            self.colour.name(),
            self.stones(),
            self.liberties
        )
    }
}

/// A board: how big it is, and what stands on each point.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Board {
    size: u8,
    stones: [Option<Colour>; MAX_POINTS],
}

impl Board {
    /// An empty board. `None` if the size is zero or off the end of
    /// [`MAX_SIZE`].
    ///
    /// Any size up to nineteen is accepted, not just the three Bragd plays.
    /// The rules do not care, and a ko or an eye written out on a 5×5 in a test
    /// is a diagram somebody can read.
    pub const fn new(size: u8) -> Option<Board> {
        if size == 0 || size > MAX_SIZE {
            return None;
        }
        Some(Board {
            size,
            stones: [None; MAX_POINTS],
        })
    }

    pub const fn size(&self) -> u8 {
        self.size
    }

    /// How many points there are. Go calls them intersections, because that is
    /// what they are: stones sit on the crossings, not in the squares.
    pub const fn intersections(&self) -> usize {
        (self.size as usize) * (self.size as usize)
    }

    pub const fn contains(&self, point: Point) -> bool {
        point.column < self.size && point.row < self.size
    }

    /// Where a point sits in the board's array. Public because the flood fills
    /// outside this module — scoring walks empty regions the same way — need
    /// somewhere to keep a mark per point.
    pub const fn index(&self, point: Point) -> usize {
        (point.row as usize) * (self.size as usize) + (point.column as usize)
    }

    /// What stands on a point. **A point off the board reads as empty**, which
    /// is a deliberate leniency in exactly one direction: the dead-stone set is
    /// handed to the engine from outside and may name anything, and a scoring
    /// call is not a place to panic.
    pub fn at(&self, point: Point) -> Option<Colour> {
        if !self.contains(point) {
            return None;
        }
        self.stones[self.index(point)]
    }

    /// # Panics
    /// If the point is not on this board.
    pub fn set(&mut self, point: Point, colour: Option<Colour>) {
        assert!(
            self.contains(point),
            "{point} is not on a {}×{} board",
            self.size,
            self.size
        );
        let index = self.index(point);
        self.stones[index] = colour;
    }

    pub fn points(&self) -> Points {
        Points {
            size: self.size,
            next: 0,
        }
    }

    pub fn neighbours(&self, point: Point) -> Neighbours {
        let last = self.size - 1;
        Neighbours {
            around: [
                (point.row < last).then(|| Point::at(point.column, point.row + 1)),
                (point.row > 0).then(|| Point::at(point.column, point.row - 1)),
                (point.column > 0).then(|| Point::at(point.column - 1, point.row)),
                (point.column < last).then(|| Point::at(point.column + 1, point.row)),
            ],
            next: 0,
        }
    }

    /// The chain the stone at `point` belongs to, or `None` if the point is
    /// empty.
    pub fn chain(&self, point: Point) -> Option<Chain> {
        self.walk(point, false)
    }

    /// Whether the chain at `point` has anywhere left to breathe.
    ///
    /// The question the rules actually ask, over and over, and it is answered
    /// by *one* liberty — so this stops at the first one rather than building
    /// the chain and counting. On a big living group that is the difference
    /// between looking at four points and looking at two hundred.
    pub fn has_liberty(&self, point: Point) -> bool {
        self.walk(point, true)
            .is_some_and(|chain| chain.liberties > 0)
    }

    /// The one flood fill.
    ///
    /// The chain being built doubles as the queue: stones are appended as they
    /// are discovered and read back in order, so there is no second stack. A
    /// point is marked the moment it is queued rather than when it is
    /// processed, which is what stops a stone being added twice by two of its
    /// own neighbours.
    ///
    /// `stop_at_first_liberty` abandons the walk as soon as the chain is known
    /// to be alive, and the chain it returns is therefore a fragment. That is
    /// why this is private and the two public entry points are not.
    fn walk(&self, from: Point, stop_at_first_liberty: bool) -> Option<Chain> {
        let colour = self.at(from)?;
        let mut chain = Chain {
            colour,
            stones: [Point::at(0, 0); MAX_POINTS],
            len: 0,
            liberties: 0,
        };
        let mut in_chain = [false; MAX_POINTS];
        let mut counted = [false; MAX_POINTS];

        in_chain[self.index(from)] = true;
        chain.push(from);
        let mut next = 0;
        while next < chain.len as usize {
            let stone = chain.stones[next];
            next += 1;
            for neighbour in self.neighbours(stone) {
                let index = self.index(neighbour);
                match self.stones[index] {
                    None => {
                        if !counted[index] {
                            counted[index] = true;
                            chain.liberties += 1;
                            if stop_at_first_liberty {
                                return Some(chain);
                            }
                        }
                    }
                    Some(other) if other == colour => {
                        if !in_chain[index] {
                            in_chain[index] = true;
                            chain.push(neighbour);
                        }
                    }
                    Some(_) => {}
                }
            }
        }
        Some(chain)
    }

    /// A number that stands for this whole board position.
    ///
    /// FNV-1a over the occupied array. It is a **filter and not an identity**:
    /// superko turns a repeated position into an illegal move, so a collision
    /// would forbid a move the rules allow, and the caller must confirm a match
    /// by comparing the boards themselves. Hashing is here only so that the
    /// comparison happens once instead of once per position ever played.
    pub fn key(&self) -> u64 {
        const BASIS: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        const fn mix(hash: u64, byte: u8) -> u64 {
            (hash ^ byte as u64).wrapping_mul(PRIME)
        }

        let mut hash = mix(BASIS, self.size);
        for stone in &self.stones[..self.intersections()] {
            hash = mix(
                hash,
                match stone {
                    None => 0,
                    Some(Colour::Black) => 1,
                    Some(Colour::White) => 2,
                },
            );
        }
        hash
    }
}

impl fmt::Debug for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Printed the way the board looks, top row first. A liberty bug is a
        // geometry bug, and no listing of indices will show you one.
        for row in (0..self.size).rev() {
            for column in 0..self.size {
                f.write_str(match self.at(Point::at(column, row)) {
                    None => ".",
                    Some(Colour::Black) => "X",
                    Some(Colour::White) => "O",
                })?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board(size: u8) -> Board {
        Board::new(size).expect("a legal size")
    }

    #[test]
    fn the_column_letters_skip_i() {
        // The eighth column is h and the ninth is j. Everything from there on
        // is shifted, which is why this is asserted at the seam rather than
        // trusted to a comment.
        assert_eq!(Point::at(7, 0).name(), "h1");
        assert_eq!(Point::at(8, 0).name(), "j1");
        assert_eq!(Point::at(18, 18).name(), "t19");
        assert_eq!(Point::parse("i5"), None, "there is no i column");
        assert_eq!(Point::parse("I5"), None);
    }

    #[test]
    fn every_point_on_a_nineteen_by_nineteen_board_round_trips_through_its_name() {
        for point in board(19).points() {
            assert_eq!(Point::parse(&point.name()), Some(point), "{point}");
        }
    }

    #[test]
    fn a_point_is_read_case_insensitively_and_in_one_spelling_only() {
        assert_eq!(Point::parse("D4"), Point::parse("d4"));
        for text in ["", "d", "4", "d0", "d20", "u1", "d04", "d+4", "dd4", "d4d"] {
            assert_eq!(Point::parse(text), None, "parsed {text:?}");
        }
    }

    #[test]
    fn a_corner_has_two_neighbours_an_edge_three_and_the_middle_four() {
        let board = board(9);
        assert_eq!(board.neighbours(Point::at(0, 0)).count(), 2);
        assert_eq!(board.neighbours(Point::at(8, 8)).count(), 2);
        assert_eq!(board.neighbours(Point::at(4, 0)).count(), 3);
        assert_eq!(board.neighbours(Point::at(4, 4)).count(), 4);
    }

    #[test]
    fn a_lone_stone_has_as_many_liberties_as_it_has_neighbours() {
        let mut board = board(9);
        let corner = Point::at(0, 0);
        board.set(corner, Some(Colour::Black));
        assert_eq!(board.chain(corner).expect("a stone").liberties(), 2);

        let middle = Point::at(4, 4);
        board.set(middle, Some(Colour::White));
        assert_eq!(board.chain(middle).expect("a stone").liberties(), 4);
    }

    #[test]
    fn a_chain_is_every_stone_connected_along_the_lines() {
        // Three in a row plus one hanging off it, and a fourth stone touching
        // only diagonally — which is not touching at all.
        let mut board = board(9);
        for point in [
            Point::at(2, 2),
            Point::at(3, 2),
            Point::at(4, 2),
            Point::at(3, 3),
        ] {
            board.set(point, Some(Colour::Black));
        }
        board.set(Point::at(5, 3), Some(Colour::Black));

        let chain = board.chain(Point::at(2, 2)).expect("a stone");
        assert_eq!(chain.stones().len(), 4);
        assert!(
            !chain.stones().contains(&Point::at(5, 3)),
            "a diagonal is not a connection"
        );
    }

    #[test]
    fn a_shared_liberty_is_counted_once() {
        // Two stones side by side on the bottom edge. Six empty points touch
        // them, not eight: the two in the middle are each shared.
        let mut board = board(9);
        board.set(Point::at(3, 0), Some(Colour::Black));
        board.set(Point::at(4, 0), Some(Colour::Black));
        assert_eq!(
            board.chain(Point::at(3, 0)).expect("a stone").liberties(),
            4
        );
    }

    #[test]
    fn a_key_follows_the_position_and_not_the_route_to_it() {
        let mut one = board(9);
        let mut other = board(9);
        one.set(Point::at(2, 2), Some(Colour::Black));
        one.set(Point::at(6, 6), Some(Colour::White));
        other.set(Point::at(6, 6), Some(Colour::White));
        other.set(Point::at(2, 2), Some(Colour::Black));
        assert_eq!(one, other);
        assert_eq!(one.key(), other.key());

        other.set(Point::at(2, 2), Some(Colour::White));
        assert_ne!(one.key(), other.key());
    }
}
