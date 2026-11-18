//! One pass over a games file: where each game starts, its headers and its first moves, for
//! files of millions of games (`scan`). A game's moves are read when it is opened. A SCID
//! database is listed the same way (`scid_heads`).

use std::io::{self, BufRead, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::openings;
use super::pgn::{self, Word};

/// The tags the table reads, by their place in `Head::tags`.
pub const WANTED: [&str; 11] =
    ["White", "Black", "WhiteElo", "BlackElo", "Result", "Date", "Event", "Site", "ECO", "Opening", "Variation"];
pub const WHITE: usize = 0;
pub const BLACK: usize = 1;
pub const WHITE_ELO: usize = 2;
pub const BLACK_ELO: usize = 3;
pub const RESULT: usize = 4;
pub const DATE: usize = 5;
pub const EVENT: usize = 6;
pub const SITE: usize = 7;
pub const ECO: usize = 8;
pub const OPENING: usize = 9;
pub const VARIATION: usize = 10;
/// The first moves kept to name the opening.
const FIRST_MOVES: usize = 24;

/// A game as the pass sees it: where it is, the headers wanted, its length in half-moves and
/// its first moves.
pub struct Head {
    pub offset: u64,
    pub len: u64,
    pub tags: [String; WANTED.len()],
    pub plies: u32,
    pub sans: Vec<String>,
    /// The game's bytes, up to `MAX_GAME` of them, when `scan` is asked to keep them.
    pub text: Vec<u8>,
    /// A chess game (`Pgn`), else draughts (`Pdn`) or xiangqi, which have no openings by name.
    pub format: super::Format,
}

impl Head {
    /// Tag `i` of `WANTED`, trimmed.
    pub fn tag(&self, i: usize) -> &str {
        self.tags[i].trim()
    }

    /// ECO code and opening name: the file's own, else by the moves, else by the code.
    pub fn opening(&self) -> (String, String) {
        if self.format != super::Format::Pgn {
            return Default::default();
        }
        let eco = self.tag(ECO).to_string();
        let mut name = self.tag(OPENING).to_string();
        if !name.is_empty() && !self.tag(VARIATION).is_empty() {
            name = format!("{name}: {}", self.tag(VARIATION));
        }
        if name.is_empty() {
            match openings::by_moves(self.sans.iter().map(String::as_str)) {
                Some((code, n)) if eco.is_empty() || eco == code => return (code.to_string(), n.to_string()),
                _ => name = openings::by_eco(&eco).unwrap_or_default().to_string(),
            }
        }
        (eco, name)
    }

}

/// Bytes as text: UTF-8, else Windows-1252 (old PGN files are Latin-1).
pub fn text(b: &[u8]) -> std::borrow::Cow<'_, str> {
    match std::str::from_utf8(b) {
        Ok(s) => s.into(),
        Err(_) => encoding_rs::WINDOWS_1252.decode_without_bom_handling(b).0,
    }
}

pub use super::lex::MAX_LINE;
use super::lex::{Lexer, Movetext, Tok};
/// The most of one game read to show it.
pub const MAX_GAME: u32 = 16 << 20;

/// The games of `r`, in order, each handed to `each` until it says to stop. Never fails on
/// what it reads: lines that are no PGN are taken for moves, and skipped there.
/// `format`: whose moves they are, chess's (`Pgn`), draughts' or xiangqi's. `full`: the moves
/// counted to the end of each game (`Head::plies`); else past its first moves only where its
/// comments start and end is looked at, to know where the next game starts. `keep`: each game's
/// bytes kept in `Head::text` (PGN, PDN and xiangqi), its moves not looked at.
pub fn scan(r: impl BufRead, stop: &AtomicBool, format: super::Format, full: bool, keep: bool, mut each: impl FnMut(&Head) -> bool) -> io::Result<()> {
    let mut r = Lexer::new(r);
    if matches!(format, super::Format::Kif | super::Format::Csa) {
        return scan_shogi(&mut r, stop, format, each);
    }
    let mut head = Head { offset: 0, len: 0, tags: Default::default(), plies: 0, sans: vec![], text: vec![], format };
    let mut line = Vec::new();
    let (mut has_tags, mut has_moves, mut in_brace, mut depth) = (false, false, false, 0u32);
    loop {
        let mut at = r.at;
        let n = r.line(&mut line)?;
        // A byte order mark before the first tag is no part of the game.
        if at == 0 && line.starts_with(b"\xEF\xBB\xBF") {
            line.drain(..3);
            at = 3;
        }
        // An unclosed `{` ends at the next game's first tag, at the start of a line.
        if in_brace && line.starts_with(b"[Event \"") {
            in_brace = false;
        }
        let starts = |head: &mut Head| {
            for t in &mut head.tags {
                t.clear();
            }
            head.offset = at;
            head.plies = 0;
            head.sans.clear();
            head.text.clear();
        };
        let trimmed = line.trim_ascii_start();
        let is_tag = n > 0 && !in_brace && trimmed.first() == Some(&b'[');
        // A new game: tags after moves, an Event tag a second time, or the end.
        let new = n == 0 || (is_tag && (has_moves || (trimmed.starts_with(b"[Event ") && !head.tags[EVENT].is_empty())));
        if new && (has_tags || has_moves) {
            head.len = at - head.offset;
            if !each(&head) || stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            (has_tags, has_moves, in_brace, depth) = (false, false, false, 0);
        }
        if n == 0 {
            return Ok(());
        }
        if new || (!has_tags && !has_moves) {
            starts(&mut head);
        }
        if keep && head.text.len() + line.len() <= MAX_GAME as usize {
            head.text.extend_from_slice(&line);
        }
        if is_tag {
            if let Some((k, v)) = pgn::tag_pair(&text(&line))
                // Xiangqi names its first player Red.
                && let Some(i) = WANTED.iter().position(|w| w.eq_ignore_ascii_case(&k)).or_else(|| k.eq_ignore_ascii_case("Red").then_some(WHITE))
            {
                head.tags[i] = v;
            }
            has_tags = true;
        } else if !trimmed.is_empty() && (trimmed[0] != b'%' || in_brace) {
            has_moves = true;
            moves(&text(&line), &mut head, &mut in_brace, &mut depth, full, keep);
        }
    }
}

/// The games of a KIF or CSA file (`crate::shogi`), read as they stream by: a CSA file's games
/// end at a line of `/`, a KIF file is one game. Each game is read in full for its tags and
/// moves, up to `MAX_GAME` bytes of it.
fn scan_shogi(r: &mut Lexer<impl BufRead>, stop: &AtomicBool, format: super::Format, mut each: impl FnMut(&Head) -> bool) -> io::Result<()> {
    let (mut game, mut line) = (Vec::new(), Vec::new());
    let mut start = 0u64;
    let mut emit = |game: &mut Vec<u8>, start: u64, end: u64| -> bool {
        let keep = game.iter().any(|c| !c.is_ascii_whitespace());
        let g = crate::shogi::parse(&crate::shogi::decode(game), format);
        game.clear();
        if !keep {
            return true;
        }
        let mut head = Head { offset: start, len: end - start, tags: Default::default(), plies: g.main_line().len() as u32, sans: vec![], text: vec![], format };
        for (k, v) in &g.tags {
            if let Some(i) = WANTED.iter().position(|w| w.eq_ignore_ascii_case(k)) {
                head.tags[i] = v.clone();
            }
        }
        head.tags[RESULT] = g.result.clone();
        each(&head)
    };
    loop {
        let at = r.at;
        if r.line(&mut line)? == 0 {
            emit(&mut game, start, at);
            return Ok(());
        }
        if format == super::Format::Csa && line.trim_ascii() == b"/" {
            if !emit(&mut game, start, at) || stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            start = r.at;
            continue;
        }
        if game.len() + line.len() <= MAX_GAME as usize {
            game.extend_from_slice(&line);
        }
    }
}

/// A line of movetext: counts the main line's moves and keeps the first few. Not `full`, once
/// the first few are kept, only where comments start and end is looked at; `skim`, only that.
fn moves(line: &str, head: &mut Head, in_brace: &mut bool, depth: &mut u32, full: bool, skim: bool) {
    let mut toks = Movetext::new(line, *in_brace);
    while !skim && (full || head.sans.len() < FIRST_MOVES) {
        let Some(tok) = toks.next() else {
            *in_brace = toks.in_brace;
            return;
        };
        match tok {
            Tok::Open => *depth = depth.saturating_add(1),
            Tok::Close => *depth = depth.saturating_sub(1),
            Tok::Word(w) if *depth == 0 => {
                let mv = match head.format {
                    super::Format::Pdn if crate::draughts::is_move(w) => Word::Move(w, None),
                    super::Format::Xiangqi if crate::xiangqi::is_move(w) => Word::Move(w, None),
                    super::Format::Pdn | super::Format::Xiangqi => Word::Other,
                    _ => pgn::word(w),
                };
                if let Word::Move(san, _) = mv {
                    head.plies = head.plies.saturating_add(1);
                    if head.sans.len() < FIRST_MOVES {
                        head.sans.push(san.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    // Past the first moves, not `full`: only where comments start and end.
    let (mut s, mut open) = (toks.rest(), toks.in_brace);
    loop {
        let b = s.as_bytes();
        if open {
            let Some(e) = memchr::memchr(b'}', b) else { break };
            (s, open) = (&s[e + 1..], false);
        } else {
            match memchr::memchr2(b'{', b';', b) {
                Some(i) if b[i] == b'{' => (s, open) = (&s[i + 1..], true),
                _ => break,
            }
        }
    }
    *in_brace = open;
}

/// Counts the bytes read through it into `read`, for the progress.
pub struct Counted<R> {
    pub inner: R,
    pub read: Arc<AtomicU64>,
}

impl<R: Read> Read for Counted<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

/// The games of a SCID database (`scid`), from its index as it streams by: a game's place in
/// the index is its `offset`. `read` counts the index bytes read, for the progress.
pub fn scid_heads(base: &super::scid::Base, stop: &AtomicBool, read: &AtomicU64, mut each: impl FnMut(&Head) -> bool) -> io::Result<()> {
    let mut last = 0u32;
    let res = base.entries(|n, e| {
        let mut head = Head { offset: n as u64, len: 0, tags: Default::default(), plies: e.plies, sans: vec![], text: vec![], format: super::Format::Pgn };
        head.tags[WHITE] = base.white(e).to_string();
        head.tags[BLACK] = base.black(e).to_string();
        head.tags[EVENT] = base.event(e).to_string();
        head.tags[SITE] = base.site(e).to_string();
        head.tags[DATE] = e.date.clone();
        head.tags[ECO] = e.eco.clone();
        head.tags[RESULT] = ["*", "1-0", "0-1", "1/2-1/2"][e.result as usize & 3].to_string();
        for (i, elo) in [(WHITE_ELO, e.white_elo), (BLACK_ELO, e.black_elo)] {
            if elo > 0 {
                head.tags[i] = elo.to_string();
            }
        }
        // Deleted games, skipped, count as read too.
        last = n + 1;
        read.store(last as u64 * base.entry_len(), Ordering::Relaxed);
        each(&head) && !stop.load(Ordering::Relaxed)
    });
    if !stop.load(Ordering::Relaxed) && last > 0 {
        read.store(base.count as u64 * base.entry_len(), Ordering::Relaxed);
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two games after a byte order mark: each where it starts, its tags, its moves counted to
    /// the end, its opening named by its moves; with `keep`, its bytes instead of its moves.
    #[test]
    fn a_pass_over_pgn() {
        let text = "\u{feff}[White \"Morphy, Paul\"]\n[Result \"1-0\"]\n\n1. e4 e5 2. Nf3 {a comment ( } (2. f4) Nc6 3. Bb5 1-0\n\n[White \"Tal\"]\n[WhiteElo \"2700\"]\n\n1. d4 Nf6 *\n";
        let mut heads = vec![];
        scan(text.as_bytes(), &AtomicBool::new(false), super::super::Format::Pgn, true, false, |h| {
            heads.push((h.offset, h.len, h.tag(WHITE).to_string(), h.tag(WHITE_ELO).to_string(), h.plies, h.opening()));
            true
        })
        .unwrap();
        assert_eq!(heads.len(), 2);
        let second = text.find("[White \"Tal").unwrap() as u64;
        assert_eq!((heads[0].0, heads[0].1, heads[1].0), (3, second - 3, second));
        assert_eq!((heads[0].2.as_str(), heads[0].4, heads[0].5.0.as_str()), ("Morphy, Paul", 5, "C60"));
        assert_eq!((heads[1].2.as_str(), heads[1].3.as_str(), heads[1].4), ("Tal", "2700", 2));
        // Kept, and told to stop after the first: one, its bytes.
        let mut kept = vec![];
        scan(text.as_bytes(), &AtomicBool::new(false), super::super::Format::Pgn, false, true, |h| {
            kept.push((h.plies, super::text(&h.text).into_owned()));
            false
        })
        .unwrap();
        assert_eq!(kept, [(0, text[3..second as usize].to_string())]);
    }
}
