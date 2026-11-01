//! A stand-in engine for the screenshots' demo home (docs/screenshots/demo-home.sh): it speaks
//! GTP as KataGo, USI (shogi) and UCI (xiangqi) as Fairy-Stockfish and GNU Backgammon's
//! command line, whichever its first command is in. Its moves are legal, taken from the rules
//! (bragd-games, rules-backgammon); its scores are made up from the position, the same each
//! time, with now and then a big swing, so a reviewed Game has its marks.
//!
//!     cargo build --release -p bragd-engines --example fake-engine
//!
//! Nothing is read but standard input; nothing is written but standard output. When a file
//! `fake-engine.slow` lies beside it, each answer waits two seconds, so a background analysis job
//! is seen running.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{BufRead, Write};

use rules_backgammon::{Layout, Position as Board, Roll, Side};

fn main() {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    let mut lines = stdin.lock().lines().map_while(Result::ok);
    let Some(first) = lines.next() else { return };
    let mut said = |s: String| {
        let _ = out.write_all(s.as_bytes());
        let _ = out.flush();
    };
    let w: Vec<&str> = first.split_whitespace().collect();
    match w.first().copied() {
        Some("name" | "version" | "boardsize") => gtp(std::iter::once(first).chain(lines), &mut said),
        Some("usi") => uci(true, std::iter::once(first).chain(lines), &mut said),
        Some("uci") => uci(false, std::iter::once(first).chain(lines), &mut said),
        _ => gnubg(std::iter::once(first).chain(lines), &mut said),
    }
}

/// Two seconds' wait before each answer, when `fake-engine.slow` lies beside the program.
fn pause() {
    if std::env::current_exe().is_ok_and(|p| p.with_file_name("fake-engine.slow").exists()) {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

/// A score from -150 to 150 for the side to move in the position `seen`, and one position in
/// seven a swing of 600: the same position always gets the same score.
fn score(seen: &str) -> i32 {
    let mut h = DefaultHasher::new();
    seen.hash(&mut h);
    let h = h.finish();
    let base = (h % 301) as i32 - 150;
    if h.is_multiple_of(7) { base + 600 } else { base }
}

/// Up to three legal moves of `key` after `entries` from `initial`, and a line of up to six
/// moves from the first: wire entries.
fn candidates(key: &str, initial: Option<&str>, entries: &[String]) -> Vec<Vec<String>> {
    let Ok(st) = bragd_games::replay(key, initial, entries) else { return vec![] };
    // Not the first moves the rules list: a little way in, so a go line is not all corner.
    let n = st.moves.len();
    let picks: Vec<usize> = [n / 2, n / 3, n / 5].into_iter().filter(|&i| i < n).collect();
    let mut out: Vec<Vec<String>> = vec![];
    for i in picks {
        let first = st.moves[i].wire.clone();
        if out.iter().any(|l| l[0] == first) {
            continue;
        }
        let mut all = entries.to_vec();
        all.push(first.clone());
        let mut line = vec![first];
        while line.len() < 6 {
            let Ok(s) = bragd_games::replay(key, initial, &all) else { break };
            let Some(m) = s.moves.get(s.moves.len() / 2) else { break };
            line.push(m.wire.clone());
            all.push(m.wire.clone());
        }
        out.push(line);
    }
    out
}

/// GTP, as KataGo: `kata-genmove_analyze` answers with `info` lines and `play`.
fn gtp(lines: impl Iterator<Item = String>, said: &mut dyn FnMut(String)) {
    let (mut size, mut moves) = (19u32, Vec::<String>::new());
    for l in lines {
        let w: Vec<&str> = l.split_whitespace().collect();
        let ok = |s: &str| format!("= {s}\n\n");
        match w.first().copied() {
            Some("name") => said(ok("KataGo")),
            Some("version") => said(ok("1.18.2")),
            Some("boardsize") => {
                size = w.get(1).and_then(|s| s.parse().ok()).unwrap_or(19);
                said(ok(""));
            }
            Some("clear_board") => {
                moves.clear();
                said(ok(""));
            }
            Some("play") => {
                moves.push(w.get(2).unwrap_or(&"pass").to_ascii_lowercase());
                said(ok(""));
            }
            Some("kata-genmove_analyze") => {
                pause();
                let key = format!("go-{size}");
                let lines = candidates(&key, None, &moves);
                let mut s = String::from("=\n");
                let base = score(&format!("{key} {}", moves.join(" ")));
                for (k, line) in lines.iter().enumerate() {
                    let lead = (base - 40 * k as i32) as f32 / 30.0;
                    let winrate = (0.5 + lead / 40.0).clamp(0.02, 0.98);
                    let pv: Vec<String> = line.iter().map(|m| m.to_ascii_uppercase()).collect();
                    s.push_str(&format!("info move {} visits {} winrate {winrate:.4} scoreLead {lead:.2} order {k} pv {}\n", pv[0], 400 - 100 * k, pv.join(" ")));
                }
                let best = lines.first().map_or("pass".to_string(), |l| l[0].to_ascii_uppercase());
                s.push_str(&format!("play {best}\n\n"));
                said(s);
            }
            Some("quit") => {
                said(ok(""));
                return;
            }
            Some(_) => said(ok("")),
            None => {}
        }
    }
}

/// UCI for xiangqi, or USI for shogi, as Fairy-Stockfish.
fn uci(usi: bool, lines: impl Iterator<Item = String>, said: &mut dyn FnMut(String)) {
    let mut key = if usi { "shogi".to_string() } else { "chess".to_string() };
    let (mut initial, mut moves): (Option<String>, Vec<String>) = (None, vec![]);
    for l in lines {
        let w: Vec<&str> = l.split_whitespace().collect();
        match w.first().copied() {
            Some("usi") => said("id name Fairy-Stockfish 14\nid author demo\nusiok\n".into()),
            Some("uci") => said("id name Fairy-Stockfish 14\nid author demo\noption name UCI_Variant type combo default chess var chess var xiangqi\noption name MultiPV type spin default 1 min 1 max 5\nuciok\n".into()),
            Some("isready") => said("readyok\n".into()),
            Some("setoption") if l.contains("UCI_Variant") => key = w.last().unwrap_or(&"chess").to_string(),
            Some("position") => {
                let at = w.iter().position(|x| *x == "moves");
                moves = at.map_or(vec![], |i| w[i + 1..].iter().map(|m| m.to_string()).collect());
                initial = match w.get(1).copied() {
                    Some("fen" | "sfen") => Some(w[2..at.unwrap_or(w.len())].join(" ")),
                    _ => None,
                };
            }
            Some("go") => {
                pause();
                let lines = candidates(&key, initial.as_deref(), &moves);
                let base = score(&format!("{key} {initial:?} {}", moves.join(" ")));
                let mut s = String::new();
                for (k, line) in lines.iter().enumerate() {
                    s.push_str(&format!("info depth 18 multipv {} score cp {} nodes 120000 pv {}\n", k + 1, base - 40 * k as i32, line.join(" ")));
                }
                s.push_str(&format!("bestmove {}\n", lines.first().map_or(if usi { "resign" } else { "0000" }, |l| l[0].as_str())));
                said(s);
            }
            Some("quit") => return,
            _ => {}
        }
    }
}

/// GNU Backgammon's own command line: `eval` before the roll, `hint` for a roll, each answer
/// ended by `show prompt`. The board is read back from its Position ID.
fn gnubg(lines: impl Iterator<Item = String>, said: &mut dyn FnMut(String)) {
    let (mut board, mut dice): (Option<String>, Option<Roll>) = (None, None);
    for l in lines {
        let w: Vec<&str> = l.split_whitespace().collect();
        match (w.first().copied(), w.get(1).copied()) {
            (Some("show"), Some("version")) => said("GNU Backgammon 1.08.003 (demo)\n".into()),
            (Some("set"), Some("prompt")) => said("The prompt has been set to `bragd>'.\n".into()),
            (Some("show"), Some("prompt")) => said("bragd>The prompt is set to `bragd>'.\n".into()),
            (Some("new"), _) => (board, dice) = (None, None),
            (Some("set"), Some("board")) => board = w.get(2).map(|s| s.to_string()),
            (Some("set"), Some("dice")) => dice = w.get(2).zip(w.get(3)).and_then(|(a, b)| Roll::new(a.parse().ok()?, b.parse().ok()?)),
            (Some("eval"), _) => {
                pause();
                let e = score(board.as_deref().unwrap_or("")) as f32 / 1000.0;
                said(format!("bragd> 2 ply: 0.525 0.150 0.007 0.125 0.005 {e:+.3} {e:+.3}\n"));
            }
            (Some("hint"), _) => {
                pause();
                let n: usize = w.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
                let Some((b, roll)) = board.as_deref().and_then(from_position_id).zip(dice) else { continue };
                let id = board.clone().unwrap_or_default();
                let mut s = String::new();
                for (k, t) in b.turns_for(roll).iter().take(n.min(3)).enumerate() {
                    let e = score(&format!("{id} {t}")) as f32 / 1000.0 - 0.04 * k as f32;
                    s.push_str(&format!("bragd>    {}. Cubeful 2-ply    {t}    Eq.:  {e:+.3}\n", k + 1));
                }
                said(s);
            }
            (Some("quit"), _) => return,
            _ => {}
        }
    }
}

/// The board of a GNU Backgammon Position ID, the player on roll as White: each side's points 1
/// to 24 in its own numbering and its bar, as many 1 bits as checkers and a 0, the player not
/// on roll first.
fn from_position_id(id: &str) -> Option<Board> {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = vec![];
    for c in id.bytes() {
        let v = A.iter().position(|a| *a == c)? as u32;
        bits.extend((0..6).rev().map(|k| v >> k & 1));
    }
    // Base64 is big-endian by six; the ID's bytes are little-endian by bit.
    let bytes: Vec<u8> = bits.chunks(8).filter(|c| c.len() == 8).map(|c| c.iter().fold(0u8, |a, b| a << 1 | *b as u8)).collect();
    let mut runs = vec![];
    let mut n = 0u8;
    for i in 0..bytes.len() * 8 {
        if bytes[i / 8] >> (i % 8) & 1 == 1 {
            n += 1;
        } else {
            runs.push(n);
            n = 0;
            if runs.len() == 50 {
                break;
            }
        }
    }
    let side = |r: &[u8]| -> (Vec<(u8, u8)>, u8, u8) {
        let points: Vec<(u8, u8)> = (0..24).filter(|&i| r[i] > 0).map(|i| (i as u8 + 1, r[i])).collect();
        let on: u8 = r.iter().sum();
        (points, r[24], 15u8.saturating_sub(on))
    };
    let (other, roll) = (side(runs.get(..25)?), side(runs.get(25..50)?));
    Board::from_parts(Layout { points: &roll.0, bar: roll.1, off: roll.2 }, Layout { points: &other.0, bar: other.1, off: other.2 }, Side::White)
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_the_opening_position_id() {
        let b = super::from_position_id("4HPwATDgc/ABMA").unwrap();
        assert_eq!(b, super::Board::starting());
    }
}
