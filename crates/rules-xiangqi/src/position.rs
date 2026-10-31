//! A position, and the moves out of it.
//!
//! Three rules here are unlike anything in the other engines, and each is
//! marked where it is enforced: the cannon's screen, the horse's hobbled leg,
//! and the generals' refusal to face each other.
//!
//! **A position knows which game it is.** Xiangqi and janggi share this board
//! and almost nothing else about how a piece moves, so [`Ruleset`] is carried
//! on the position rather than passed at the call site: a position and its
//! rules are not independently selectable, and a xiangqi position generated
//! under janggi's rules would be a wrong answer that looks like a right one.
//! Move generation branches once, at `Position::moves_for`, into two bodies
//! that read as one game's rules each — rather than into one body speckled with
//! `if janggi`, which would make neither game readable and would hide how
//! little the two share.

use crate::board::{Bitboard, FILES, Point, RANKS};
use crate::ruleset::{JANGGI_START, Ruleset, Setup};
use core::fmt;

/// The four ways a line runs across the board.
const ORTHOGONAL: [(i16, i16); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];

/// The four diagonals. Drawn on the palaces only, and only janggi treats them
/// as lines any piece may travel — see [`Point::palace_diagonal`].
const DIAGONAL: [(i16, i16); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

/// The two ways a leap continues after its orthogonal first step, outward.
///
/// Shared by the horse and by janggi's elephant, which are the same move at two
/// lengths: one orthogonal step and then one diagonal step for the horse, two
/// for the elephant.
const fn spread(df: i16, dr: i16) -> [(i16, i16); 2] {
    if df == 0 {
        [(1, dr), (-1, dr)]
    } else {
        [(df, 1), (df, -1)]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Side {
    /// Moves first, and starts on ranks 1–4.
    Red,
    Black,
}

impl Side {
    pub const ALL: [Side; 2] = [Side::Red, Side::Black];

    pub const fn other(self) -> Side {
        match self {
            Side::Red => Side::Black,
            Side::Black => Side::Red,
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Side::Red => 0,
            Side::Black => 1,
        }
    }

    pub const fn is_red(self) -> bool {
        matches!(self, Side::Red)
    }

    /// Which way this side's soldiers advance.
    pub const fn forward(self) -> i16 {
        if self.is_red() { 1 } else { -1 }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Role {
    General,
    Advisor,
    Elephant,
    Horse,
    Chariot,
    Cannon,
    Soldier,
}

impl Role {
    pub const ALL: [Role; 7] = [
        Role::General,
        Role::Advisor,
        Role::Elephant,
        Role::Horse,
        Role::Chariot,
        Role::Cannon,
        Role::Soldier,
    ];

    pub const fn index(self) -> usize {
        match self {
            Role::General => 0,
            Role::Advisor => 1,
            Role::Elephant => 2,
            Role::Horse => 3,
            Role::Chariot => 4,
            Role::Cannon => 5,
            Role::Soldier => 6,
        }
    }

    /// The letter used in the de-facto standard FEN. Red is uppercase.
    pub const fn letter(self) -> char {
        match self {
            Role::General => 'k',
            Role::Advisor => 'a',
            Role::Elephant => 'b',
            Role::Horse => 'n',
            Role::Chariot => 'r',
            Role::Cannon => 'c',
            Role::Soldier => 'p',
        }
    }

    pub fn from_letter(letter: char) -> Option<Role> {
        Role::ALL
            .into_iter()
            .find(|role| role.letter() == letter.to_ascii_lowercase())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Piece {
    pub side: Side,
    pub role: Role,
}

/// A move: two points, and nothing else.
///
/// No move kinds. There is no promotion in either game, no castling, no en
/// passant, and a capture is always on the destination — even the cannon's,
/// which merely reaches it differently.
///
/// **A janggi pass is a move from a point to itself**, and specifically from
/// the general to himself. It needed no new kind and no new wire format, which
/// is why it is done this way: `from == to` is not a legal ordinary move for
/// any piece in either game, so the encoding is unambiguous, and the platform —
/// which stores moves as opaque tokens — needs to learn nothing.
/// The general is chosen over any other piece so that the token is the same
/// however many pieces are on the board.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move {
    pub from: Point,
    pub to: Point,
}

impl Move {
    pub const fn new(from: Point, to: Point) -> Move {
        Move { from, to }
    }

    /// Whether this is janggi's null move.
    pub const fn is_pass(self) -> bool {
        self.from.index() == self.to.index()
    }

    pub fn to_wire(self) -> String {
        format!("{}{}", self.from, self.to)
    }

    pub fn parse(text: &str) -> Option<Move> {
        if !text.is_ascii() {
            return None;
        }
        for split in 2..=3.min(text.len().saturating_sub(2)) {
            if let (Some(from), Some(to)) =
                (Point::parse(&text[..split]), Point::parse(&text[split..]))
            {
                return Some(Move::new(from, to));
            }
        }
        None
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_wire())
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_wire())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    by_side: [Bitboard; 2],
    by_role: [Bitboard; 7],
    to_move: Side,
    rules: Ruleset,
}

/// The opening array, as the published perft positions write it.
pub const START: &str = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";

impl Position {
    pub fn start() -> Position {
        Position::from_fen(START).expect("the opening array is a valid FEN")
    }

    /// A janggi opening array with both players in the diagrammed arrangement.
    pub fn janggi() -> Position {
        Position::from_fen_with(Ruleset::Janggi, JANGGI_START).expect("a valid FEN")
    }

    /// A janggi opening array with each player's chosen arrangement.
    ///
    /// Two arguments because the choice is each player's own and they are made
    /// independently: sixteen openings, not four.
    pub fn janggi_from(red: Setup, black: Setup) -> Position {
        let fen = format!(
            "{}/4k4/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/4K4/{} w - - 0 1",
            black.back_rank(Side::Black),
            red.back_rank(Side::Red)
        );
        Position::from_fen_with(Ruleset::Janggi, &fen).expect("a setup is a valid FEN")
    }

    /// Reads the de-facto standard FEN: rank 10 first, uppercase for Red.
    ///
    /// Xiangqi, because that is what a bare FEN on this board has always meant.
    /// Janggi's is [`Position::from_fen_with`], which the caller reaches only
    /// having said which game it is holding.
    pub fn from_fen(fen: &str) -> Option<Position> {
        Position::from_fen_with(Ruleset::Xiangqi, fen)
    }

    pub fn from_fen_with(rules: Ruleset, fen: &str) -> Option<Position> {
        let mut fields = fen.split_whitespace();
        let placement = fields.next()?;
        let side = match fields.next() {
            Some("w") | Some("r") | None => Side::Red,
            Some("b") => Side::Black,
            _ => return None,
        };

        let mut by_side = [Bitboard::EMPTY; 2];
        let mut by_role = [Bitboard::EMPTY; 7];
        let rows: Vec<&str> = placement.split('/').collect();
        if rows.len() != RANKS as usize {
            return None;
        }
        for (index, row) in rows.iter().enumerate() {
            // Written from rank 10 down.
            let rank = RANKS - 1 - index as u8;
            let mut file = 0u8;
            for character in row.chars() {
                if let Some(skip) = character.to_digit(10) {
                    file += skip as u8;
                    continue;
                }
                let role = Role::from_letter(character)?;
                let side = if character.is_ascii_uppercase() {
                    Side::Red
                } else {
                    Side::Black
                };
                let point = Point::from_file_rank(file as i16, rank as i16)?;
                by_side[side.index()] = by_side[side.index()].with(point);
                by_role[role.index()] = by_role[role.index()].with(point);
                file += 1;
            }
            if file != FILES {
                return None;
            }
        }

        let position = Position {
            by_side,
            by_role,
            to_move: side,
            rules,
        };
        // One general each, or it is not a position.
        for side in Side::ALL {
            position.general(side)?;
        }
        Some(position)
    }

    pub fn to_fen(&self) -> String {
        let mut fen = String::with_capacity(90);
        for rank in (0..RANKS).rev() {
            let mut empty = 0;
            for file in 0..FILES {
                let point = Point::from_file_rank(file as i16, rank as i16).expect("on the board");
                match self.piece_at(point) {
                    Some(piece) => {
                        if empty > 0 {
                            fen.push_str(&empty.to_string());
                            empty = 0;
                        }
                        let letter = piece.role.letter();
                        fen.push(if piece.side.is_red() {
                            letter.to_ascii_uppercase()
                        } else {
                            letter
                        });
                    }
                    None => empty += 1,
                }
            }
            if empty > 0 {
                fen.push_str(&empty.to_string());
            }
            if rank > 0 {
                fen.push('/');
            }
        }
        fen.push(' ');
        fen.push(if self.to_move.is_red() { 'w' } else { 'b' });
        fen.push_str(" - - 0 1");
        fen
    }

    pub fn to_move(&self) -> Side {
        self.to_move
    }

    pub fn rules(&self) -> Ruleset {
        self.rules
    }

    pub fn side(&self, side: Side) -> Bitboard {
        self.by_side[side.index()]
    }

    pub fn role(&self, role: Role) -> Bitboard {
        self.by_role[role.index()]
    }

    pub fn pieces(&self, side: Side, role: Role) -> Bitboard {
        self.side(side).intersection(self.role(role))
    }

    pub fn occupied(&self) -> Bitboard {
        self.by_side[0].union(self.by_side[1])
    }

    pub fn general(&self, side: Side) -> Option<Point> {
        self.pieces(side, Role::General).first()
    }

    pub fn piece_at(&self, point: Point) -> Option<Piece> {
        let side = Side::ALL
            .into_iter()
            .find(|&side| self.side(side).contains(point))?;
        let role = Role::ALL
            .into_iter()
            .find(|&role| self.role(role).contains(point))?;
        Some(Piece { side, role })
    }

    pub fn make_move(&self, mv: Move) -> Position {
        let mut next = *self;
        // A pass changes nothing but the turn. Written out rather than left to
        // fall through the code below — which would happen to produce the right
        // board, by removing the general as a "capture" and then putting him
        // back — because a thing that works by accident is a thing the next
        // person to touch it will break.
        if mv.is_pass() {
            next.to_move = self.to_move.other();
            return next;
        }
        let piece = self.piece_at(mv.from).expect("a move from an empty point");

        if let Some(captured) = self.piece_at(mv.to) {
            next.by_side[captured.side.index()] =
                next.by_side[captured.side.index()].without(mv.to);
            next.by_role[captured.role.index()] =
                next.by_role[captured.role.index()].without(mv.to);
        }
        next.by_side[piece.side.index()] = next.by_side[piece.side.index()]
            .without(mv.from)
            .with(mv.to);
        next.by_role[piece.role.index()] = next.by_role[piece.role.index()]
            .without(mv.from)
            .with(mv.to);
        next.to_move = self.to_move.other();
        next
    }

    /// Every move the side to move may make, legality included.
    pub fn legal_moves(&self) -> Vec<Move> {
        self.pseudo_legal_moves()
            .into_iter()
            .filter(|&mv| {
                let after = self.make_move(mv);
                !after.is_in_check(self.to_move)
            })
            .collect()
    }

    /// Whether `side`'s general is attacked — **or, in xiangqi, facing the
    /// other general**.
    ///
    /// The facing rule is folded in here rather than filtered later, and that
    /// is what makes it right: it is not a rule about which moves are legal,
    /// it is that two generals on an open file are an illegal *position*. So a
    /// move that creates one is refused by exactly the same code that refuses
    /// moving into check.
    ///
    /// **Janggi does not fold it in, because janggi does not forbid it.** Two
    /// generals facing down an open file is *bikjang*, a legal position that
    /// ends the game — see [`Position::generals_face`] and the draw in
    /// [`crate::game`]. Carrying xiangqi's reading into janggi would delete a
    /// whole class of legal moves and, worse, would silently turn every
    /// bikjang into a checkmate.
    pub fn is_in_check(&self, side: Side) -> bool {
        let Some(general) = self.general(side) else {
            return true;
        };
        if self.rules == Ruleset::Xiangqi && self.generals_face() {
            return true;
        }
        self.is_attacked(general, side.other())
    }

    /// Whether the two generals see each other down an empty file.
    ///
    /// The same geometry means opposite things in the two games: xiangqi
    /// forbids the position outright, janggi calls it *bikjang* and ends the
    /// game with it.
    pub fn generals_face(&self) -> bool {
        let (Some(red), Some(black)) = (self.general(Side::Red), self.general(Side::Black)) else {
            return false;
        };
        if red.file() != black.file() {
            return false;
        }
        let occupied = self.occupied();
        let mut rank = red.rank() + 1;
        while rank < black.rank() {
            let between =
                Point::from_file_rank(red.file() as i16, rank as i16).expect("on the board");
            if occupied.contains(between) {
                return false;
            }
            rank += 1;
        }
        true
    }

    /// Whether `by` attacks `target`.
    pub fn is_attacked(&self, target: Point, by: Side) -> bool {
        self.pseudo_legal_for(by)
            .into_iter()
            .any(|mv| mv.to == target)
    }

    /// Janggi's pass, if this game has one.
    ///
    /// **한수쉼** (*hansu-shim*, "resting a move"). Nothing else in the chess
    /// family has it, and two consequences run through the rest of the engine:
    ///
    /// - **Stalemate cannot happen.** A player who is not in check always has
    ///   at least this move, so an empty move list means checkmate and
    ///   [`crate::WinReason::Stalemate`] is unreachable under janggi.
    /// - **You may not pass out of check.** Wikipedia and the Korean rules
    ///   agree — "unless the general is in check, there is no restriction on
    ///   when or how many times one can pass" — and that needs no rule of its
    ///   own here: a pass leaves the board exactly as it was, so if you were in
    ///   check before it you are in check after it, and the ordinary
    ///   own-general-not-left-in-check filter in [`Position::legal_moves`]
    ///   throws it out. That is why the pass is generated here, among the
    ///   pseudo-legal moves, rather than appended to the legal ones.
    pub fn pass(&self) -> Option<Move> {
        if self.rules != Ruleset::Janggi {
            return None;
        }
        let general = self.general(self.to_move)?;
        Some(Move::new(general, general))
    }

    pub fn pseudo_legal_moves(&self) -> Vec<Move> {
        let mut moves = self.pseudo_legal_for(self.to_move);
        moves.extend(self.pass());
        moves
    }

    /// What `side` could play if it were their turn, **without the pass**.
    ///
    /// The omission is deliberate and is what makes [`Position::is_attacked`]
    /// correct: that asks which points a side's pieces bear on, and a pass
    /// bears on nothing. Including it would have a general appear to attack the
    /// point he is standing on.
    fn pseudo_legal_for(&self, side: Side) -> Vec<Move> {
        let mut moves = Vec::with_capacity(48);
        let ours = self.side(side);
        for from in ours {
            let role = Role::ALL
                .into_iter()
                .find(|&role| self.role(role).contains(from))
                .expect("a piece has a role");
            self.moves_for(side, role, from, &mut moves);
        }
        moves
    }

    fn moves_for(&self, side: Side, role: Role, from: Point, moves: &mut Vec<Move>) {
        match self.rules {
            Ruleset::Xiangqi => self.xiangqi_moves(side, role, from, moves),
            Ruleset::Janggi => self.janggi_moves(side, role, from, moves),
        }
    }

    fn xiangqi_moves(&self, side: Side, role: Role, from: Point, moves: &mut Vec<Move>) {
        let ours = self.side(side);
        let occupied = self.occupied();
        let offer = |to: Point, moves: &mut Vec<Move>| {
            if !ours.contains(to) {
                moves.push(Move::new(from, to));
            }
        };

        match role {
            // One point orthogonally, and never off the palace. The general
            // does **not** use the palace diagonals — those are drawn for the
            // advisor, and janggi is the game where the general uses them.
            Role::General => {
                for (df, dr) in ORTHOGONAL {
                    if let Some(to) = from.offset(df, dr)
                        && to.is_palace(side.is_red())
                    {
                        offer(to, moves);
                    }
                }
            }
            // Exactly one point diagonally, inside the palace. Five reachable
            // points in all, and nothing can block it.
            Role::Advisor => {
                for (df, dr) in DIAGONAL {
                    if let Some(to) = from.offset(df, dr)
                        && to.is_palace(side.is_red())
                    {
                        offer(to, moves);
                    }
                }
            }
            // Two points diagonally, blocked at the midpoint — the "elephant's
            // eye" — by anything at all, and it may never cross the river.
            Role::Elephant => {
                for (df, dr) in [(2, 2), (2, -2), (-2, 2), (-2, -2)] {
                    let Some(to) = from.offset(df, dr) else {
                        continue;
                    };
                    if !to.is_own_half(side.is_red()) {
                        continue;
                    }
                    let eye = from
                        .offset(df / 2, dr / 2)
                        .expect("halfway is on the board");
                    if occupied.contains(eye) {
                        continue;
                    }
                    offer(to, moves);
                }
            }
            Role::Horse => self.horse_moves(side, from, moves),
            Role::Chariot => self.slide_orthogonally(side, from, moves),
            // **The cannon.** It moves like a chariot and captures like
            // nothing else: it must leap exactly one piece — the screen — and
            // takes the first piece beyond it. Two separate walks, because the
            // rules for moving and taking do not overlap at all.
            Role::Cannon => {
                for (df, dr) in ORTHOGONAL {
                    let mut steps = 1;
                    // Quiet moves: over empty points only.
                    while let Some(to) = from.offset(df * steps, dr * steps) {
                        if occupied.contains(to) {
                            break;
                        }
                        moves.push(Move::new(from, to));
                        steps += 1;
                    }
                    // The screen is whatever stopped that walk; the target is
                    // the next piece beyond it, of either side — but only an
                    // enemy can be taken. If the walk ran off the board there
                    // is nothing to leap.
                    if from.offset(df * steps, dr * steps).is_none() {
                        continue;
                    }
                    let mut beyond = steps + 1;
                    while let Some(to) = from.offset(df * beyond, dr * beyond) {
                        if occupied.contains(to) {
                            if !ours.contains(to) {
                                moves.push(Move::new(from, to));
                            }
                            break;
                        }
                        beyond += 1;
                    }
                }
            }
            // One point forward; sideways too, once across the river. Never
            // backwards, and there is no promotion — a soldier on the last
            // rank can only shuffle along it.
            Role::Soldier => {
                let forward = side.forward();
                if let Some(to) = from.offset(0, forward) {
                    offer(to, moves);
                }
                if !from.is_own_half(side.is_red()) {
                    for df in [1, -1] {
                        if let Some(to) = from.offset(df, 0) {
                            offer(to, moves);
                        }
                    }
                }
            }
        }
    }

    /// Janggi's moves.
    ///
    /// Only the horse is unchanged from xiangqi. Everything else on this board
    /// moves differently, and the four that catch people out are marked below:
    /// the elephant's longer leap, the palace lines, the cannon that cannot
    /// move at all without a screen, and the soldier that never had a river to
    /// cross.
    fn janggi_moves(&self, side: Side, role: Role, from: Point, moves: &mut Vec<Move>) {
        let ours = self.side(side);
        let occupied = self.occupied();
        let cannons = self.role(Role::Cannon);
        let offer = |to: Point, moves: &mut Vec<Move>| {
            if !ours.contains(to) {
                moves.push(Move::new(from, to));
            }
        };

        match role {
            // **The general and the guards move alike**: one point along any
            // line drawn on the palace, orthogonal or diagonal, and never off
            // it. Two differences from xiangqi in one arm — xiangqi's general
            // has no diagonal and xiangqi's advisor has nothing else — which is
            // why they are written together here and separately above.
            Role::General | Role::Advisor => {
                for (df, dr) in ORTHOGONAL {
                    if let Some(to) = from.offset(df, dr)
                        && to.is_palace(side.is_red())
                    {
                        offer(to, moves);
                    }
                }
                // No palace test on these: a diagonal that is a drawn line
                // already has both ends in one palace, and a general who is in
                // his own palace cannot be led out of it by one.
                for (df, dr) in DIAGONAL {
                    if let Some(to) = from.palace_diagonal(df, dr) {
                        offer(to, moves);
                    }
                }
            }
            // **A 2-3 leap, and blocked at both of its intermediate points.**
            // One orthogonal step and then two diagonal ones, all outward, so
            // it lands on the far corner of a two-by-three rectangle. It has no
            // river to respect and ranges over the whole board, which makes it
            // a genuinely long-range piece rather than xiangqi's home defender.
            //
            // Both blocking points are real and forgetting the second is the
            // classic bug: a piece on the first diagonal point stops the
            // elephant exactly as a piece on the orthogonal step does.
            Role::Elephant => {
                for (df, dr) in ORTHOGONAL {
                    let Some(leg) = from.offset(df, dr) else {
                        continue;
                    };
                    if occupied.contains(leg) {
                        continue;
                    }
                    for (sf, sr) in spread(df, dr) {
                        let Some(knee) = leg.offset(sf, sr) else {
                            continue;
                        };
                        if occupied.contains(knee) {
                            continue;
                        }
                        if let Some(to) = knee.offset(sf, sr) {
                            offer(to, moves);
                        }
                    }
                }
            }
            Role::Horse => self.horse_moves(side, from, moves),
            // A chariot on a palace line may travel along it, which is the
            // rule people forget applies to pieces other than the general: a
            // chariot on d1 attacks f3 through the centre of the palace.
            Role::Chariot => {
                self.slide_orthogonally(side, from, moves);
                for (df, dr) in DIAGONAL {
                    let mut at = from;
                    while let Some(to) = at.palace_diagonal(df, dr) {
                        offer(to, moves);
                        if occupied.contains(to) {
                            break;
                        }
                        at = to;
                    }
                }
            }
            // **The biggest single difference between the two games.** Three
            // rules, and each of them is a way to get janggi wrong by writing
            // xiangqi:
            //
            // - It needs a screen **to move at all**, not only to capture.
            //   Xiangqi's cannon slides freely and leaps only to take; janggi's
            //   cannot take a single step without something to jump. A cannon
            //   with no screen anywhere on its lines has no moves.
            // - **It may not leap a cannon**, of either colour.
            // - **It may not capture a cannon**, of either colour.
            //
            // The consequence is that cannons are nearly harmless to each
            // other and that the opening's cannon-swapping tactics simply do
            // not exist.
            Role::Cannon => {
                for (df, dr) in ORTHOGONAL {
                    let mut steps = 1;
                    let screen = loop {
                        match from.offset(df * steps, dr * steps) {
                            Some(point) if occupied.contains(point) => break Some(point),
                            Some(_) => steps += 1,
                            None => break None,
                        }
                    };
                    let Some(screen) = screen else {
                        continue;
                    };
                    if cannons.contains(screen) {
                        continue;
                    }
                    let mut beyond = steps + 1;
                    while let Some(to) = from.offset(df * beyond, dr * beyond) {
                        if occupied.contains(to) {
                            if !ours.contains(to) && !cannons.contains(to) {
                                moves.push(Move::new(from, to));
                            }
                            break;
                        }
                        moves.push(Move::new(from, to));
                        beyond += 1;
                    }
                }
                // Along a palace line there is exactly one shape that works —
                // a corner, a screen on the centre, the opposite corner —
                // because a line is only two steps long and the screen has to
                // be on the one point between them.
                for (df, dr) in DIAGONAL {
                    let Some(screen) = from.palace_diagonal(df, dr) else {
                        continue;
                    };
                    let Some(to) = screen.palace_diagonal(df, dr) else {
                        continue;
                    };
                    if !occupied.contains(screen) || cannons.contains(screen) {
                        continue;
                    }
                    if occupied.contains(to) && (ours.contains(to) || cannons.contains(to)) {
                        continue;
                    }
                    moves.push(Move::new(from, to));
                }
            }
            // **Sideways from the first move.** There is no river, so there is
            // nothing to cross and nothing to gain by crossing it: a janggi
            // soldier has its full move from the opening array. Still never
            // backwards, and still no promotion.
            //
            // It uses the palace lines too — forward along them only, which
            // falls out of asking for the diagonal that goes the way it is
            // already going. That is the rule most often left out of a first
            // implementation, and it matters: a soldier on the palace centre
            // attacks both far corners at once.
            Role::Soldier => {
                let forward = side.forward();
                if let Some(to) = from.offset(0, forward) {
                    offer(to, moves);
                }
                for df in [1, -1] {
                    if let Some(to) = from.offset(df, 0) {
                        offer(to, moves);
                    }
                    if let Some(to) = from.palace_diagonal(df, forward) {
                        offer(to, moves);
                    }
                }
            }
        }
    }

    /// One point orthogonally then one diagonally outward, blocked at the
    /// **orthogonal** step — "hobbling the horse's leg".
    ///
    /// Shared between the two games because it is the one piece they agree
    /// about completely.
    fn horse_moves(&self, side: Side, from: Point, moves: &mut Vec<Move>) {
        let ours = self.side(side);
        let occupied = self.occupied();
        for (df, dr) in ORTHOGONAL {
            let Some(leg) = from.offset(df, dr) else {
                continue;
            };
            if occupied.contains(leg) {
                continue;
            }
            for (sf, sr) in spread(df, dr) {
                if let Some(to) = leg.offset(sf, sr)
                    && !ours.contains(to)
                {
                    moves.push(Move::new(from, to));
                }
            }
        }
    }

    /// A chariot's ordinary move: any distance along a file or a rank.
    ///
    /// Shared, and it is the whole of xiangqi's chariot. Janggi's adds the
    /// palace lines on top.
    fn slide_orthogonally(&self, side: Side, from: Point, moves: &mut Vec<Move>) {
        let ours = self.side(side);
        let occupied = self.occupied();
        for (df, dr) in ORTHOGONAL {
            let mut steps = 1;
            while let Some(to) = from.offset(df * steps, dr * steps) {
                if !ours.contains(to) {
                    moves.push(Move::new(from, to));
                }
                if occupied.contains(to) {
                    break;
                }
                steps += 1;
            }
        }
    }
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in (0..RANKS).rev() {
            for file in 0..FILES {
                let point = Point::from_file_rank(file as i16, rank as i16).expect("on the board");
                match self.piece_at(point) {
                    Some(piece) => {
                        let letter = piece.role.letter();
                        f.write_str(&if piece.side.is_red() {
                            letter.to_ascii_uppercase().to_string()
                        } else {
                            letter.to_string()
                        })?
                    }
                    None => f.write_str(".")?,
                }
            }
            writeln!(f)?;
            if rank == 5 && self.rules == Ruleset::Xiangqi {
                writeln!(f, "~~~~~~~~~")?;
            }
        }
        write!(f, "{:?} to move in {}", self.to_move, self.rules.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_opening_array_round_trips() {
        let start = Position::start();
        assert_eq!(start.to_fen(), START);
        assert_eq!(start.to_move(), Side::Red);
        assert_eq!(start.side(Side::Red).count(), 16);
        assert_eq!(start.side(Side::Black).count(), 16);
        assert_eq!(start.pieces(Side::Red, Role::Soldier).count(), 5);
        assert_eq!(start.pieces(Side::Red, Role::Cannon).count(), 2);
        assert_eq!(start.general(Side::Red), Point::parse("e1"));
        assert_eq!(start.general(Side::Black), Point::parse("e10"));
    }

    #[test]
    fn there_are_forty_four_opening_moves() {
        // The published perft(1). Worth its own test: a bare node count at
        // depth one is the cheapest possible check that every piece moves.
        assert_eq!(Position::start().legal_moves().len(), 44);
    }

    #[test]
    fn the_generals_may_not_face_each_other() {
        // Not a rule about moves — a rule about positions. Two generals on an
        // open file is illegal however it came about, which is why it lives
        // with the check test rather than beside it.
        let facing = Position::from_fen("4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1").expect("valid");
        assert!(facing.generals_face());
        assert!(facing.is_in_check(Side::Red));
        assert!(facing.is_in_check(Side::Black));

        let blocked = Position::from_fen("4k4/9/9/9/9/4p4/9/9/9/4K4 w - - 0 1").expect("valid");
        assert!(!blocked.generals_face());
    }

    #[test]
    fn a_cannon_needs_a_screen_to_capture_and_none_to_move() {
        // Red cannon on e1, black soldier on e5, nothing between.
        let no_screen = Position::from_fen("3k5/9/9/9/9/4p4/9/9/9/4K3C w - - 0 1").expect("valid");
        let cannon = Point::parse("i1").unwrap();
        let moves: Vec<String> = no_screen
            .legal_moves()
            .into_iter()
            .filter(|mv| mv.from == cannon)
            .map(|mv| mv.to_wire())
            .collect();
        assert!(!moves.is_empty(), "it can still move");

        // With a screen, it reaches past it.
        let screened = Position::from_fen("3k5/9/9/9/8p/9/9/9/8P/4K3C w - - 0 1").expect("valid");
        let takes: Vec<String> = screened
            .legal_moves()
            .into_iter()
            .filter(|mv| mv.from == cannon)
            .map(|mv| mv.to_wire())
            .collect();
        assert!(takes.contains(&"i1i6".to_string()), "over i2: {takes:?}");
    }

    #[test]
    fn a_horse_is_hobbled_by_its_own_leg() {
        // The asymmetry: whether a horse attacks a point depends on what
        // stands beside the *horse*, not beside the target.
        let free = Position::from_fen("3k5/9/9/9/9/9/9/9/9/4K3N w - - 0 1").expect("valid");
        let horse = Point::parse("i1").unwrap();
        let open = free
            .legal_moves()
            .into_iter()
            .filter(|mv| mv.from == horse)
            .count();
        assert!(open > 0);

        // Block the leg directly above it and the two moves that way vanish.
        let blocked = Position::from_fen("3k5/9/9/9/9/9/9/9/8P/4K3N w - - 0 1").expect("valid");
        let fewer = blocked
            .legal_moves()
            .into_iter()
            .filter(|mv| mv.from == horse)
            .count();
        assert!(fewer < open, "the leg is hobbled: {fewer} vs {open}");
    }

    #[test]
    fn an_elephant_may_not_cross_the_river() {
        let start = Position::start();
        let elephant = Point::parse("c1").unwrap();
        for mv in start
            .legal_moves()
            .into_iter()
            .filter(|mv| mv.from == elephant)
        {
            assert!(mv.to.is_own_half(true), "{mv} crosses the river");
        }
    }

    #[test]
    fn a_soldier_gains_sideways_moves_after_crossing() {
        let before = Position::start();
        let soldier = Point::parse("a4").unwrap();
        let sideways = before
            .legal_moves()
            .into_iter()
            .filter(|mv| mv.from == soldier && mv.to.rank() == mv.from.rank())
            .count();
        assert_eq!(sideways, 0, "not yet across");

        let across = Position::from_fen("3k5/9/9/9/P8/9/9/9/9/4K4 w - - 0 1").expect("valid");
        let over = Point::parse("a6").unwrap();
        let moves: Vec<String> = across
            .legal_moves()
            .into_iter()
            .filter(|mv| mv.from == over)
            .map(|mv| mv.to_wire())
            .collect();
        assert!(moves.contains(&"a6b6".to_string()), "{moves:?}");
        assert!(!moves.contains(&"a6a5".to_string()), "never backwards");
    }

    /// A janggi position from a FEN, and the wire moves one point offers.
    fn janggi(fen: &str) -> Position {
        Position::from_fen_with(Ruleset::Janggi, fen).expect("valid")
    }

    fn from(position: &Position, point: &str) -> Vec<String> {
        let point = Point::parse(point).expect("a point");
        let mut moves: Vec<String> = position
            .legal_moves()
            .into_iter()
            .filter(|mv| mv.from == point && !mv.is_pass())
            .map(|mv| mv.to_wire())
            .collect();
        moves.sort();
        moves
    }

    #[test]
    fn the_janggi_opening_array_stands_differently_from_xiangqis() {
        let start = Position::janggi();
        assert_eq!(start.side(Side::Red).count(), 16);
        assert_eq!(
            start.general(Side::Red),
            Point::parse("e2"),
            "in the middle"
        );
        assert_eq!(start.general(Side::Black), Point::parse("e9"));
        assert_eq!(start.pieces(Side::Red, Role::Advisor).count(), 2);
        assert!(
            start.side(Side::Red).contains(Point::parse("d1").unwrap()),
            "the guards took the back rank the general left"
        );
        assert!(
            start.side(Side::Red).contains(Point::parse("a4").unwrap()),
            "soldiers on the fourth rank, with no river ahead of them"
        );
    }

    #[test]
    fn a_janggi_soldier_moves_sideways_from_the_opening_array() {
        // The river is the rule that is *gone*, so the test is that nothing
        // has to happen first. A xiangqi soldier would need five moves to earn
        // this.
        let start = Position::janggi();
        assert_eq!(from(&start, "c4"), ["c4b4", "c4c5", "c4d4"]);
    }

    #[test]
    fn a_janggi_elephant_is_blocked_at_both_of_its_intermediate_points() {
        // The 2-3 leap has two points on the way and either of them stops it.
        // Forgetting the second is the classic bug, and it is invisible in most
        // positions: the elephant still reaches six or seven of its eight
        // points, so only a test that blocks the far one catches it.
        let open = janggi("3k5/9/9/9/9/4B4/9/9/9/4K4 w - - 0 1");
        assert_eq!(
            from(&open, "e5"),
            [
                "e5b3", "e5b7", "e5c2", "e5c8", "e5g2", "e5g8", "e5h3", "e5h7"
            ],
            "eight points, and c8 and g8 are deep in Black's half"
        );

        // f7 is the *second* point on the way to g8 and blocks it alone.
        let far = janggi("3k5/9/9/5p3/9/4B4/9/9/9/4K4 w - - 0 1");
        let moves = from(&far, "e5");
        assert!(!moves.contains(&"e5g8".to_string()), "{moves:?}");
        assert!(moves.contains(&"e5c8".to_string()), "and only that one");

        // e6 is the first, and it is shared by both leaps that go north.
        let near = janggi("3k5/9/9/9/4p4/4B4/9/9/9/4K4 w - - 0 1");
        let moves = from(&near, "e5");
        assert!(!moves.contains(&"e5g8".to_string()), "{moves:?}");
        assert!(!moves.contains(&"e5c8".to_string()), "{moves:?}");
        assert!(
            moves.contains(&"e5h3".to_string()),
            "southward is untouched"
        );
    }

    #[test]
    fn the_general_and_the_guards_travel_the_palace_diagonals() {
        // Both of them, and by the same rule. In xiangqi the general has no
        // diagonal at all and the advisor has nothing else.
        let position = janggi("3k5/9/9/9/9/9/9/9/4A4/4K4 w - - 0 1");
        assert_eq!(
            from(&position, "e2"),
            ["e2d1", "e2d2", "e2d3", "e2e3", "e2f1", "e2f2", "e2f3"],
            "the guard on the palace centre reaches all four corners"
        );
        assert_eq!(
            from(&position, "e1"),
            ["e1d1", "e1f1"],
            "and the general beneath him has only the back rank left"
        );
    }

    #[test]
    fn a_diagonal_that_is_not_a_drawn_line_is_not_a_move() {
        // d2 is inside the palace and on no line. A general there has three
        // moves, all orthogonal — which is what says the rule is about the
        // lines rather than about being in the palace.
        let position = janggi("3k5/9/9/9/9/9/9/9/3K5/9 w - - 0 1");
        assert_eq!(from(&position, "d2"), ["d2d1", "d2d3", "d2e2"]);
    }

    #[test]
    fn a_chariot_on_a_palace_line_may_travel_it() {
        // The rule people forget applies to anything but the general: a
        // chariot on d1 bears on f3 through the centre of the palace.
        let open = janggi("3k5/9/9/9/9/9/9/3K5/9/3R5 w - - 0 1");
        let moves = from(&open, "d1");
        assert!(moves.contains(&"d1e2".to_string()), "{moves:?}");
        assert!(moves.contains(&"d1f3".to_string()), "{moves:?}");
        assert!(
            !moves.contains(&"d1g4".to_string()),
            "and stops where the line does"
        );

        // The centre blocks it like any other point on a chariot's line.
        let blocked = janggi("3k5/9/9/9/9/9/9/3K5/4p4/3R5 w - - 0 1");
        let moves = from(&blocked, "d1");
        assert!(moves.contains(&"d1e2".to_string()), "it takes the blocker");
        assert!(!moves.contains(&"d1f3".to_string()), "{moves:?}");
    }

    #[test]
    fn a_janggi_cannon_cannot_move_at_all_without_a_screen() {
        // **The biggest single difference.** A xiangqi cannon in this position
        // would have the whole file and rank; this one has nothing.
        let alone = janggi("3k5/9/9/9/9/4C4/9/9/9/4K4 w - - 0 1");
        assert_eq!(from(&alone, "e5"), Vec::<String>::new());

        let screened = janggi("3k5/9/9/4p4/9/4C4/9/9/9/4K4 w - - 0 1");
        assert_eq!(
            from(&screened, "e5"),
            ["e5e10", "e5e8", "e5e9"],
            "over the soldier on e7 and on as far as it likes"
        );
    }

    #[test]
    fn a_janggi_cannon_will_neither_leap_a_cannon_nor_take_one() {
        // Two rules, and each has to be tested against the position that would
        // pass with only the other implemented.
        let screen_is_a_cannon = janggi("3k5/9/9/4c4/9/4C4/9/9/9/4K4 w - - 0 1");
        assert_eq!(
            from(&screen_is_a_cannon, "e5"),
            Vec::<String>::new(),
            "an enemy cannon is not a screen"
        );

        let target_is_a_cannon = janggi("3k5/4c4/9/4p4/9/4C4/9/9/9/4K4 w - - 0 1");
        let moves = from(&target_is_a_cannon, "e5");
        assert!(moves.contains(&"e5e8".to_string()), "{moves:?}");
        assert!(
            !moves.contains(&"e5e9".to_string()),
            "the cannon on e9 is not takeable: {moves:?}"
        );
    }

    #[test]
    fn a_cannon_leaps_the_palace_centre_along_the_drawn_line() {
        // The one diagonal shape a cannon has: a corner, a screen on the
        // centre, the opposite corner. There is nowhere else on a two-step
        // line to put a screen.
        let over_a_soldier = janggi("3k5/9/9/9/9/9/9/9/4P4/3CK4 w - - 0 1");
        assert!(
            from(&over_a_soldier, "d1").contains(&"d1f3".to_string()),
            "{:?}",
            from(&over_a_soldier, "d1")
        );

        let over_a_cannon = janggi("3k5/9/9/9/9/9/9/9/4C4/3CK4 w - - 0 1");
        assert!(!from(&over_a_cannon, "d1").contains(&"d1f3".to_string()));

        let onto_a_cannon = janggi("3k5/9/9/9/9/9/9/5c3/4P4/3CK4 w - - 0 1");
        assert!(!from(&onto_a_cannon, "d1").contains(&"d1f3".to_string()));
    }

    #[test]
    fn a_soldier_uses_the_palace_lines_forwards_only() {
        let entering = janggi("3k5/9/3P5/9/9/9/9/9/9/4K4 w - - 0 1");
        assert_eq!(
            from(&entering, "d8"),
            ["d8c8", "d8d9", "d8e8", "d8e9"],
            "the line from the palace corner to its centre"
        );

        let at_the_centre = janggi("3k5/4P4/9/9/9/9/9/9/9/4K4 w - - 0 1");
        let moves = from(&at_the_centre, "e9");
        assert!(moves.contains(&"e9f10".to_string()), "{moves:?}");
        assert!(moves.contains(&"e9d10".to_string()), "taking the general");
        assert!(
            !moves.contains(&"e9d8".to_string()) && !moves.contains(&"e9f8".to_string()),
            "and never a step backwards, even along a line: {moves:?}"
        );
    }

    #[test]
    fn passing_is_a_move_and_only_janggi_has_it() {
        let start = Position::janggi();
        let pass = start.pass().expect("janggi may pass");
        assert!(pass.is_pass());
        assert_eq!(pass.to_wire(), "e2e2", "the general, to himself");
        assert!(start.legal_moves().contains(&pass));

        assert_eq!(Position::start().pass(), None);
        assert!(
            !Position::start()
                .legal_moves()
                .iter()
                .any(|mv| mv.is_pass()),
            "xiangqi has no null move"
        );
    }

    #[test]
    fn you_may_not_pass_out_of_check() {
        // Wikipedia and the Korean rules both put it as "unless the general is
        // in check, there is no restriction on when or how many times one can
        // pass". No rule enforces it here: a pass leaves the board alone, so
        // the ordinary filter that refuses to leave your own general attacked
        // refuses it, which is the reason the pass is generated among the
        // pseudo-legal moves rather than appended to the legal ones.
        let in_check = janggi("3k5/9/9/9/9/4r4/9/9/4K4/9 w - - 0 1");
        assert!(in_check.is_in_check(Side::Red));
        let moves = in_check.legal_moves();
        assert!(!moves.is_empty(), "he can still step aside");
        assert!(!moves.iter().any(|mv| mv.is_pass()), "but not stand still");
    }

    #[test]
    fn two_generals_may_face_each_other_in_janggi() {
        // The same position, read two ways. Xiangqi calls it an illegal
        // position and folds it into check; janggi calls it bikjang, plays it,
        // and ends the game with it.
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1";

        let janggi = janggi(fen);
        assert!(janggi.generals_face());
        assert!(!janggi.is_in_check(Side::Red));
        assert!(!janggi.is_in_check(Side::Black));

        let xiangqi = Position::from_fen(fen).expect("valid");
        assert!(xiangqi.is_in_check(Side::Red));
    }

    #[test]
    fn the_two_games_agree_about_the_horse_and_about_nothing_else() {
        // A middlegame-ish position read under both rulesets, piece by piece.
        // The horse is the only one whose move list is identical, and saying so
        // as a test is what stops a future edit quietly sharing more than it
        // should.
        let fen = "3k5/9/9/4n4/4b4/2N1B2C1/4c4/9/9/3RK4 w - - 0 1";
        let under_xiangqi = Position::from_fen(fen).expect("valid");
        let under_janggi = janggi(fen);

        assert_eq!(
            from(&under_xiangqi, "c5"),
            from(&under_janggi, "c5"),
            "the horse"
        );
        for (point, piece) in [("e5", "elephant"), ("h5", "cannon"), ("d1", "chariot")] {
            assert_ne!(
                from(&under_xiangqi, point),
                from(&under_janggi, point),
                "the {piece} should not move alike"
            );
        }
    }
}
