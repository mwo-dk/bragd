//! Teaching positions: a board with a common mistake in it, and a paragraph
//! on the move nearly everybody plays there and why it loses.
//!
//! Every Family has some. A position is reached one of two ways:
//!
//! - `line` is empty: the Game starts from `diagram`, which is in the
//!   Family's own notation.
//! - `line` is not empty: the Game starts from the Variant's own start (in
//!   `arrangement`, when the Variant has several) and plays `line`. This is
//!   for the Families whose position notation loses something: Seirawan's FEN
//!   drops the pieces in hand, and the draughts, backgammon, Ur and senet
//!   positions end on the throw or capture the lesson is about. `diagram` is
//!   then only the picture, and the test holds the two together.
//!
//! `title` and `text` are English for now; `key` is stable and will become the
//! text key when the texts are translated.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    /// A Variant key from the catalogue.
    pub variant: &'static str,
    /// Stable and unique: `<variant>.<slug>`.
    pub key: &'static str,
    pub title: &'static str,
    pub text: &'static str,
    /// The position in the Family's own notation, as the lesson draws it.
    pub diagram: &'static str,
    /// Entries from the Variant's start that reach the position, or empty.
    pub line: &'static [&'static str],
    /// Which starting arrangement `line` is played from, for backgammon and Ur.
    pub arrangement: Option<u16>,
    /// The seat the text addresses: 0 for the first Side, 1 for the second.
    pub to_move: u8,
    /// The move the text names, in the Family's wire form.
    pub answer: Option<&'static str>,
}

impl Position {
    /// The initial position and entries to hand to [`crate::replay`].
    pub fn start(&self) -> (Option<&'static str>, &'static [&'static str]) {
        if self.line.is_empty() {
            (Some(self.diagram), self.line)
        } else {
            (None, self.line)
        }
    }
}

pub const POSITIONS: &[Position] = &[
    Position {
        variant: "chess",
        key: "chess.back-rank-mate",
        title: "The back-rank mate",
        text: "White plays Rd8 and the game is over. Black has three pawns in front of his king and not one of them has moved, so the king has nowhere on the board to go: f8 and h8 are the rook’s, and f7, g7 and h7 are his own. Nothing can be put in the way and nothing can take the rook. This is the first way most people lose a game they were not losing, and it happens in the middlegame far more often than in the ending — a rook swings to an empty file, and the file happens to be the back one. The defence costs a single tempo and is worth spending long before it looks necessary: push one of those three pawns a square, usually the h-pawn, and the mate stops existing. Players call that square the king’s air.",
        diagram: "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d1d8"),
    },
    Position {
        variant: "chess",
        key: "chess.stalemate-thrown-away",
        title: "A queen up, and one move from a draw",
        text: "White is a queen up against a bare king and there are two moves here: one wins and one throws the whole game away. Qg6 looks natural — it takes every square from the black king and it is the sort of tidy, shrinking move that has worked all game. It is stalemate. Black is not in check, has no piece but the king, and has no legal move, so the game is drawn on the spot and the queen counts for nothing. Qg7 is mate instead, because the king on f6 defends the queen the moment it lands beside the enemy king. The habit worth building is the one that fires whenever the other side is down to a lone king: before playing a move that takes squares away, count the squares the other king has left, and if the answer is none, make sure you are giving check.",
        diagram: "7k/8/5K2/6Q1/8/8/8/8 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("g5g7"),
    },
    Position {
        variant: "chess",
        key: "chess.knight-fork-on-the-queen",
        title: "The fork nobody sees coming",
        text: "Nd6 is check, and from the same square the knight also attacks the queen on b5. A knight check cannot be blocked and this one cannot be captured — neither the king nor the queen reaches d6 — so Black has to move the king and White takes the queen next move. What makes this worth a page is that the fork was visible a move earlier, from the position of the two black pieces alone: a king on e8 and a queen on b5 are both a knight’s move from d6, and a knight only needs one square to attack two things. Chess players learn to see it as a colour. The king stands on a light square and the queen stands on a light square, and a knight always attacks squares of the opposite colour to the one it stands on — so any knight that can reach a dark square touching both is a fork waiting to happen. When your king and your queen sit on the same colour, look for that square before you do anything else.",
        diagram: "4k3/5ppp/8/1q6/2N5/8/5PPP/6K1 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("c4d6"),
    },
    Position {
        variant: "chess960",
        key: "chess960.castling-goes-somewhere-else",
        title: "Castling, and where it actually puts the king",
        text: "White’s king started on b1 with a rook either side of it, both rooks are still at home, and both castling rights are alive — and exactly one of the two castles is legal. The rule people carry over from chess is that castling moves the king two squares towards a rook. It does not. In every one of the 960 arrangements the king finishes on c1 or g1 and its rook on d1 or f1, whatever squares the two of them started on, and what has to be safe is every square the *king* crosses. Castling kingside from b1 walks the king over c1, d1, e1, f1 and g1 — five squares, none of them anywhere near the rook it is castling with — and the black rook on e8 covers one of them, so it is not a move here at all. Castling queenside walks the king one square, to c1, and lifts the rook from a1 over it onto d1, where it faces the open file. Bragd spells a castle as the king taking its own rook, so it is written b1a1 rather than 0-0-0, and that spelling is doing real work: Kc1 is legal too and puts the king on the same square, without the rook.",
        diagram: "r3r1k1/pp3ppp/8/8/8/8/PP3PPP/RK5R w KQ - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("b1a1"),
    },
    Position {
        variant: "shatranj",
        key: "shatranj.baring-the-king",
        title: "A ferz and an alfil, and no mate anywhere",
        text: "The alfil takes the pawn on b5, and the game is over on the spot. Both halves of that are strange to a chess player. It is over because shatranj has a way to win that chess does not have: leave your opponent with nothing but a king and you have won outright, and that pawn is the last thing Black owns. It does not matter that the black king is standing next to b5 and could take the alfil — a baring is only answered by baring back, and White would still have a ferz, so the reply never gets played. The reason to do it now rather than in a few moves is the piece itself. An alfil leaps two squares diagonally and never looks at the square it leaps over, which sounds like freedom and is the opposite: it leaves the piece with eight squares in the whole world. From d3 this one can only ever stand on b1, f1, h3, b5, f5, d7, h7 — and d3. The pawn happens to be on one of them. Let it step to b4 and no alfil White owns will ever touch it again, and once it promotes Black is never bare, and a ferz and an alfil are not a mating force. Put a queen where the ferz is and a bishop where the alfil is and White mates in a handful of moves. That gap is the measure of these two pieces, and it is why shatranj is won by stripping the board rather than by hunting the king.",
        diagram: "8/8/k7/1p1F4/4K3/3E4/8/8 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d3b5"),
    },
    Position {
        variant: "chaturanga",
        key: "chaturanga.the-elephant-steps-forward",
        title: "The elephant’s forward step",
        text: "Black is a rook up for an elephant and a pawn, and the pawn on c6 is hanging to that rook, so White needs this move to be worth something. It is. The elephant steps straight forward, d6 to d7, and that is the move nobody looks for — because on this board every other elephant is a diagonal piece. Chess’s bishop slides along the diagonals, shatranj’s alfil leaps along them, and neither has ever moved one square straight ahead in its life. The gaja does: one square forward, or one square to any of the four diagonals, and that forward step is the entire difference between the two elephants. From d7 it attacks c8 and e8 at once, which is check and an attack on the rook together, and the pawn on c6 guards d7 so the king cannot simply take it. Black has two legal replies and both of them walk away from the rook. The same rule costs chaturanga its opening: a gaja cannot leap, so on the first move it stands behind its own pawns with nowhere whatsoever to go, which is why this game begins with twelve legal moves where shatranj has sixteen.",
        diagram: "2r1k3/5ppp/2PG4/8/8/8/5PPP/6K1 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d6d7"),
    },
    Position {
        variant: "seirawan",
        key: "seirawan.spending-a-gate-square",
        title: "The gate square you spend by leaving it",
        text: "Castle, and bring the hawk in behind the rook — O-O with the hawk landing on h1, which Bragd spells e1g1H. Then look at what the two sides have spent. Black’s hawk is on b8 and Black’s elephant is on g8: both came in on the squares the knights left, as the knights left them, and Black’s hand is now empty in the good way. White still holds both pieces and has six squares left to put them on — a1, b1, c1, d1, e1 and h1. What White no longer has is f1 and g1, and nothing arrived on either. That is the rule the whole opening turns on, and it is the one people read past: the right belongs to the square and it is spent the moment the piece leaves, gate or no gate. Nf3 and Bc4 are perfectly good moves that each threw a square away for nothing. Castling is the only move in the game that empties two home squares at once, the king’s and the rook’s, and it gives you exactly one of them back, so this is the cheapest gate White will ever be offered. Put the hawk on h1 rather than e1, because the rook is about to land on f1 and the square it wants next is e1. After that, White’s elephant has a1, b1, c1 and d1 to arrive on — every one of them under a piece that has to move first and behind a pawn that has not moved at all. The arithmetic worth carrying away is that a square is worth nothing without a piece in hand and a piece in hand is worth nothing without a square. Black has six live gate squares in this position and nothing whatsoever left to stand on them.",
        diagram: "rhbqkbsr/pppp1ppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
        line: &["e2e4", "e7e5", "g1f3", "b8c6h", "f1c4", "g8f6e"],
        arrangement: None,
        to_move: 0,
        answer: Some("e1g1H"),
    },
    Position {
        variant: "seirawan",
        key: "seirawan.the-hawk-does-not-come-back",
        title: "A bishop for a hawk, which is a bargain",
        text: "Black gated a hawk onto b8 two moves ago and has just brought it to a6, where it attacks the bishop on b5 and nothing at all defends that bishop. So White has been asked to move it. Bxa6 moves it. The only recapture is bxa6 — the rook on a8 is stuck behind its own pawn — so Black is left with doubled pawns on a file that will never open, and White has given a bishop for a piece that moves as a bishop and a knight in one body. Count that the way this game asks rather than the way chess taught you: a minor piece for two is a bargain, and it is a bargain a beginner will hand you, because a new piece on a strange square looks like something to be traded rather than something to be hidden. The part that does not appear in the count at all is that it does not come back. Shogi is the other game in Bragd with pieces held off the board, and there a captured piece turns up in the other player’s hand and returns; here nothing returns a hawk. It is not taken prisoner, the hand it was gated from cannot take it back, and once both home squares and both pieces are spent a player has the pieces on the board and no others for the rest of the game. The one exception is a promoted pawn, which may become a hawk or an elephant here and is eight ranks away. Black still holds the elephant, with seven squares to bring it in on. It is worth keeping it there until it has somewhere to arrive that a bishop cannot reach.",
        diagram: "r1bqkbnr/pppp1ppp/h1n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4",
        line: &["e2e4", "e7e5", "g1f3", "b8c6h", "f1b5", "b8a6"],
        arrangement: None,
        to_move: 0,
        answer: Some("b5a6"),
    },
    Position {
        variant: "capablanca",
        key: "capablanca.the-archbishop-mates-alone",
        title: "The mate no other piece can give",
        text: "Ac6 is checkmate, with the white king nine files away doing nothing whatsoever. One piece is covering the whole corner. An archbishop moves as a bishop or as a knight, and from c6 the bishop half runs c6–b7–a8, which is the check and which also takes b7 on the way, while the knight half reaches a7 and b8. Those four squares are the entire world of a king on a8, there is nothing to interpose, and nothing can get near enough to take on c6. No single piece in chess can do this and none ever could. A lone queen cannot mate a bare king — from b6 it is stalemate and from any square further out the king simply walks — and a lone rook cannot either; every mate in chess needs a second attacker, and the second attacker is nearly always your own king. What makes the archbishop different is that its two halves attack opposite colours. The diagonals cover squares of the colour it is standing on, the knight leaps cover the other colour, and so one piece can blanket a two-by-two block that no chess piece can blanket at all. There are exactly four squares on this whole board where it works, one per corner — c3, h3, c6 and h6, each of them two diagonal steps out from the corner it kills — and the chancellor, the other compound and the one usually reckoned the stronger, cannot do it from anywhere. Learn the picture rather than the square. On this board a corner is not a place a king is safe.",
        diagram: "k9/10/10/4A5/10/10/10/9K w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("e5c6"),
    },
    Position {
        variant: "capablanca",
        key: "capablanca.a-fork-with-two-different-legs",
        title: "The fork that has nothing to do with itself",
        text: "Cd7 is check from the knight half and hits the archbishop on d5 with the rook half, and Black is left with two legal moves in the entire position, both of them king steps. That count is the lesson. A knight’s check cannot be blocked and cannot be answered from a distance, so the only replies to one are to move the king or to take the piece giving it — and the archbishop cannot take on d7, because d7 is two squares straight up the file from d5 and straight lines are the one thing an archbishop does not have. Ke8 and Kg8 are what is left, neither of them defends d5, and the chancellor collects it next move. The reason no chess piece forks like this is that a chancellor’s two legs are of different kinds. A knight forks with two legs of the same kind, so both of its victims have to sit on the same eight-square star around one square, which is why a knight fork is a thing you spot by looking for two pieces close together on the same colour. A queen forks with two legs of the same kind as well — rays — and a check along a ray can usually be blocked, and a queen that has come close enough to fork can often just be taken. The chancellor needs the king to be a knight’s leap away and the other piece merely somewhere on a rank or a file, and those two conditions have nothing to do with each other, which is why the fork can be found from across the board. It is also why the compound is worth a shade more than the rook and the knight it is made of. The habit to build on this board is to check the eight squares a knight’s leap from your own king every move, and not only the diagonals and files a chess player has learned to watch.",
        diagram: "5k4/C4ppp2/10/3a6/10/10/5PPP2/6K3 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("a7d7"),
    },
    Position {
        variant: "kingofthehill",
        key: "kingofthehill.the-hill-is-four-squares",
        title: "A rook down, and the game ends this move",
        text: "Count squares, not pieces. Black is a rook and a knight up against a white king and three pawns, and by every habit a chess player has this is over — White would be looking for somewhere to resign. It is over the other way. A king that reaches d4, e4, d5 or e5 has won that instant, whatever else is on the board and whoever else is winning, and White’s king is on d3, one step from two of them. Kd4 is not available: the knight on c6 covers it. That same knight also covers e5, which is the shape of the mistake — it is guarding half the hill. It does not cover e4, and Ke4 ends the game before Black’s extra rook has done anything at all. The lesson is not the trick, it is the arithmetic. The hill is four squares and they have to be guarded as one thing; a piece watching d4 and e5 has left d5 and e4 open, and against a king already standing next to the centre that is the same as leaving all four open. It is also why the king stops being the piece you hide in this variant. Yours is a runner, and so is theirs.",
        diagram: "r5k1/5ppp/2n5/8/8/3K4/5PPP/8 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d3e4"),
    },
    Position {
        variant: "racingkings",
        key: "racingkings.the-check-you-cannot-play",
        title: "The check that is not a move",
        text: "Both kings are one step from the eighth rank and it is White to move, which in any other game would be the end of the discussion. Not here. Kb8 arrives, Black answers Ke8, and a race both kings finish is a draw — Black is always owed the reply, because a race decided by who moved first would be a race decided by the coin toss that handed out the colours. So White has to arrive *and* stop Black arriving, and the natural way to stop a king is the one thing this variant forbids. Rh7, sweeping the seventh rank and pushing the black king backwards, is not a move that looks good and fails; it is not a move. No move may give check, in either direction, so Bragd will not offer it, and neither will it offer Re1 down the open file. What is legal is Rh8 — the same rook, the far end of the same file, no check at all because the king is on e7 rather than on the rank the rook now owns. From h8 it covers d8, e8, f8 and g8, and Black’s king cannot step onto the eighth rank anywhere. White walks to b8 next move, and the single reply Black is owed cannot answer it. Every piece in this game is a roadblock rather than a weapon, and where the enemy king stands is what decides which squares your own pieces are allowed to occupy.",
        diagram: "8/1K2k3/8/8/8/8/8/7R w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("h1h8"),
    },
    Position {
        variant: "checkers",
        key: "checkers.past-the-line",
        title: "The man who walked past",
        text: "Black pushes the man on e3 down to d2, and the white man on c3 is left standing diagonally behind it with an empty square, e1, beyond. On this same eight-by-eight board a Brazilian or a Russian player would take it without breaking stride, and so would an International player on the larger one, because in those three a man captures in any direction it can reach. In checkers it captures forwards only. The man on c3 can take something on d4 or on b4 and nothing else for as long as it lives, and nothing else of White’s can reach d2 either: a man could only jump it from e1 or c1, and White’s men walk up the board, so those two squares will be empty for the rest of the game. The man on d2 is safe, and it is crowned on the next move it is allowed to make. What to carry away is a counting habit. When you are asking whether a square is safe, count the enemy men in front of it and ignore the ones behind — they are scenery. It is also the one habit on this shelf that does not travel: three of the four games here take it straight back off you.",
        diagram: "w..w..ww.wb..w.wb.....bbbb.bbb.b",
        line: &["10-14", "22-17", "11-16", "17x10:14", "7x14:10", "26-22", "9-13", "30-26", "3-7", "21-17", "14x30:25:17", "24-20", "7-11", "23-19", "30x23:26", "27x18:23", "16x23:19", "31-27"],
        arrangement: None,
        to_move: 1,
        answer: Some("23-26"),
    },
    Position {
        variant: "draughts",
        key: "draughts.must-take-the-most",
        title: "Two men down, and the rule gives them back",
        text: "White is two men down and plays d4–c5, which looks like the third. Black has two ways to take it. The man on d6 jumps it and stops on b4 — one man, quietly, and nothing else happens. The man on b6 jumps it too and then cannot stop: on through d4 over the man on e3 to f2, over the man on g3 to h4, three white men in one turn. Under International rules Black does not get to pick between them. You must take the most, so the three-piece jump is his only legal move on the board, and it is the one that loses, because it delivers his own man to h4 — and from h4 White’s man on i3 sweeps five of them back, h4 and h6 and h8 and f8 and d6, coming to rest on c5. Two men down is level again, a move and a half later. None of this exists in checkers or in Russian draughts, where he would have taken one with the d6 man and shrugged. That is what the obligation is really worth on a big board: it is not a weight you carry, it is a lever on the other player. Before you count what a sacrifice costs you, count his captures, and go looking for the position where his largest one is his worst one.",
        diagram: "wwwwwww.wwwwwww.w..w.....bb.b.b..b.bbbb.bbb.bbbbbb",
        line: &["31-27", "18-23", "32-28", "23x21:27:28", "33-29", "20-24", "29x20:24", "14x25:20", "38-32", "25-30", "34x25:30", "9-14", "25-20", "15x24:20", "43-38", "17-22"],
        arrangement: None,
        to_move: 0,
        answer: Some("32-27"),
    },
    Position {
        variant: "draughts-brazilian",
        key: "draughts-brazilian.where-the-king-stops",
        title: "Three squares to stop on, and two of them lose the king",
        text: "White has to capture — capturing is compulsory in every version of this game — and there is exactly one capture on the board: the king on b8 takes the black man on c7. That is not the decision. Because a Brazilian king flies, it does not land on the square beyond its victim and stop there; it may come to rest anywhere along the rest of the diagonal, which here is three squares, d6 or e5 or f4. Two of them lose it. Stop on d6 and the man on e7 jumps the king for one man. Stop on e5 and it is worse, because the man on f6 takes e5, e3 and g3 in a single turn — three white pieces, one of them the king. f4 is the square nothing is looking at, and after it Black has no capture anywhere. On the identical board, checkers gives you none of this: a king there moves and jumps one square, like a man that has learnt to go backwards, so the landing square is chosen for you and there is nothing to get wrong. The whole difference between the two games is in those three squares behind the man on c7, and the question to ask at a flying capture is never whether to take — you must — but where you are willing to stand afterwards.",
        diagram: "wwwwww.wb.ww..........bbbbbbW.bb",
        line: &["22-18", "9-14", "18x9:14", "5x14:9", "21-17", "14x21:17", "23-19", "1-5", "27-23", "10-15", "19x1:15:6", "2-6"],
        arrangement: None,
        to_move: 0,
        answer: Some("1x19:6"),
    },
    Position {
        variant: "draughts-russian",
        key: "draughts-russian.you-choose-which-capture",
        title: "Three captures, and the biggest is not the one",
        text: "Black has to take, which is the same in all four of these games, and he has three ways to do it. a3xc5 takes the white man on b4. a7xc5 takes the white man on b6 and lands on the very same square. Or c7xc3 takes b6 and b4 together in one chain, which is two men rather than one and is the move a Brazilian or an International player would be obliged to play, because those two rulesets make you take the most. It loses. The black man that arrives on c3 is jumped by White’s man on b2, which carries on through e5 and g5 for three. So does a3xc5, and the reason is worth sitting with: it puts a black man on exactly the same square that a7xc5 does, but it leaves the white man on b6 alive, and b6 is the man that jumps c5 and then runs on through e5 and g5. The same square, a different man, three pieces of difference. Only a7xc5 leaves White with no capture at all and Black still two men up. This is what Russian’s free choice is for. You are obliged to take and never obliged to take the most, so the question at every capture is not how many you can get but what your own men are standing on when the turn ends.",
        diagram: "wwwww.wwb.www.....bbwb.bbb.bbbbb",
        line: &["22-17", "11-16", "17-13", "7-11", "26-22", "11-15", "21-17", "9-14", "13-9", "14x21:17", "22-17"],
        arrangement: None,
        to_move: 1,
        answer: Some("5x14:9"),
    },
    Position {
        variant: "hnefatafl",
        key: "hnefatafl.shieldwall",
        title: "The shieldwall",
        text: "The defenders have laid their last men flat along the bottom rank with the king in the middle of them, and it looks like the safest thing on the board: a king standing on an edge cannot be captured at all under these rules, and no ordinary sandwich can reach d1 or f1 while he stands between them. The attackers play g5–g1 and the row comes off anyway. This is the shieldwall, which Copenhagen added to the older rules and which is the only capture in the game that is not a sandwich — a row of enemies lying along an edge, every one of them with an attacker directly inward, is taken whole the moment a move closes the second end of it, and c1 had already closed the first. The king is the exception; a shieldwall never removes him, so what is left is a king alone on the bottom rank with nobody to escort him anywhere. What is worth taking away is that the defenders’ formation was the mistake rather than the attackers’ move. A flat line of defenders against an edge, with attackers already pressing it from inside, is not a fort. It is the exact shape the shieldwall was invented to punish.",
        diagram: ".........../.......X.../.........../........X../.X........./..........X/......XO.../.......X.../........X../...XXX...../..XOKO.....",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("g5g1"),
    },
    Position {
        variant: "hnefatafl",
        key: "hnefatafl.caught-beside-the-throne",
        title: "Three men and an empty throne",
        text: "The king has stepped off his throne and is standing beside it, and the attackers can finish the game in one. Count his sides. West is an attacker on d6, north is an attacker on e7, and east is the throne he has just left — which turns hostile the moment he leaves it and does the work of a fourth man. Only the south is open, so e1–e5 closes the fourth side and the king is taken. What earns this a page is the move most people play instead. k4–d4 takes the defender on c4 against the attacker on b4: a free man, and exactly the sort of move that has been right all game. It also empties the k-file. The king answers e6–k6, straight across his own throne — a soldier may not stand on that square and the king may — and from k6 he is one move from k11 and one move from k1, and the attackers can block one of them. That is the arithmetic this game asks a chess player to relearn. The defenders are not trying to take anything, so a man they lose costs them very little, and a line you open costs you everything.",
        diagram: ".........../.........../.........../.........../....X..O.../...XK....../.........../.XO.......X/.........../.......X.../....X......",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("e1e5"),
    },
    Position {
        variant: "hnefatafl-fetlar",
        key: "hnefatafl-fetlar.no-shieldwall-in-fetlar",
        title: "The same wall, and nothing takes it",
        text: "This is the board from “The shieldwall” on the Copenhagen side, character for character, and here g5–g1 takes nothing whatever: Fetlar has no shieldwall, so a row of defenders lying along an edge is three pieces standing on three squares and nothing more. It is still the move. Count what the defenders can do after it. The king is walled in by his own men, d1 has an attacker above it and the board’s edge below it, and f1 has just lost the last square it had to run to — so the only piece on the board with a legal move is the man on h5. A side that cannot move loses in tafl; it is not stalemate and it is not a draw. So the attackers have stopped playing for the king altogether and are hunting one soldier, and the game ends when they catch him. One thing to watch while they do it: if that soldier ever reaches d3, e3 or f3 he takes the attacker below him against the defender on the bottom rank, and the box springs open. None of this exists in Copenhagen, where an attacker arriving on g1 has no say in the matter — the wall comes off, and the box can never be closed with anybody still inside it.",
        diagram: ".........../.......X.../.........../........X../.X........./..........X/......XO.../.......X.../........X../...XXX...../..XOKO.....",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("g5g1"),
    },
    Position {
        variant: "hnefatafl-fetlar",
        key: "hnefatafl-fetlar.fort-that-wins-nothing",
        title: "The fort that wins nothing",
        text: "The king is on d1 with one square of his own to step into and a ring of his own men round it, and there is nothing the attackers can do to it. c1 and e1 cannot be sandwiched, because the king himself is standing between them and the board’s edge is behind them; c2 and e2 have a defender above and a defender below; d3 would have to be pressed from d2, and nothing can get to d2. Under Copenhagen this game is already over — that shape is an exit fort, and the defenders win the moment the attackers touch anything at all. Fetlar has no exit fort, so the position is exactly what it looks like and no more: a wall that cannot be broken, round a king who cannot get out, in a ruleset that also has no ban on the defenders repeating a position. Neither side can make progress, and the game runs to the rules’ move limit and is drawn. It is worth playing this one out against yourself, because it teaches a thing you can only learn by failing to do it. The attackers’ mistake is several moves behind them, from when the wall was still one man short, and no move on the board now recovers it.",
        diagram: ".........../.........../.........../.........../.........../...X......./.X.X.X..X../..X.X....../..OOO....../..O.O....../..OKO......",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: None,
    },
    Position {
        variant: "brandubh",
        key: "brandubh.captured-against-the-corner",
        title: "Captured against the corner",
        text: "The defender on b1 is standing next to a corner, which on a board seven squares wide is where defenders keep ending up, and it is the commonest way a beginner loses a man in brandubh for nothing at all. The attackers play f1–c1 and take him. Nothing is standing on a1: the corner does the work of the second attacker, exactly as though a piece were on it. The rule is easy to read past on the rules page and hard to see on a board, because it inverts what a chessboard has taught — a corner here is not a quiet square, it is the most dangerous square on the board to stand beside, and it is dangerous to both sides equally. An attacker who parks on b1 is taken by a single defender arriving on c1 just as surely, and on a board this small there is always a piece three squares away that can arrive. The other half of it is that the man on b1 was not doing anything worth a man. He blocks nothing the attackers wanted, and he blocks the king’s own road along the first rank to a1.",
        diagram: "...X.../..X..../...OO../X..K..X/......./...X.../.O...X.",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("f1c1"),
    },
    Position {
        variant: "tablut",
        key: "tablut.lines-before-material",
        title: "The line, not the man",
        text: "There is one attacker move here that does not lose, and it takes nothing. The king on c3 has a clear file underneath him, and if he reaches c1 he is standing on an empty first rank with a1 at one end of it and i1 at the other — the attackers can block one of those and they cannot block both. So the move is h2–c2. It wins no material, it reads as a wasted tempo, and it is the only thing on the board that holds. There are exactly two captures available and both of them take the same man, the last defender on g6, against the attacker on f6; one of the two is played by h2 itself, which is the only piece that could have stood on c2. After either of them the king plays c3–c1 and the game is decided. This is the habit tafl asks a chess player to build, and it is the whole of the attackers’ side of the game: they have twice the material and no king of their own to lose, so material is not what they are playing for. They are playing to keep every line that runs corner to corner blocked, and a man won is worth nothing against a rank left open.",
        diagram: "........./.......X./..X....../.....XO../X.......X/........./..K....../.......X./.........",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("h2c2"),
    },
    Position {
        variant: "xiangqi",
        key: "xiangqi.cannon-needs-a-screen",
        title: "The cannon that cannot see",
        text: "The red cannon on e1 looks straight up an open file at the black general, and it is not check. It is not even a threat. A cannon travels like a chariot and takes like nothing else in any game: to capture it must leap exactly one piece — the screen — and take the first thing beyond it, so on an empty file it can slide the whole way to e9 and stand beside the general without touching a hair on his head. What wins here is Ne8, which puts a red horse in the middle of the file. The cannon now has its screen and the general is in check with nowhere to go: d10 and f10 are both covered by that same horse, which steps through e9 to reach them, e9 is still on the cannon’s line, and the horse is two points away and cannot be taken. Chess spends years teaching you that blocking a line kills an attack. Here it starts one, and unlearning that is the single hardest thing about the game.",
        diagram: "4k4/9/b8/9/5N3/9/9/1p4p2/9/3KC4 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("f6e8"),
    },
    Position {
        variant: "xiangqi",
        key: "xiangqi.generals-may-not-face",
        title: "The square the general may not stand on",
        text: "Red pushes the soldier to e9 and it is mate, which nobody counting flight squares the way a chess player counts them will believe. The soldier gives check and is defended by the chariot behind it on e5, so the general may not take it. f10 is his own chariot. And d10 — empty, unattacked, marked by nothing, the square every chess player awards him — is not a square he may occupy at all: the red general has been sitting on d1 since the opening and the d-file between them is clear, and two generals may never face each other down a file with nothing in between. That is a rule about positions rather than about moves, which is why nothing on the board shows it. It is worth testing every time you count, because it is a fact about a piece that has not moved and is nowhere near the fighting: put anything at all on the d-file and the general steps to d10 and lives.",
        diagram: "4kr3/9/4P4/9/9/4R4/9/9/9/3K5 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("e8e9"),
    },
    Position {
        variant: "janggi",
        key: "janggi.cannon-with-nothing-to-leap",
        title: "A cannon with nothing to leap",
        text: "Cho’s cannon on e5 has an empty rank and an empty file in front of it and not one legal move. This is the largest single difference between janggi and xiangqi and it catches everybody who arrives from the other game: a xiangqi cannon slides freely and leaps only in order to capture, and the same cannon on the same point under those rules has sixteen moves. A janggi cannon needs a screen to move at all. It also may not leap a cannon of either colour, and may not capture one either, which is why cannons on this board are very nearly harmless to one another and why none of xiangqi’s opening cannon trades exists here. The move is to step the soldier sideways to e6 — janggi soldiers go sideways from the opening array, there being no river to cross and nothing to earn it by — and the cannon that had nothing now leaps it and looks all the way through to Han’s general on e10. Han has exactly two answers, and after either of them the cannon that could not move owns the file.",
        diagram: "3ak4/9/9/p8/3P5/4C4/9/9/3K5/9 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d6e6"),
    },
    Position {
        variant: "janggi",
        key: "janggi.bikjang",
        title: "The free horse that draws the game",
        text: "Cho is two chariots up against nothing and there is a horse hanging on f6, right beside his soldier. Taking it with the soldier is legal, it is free, and it draws the game on the spot. The soldier on e6 is the only thing standing between the two generals; step it aside and they are facing each other down an open e-file, which is 빅장 — bikjang — and janggi ends the game there and then with half a point each. A xiangqi player has never had to think about this, because xiangqi forbids the position outright: under those rules this same soldier has exactly one move, straight forward, and both sideways steps are not moves at all. Janggi allows the same geometry and scores it. So take the horse with the chariot on f1 instead. It wins the identical piece, it leaves the file blocked, and it is the difference between a won game and a shrug.",
        diagram: "2ba5/4k4/9/9/4Pn3/9/9/9/4K4/R2A1R3 w - - 0 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("f1f6"),
    },
    Position {
        variant: "shogi",
        key: "shogi.pawn-drop-that-may-not-mate",
        title: "The drop you may not make",
        text: "Black — who moves first in shogi — holds a gold and a pawn, and every piece he has taken this game can be put back on the board as a move. The white king on 1a is already boxed: his own knight sits on 2a, and Black’s knights on 1d and 2d cover 1b and 2b between them. Dropping either piece on 1b is mate, and only one of the two is legal. **A pawn may not be dropped to give checkmate.** Uchifuzume is the strangest rule in the game and the only one that requires looking a move ahead, so P*1b is simply not in the list of moves — while P*1c and every other pawn drop down that same file is, and so is P*2b right beside it. You may drop a pawn to give check. You may push a pawn to give mate. You may not drop one to give mate. So the move is G*1b, defended by the knight on 2d and covering 2a and 2b as well as 1a. Learn the rule before you need it: it arrives exactly when you are counting on a pawn to finish, and nothing on the board will warn you.",
        diagram: "l6nk/9/2p1p4/7NN/9/9/2P1P4/9/4K4 b GP 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("G*1b"),
    },
    Position {
        variant: "shogi",
        key: "shogi.knight-out-of-nowhere",
        title: "A knight out of nowhere",
        text: "Not one piece on this board attacks another, and Black wins a rook. The knight in hand may be dropped on any empty square, and a shogi knight is not chess’s: it has exactly two destinations, both of them two ranks forward and one file to the side, it jumps whatever stands in the way, and it can never come back. Put those two facts together and the fork writes itself — N*4c strikes 5a and 3a at once, the king and the rook, from a square nothing was looking at. A knight check cannot be blocked and this knight cannot be captured, so White must move his king, and then the knight takes the rook and promotes, because a knight on the last rank would have nowhere left to go and shogi will not let you strand one. Two of his four retreats leave him standing next to 3a — 4a and 4b — and from either one he takes the promoted knight back afterwards; from 4a he is answering a check while he does it, because a promoted knight moves as a gold and a gold on 3a attacks 4a and not 4b. Rook for knight is a trade to make every time. The habit to build is looking at the two squares your held knight would hit, before you look at anything on the board.",
        diagram: "4k1r1l/3g5/pppp2ppp/9/9/9/PPPP1PPPP/9/4K4 b N 1",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("N*4c"),
    },
    Position {
        variant: "go-19",
        key: "go-19.the-ladder",
        title: "The ladder",
        text: "The white stone on d4 has two liberties and Black is to play. Only one of the two ataris works, and which one it is decides the whole corner. Play d5 and White must run to e4 — where the black stone already sitting on e3 leaves him two liberties again, so the same thing happens, and happens, and goes on happening. Each black move takes one liberty, each white move is forced, and the two of them walk together up the board in a staircase until White reaches the top right corner with nowhere left to go: thirty white stones, all captured at once. Play the atari on the other side, e4, and White runs the other way into open board with three liberties and is gone. The habit worth building is not the sequence — it is counting it before you start. A ladder is the one tactic in Go you can read all the way to the end without guessing, because every single move in it is forced.",
        diagram: ".................../.................../.................../.................../.................../.................../.................../.................../.................../.................../.................../.................../.................../.................../.................../..XO.............../...XX............../.................../................... b",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d5"),
    },
    Position {
        variant: "go-19",
        key: "go-19.the-ladder-that-does-not-work",
        title: "The ladder that does not work",
        text: "The corner is the same corner and the atari at d5 is the same atari, and it loses the game. Follow the staircase with a finger before you play: e4, e5, f5, f6, and on up to the right, and on the eleventh step it arrives at k9 — touching the white stone that has been standing on k10, the middle of the board, since before any of this started. White connects, the running group jumps from one liberty to three, and it is out. What Black is left with is worse than nothing: a wall of stones facing the wrong way, every one of them played to chase something that got away, and a white group cutting straight through the middle of them. A stone fifteen moves away and on the other side of the board decides whether this works, which is why Go players read a ladder to the very end before they begin it, and why the first thing you look for when you see one is what is standing in its path.",
        diagram: ".................../.................../.................../.................../.................../.................../.................../.................../.................../.........O........./.................../.................../.................../.................../.................../..XO.............../...XX............../.................../................... b",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: None,
    },
    Position {
        variant: "go-13",
        key: "go-13.the-cut-that-hits-both",
        title: "The cut that hits both",
        text: "White has jumped from d4 to d6 and Black is pressing on both stones. Each of them has exactly two liberties and — this is the thing to see — they share one of them, the empty point between them. The move that ataris one stone is the wrong move: White answers on d5, the two stones become one group of three with room to breathe, and the whole attack has finished by making White stronger. The move that takes d5 first ataris both at once. They are two separate strings with two separate last liberties, so White can save whichever one he likes and Black takes the other. The general shape of it is worth carrying away from this board: when a player leaves a gap, the point in the gap is the point that joins them, and the point that joins them is the point that cuts them. Take it before he does.",
        diagram: "............./............./............./............./............./............./...X........./..XO........./............./..XO........./...X........./............./............. b",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d5"),
    },
    Position {
        variant: "go-9",
        key: "go-9.the-vital-point",
        title: "The vital point",
        text: "Black is shut in along the bottom edge with three empty points — c1, d1 and e1 — and nothing else. Three points is exactly the size where it matters who plays first, and the whole question is whether Black can turn one space into two eyes. Play d1, in the middle, and he can: c1 and e1 become two separate one-point eyes, White cannot fill either of them without killing his own stone, and the group is alive for the rest of the game with nothing more to do about it. Play anywhere else on the board and White plays d1 himself, and what is left is one eye split in two halves that Black can never make into a pair. The group is dead where it stands. This is the first shape to learn by heart: a straight three lives if you take the middle and dies if he does, and there is no third answer.",
        diagram: "........./........./........./........./........./........./OOOOOOO../OXXXXXO../OX...XO.. b",
        line: &[],
        arrangement: None,
        to_move: 0,
        answer: Some("d1"),
    },
    Position {
        variant: "backgammon",
        key: "backgammon.the-two-dice-are-one-move",
        title: "The two dice are one move",
        text: "White has rolled 6-1. The six plays itself — 13/7, down onto the bar point — and then comes the question that decides the roll, which is what to do with the one. The honest answer is that there is nothing to do with it anywhere else on the board worth as much as covering what the six has just left behind. A lone checker on the seven point is a target. A pair of them is the best point in the game, standing directly in the path of the checker Black still has to bring all the way round from the far corner. There is a hit on offer as well, right down in the home board, and it is the trap: it sends a checker back a single pip, and it leaves one of your own sitting alone on the point you want least, for the sake of it. Look at the two numbers together before you touch anything. A roll is one move, not two.",
        diagram: "B1,-,-,-,W2,W4,-,W2,-,-,-,B6,W5,-,-,-,B3,-,B5,-,-,-,-,W2 bar:0/0 off:0/0 cube:1- w",
        line: &["3-1", "8/5 6/5", "6-5", "24/18 18/13", "6-1"],
        arrangement: Some(0),
        to_move: 0,
        answer: Some("13/7 8/7"),
    },
    Position {
        variant: "backgammon",
        key: "backgammon.the-anchor-is-the-take",
        title: "The anchor is the take",
        text: "Nothing is loose, nobody is on the bar, and there is nothing anywhere to hit. White is on roll and leads the race by twenty-two pips — a hundred and nine against a hundred and thirty-one — and the cube is still sitting in the middle. This is what a double is for, and this is the moment: turn it before the position turns into one he can simply refuse. The part worth learning is what happens next, because the natural reaction to being twenty-two pips down is to hand it straight back, and that would be the mistake. Look at where the two black checkers are standing. They are on the five point, in the middle of the board White still has ten checkers to bring home into and then bear off from, and they are under no obligation to move at all. Sooner or later White runs out of safe numbers, has to leave one, and they are already there waiting for it. A take is not a claim to be winning. It is a claim to win about one game in four, and an anchor on that point is worth a good deal more than that.",
        diagram: "-,-,-,-,B2,W5,W5,W3,-,W2,-,B3,-,-,-,-,-,-,B6,-,B4,-,-,- bar:0/0 off:0/0 cube:1- w",
        line: &["6-5", "24/18 18/13", "4-4", "24/20 24/20 13/9 13/9", "6-5", "24/18 18/13", "5-4", "9/4 8/4", "6-6", "13/7 13/7 13/7 13/7", "3-2", "9/6 8/6", "3-3", "13/10 13/10 13/10 10/7", "4-2", "8/4 6/4"],
        arrangement: Some(0),
        to_move: 0,
        answer: None,
    },
    Position {
        variant: "ur",
        key: "ur.the-safe-square-is-worth-sitting-on",
        title: "The safe square is worth sitting on",
        text: "Light has a disc on B4, the rosette in the middle of the shared row, and Dark has one on B3, a single square behind it. The throw is a one, and there are exactly three things Light may do with it: enter a new disc on A4, walk the disc on A3 up to A2, or advance the disc on B4 to B5. Almost everybody plays the third, because that disc has come furthest and moving it feels like getting on with the race. It is also the only one of the three that can lose anything. B4 is the one square on this board where a disc cannot be taken, and an enemy disc may not even land on it — which is precisely why Dark has had to stop a square short. B5 is an ordinary square, two steps from that waiting disc on B3, and a two is the commonest throw there is: six times in sixteen. Losing it there does not cost a step, it costs all nine, because a disc that is taken goes back to your hand and starts the fourteen again. The disc on B4 is not stuck. It is parked, on the only square in the game worth parking on, and everything else you own is what you move while it sits there — so walk A3 up to A2, one throw away from your own rosette on A1, where Dark can never go at all.",
        diagram: "..W.....BW.......... 56 00 w",
        line: &["4", "0-4", "4", "4-8", "2", "0-2", "4", "0-4", "3", "4-7", "1"],
        arrangement: Some(0),
        to_move: 0,
        answer: Some("2-3"),
    },
    Position {
        variant: "senet",
        key: "senet.the-water-is-not-progress",
        title: "The water is not progress",
        text: "Evens has a counter on house 26, the House of Beauty, and six more still strung along houses 2 to 12. The throw is a one, and seven moves are legal — six of them down at the start, and one that moves the counter on 26. That one goes to 27, the House of Water, and it is the move almost everybody plays, because the counter on 26 is the one that has come furthest and because it really is in the way: nothing may pass the House of Beauty, and your own counter blocks your own army exactly as an enemy would. So the urge to get it moving is a real urge. It is simply the wrong throw to obey it on. Look at what the other four throws would have done with that counter: a two puts it on 28, a three on 29, a four on 30, and a five takes it straight off the board. Every throw but this one is a good throw for that counter. And the one is the throw that hands you another one — a one, a four and a five all earn a second throw after you move — unless the counter you moved landed in the water, which ends your turn where it stands, whatever you threw. Then it gets worse. While you have a counter in the water, nothing else of yours may move at all: six counters on the board and not one of them legal. You throw, and unless it is a four, nothing happens and the turn is over. The only ways out are that exact four, which bears the drowned counter off, and house 15, the House of Rebirth, which rescues it — and 15 is where Odds has just put a counter. So play the one anywhere else, and play it where it earns most: 10-11 lands on Odds’ lone counter there and exchanges the two, sending it back to 10 while yours goes forward to 11, where your own counter on 12 protects it. Then throw again, and hope for the five.",
        diagram: "BWBWBWBWBWBW..B..........W.... 00 w",
        line: &["1", "14-15", "4", "15-19", "4", "19-23", "3", "23-26", "2", "13-15", "1"],
        arrangement: None,
        to_move: 0,
        answer: Some("10-11"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_position_names_a_variant_and_has_a_unique_key() {
        let mut keys: Vec<&str> = POSITIONS.iter().map(|p| p.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), POSITIONS.len());
        for position in POSITIONS {
            assert!(crate::variant(position.variant).is_ok(), "{}", position.key);
            assert!(position.key.starts_with(position.variant), "{}", position.key);
        }
    }

    #[test]
    fn every_position_replays_and_its_answer_is_legal() {
        for position in POSITIONS {
            let (initial, line) = position.start();
            let state = crate::replay(position.variant, initial, line)
                .unwrap_or_else(|e| panic!("{}: {e}", position.key));
            assert_eq!(
                state.to_move.map(|seat| seat.index),
                Some(position.to_move),
                "{}",
                position.key
            );
            if let Some(answer) = position.answer {
                crate::apply(position.variant, initial, line, answer)
                    .unwrap_or_else(|e| panic!("{}: {e}", position.key));
            }
        }
    }
}
