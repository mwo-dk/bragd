//! Engines and bragd-games' seam: which engines play each Variant of the catalogue, the name
//! each protocol gives that Variant, and a Game's entries turned into the position an engine
//! is asked about and its moves turned back into entries.
//!
//! An engine's move is never trusted: it is matched against the seam's legal moves, so what
//! comes back is always an entry the rules accept, or nothing.

use bragd_games::{Family, LegalMove, State, Variant};

use crate::known::{self, Known};
use crate::protocol::{Position, Protocol, Start};

/// The engines Bragd knows for the Variant `key`.
pub fn engines(key: &str) -> Vec<&'static Known> {
    known::for_variant(key).collect()
}

/// What the protocol calls the Variant, where the engine needs telling: Fairy-Stockfish's
/// `UCI_Variant`, Scan's `variant`. None: the engine plays only that Variant.
pub fn protocol_variant(engine: &Known, key: &str) -> Option<&'static str> {
    match engine.key {
        "fairy-stockfish" => crate::protocol::uci::fairy_variant(key),
        "scan" if key == "draughts" => Some("normal"),
        _ => None,
    }
}

/// The protocols an engine of a Family is tried in when it is chosen by its path.
pub fn protocols(family: Family) -> &'static [Protocol] {
    match family {
        Family::Chess => &[Protocol::Uci],
        Family::Xiangqi => &[Protocol::Uci, Protocol::Ucci],
        Family::Shogi => &[Protocol::Usi, Protocol::Uci],
        Family::Go => &[Protocol::Gtp],
        Family::Draughts => &[Protocol::Hub],
        Family::Backgammon => &[Protocol::Gnubg],
        Family::Tafl | Family::Ur | Family::Senet => &[],
    }
}

/// How an engine writes moves, against the seam's entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notation {
    /// The entries themselves: UCI for chess and its Variants, USI for shogi, and xiangqi as
    /// Fairy-Stockfish writes it (ranks from 1).
    Wire,
    /// Xiangqi as Pikafish and UCCI engines write it: ranks from 0 (`h2e2`, ICCS).
    RanksFromZero,
    /// GTP's points (`D4`, `pass`).
    Gtp,
    /// Hub's draughts moves (`32-28`, `28x19x23`).
    Hub,
    /// GNU Backgammon's turns (`24/18 13/10`, `8/5(2)`, `13/7*`).
    Gnubg,
}

/// How the engine called `name` (as it says it is) writes moves of `key` in `protocol`.
pub fn notation(key: &str, protocol: Protocol, name: &str) -> Notation {
    match protocol {
        Protocol::Gtp => Notation::Gtp,
        Protocol::Hub => Notation::Hub,
        Protocol::Gnubg => Notation::Gnubg,
        Protocol::Ucci => Notation::RanksFromZero,
        _ if known::family(key) == Some(Family::Xiangqi) && !name.to_ascii_lowercase().starts_with("fairy") => Notation::RanksFromZero,
        _ => Notation::Wire,
    }
}

/// Why a Game cannot be put to an engine.
pub type Refusal = String;

/// The Game `entries` from `initial` as the engine is asked about it, and the Game as the
/// seam has it.
pub fn position(key: &str, initial: Option<&str>, entries: &[String], notation: Notation) -> Result<(Position, State), Refusal> {
    let state = bragd_games::replay(key, initial, entries).map_err(|e| e.to_string())?;
    let mut p = Position { variant: key.to_string(), ..Default::default() };
    match notation {
        Notation::Gnubg => {
            let dice = &state.board.dice;
            if state.awaiting != bragd_games::Awaiting::Move || dice.len() < 2 {
                return Err("Throw the dice first: GNU Backgammon is asked about a roll.".into());
            }
            p.start = Start::Setup(gnubg_position_id(&state).ok_or("This backgammon position cannot be written for GNU Backgammon.")?);
            p.dice = Some([dice[0], dice[1]]);
        }
        Notation::Gtp => {
            if initial.is_some_and(|i| !i.trim().is_empty()) {
                return Err("A set-up go position cannot be given to a GTP engine; play it from the start.".into());
            }
            // The scoring entries after two passes are not moves.
            p.moves = entries.iter().take_while(|e| !e.starts_with("dead:") && !matches!(e.as_str(), "accept" | "resume")).map(|e| to_engine(notation, e)).collect();
        }
        _ => {
            if let Some(i) = initial.filter(|i| !i.trim().is_empty()) {
                let start = if notation == Notation::Hub { hub_position(&bragd_games::replay(key, Some(i), &[] as &[&str]).map_err(|e| e.to_string())?) } else { i.to_string() };
                p.start = Start::Setup(start);
            }
            p.moves = entries.iter().map(|e| to_engine(notation, e)).collect();
        }
    }
    Ok((p, state))
}

/// An entry as the engine writes it.
pub fn to_engine(notation: Notation, entry: &str) -> String {
    match notation {
        Notation::Wire | Notation::Gnubg => entry.to_string(),
        Notation::RanksFromZero => shift_ranks(entry, -1).unwrap_or_else(|| entry.to_string()),
        Notation::Gtp if entry == "pass" => "pass".into(),
        Notation::Gtp => entry.to_ascii_uppercase(),
        // `23x14:18:9` → `23x14x18x9`.
        Notation::Hub => entry.replace(':', "x"),
    }
}

/// The legal move of `state` that the engine's `mv` is; None when it is none of them.
pub fn entry<'a>(state: &'a State, notation: Notation, mv: &str) -> Option<&'a LegalMove> {
    let mv = mv.trim();
    let moves = &state.moves;
    match notation {
        Notation::Wire => moves.iter().find(|m| m.wire == mv),
        Notation::RanksFromZero => {
            let wire = shift_ranks(mv, 1)?;
            moves.iter().find(|m| m.wire == wire)
        }
        Notation::Gtp => moves.iter().find(|m| m.wire.eq_ignore_ascii_case(mv)),
        Notation::Hub => {
            let (ends, taken) = hub_parts(mv)?;
            let mut found = moves.iter().filter(|m| {
                let (e, t) = hub_parts(&m.wire.replace(':', "x")).unwrap_or_default();
                e == ends && (taken.is_empty() || t == taken)
            });
            let first = found.next()?;
            // Two captures with the same ends and no squares said: the engine was not clear.
            found.next().is_none().then_some(first)
        }
        Notation::Gnubg => {
            let want = net(mv)?;
            moves.iter().find(|m| net(&m.wire).as_ref() == Some(&want))
        }
    }
}

/// The engine's line `pv` from the Game `entries`, as far as its moves are legal: each move's
/// entry and how a Player writes it (SAN for chess).
pub fn line(key: &str, initial: Option<&str>, entries: &[String], notation: Notation, pv: &[String]) -> Vec<(String, String)> {
    let mut all = entries.to_vec();
    let mut out = vec![];
    for mv in pv {
        let Ok(state) = bragd_games::replay(key, initial, &all) else { break };
        let Some(m) = entry(&state, notation, mv) else { break };
        out.push((m.wire.clone(), m.notation.clone()));
        all.push(m.wire.clone());
    }
    out
}

/// A xiangqi move with each rank number moved by `by` (`h3e3` ⇄ `h2e2`); a janggi pass
/// (`e2e2`) too.
fn shift_ranks(mv: &str, by: i32) -> Option<String> {
    let b = mv.as_bytes();
    let file_at = |i: usize| b.get(i).filter(|c| c.is_ascii_lowercase()).map(|c| *c as char);
    let mut out = String::new();
    let mut i = 0;
    for _ in 0..2 {
        out.push(file_at(i)?);
        i += 1;
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        let rank: i32 = mv.get(start..i)?.parse().ok()?;
        let r = rank + by;
        if !(0..=10).contains(&r) {
            return None;
        }
        out.push_str(&r.to_string());
    }
    (i == b.len()).then_some(out)
}

/// A Hub move's two ends, and the squares it takes, sorted.
fn hub_parts(mv: &str) -> Option<((u16, u16), Vec<u16>)> {
    let sq: Vec<u16> = mv.split(['-', 'x']).map(|s| s.parse().ok()).collect::<Option<_>>()?;
    let (&from, rest) = sq.split_first()?;
    let (&to, taken) = rest.split_first()?;
    let mut taken = taken.to_vec();
    taken.sort_unstable();
    Some(((from, to), taken))
}

/// What a backgammon turn does to the mover's checkers: how many leave and arrive at each
/// place, in the mover's numbering, with hits, repeats (`(2)`) and chains (`24/18/12`)
/// written out. Two ways of writing one turn give the same.
fn net(turn: &str) -> Option<Vec<(String, i32)>> {
    let mut count: std::collections::BTreeMap<String, i32> = std::collections::BTreeMap::new();
    let turn = turn.trim();
    if turn.is_empty() || turn.starts_with('(') {
        return Some(vec![]);
    }
    for part in turn.split_whitespace() {
        let (part, times) = match part.split_once('(') {
            Some((p, n)) => (p, n.trim_end_matches(')').parse::<i32>().ok()?),
            None => (part, 1),
        };
        let places: Vec<&str> = part.split('/').map(|p| p.trim_end_matches('*')).collect();
        if places.len() < 2 {
            return None;
        }
        *count.entry(places[0].to_ascii_lowercase()).or_default() -= times;
        *count.entry(places[places.len() - 1].to_ascii_lowercase()).or_default() += times;
    }
    Some(count.into_iter().filter(|(_, n)| *n != 0).collect())
}

/// The draughts position of `state` as Hub writes it: the side to move (`W`, `B`), then each of
/// the fifty squares (`w`, `b` men, `W`, `B` kings, `e` empty).
fn hub_position(state: &State) -> String {
    let mut squares = vec!['e'; 50];
    for p in &state.board.pieces {
        if let Ok(n) = p.square.parse::<usize>()
            && (1..=50).contains(&n)
        {
            let white = p.sprite.contains("-w");
            let king = p.sprite.ends_with('K');
            squares[n - 1] = match (white, king) {
                (true, false) => 'w',
                (false, false) => 'b',
                (true, true) => 'W',
                (false, true) => 'B',
            };
        }
    }
    let side = if state.to_move.is_none_or(|s| s.index == 0) { 'W' } else { 'B' };
    std::iter::once(side).chain(squares).collect()
}

/// GNU Backgammon's Position ID of `state`: for the player not on roll, then the player on
/// roll, each of their points 1 to 24 in their own numbering and then their bar, as many 1
/// bits as checkers and a 0; 80 bits, the first in the lowest bit of the first byte, in
/// Base64 without padding.
pub fn gnubg_position_id(state: &State) -> Option<String> {
    // Points in White's numbering on the drawn board; White's bar at 0, Black's at 25.
    let mut white = [0u8; 26];
    let mut black = [0u8; 26];
    for p in &state.board.pieces {
        let x = p.x as usize;
        if x > 25 {
            return None;
        }
        if p.sprite.ends_with("-w") {
            white[if x == 0 { 25 } else { x }] += p.count;
        } else {
            // Black's own point n is White's 25 - n; its bar sits at 25 either way.
            black[if x == 25 { 25 } else { 25 - x }] += p.count;
        }
    }
    let on_roll_white = state.to_move?.index == 0;
    let (roll, other) = if on_roll_white { (white, black) } else { (black, white) };
    let mut bits = vec![];
    for side in [other, roll] {
        for n in side.iter().skip(1) {
            bits.extend(std::iter::repeat_n(true, *n as usize));
            bits.push(false);
        }
    }
    if bits.len() > 80 {
        return None;
    }
    let mut bytes = [0u8; 10];
    for (i, b) in bits.iter().enumerate() {
        if *b {
            bytes[i / 8] |= 1 << (i % 8);
        }
    }
    Some(base64(&bytes)[..14].to_string())
}

fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..=c.len() {
            out.push(A[(n >> (18 - 6 * k) & 63) as usize] as char);
        }
    }
    out
}

/// Whether the engine `about` can be asked about `key` at all: a UCI engine without
/// `UCI_Variant` plays only chess and Chess960.
pub fn plays(about: &crate::About, protocol: Protocol, key: &str) -> bool {
    match Variant::from_key(key).map(Variant::family) {
        Some(Family::Chess) if !matches!(key, "chess" | "chess960") => about.option("UCI_Variant").is_some_and(|o| o.vars.is_empty() || o.vars.iter().any(|v| Some(v.as_str()) == crate::protocol::uci::fairy_variant(key))),
        Some(Family::Xiangqi) if key == "janggi" => about.option("UCI_Variant").is_some(),
        Some(Family::Shogi) => protocol == Protocol::Usi,
        Some(Family::Draughts) => key == "draughts",
        Some(_) => true,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every Variant of the catalogue: the engines that play it, and what Fairy-Stockfish and
    /// Scan call it.
    #[test]
    fn every_variant_has_its_engines() {
        let want: &[(&str, &[&str], Option<&str>)] = &[
            ("chess", &["stockfish", "fairy-stockfish"], Some("chess")),
            ("chess960", &["stockfish", "fairy-stockfish"], Some("chess")),
            ("shatranj", &["fairy-stockfish"], Some("shatranj")),
            ("chaturanga", &["fairy-stockfish"], Some("chaturanga")),
            ("kingofthehill", &["fairy-stockfish"], Some("kingofthehill")),
            ("racingkings", &["fairy-stockfish"], Some("racingkings")),
            ("seirawan", &["fairy-stockfish"], Some("seirawan")),
            ("capablanca", &["fairy-stockfish"], Some("capablanca")),
            ("xiangqi", &["fairy-stockfish", "pikafish"], Some("xiangqi")),
            ("janggi", &["fairy-stockfish"], Some("janggi")),
            ("shogi", &["fairy-stockfish", "yaneuraou"], Some("shogi")),
            ("go-9", &["katago", "gnugo"], None),
            ("go-13", &["katago", "gnugo"], None),
            ("go-19", &["katago", "gnugo"], None),
            ("checkers", &[], None),
            ("draughts", &["scan"], Some("normal")),
            ("draughts-brazilian", &[], None),
            ("draughts-russian", &[], None),
            ("hnefatafl", &[], None),
            ("hnefatafl-fetlar", &[], None),
            ("tablut", &[], None),
            ("brandubh", &[], None),
            ("backgammon", &["gnubg"], None),
            ("ur", &[], None),
            ("senet", &[], None),
        ];
        assert_eq!(want.len(), bragd_games::VARIANTS.len());
        for (info, (key, engines_want, name)) in bragd_games::VARIANTS.iter().zip(want) {
            assert_eq!(info.key, *key);
            let got: Vec<&str> = engines(key).iter().map(|k| k.key).collect();
            assert_eq!(&got, engines_want, "{key}");
            let named = engines(key).iter().find_map(|k| protocol_variant(k, key));
            assert_eq!(named, *name, "{key}");
            if !info.family.has_engine() {
                assert!(got.is_empty() && protocols(info.family).is_empty());
            }
        }
        for k in known::KNOWN {
            assert!(k.variants.iter().all(|v| Variant::from_key(v).is_some()), "{}: not a catalogue key", k.key);
        }
    }

    /// Plays `entries` of `key`, each one turned into the engine's notation and back by
    /// matching the legal moves: the same entry every time.
    fn round_trip(key: &str, notation: Notation, entries: &[&str]) {
        let mut played: Vec<String> = vec![];
        for e in entries {
            let state = bragd_games::replay(key, None, &played).unwrap_or_else(|err| panic!("{key}: {err}"));
            let theirs = to_engine(notation, e);
            let back = entry(&state, notation, &theirs).unwrap_or_else(|| panic!("{key}: {e} → {theirs} not legal"));
            assert_eq!(back.wire, *e, "{key}: {theirs}");
            played.push(e.to_string());
        }
    }

    #[test]
    fn moves_go_to_engines_and_come_back() {
        round_trip("chess", Notation::Wire, &["e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "g8f6", "e1g1"]);
        round_trip("capablanca", Notation::Wire, &["e2e4"]);
        round_trip("shogi", Notation::Wire, &["7g7f", "3c3d", "8h2b+", "3a2b", "B*4e"]);
        round_trip("xiangqi", Notation::Wire, &["h3e3", "h10g8"]);
        round_trip("xiangqi", Notation::RanksFromZero, &["h3e3", "h10g8", "h1g3"]);
        assert_eq!(to_engine(Notation::RanksFromZero, "h10g8"), "h9g7");
        round_trip("go-9", Notation::Gtp, &["e5", "c3", "pass"]);
        assert_eq!(to_engine(Notation::Gtp, "e5"), "E5");
        round_trip("draughts", Notation::Hub, &["32-28", "19-23", "28x19:23", "14x23:19"]);
        assert_eq!(to_engine(Notation::Hub, "28x19:23"), "28x19x23");
        // Scan names a capture by its ends alone when that is enough.
        let state = bragd_games::replay("draughts", None, &["32-28", "19-23"]).unwrap();
        assert_eq!(entry(&state, Notation::Hub, "28x19").unwrap().wire, "28x19:23");
        // A move that is not legal comes back as nothing.
        assert!(entry(&bragd_games::replay("chess", None, &[] as &[&str]).unwrap(), Notation::Wire, "e2e5").is_none());
        assert!(entry(&bragd_games::replay("go-9", None, &[] as &[&str]).unwrap(), Notation::Gtp, "resign").is_none());
    }

    #[test]
    fn backgammon_turns_and_positions() {
        let start = bragd_games::replay("backgammon", None, &["3-1"]).unwrap();
        // GNU Backgammon's way of writing 8/5 6/5, and the same turn as a chain.
        let m = entry(&start, Notation::Gnubg, "8/5 6/5").unwrap();
        assert_eq!(net(&m.wire), net("6/5 8/5"));
        assert_eq!(net("24/18/12"), net("24/18 18/12"));
        assert_eq!(net("8/5(2)"), net("8/5 8/5"));
        assert_eq!(net("13/7*"), net("13/7"));
        let (p, _) = position("backgammon", None, &["3-1".into()], Notation::Gnubg).unwrap();
        assert_eq!(p.start, Start::Setup("4HPwATDgc/ABMA".into()), "the start, as GNU Backgammon writes it");
        assert_eq!(p.dice, Some([3, 1]));
        assert!(position("backgammon", None, &[], Notation::Gnubg).is_err(), "no roll yet");
        // The ID changes with the board.
        let after = bragd_games::replay("backgammon", None, &["3-1", m.wire.as_str(), "6-5"]).unwrap();
        assert_ne!(gnubg_position_id(&after).unwrap(), "4HPwATDgc/ABMA");
    }

    #[test]
    fn positions_for_engines() {
        let e: Vec<String> = vec!["e2e4".into()];
        let (p, s) = position("chess", None, &e, Notation::Wire).unwrap();
        assert_eq!((p.moves.clone(), p.start.clone(), s.history.clone()), (e.clone(), Start::Initial, vec!["e4".to_string()]));
        let (p, _) = position("draughts", Some(&bragd_games::start_position("draughts").unwrap()), &[], Notation::Hub).unwrap();
        assert_eq!(p.start, Start::Setup(crate::protocol::hub::START.into()));
        assert!(position("go-9", Some("x"), &[], Notation::Gtp).is_err());
        let (p, _) = position("go-9", None, &["e5".into(), "pass".into(), "pass".into(), "dead:".into()], Notation::Gtp).unwrap();
        assert_eq!(p.moves, ["E5", "pass", "pass"]);
        // A line in SAN, as far as it is legal.
        let l = line("chess", None, &e, Notation::Wire, &["e7e5".into(), "g1f3".into(), "a1a8".into()]);
        assert_eq!(l, [("e7e5".to_string(), "e5".to_string()), ("g1f3".to_string(), "Nf3".to_string())]);
        assert_eq!(notation("xiangqi", Protocol::Uci, "Pikafish 2026"), Notation::RanksFromZero);
        assert_eq!(notation("xiangqi", Protocol::Uci, "Fairy-Stockfish 14"), Notation::Wire);
    }
}
