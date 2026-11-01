//! Draughts in PDN: a PDN file's games go through
//! the chess table (`chess::Table`), each game played out here on its board. International
//! draughts (10×10, `GameType 20`, the default) and English draughts or checkers (8×8,
//! `GameType 21`) by their rules: men capture backwards in international draughts, kings fly
//! there and move one square in English draughts, a capture of several pieces goes on from
//! square to square. Reading only: nothing is played or checked beyond what showing it needs.
//!
//! Any text at all is read without a panic and without recursion. A capture is looked for
//! along at most `MAX_SEARCH` steps, and all the captures of one game together along at most
//! `MAX_GAME_SEARCH`: past that, a capture is not played (its line stops), however the game
//! was made up. The board and its search live in `rules_draughts::record`; this file keeps
//! PDN: the tags, the movetext and the square numbers.

use crate::chess::pgn::{self, Game, Rules, Word};
pub use rules_draughts::record::{
    BK, BM, Board, Captures, ENGLISH, INTERNATIONAL, MAX_GAME_SEARCH, MAX_SEARCH, MAX_SIZE, Variant, WK, WM,
};

/// The variant of a `GameType` tag: `20`, or in full `20,W,10,10,N2,0` (type, who starts,
/// width, height, notation, inverted).
fn by_type(tag: &str) -> Option<Variant> {
    let f: Vec<&str> = tag.split(',').map(str::trim).collect();
    let kind: u8 = f.first()?.parse().ok()?;
    let mut v = match kind {
        20 => INTERNATIONAL,
        21 => ENGLISH,
        // Pool, Russian, Brazilian: flying kings, men capturing backwards, on 8×8.
        23 | 25 | 26 => Variant { kind, width: 8, height: 8, black_first: kind == 23, ..INTERNATIONAL },
        27 => Variant { kind, width: 12, height: 12, ..INTERNATIONAL },
        // Italian, Spanish, Portuguese, Czech and the rest: 8×8, men forward only.
        _ => Variant { kind, black_first: false, ..ENGLISH },
    };
    if let Some(c) = f.get(1) {
        match c.to_ascii_uppercase().as_str() {
            "W" => v.black_first = false,
            "B" => v.black_first = true,
            _ => {}
        }
    }
    let size = |i: usize| f.get(i).and_then(|s| s.parse::<u8>().ok()).filter(|n| (4..=MAX_SIZE).contains(n) && n % 2 == 0);
    if let (Some(w), Some(h)) = (size(2), size(3)) {
        (v.width, v.height) = (w, h);
    }
    Some(v)
}

/// The variant of a game: its `GameType` tag; without one, English draughts when the first
/// move starts at a square up to 20 (Black's men in English draughts) or a FEN tag has no
/// square past 32, else international draughts, PDN's default.
pub fn variant_of(tags: &[(String, String)], first_move: Option<&str>) -> Variant {
    let tag = |k: &str| tags.iter().find(|(n, _)| n.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str());
    if let Some(v) = tag("GameType").and_then(by_type) {
        return v;
    }
    if let Some(fen) = tag("FEN") {
        let big = fen.split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse::<u32>().ok()).any(|n| n > 32);
        return if big { INTERNATIONAL } else { ENGLISH };
    }
    let from = first_move.and_then(|m| m.split(['-', 'x', 'X', ':']).next()).and_then(|s| s.parse::<u32>().ok());
    if from.is_some_and(|n| n <= 20) { ENGLISH } else { INTERNATIONAL }
}

/// The variant's name, as the board's heading says it.
pub fn variant_name(v: &Variant) -> String {
    let size = format!("{}×{}", v.width, v.height);
    match v.kind {
        20 if v.width == 10 => format!("International draughts, {size}"),
        21 if v.width == 8 => format!("English draughts (checkers), {size}"),
        n => format!("Draughts, GameType {n}, {size}"),
    }
}

/// Is it a result? PDN's (`2-0`, `0-2`, `1-1` in international draughts) or PGN's.
fn is_result(w: &str) -> bool {
    pgn::is_result(w) || matches!(w, "2-0" | "0-2" | "1-1" | "0-0")
}

/// A move of the movetext (`32-28`, `28x19x10`, `28x10`, `c3-d4`) played: None when it cannot be.
fn play_written(b: &Board, mv: &str, search: &Captures) -> Option<(Board, usize, usize)> {
    let capture = mv.contains(['x', 'X', ':']);
    let squares: Vec<usize> = mv.split(['-', 'x', 'X', ':']).map(|s| b.v.parse_square(s)).collect::<Option<_>>()?;
    b.play(&squares, capture, search)
}

/// A word of PDN movetext: a move (`32-28`, `19x30`, `28x19x10`, `c3-d4`), a result, or
/// something else; a move number glued to the front (`1.32-28`) is taken off.
fn word(w: &str) -> Word<'_> {
    if is_result(w) {
        return Word::Result(w);
    }
    let mut w = w;
    let digits = w.find(|c: char| !c.is_ascii_digit()).unwrap_or(w.len());
    if digits > 0 && w[digits..].starts_with(['.', '…']) {
        w = w[digits..].trim_start_matches(['.', '…']);
    }
    let body = w.trim_end_matches(['!', '?', '*', '+']);
    let nag = pgn::suffix_nag(w[body.len()..].trim_end_matches(['*', '+']));
    let parts: Vec<&str> = body.split(['-', 'x', 'X', ':']).collect();
    let square = |s: &str| {
        let b = s.as_bytes();
        match b.first() {
            Some(c) if c.is_ascii_digit() => b.len() <= 3 && b.iter().all(u8::is_ascii_digit),
            Some(c) if c.is_ascii_alphabetic() => (2..=3).contains(&b.len()) && b[1..].iter().all(u8::is_ascii_digit),
            _ => false,
        }
    };
    if parts.len() >= 2 && parts.iter().all(|p| square(p)) { Word::Move(body, nag) } else { Word::Other }
}

/// The first move of a game's movetext, outside variations and comments.
fn first_move(moves: &str) -> Option<&str> {
    let mut depth = 0u32;
    for tok in crate::chess::lex::Movetext::new(moves, false) {
        match tok {
            crate::chess::lex::Tok::Open => depth += 1,
            crate::chess::lex::Tok::Close => depth = depth.saturating_sub(1),
            crate::chess::lex::Tok::Word(w) if depth == 0 => {
                if let Word::Move(m, _) = word(w) {
                    return Some(m);
                }
            }
            _ => {}
        }
    }
    None
}

/// Draughts, as `pgn::parse_with` plays it out: one per game, with its search.
pub(crate) struct Draughts {
    search: Captures,
}

impl Draughts {
    fn new() -> Draughts {
        Draughts { search: Captures::new() }
    }
}

impl Rules for Draughts {
    type Board = Board;

    fn start(&self, tags: &[(String, String)], moves: &str) -> (Board, String) {
        let v = variant_of(tags, first_move(moves));
        let fen = tags.iter().find(|(k, _)| k.eq_ignore_ascii_case("FEN")).map(|(_, f)| f.as_str());
        let b = fen.and_then(|f| Board::from_fen(v, f)).unwrap_or_else(|| Board::start(v));
        let fen = b.fen();
        (b, fen)
    }

    fn word<'a>(&self, w: &'a str) -> Word<'a> {
        word(w)
    }

    fn turn(&self, b: &Board) -> (bool, u32) {
        (b.white != b.v.black_first, b.number)
    }

    fn play(&self, b: &Board, mv: &str) -> Option<(Board, String, Option<u8>, Option<u8>)> {
        let (nb, from, to) = play_written(b, mv, &self.search)?;
        let fen = nb.fen();
        Some((nb, fen, Some(from as u8), Some(to as u8)))
    }
}

/// A PDN game's text, read and played out. `Node::white` is the first player's move (Black's
/// in English draughts); `fen` is `Board::fen`, `from` and `to` cells (`x + y × width`).
pub fn parse(text: &str) -> Game {
    pgn::parse_with(text, &Draughts::new())
}

/// The variant of a game read by `parse`.
pub fn variant(g: &Game) -> Variant {
    variant_of(&g.tags, g.roots.first().map(|&r| g.nodes[r as usize].san.as_str()))
}

/// The word `w` of movetext as the table counts it: a move or not.
pub(crate) fn is_move(w: &str) -> bool {
    matches!(word(w), Word::Move(..))
}

/// A board drawn from `Board::fen`: rows from the top, each cell `None` or its piece letter.
pub fn cells(fen: &str) -> Vec<Vec<Option<char>>> {
    let placement = fen.split_whitespace().next().unwrap_or("");
    placement
        .split('/')
        .take(MAX_SIZE as usize)
        .map(|row| {
            let mut out = vec![];
            let mut n = 0usize;
            for c in row.chars() {
                if let Some(d) = c.to_digit(10) {
                    n = n * 10 + d as usize;
                    continue;
                }
                out.extend(std::iter::repeat_n(None, n.min(MAX_SIZE as usize)));
                n = 0;
                out.push(Some(c));
            }
            out.extend(std::iter::repeat_n(None, n.min(MAX_SIZE as usize)));
            out.truncate(MAX_SIZE as usize);
            out
        })
        .collect()
}

/// The number of the dark square at `x`, `y` of a board `width` wide (rows from the top).
pub fn square_number(width: usize, x: usize, y: usize) -> Option<usize> {
    ((x + y) % 2 == 1).then(|| y * (width / 2) + x / 2 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn play(fen_tag: Option<&str>, game_type: &str, moves: &str) -> Game {
        let mut text = format!("[GameType \"{game_type}\"]\n");
        if let Some(f) = fen_tag {
            text.push_str(&format!("[FEN \"{f}\"]\n"));
        }
        text.push('\n');
        text.push_str(moves);
        parse(&text)
    }

    fn last(g: &Game) -> String {
        let main = g.main_line();
        main.last().map_or(g.start.clone(), |&n| g.nodes[n as usize].fen.clone().unwrap_or_default())
    }

    #[test]
    fn squares_are_numbered() {
        let v = INTERNATIONAL;
        assert_eq!(v.cell(1), Some(1));
        assert_eq!(v.cell(5), Some(9));
        assert_eq!(v.cell(6), Some(10));
        assert_eq!(v.cell(46), Some(90));
        assert_eq!(v.cell(50), Some(98));
        assert_eq!((v.cell(0), v.cell(51)), (None, None));
        assert_eq!(ENGLISH.cell(29), Some(56));
        assert_eq!(ENGLISH.parse_square("a1"), Some(56));
        assert_eq!(ENGLISH.parse_square("b1"), None, "a light square");
        for n in 1..=50 {
            let c = v.cell(n).unwrap();
            assert_eq!(square_number(10, c % 10, c / 10), Some(n as usize));
        }
        let b = Board::start(INTERNATIONAL);
        assert_eq!(b.fen(), "1m1m1m1m1m/m1m1m1m1m1/1m1m1m1m1m/m1m1m1m1m1/10/10/1M1M1M1M1M/M1M1M1M1M1/1M1M1M1M1M/M1M1M1M1M1 w");
        assert_eq!(Board::start(ENGLISH).fen().split(' ').nth(1), Some("b"));
        assert_eq!(cells(&b.fen())[0], [None, Some('m'), None, Some('m'), None, Some('m'), None, Some('m'), None, Some('m')]);
    }

    #[test]
    fn variants_by_tag_and_moves() {
        let t = |k: &str, v: &str| vec![(k.to_string(), v.to_string())];
        assert_eq!(variant_of(&t("GameType", "21"), None), ENGLISH);
        assert_eq!(variant_of(&t("GameType", "20,W,10,10,N2,0"), None), INTERNATIONAL);
        let v = variant_of(&t("GameType", "25,W,8,8,A0,0"), None);
        assert_eq!((v.width, v.flying, v.men_back), (8, true, true));
        // A size past the largest, or odd, is not taken.
        assert_eq!(variant_of(&t("GameType", "20,W,99,99"), None).width, 10);
        assert_eq!(variant_of(&[], Some("11-15")), ENGLISH);
        assert_eq!(variant_of(&[], Some("32-28")), INTERNATIONAL);
        assert_eq!(variant_of(&[], None), INTERNATIONAL);
        assert_eq!(variant_of(&t("FEN", "W:W21,22:B1,2"), None), ENGLISH);
    }

    #[test]
    fn an_international_game() {
        let g = parse("[White \"A\"]\n[Black \"B\"]\n[Result \"2-0\"]\n\n1. 32-28 19-23 2. 28x19 14x23 {back} 3. 37-32 (3. 33-28 23x32 4. 38x27) 3... 10-14 2-0\n");
        assert_eq!(g.error, None, "{:?}", g.nodes);
        assert_eq!(g.result, "2-0");
        let main = g.main_line();
        assert_eq!(main.len(), 6);
        assert!(g.nodes[main[0] as usize].white);
        assert!(!g.nodes[main[1] as usize].white);
        assert_eq!(g.nodes[main[3] as usize].after, "back");
        // 28x19 took the man on 23; 14x23 took the man on 19.
        let b = cells(g.nodes[main[3] as usize].fen.as_deref().unwrap());
        let at = |n: u32| {
            let c = INTERNATIONAL.cell(n).unwrap();
            b[c / 10][c % 10]
        };
        assert_eq!((at(19), at(23), at(14), at(28)), (None, Some('m'), None, None));
        // The variation's capture backwards by a man: 38x27 takes the man on 32.
        let v = g.nodes.iter().find(|n| n.san == "38x27").unwrap();
        assert!(v.fen.is_some() && v.depth == 1);
        assert!(variant(&g) == INTERNATIONAL);
    }

    #[test]
    fn multiple_captures_and_kings() {
        // A white man on 37 takes 32 and 22 (written in full, and from and to only), then
        // a black man is crowned on 46.
        for mv in ["37x28x17", "37x17"] {
            let g = play(Some("W:W37:B32,22,41"), "20", &format!("1. {mv} 41-46"));
            assert_eq!(g.error, None, "{mv}");
            let b = cells(&last(&g));
            let c = INTERNATIONAL.cell(17).unwrap();
            assert_eq!(b[c / 10][c % 10], Some('M'), "{mv}");
            let c = INTERNATIONAL.cell(46).unwrap();
            assert_eq!(b[c / 10][c % 10], Some('k'));
            assert_eq!(b.iter().flatten().filter(|p| p.is_some()).count(), 2);
        }
        // A flying king takes from afar and lands beyond: 46x5 over 23.
        let g = play(Some("W:WK46:B23"), "20", "1. 46x5");
        assert_eq!(g.error, None);
        let b = cells(&last(&g));
        assert_eq!(b.iter().flatten().filter(|p| p.is_some()).count(), 1);
        // English draughts: a man takes forward only, and a king one square at a time.
        let g = play(Some("B:W18:B23"), "21", "1. 23x14");
        assert!(g.error.is_some(), "a man takes forward only: 23 cannot take 18 backwards");
        let g = play(Some("B:W18:B14"), "21", "1. 14x23");
        assert_eq!(g.error, None);
        let g = play(Some("B:W18:BK9"), "21", "1. 9x23");
        assert!(g.error.is_some(), "an English king does not fly");
        // A man crowned by a capture stops there in English draughts.
        let g = play(Some("W:W10:B6,7"), "21", "1. 10x1");
        assert_eq!(g.error, None, "{g:?}");
        assert!(last(&g).starts_with("1K"), "{}", last(&g));
    }

    #[test]
    fn checkers_numbers_moves_by_black_first() {
        let g = parse("[GameType \"21\"]\n\n1. 11-15 23-19 2. 8-11 22-17 *");
        assert_eq!(g.error, None);
        let w: Vec<String> = g.layout().iter().filter_map(|t| match t {
            pgn::Token::Move { node, number } => Some(format!("{}{}", number.clone().unwrap_or_default(), g.nodes[*node as usize].san)),
            _ => None,
        }).collect();
        assert_eq!(w, ["1.11-15", "23-19", "2.8-11", "22-17"]);
    }

    #[test]
    fn words_of_pdn() {
        assert!(matches!(word("32-28"), Word::Move("32-28", None)));
        assert!(matches!(word("12.32-28!"), Word::Move("32-28", Some(1))));
        assert!(matches!(word("28x19x10"), Word::Move(..)));
        assert!(matches!(word("c3-d4"), Word::Move(..)));
        assert!(matches!(word("2-0"), Word::Result(_)));
        assert!(matches!(word("12."), Word::Other));
        assert!(matches!(word("1234-5"), Word::Other));
        assert!(matches!(word("Nf3"), Word::Other));
    }

    /// Hostile text: any bytes, deep variations, many moves, a capture with countless paths.
    #[test]
    fn whatever_the_text() {
        for t in ["", "[", "[GameType \"20,W,0,0\"]", "[FEN \"W:W1-99999999:B\"]\n1. 1-6", "1. 99-1", "((((", "1. 32x1x2x3x4x5", "[GameType \"255,X,12,12\"]\n1. 1-2", "1. a1-zz9", "[FEN \"::::K:W,,K\"]", "[FEN \"W:Wé5:B1\"]\n1. é5-1", "[FEN \"W:W\u{1F600},K\u{e9}:B\u{3042}1\"]", "1. \u{e9}5x10"] {
            let g = parse(t);
            let _ = variant(&g);
            for n in &g.nodes {
                let _ = n.fen.as_deref().map(cells);
            }
        }
        // Deeper than MAX_DEPTH: skipped, no stack used.
        let deep = format!("1. 32-28 {}19-23{}", "( 33-28 ".repeat(100_000), ")".repeat(100_000));
        let g = std::thread::Builder::new().stack_size(256 * 1024).spawn(move || parse(&deep)).unwrap().join().unwrap();
        assert!(g.nodes.len() <= pgn::MAX_DEPTH as usize + 3);
        // More than MAX_MOVES: the rest not read.
        let long = "1. 32-28 19-23 2. 28-32 23-19 ".repeat(pgn::MAX_MOVES);
        assert_eq!(parse(&long).nodes.len(), pgn::MAX_MOVES);
    }

    #[test]
    fn capture_search_is_capped() {
        // Kings everywhere: a white king among black men with gaps, many paths.
        let mut fen = String::from("W:WK46:B");
        let blacks: Vec<String> = (7..=44).filter(|n| n % 3 == 0).map(|n| n.to_string()).collect();
        fen.push_str(&blacks.join(","));
        let v = INTERNATIONAL;
        let b = Board::from_fen(v, &fen).unwrap();
        let start = std::time::Instant::now();
        let paths = b.captures(v.cell(46).unwrap(), &Cell::new(usize::MAX)).unwrap();
        assert!(paths.len() <= MAX_SEARCH);
        // An empty budget: no capture at all.
        assert!(b.captures(v.cell(46).unwrap(), &Cell::new(0)).is_none());
        assert!(start.elapsed() < crate::test_limit(std::time::Duration::from_secs(2)));
        // And a 12×12 board full of pieces: still bounded.
        let v = variant_of(&[("GameType".into(), "27".into())], None);
        let mut b = Board::start(v);
        b.cells.iter_mut().filter(|c| **c == WM).take(1).for_each(|c| *c = WK);
        for c in 0..b.cells.len() {
            let _ = b.captures(c, &Cell::new(MAX_SEARCH));
        }
    }

    #[test]
    fn a_file_in_the_table() {
        let d = std::env::temp_dir().join(format!("bragd-pdn-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("games.pdn");
        std::fs::write(&p, "[Event \"Club\"]\n[White \"Anna\"]\n[Black \"Bram\"]\n[Result \"2-0\"]\n\n1. 32-28 19-23 2. 28x19 14x23 2-0\n\n[GameType \"21\"]\n[White \"Carl\"]\n[Black \"Dora\"]\n[Result \"0-1\"]\n\n1. 11-15 23-19 0-1\n").unwrap();
        let lib = crate::chess::Table::open(&p).unwrap();
        while !lib.progress().done {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(lib.progress().games, 2);
        let r = lib.row(0).unwrap();
        assert_eq!((r.white.as_str(), r.result.as_str(), r.moves, r.eco.as_str(), r.opening.as_str()), ("Anna", "2-0", 2, "", ""));
        let q = crate::chess::Query { result: "1-0".into(), ..Default::default() };
        assert_eq!(lib.query(&q), vec![0]);
        let g = lib.game(1).unwrap();
        assert_eq!((g.error.clone(), variant(&g)), (None, ENGLISH));
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// A game made up to cost: thousands of variations, each a capture by a flying king among
    /// pieces with countless paths. It is read fast, its search held to the game's budget.
    #[test]
    fn crafted_captures_cost_little() {
        let blacks: Vec<String> = (7..=44).filter(|n| n % 3 == 0).map(|n| n.to_string()).collect();
        let mut text = format!("[GameType \"20\"]\n[FEN \"W:WK46,K50:B{}\"]\n\n1. 46x5 ", blacks.join(","));
        for k in 0..5000 {
            // Other kings' captures too, so that not every one is the same position.
            text.push_str(if k % 2 == 0 { "(1. 46x5) " } else { "(1. 50x1) " });
        }
        let start = std::time::Instant::now();
        let g = std::thread::Builder::new().stack_size(256 * 1024).spawn(move || parse(&text)).unwrap().join().unwrap();
        assert!(g.nodes.len() > 5000);
        assert!(start.elapsed() < crate::test_limit(std::time::Duration::from_secs(3)), "{:?}", start.elapsed());
        // Without the cache, the budget alone ends it: each position new.
        let r = Draughts::new();
        let b = Board::from_fen(INTERNATIONAL, &format!("W:WK46:B{}", blacks.join(","))).unwrap();
        let c = INTERNATIONAL.cell(46).unwrap();
        let mut n = 0;
        while b.captures(c, &r.search.budget).is_some() {
            n += 1;
        }
        assert!(n > 0 && r.search.budget.get() == 0);
    }

    /// One position with many capture paths, asked again and again in thousands of variations:
    /// the cached answer is charged each time, so the game's budget runs out and the rest of
    /// the captures are not played, fast.
    #[test]
    fn cached_captures_are_charged() {
        let blacks: Vec<String> = (6..=45).filter(|n| n % 2 == 0 && n % 10 != 0).map(|n| n.to_string()).collect();
        let fen = format!("W:WK46,K50,K1,K5:B{}", blacks.join(","));
        let mut text = format!("[GameType \"20\"]\n[FEN \"{fen}\"]\n\n1. 46x5 ");
        for _ in 0..50_000 {
            text.push_str("(1. 46x5) ");
        }
        let start = std::time::Instant::now();
        let r = Draughts::new();
        let g = pgn::parse_with(&text, &r);
        assert!(g.nodes.len() > 1000);
        assert!(r.search.budget.get() < MAX_GAME_SEARCH, "hits are charged");
        assert!(start.elapsed() < crate::test_limit(std::time::Duration::from_secs(5)), "{:?}", start.elapsed());
    }
}
