//! Reading and writing FIDE Tournament Report Files: TRF-16 and TRF-2026 (FIDE
//! Handbook C.02 Appendix A), with the `XXR` rounds and `XXC` initial-colour
//! extensions pairing programs share.
//!
//! [`parse`] keeps what pairing and a report need; [`write`] writes it back in
//! the same columns.

use crate::Colour;

/// A parsed tournament report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trf {
    /// Line `012`, trimmed.
    pub name: String,
    /// Line `022`.
    pub city: String,
    /// Line `032`, the three-letter federation.
    pub federation: String,
    /// Lines `042` and `052`, `YYYY/MM/DD`.
    pub start: String,
    pub end: String,
    /// Line `102`.
    pub arbiter: String,
    /// Line `192`, TRF-2026's tournament type code, such as `FIDE_DUTCH_2025`.
    pub kind: String,
    /// Line `202`, TRF-2026's tiebreak codes, comma-separated: `BH/C1,BH,SB`.
    pub tiebreaks: String,
    /// Line `XXR`, the number of rounds declared.
    pub rounds: Option<u32>,
    /// Line `XXC`: `white1` gives White, `black1` gives Black.
    pub initial_colour: Option<Colour>,
    /// Lines `001`, in file order.
    pub players: Vec<TrfPlayer>,
}

/// One `001` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrfPlayer {
    /// Columns 5-8.
    pub start_rank: u32,
    /// Column 10: `m`, `w` or blank.
    pub sex: char,
    /// Columns 11-13, trimmed: `GM`, `WIM`, ….
    pub title: String,
    /// Columns 15-47, trimmed.
    pub name: String,
    /// Columns 49-52; 0 when blank.
    pub rating: u32,
    /// Columns 54-56, trimmed.
    pub federation: String,
    /// Columns 58-68, trimmed; empty when blank or 0.
    pub fide_id: String,
    /// Columns 70-79, trimmed: `YYYY/MM/DD`.
    pub born: String,
    /// Columns 81-84 in points, as `3.5` is written: halves of a point here.
    pub points: u32,
    /// Columns 86-89; 0 when blank.
    pub rank: u32,
    /// One entry per round present on the line, starting at column 92, ten
    /// columns apart.
    pub rounds: Vec<TrfGame>,
}

/// One round cell of a `001` line: columns 92-95 opponent, 97 colour, 99 result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrfGame {
    /// The opponent's start rank; `None` for `0000` or blank.
    pub opponent: Option<u32>,
    /// `w` or `b`; `None` for `-` or blank.
    pub colour: Option<Colour>,
    /// The result character as written: `1 0 = + - W D L H F U Z`, or ' ' when blank.
    pub result: char,
}

/// Why a file could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrfError {
    /// 1-based line number.
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for TrfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for TrfError {}

/// Writes a report: the header lines that are not empty, `XXR` and `XXC` when
/// known, then one `001` line per player in the columns [`parse`] reads.
pub fn write(t: &Trf) -> String {
    let mut out = String::new();
    for (code, v) in [
        ("012", &t.name),
        ("022", &t.city),
        ("032", &t.federation),
        ("042", &t.start),
        ("052", &t.end),
        ("102", &t.arbiter),
        ("192", &t.kind),
        ("202", &t.tiebreaks),
    ] {
        if !v.is_empty() {
            out.push_str(&format!("{code} {v}\n"));
        }
    }
    if let Some(r) = t.rounds {
        out.push_str(&format!("XXR {r}\n"));
    }
    if let Some(c) = t.initial_colour {
        out.push_str(if c == Colour::White { "XXC white1\n" } else { "XXC black1\n" });
    }
    let blank = |n: u32| if n == 0 { String::new() } else { n.to_string() };
    for p in &t.players {
        // Fields cut to their columns, so a long name cannot shift the rest.
        let fit = |s: &str, w: usize| s.chars().take(w).collect::<String>();
        let mut line = format!(
            "001 {:>4} {}{:>3} {:<33} {:>4} {:<3} {:>11} {:<10} {:>4} {:>4}  ",
            p.start_rank,
            p.sex,
            fit(&p.title, 3),
            fit(&p.name, 33),
            blank(p.rating),
            fit(&p.federation, 3),
            fit(&p.fide_id, 11),
            fit(&p.born, 10),
            format!("{}.{}", p.points / 2, if p.points % 2 == 1 { 5 } else { 0 }),
            blank(p.rank),
        );
        for g in &p.rounds {
            let colour = match g.colour {
                Some(Colour::White) => 'w',
                Some(Colour::Black) => 'b',
                None => '-',
            };
            line.push_str(&format!("{:>4} {colour} {}  ", g.opponent.unwrap_or(0), g.result));
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

/// Parses a TRF-16 or TRF-2026 file.
pub fn parse(text: &str) -> Result<Trf, TrfError> {
    let mut t = Trf {
        name: String::new(),
        city: String::new(),
        federation: String::new(),
        start: String::new(),
        end: String::new(),
        arbiter: String::new(),
        kind: String::new(),
        tiebreaks: String::new(),
        rounds: None,
        initial_colour: None,
        players: Vec::new(),
    };
    for (k, raw) in text.lines().enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let err = |message: String| TrfError {
            line: k + 1,
            message,
        };
        let header = [
            ("012 ", &mut t.name),
            ("022 ", &mut t.city),
            ("032 ", &mut t.federation),
            ("042 ", &mut t.start),
            ("052 ", &mut t.end),
            ("102 ", &mut t.arbiter),
            ("192 ", &mut t.kind),
            ("202 ", &mut t.tiebreaks),
        ];
        if let Some((rest, field)) = header.into_iter().find_map(|(code, f)| line.strip_prefix(code).map(|r| (r, f))) {
            *field = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("XXR ") {
            t.rounds = Some(
                rest.trim()
                    .parse()
                    .map_err(|_| err(format!("bad round count {:?}", rest.trim())))?,
            );
        } else if let Some(rest) = line.strip_prefix("XXC ") {
            t.initial_colour = match rest.trim() {
                "white1" => Some(Colour::White),
                "black1" => Some(Colour::Black),
                other => return Err(err(format!("bad initial colour {other:?}"))),
            };
        } else if line.starts_with("001") {
            t.players.push(player(line).map_err(err)?);
        }
    }
    Ok(t)
}

/// Columns `from..=to` (1-based, in characters), clipped to the line; empty when
/// the line is shorter.
fn cols(line: &str, from: usize, to: usize) -> String {
    line.chars().skip(from - 1).take(to + 1 - from).collect()
}

/// A whole number in columns `from..=to`; 0 when blank.
fn number(line: &str, from: usize, to: usize, what: &str) -> Result<u32, String> {
    let s = cols(line, from, to);
    let s = s.trim();
    if s.is_empty() { Ok(0) } else { s.parse().map_err(|_| format!("bad {what} {s:?}")) }
}

fn player(line: &str) -> Result<TrfPlayer, String> {
    let rank = cols(line, 5, 8);
    let rank = rank.trim();
    let start_rank = rank
        .parse()
        .map_err(|_| format!("bad start rank {rank:?}"))?;
    let rating = number(line, 49, 52, "rating")?;
    let points = cols(line, 81, 84).trim().replace(',', ".");
    let points = match points.split_once('.') {
        _ if points.is_empty() => Some(0),
        None => points.parse::<u32>().ok().map(|p| 2 * p),
        Some((whole, "5")) => whole.parse::<u32>().ok().map(|p| 2 * p + 1),
        Some((whole, "0")) => whole.parse::<u32>().ok().map(|p| 2 * p),
        Some(_) => None,
    }
    .ok_or_else(|| format!("bad points {points:?}"))?;
    let chars = line.chars().count();
    let mut rounds = Vec::new();
    let mut at = 92;
    while chars >= at + 3 {
        let opp = cols(line, at, at + 3);
        let opponent = match opp.trim() {
            "" | "0000" => None,
            o => Some(o.parse().map_err(|_| format!("bad opponent {o:?}"))?)
                .filter(|&n: &u32| n != 0),
        };
        let colour = match cols(line, at + 5, at + 5).as_str() {
            "w" => Some(Colour::White),
            "b" => Some(Colour::Black),
            "" | " " | "-" => None,
            c => return Err(format!("bad colour {c:?}")),
        };
        let result = cols(line, at + 7, at + 7).chars().next().unwrap_or(' ');
        rounds.push(TrfGame {
            opponent,
            colour,
            result,
        });
        at += 10;
    }
    let text = |from, to| cols(line, from, to).trim().to_string();
    Ok(TrfPlayer {
        start_rank,
        sex: cols(line, 10, 10).chars().next().unwrap_or(' '),
        title: text(11, 13),
        name: text(15, 47),
        rating,
        federation: text(54, 56),
        fide_id: Some(text(58, 68)).filter(|s| s != "0").unwrap_or_default(),
        born: text(70, 79),
        points,
        rank: number(line, 86, 89, "rank")?,
        rounds,
    })
}
