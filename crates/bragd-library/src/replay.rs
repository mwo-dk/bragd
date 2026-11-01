//! A stored Game played again in `bragd_games`, so an Engine can be asked about its positions.
//!
//! A Library keeps a Game's moves as its file wrote them: SAN, PDN, ICCS, WXF or Chinese for
//! xiangqi, Japanese for shogi, SGF for go, and a Player's notation for Games Bragd played. Only
//! the keys of its positions (`position_keys`, the `positions` table) say for sure where each
//! move went. So each move is found among the legal moves of the position before it:
//!
//! - chess, xiangqi, shogi, draughts and tafl: the legal move whose position has the stored
//!   key, the one written as the file wrote it tried first;
//! - go: SGF's points turned into `bragd_games`' points, stones set up before the first move
//!   played as moves, with a pass where one side has none left, as a GTP engine takes no set-up;
//! - backgammon and the other dice Games: the throws and turns as Bragd wrote them, whose parts
//!   the Library keeps apart (`13/8 8/5` is two words of its moves).
//!
//! What an Engine is asked about each position (`Asked`) is the position itself where the
//! Family can be set up from its text, and else the Game's start and the moves to it (go,
//! backgammon).

use bragd_games::{Awaiting, Family, State, Variant};

use crate::library::{Stored, position_keys};

/// One position of a Game, as an Engine is asked about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asked {
    /// Its key, as `position_keys` makes it.
    pub key: u64,
    /// The position in the Family's notation; for go and the dice Games, the Game's start
    /// (empty: the Variant's usual one).
    pub setup: String,
    /// The entries from `setup` to the position: empty where `setup` is the position itself.
    pub moves: Vec<String>,
    /// Whose turn it is: a move (`Awaiting::Move`), or a throw of the dice before it.
    pub awaiting: Awaiting,
}

/// The positions of `s` before its first move and after each of its first `upto` moves, as far
/// as its moves can be played: `out[ply]`. `keys`: the keys of its positions after each move,
/// as the `positions` table has them. Empty when its Variant is not one Bragd plays.
pub fn asked(s: &Stored, keys: &[u64], upto: usize) -> Vec<Asked> {
    let Some(variant) = Variant::from_key(&s.variant) else { return vec![] };
    let family = variant.family();
    let upto = upto.min(s.moves.len());
    let (initial, states) = match family {
        Family::Go => go(s, variant, upto),
        _ if set_up(s, variant) => (s.start.clone(), by_keys(s, variant, keys, upto)),
        _ => (s.start.clone(), by_words(s, variant, keys, upto)),
    };
    let texts: Vec<String> = states.iter().map(|(st, _)| st.position.clone()).collect();
    let keyed = position_keys(family.key(), &texts);
    let set_up = family != Family::Go && set_up(s, variant);
    let mut entries: Vec<String> = vec![];
    states
        .iter()
        .zip(keyed)
        .map(|((st, entry), key)| {
            entries.extend(entry.iter().cloned());
            if set_up {
                Asked { key, setup: st.position.clone(), moves: vec![], awaiting: st.awaiting }
            } else {
                Asked { key, setup: initial.clone().unwrap_or_default(), moves: entries.clone(), awaiting: st.awaiting }
            }
        })
        .collect()
}

/// Whether the Family's positions are set up from their own text: the text of the position
/// after a first move, read back, is that position. Not draughts (`bragd_games` plays it from
/// its opening array only), nor the dice Games, whose text leaves out who threw what.
fn set_up(s: &Stored, variant: Variant) -> bool {
    let start = bragd_games::replay::<&str>(variant.key(), s.start.as_deref(), &[]);
    let Some(after) = start.ok().and_then(|st| bragd_games::replay(variant.key(), s.start.as_deref(), &[st.moves.first()?.wire.as_str()]).ok()) else { return false };
    !variant.family().race() && bragd_games::replay::<&str>(variant.key(), Some(&after.position), &[]).is_ok_and(|again| again.position == after.position)
}

/// The state before the first move, then after each: each with the entries that led to it from
/// the one before (several for go's set-up stones before its first move).
type States = Vec<(State, Vec<String>)>;

/// Each move found by the key of the position it leads to; `s.moves[i]` as the file wrote it is
/// tried first. A Family whose positions are set up from their text, so each move is tried on
/// the position alone.
fn by_keys(s: &Stored, variant: Variant, keys: &[u64], upto: usize) -> States {
    let family = variant.family().key();
    let key_of = |st: &State| position_keys(family, std::slice::from_ref(&st.position))[0];
    let replay = |entries: &[&str]| bragd_games::replay(variant.key(), s.start.as_deref(), entries).ok();
    let Some(start) = replay(&[]) else { return vec![] };
    let mut out = vec![(start, vec![])];
    let mut entries: Vec<&str> = vec![];
    for (i, &want) in keys.iter().enumerate().take(upto) {
        let before = &out[out.len() - 1].0;
        let written = s.moves.get(i).map(|m| m.trim_end_matches(['+', '#', '!', '?']));
        let mut tried: Vec<&str> = before.moves.iter().filter(|m| Some(m.notation.trim_end_matches(['+', '#'])) == written || Some(m.wire.as_str()) == written).map(|m| m.wire.as_str()).collect();
        tried.extend(before.moves.iter().map(|m| m.wire.as_str()));
        // Tried on the position alone first; a text that leaves something out (Seirawan's gating
        // rights) has every move tried on the whole Game.
        let one = tried.iter().find(|w| bragd_games::replay(variant.key(), Some(&before.position), &[**w]).is_ok_and(|a| key_of(&a) == want));
        let found = one.and_then(|w| replay(&[entries.as_slice(), &[*w]].concat()).filter(|a| key_of(a) == want).map(|a| (*w, a)));
        let found = found.or_else(|| tried.iter().find_map(|w| replay(&[entries.as_slice(), &[*w]].concat()).filter(|a| key_of(a) == want).map(|a| (*w, a))));
        let Some((w, after)) = found else { break };
        let w = w.to_string();
        out.push((after, vec![w]));
        entries = out.iter().flat_map(|(_, e)| e.iter().map(String::as_str)).collect();
    }
    out
}

/// Each entry from the start, for a Family whose positions cannot be set up from their text
/// (draughts) or whose text leaves out what the next entry needs (the dice Games): one word of
/// the stored moves or several, the longest that is an entry the rules accept (a backgammon
/// turn is several words). In a Family without dice, a move is checked by the key of the
/// position it leads to, and found by that key when its words name none.
fn by_words(s: &Stored, variant: Variant, keys: &[u64], upto: usize) -> States {
    let family = variant.family();
    let replay = |entries: &[String]| bragd_games::replay(variant.key(), s.start.as_deref(), entries).ok();
    let keyed = |st: &State, i: usize| family.race() || keys.get(i).is_some_and(|&k| position_keys(family.key(), std::slice::from_ref(&st.position))[0] == k);
    let Some(start) = replay(&[]) else { return vec![] };
    let mut out = vec![(start, vec![])];
    let mut entries: Vec<String> = vec![];
    let mut at = 0;
    while out.len() <= upto {
        let i = out.len() - 1;
        let before = &out[i].0;
        let accepted = |e: &str| before.throws.iter().any(|t| t == e) || before.moves.iter().any(|m| m.wire == e || m.notation == e);
        let words = (1..=(s.moves.len().saturating_sub(at)).min(8)).rev().map(|n| (n, s.moves[at..at + n].join(" "))).find(|(_, e)| accepted(e));
        let mut tried: Vec<(usize, String)> = words.map(|(n, e)| (n, before.moves.iter().find(|m| m.notation == e).map_or(e, |m| m.wire.clone()))).into_iter().collect();
        if !family.race() {
            tried.extend(before.moves.iter().map(|m| (1, m.wire.clone())));
        }
        let next = tried.into_iter().find_map(|(n, wire)| {
            let mut all = entries.clone();
            all.push(wire.clone());
            let after = replay(&all)?;
            keyed(&after, i).then_some((n, wire, after))
        });
        let Some((n, wire, after)) = next else { break };
        entries.push(wire.clone());
        out.push((after, vec![wire]));
        at += n;
    }
    out
}

/// A go Game from SGF: its set-up stones played first (folded into the start's entries), then
/// each `B[qd]` or `W[]` as `bragd_games`' point or pass, with a pass where the same colour
/// plays twice. Its start is always the empty board.
fn go(s: &Stored, variant: Variant, upto: usize) -> (Option<String>, States) {
    let size = variant.info().files as usize;
    let point = |sgf: &str| -> Option<String> {
        let b = sgf.as_bytes();
        if b.is_empty() {
            return Some("pass".into());
        }
        let (x, y) = (b.first()?.checked_sub(b'a')? as usize, b.get(1)?.checked_sub(b'a')? as usize);
        // Go's columns skip `i`.
        let col = (b'a'..=b'z').filter(|&c| c != b'i').nth(x)? as char;
        (x < size && y < size && b.len() == 2).then(|| format!("{col}{}", size - y))
    };
    // `B[qd]` → (black, "qd").
    let stone = |w: &str| -> Option<(bool, String)> {
        let (colour, rest) = w.split_once('[')?;
        let black = match colour {
            "B" | "AB" => true,
            "W" | "AW" => false,
            _ => return None,
        };
        Some((black, rest.strip_suffix(']')?.to_string()))
    };
    let mut setup: (Vec<String>, Vec<String>) = (vec![], vec![]);
    if let Some(start) = &s.start {
        for w in start.split(']').filter(|w| w.starts_with("AB[") || w.starts_with("AW[")) {
            let Some((black, at)) = stone(&format!("{w}]")) else { return (None, vec![]) };
            let Some(p) = point(&at) else { return (None, vec![]) };
            if black { setup.0.push(p) } else { setup.1.push(p) }
        }
    }
    let mut black_next = true;
    let mut entries: Vec<String> = vec![];
    let (mut blacks, mut whites) = (setup.0.into_iter(), setup.1.into_iter());
    let mut left = blacks.len() + whites.len();
    while left > 0 {
        let p = if black_next { blacks.next() } else { whites.next() };
        left -= usize::from(p.is_some());
        entries.push(p.unwrap_or_else(|| "pass".into()));
        black_next = !black_next;
    }
    let Ok(start) = bragd_games::replay(variant.key(), None, &entries) else { return (None, vec![]) };
    let mut out = vec![(start, entries.clone())];
    for w in s.moves.iter().take(upto) {
        let Some((black, p)) = stone(w).and_then(|(b, at)| Some((b, point(&at)?))) else { break };
        let mut these = vec![];
        if black != black_next {
            these.push("pass".to_string());
        }
        these.push(p);
        black_next = !black;
        entries.extend(these.iter().cloned());
        let Ok(after) = bragd_games::replay(variant.key(), None, &entries) else { break };
        out.push((after, these));
    }
    (None, out)
}

impl crate::library::Library {
    /// The positions of the Game `id` before its first move and after each of its first `upto`
    /// moves, as an Engine is asked about them (`asked`).
    pub fn asked(&self, id: i64, upto: usize) -> crate::library::Result<Vec<Asked>> {
        let s = self.stored(id)?;
        let mut st = self.conn().prepare_cached("SELECT hash FROM positions WHERE game_id = ?1 ORDER BY ply LIMIT ?2")?;
        let keys: Vec<u64> = st.query_map(rusqlite::params![id, upto.min(i64::MAX as usize) as i64], |r| r.get::<_, i64>(0))?.map(|k| k.map(|k| k as u64)).collect::<rusqlite::Result<_>>()?;
        Ok(asked(&s, &keys, upto))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::{Format, pgn};
    use crate::library::tests::{CENTURY, Dir, pgn as pgn_text};
    use crate::library::{Added, Library, NewGame, from_parsed};

    fn add(lib: &mut Library, g: &NewGame) -> i64 {
        match lib.add(g, "test", &format!("t{}", lib.count().unwrap())).unwrap() {
            Added::New(id) => id,
            other => panic!("{other:?}"),
        }
    }

    /// A Game Bragd played: `pick` chooses each entry from the state before it.
    fn played(variant: &str, plies: usize, pick: impl Fn(&State) -> String) -> NewGame {
        let mut entries: Vec<String> = vec![];
        for _ in 0..plies {
            let st = bragd_games::replay(variant, None, &entries).unwrap();
            entries.push(pick(&st));
        }
        let positions: Vec<String> = (0..=entries.len()).map(|k| bragd_games::replay(variant, None, &entries[..k]).unwrap().position).collect();
        let history = bragd_games::replay(variant, None, &entries).unwrap().history;
        let family = bragd_games::variant(variant).unwrap().family().key();
        NewGame::played(family, variant, None, history, &positions, vec![], "*")
    }

    /// Every stored Game of every Family with an Engine comes back whole, each position with
    /// the key the Library keeps for it, and the position an Engine is asked about is that one.
    #[test]
    fn a_stored_game_of_every_family_replays_to_its_positions() {
        let d = Dir::new("replay");
        let mut lib = Library::open(&d.0.join("r.bragd")).unwrap();
        let games = [
            NewGame::from_chess(&pgn::parse(&pgn_text(&[], CENTURY, "0-1"))),
            // Chinese notation.
            from_parsed(Format::Xiangqi, &crate::xiangqi::parse("[Game \"Chinese Chess\"]\n\n1. 炮二平五 马８进７ 2. 马二进三 车９平８ 3. 车一平二 马２进３ *")),
            // `△同　銀`: a move the Library keeps as two words.
            from_parsed(Format::Csa, &crate::shogi::parse("V2.2\nPI\n+\n+7776FU\n-3334FU\n+8822UM\n-3122GI\n+0055KA\n%TORYO\n", Format::Csa)),
            from_parsed(Format::Pdn, &crate::draughts::parse("[GameType \"20\"]\n\n1. 32-28 19-23 2. 28x19 14x23 3. 37-32 *")),
            // Two handicap stones, then White first.
            NewGame::from_go(&crate::go::parse(b"(;FF[4]GM[1]SZ[9]HA[2]AB[cc][gg];W[ee];B[ce];W[ge];B[])")),
            played("janggi", 6, |st| st.moves[st.moves.len() / 2].wire.clone()),
            played("backgammon", 9, |st| st.throws.first().cloned().unwrap_or_else(|| st.moves[0].wire.clone())),
            played("seirawan", 6, |st| st.moves[st.moves.len() / 3].wire.clone()),
        ];
        for g in &games {
            let id = add(&mut lib, g);
            let a = lib.asked(id, usize::MAX).unwrap();
            let v = g.variant.as_str();
            assert_eq!(a.len(), g.moves.len() + 1, "{v}: every move found");
            assert_eq!(a.iter().skip(1).map(|p| p.key).collect::<Vec<_>>(), g.keys, "{v}: the keys the Library keeps");
            for p in &a {
                let setup = Some(p.setup.as_str()).filter(|s| !s.is_empty());
                let st = bragd_games::replay(v, setup, &p.moves).unwrap();
                assert_eq!(position_keys(&g.family, &[st.position])[0], p.key, "{v}: asked about {p:?}");
            }
            // Only the start and the first move, when that is all that is asked.
            assert_eq!(lib.asked(id, 1).unwrap().len(), 2, "{v}");
        }
        // Go's set-up stones are played first, as a GTP engine takes no set-up.
        let go = lib.asked(5, 1).unwrap();
        assert_eq!((go[0].setup.as_str(), go[0].moves.clone()), ("", vec!["c7".to_string(), "pass".into(), "g3".into()]));
        assert_eq!(go[1].moves.last().map(String::as_str), Some("e5"));
        // A backgammon position before the roll, and after it.
        let bg = lib.asked(7, 2).unwrap();
        assert_eq!((bg[0].awaiting, bg[1].awaiting), (Awaiting::Throw, Awaiting::Move));
    }

    /// A Game of a Variant Bragd does not play gives nothing, and a move that cannot be found
    /// ends the Game there.
    #[test]
    fn what_cannot_be_played_stops() {
        let s = Stored { variant: "gametype-23".into(), family: "draughts".into(), moves: vec!["9-13".into()], ..Default::default() };
        assert!(asked(&s, &[1], 9).is_empty());
        let s = Stored { variant: "chess".into(), family: "chess".into(), moves: vec!["e4".into(), "e5".into()], ..Default::default() };
        let e4 = bragd_games::replay("chess", None, &["e2e4"]).unwrap().position;
        let a = asked(&s, &[position_keys("chess", &[e4])[0], 7], 9);
        assert_eq!(a.len(), 2);
        assert!(a[1].setup.starts_with("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b"), "{}", a[1].setup);
    }
}
