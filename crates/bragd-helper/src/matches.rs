//! Engine matches in the Helper (`Work::Match`): the engines started as the app starts them,
//! the openings from a book, the tablebases that end a chess endgame, and each Game written to
//! a Library as it ends. The match itself is `bragd_engines::matches`.

use std::path::Path;

use bragd_core::config::Config;
use bragd_engines::matches::{self, Ended, Opening, Played, Player, Run, Setup};
use bragd_engines::syzygy::Tablebases;
use bragd_engines::protocol::MATE_CP;
use bragd_engines::{lichess, local};
use bragd_games::State;
use bragd_library::chess::polyglot;
use bragd_library::library::NewGame;

/// The Source a match's Games are kept under in their Library.
pub const SOURCE: &str = "engine-match";

/// SplitMix64: the same openings for the same seed, so a match started again plays them again.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The entries of `tokens` played from `initial`, each one a move's entry or as a Player writes
/// it (`e2e4` or `e4`); None when one is not legal.
fn entries(variant: &str, initial: Option<&str>, tokens: &[&str]) -> Option<Vec<String>> {
    let mut out: Vec<String> = vec![];
    for t in tokens {
        let s = bragd_games::replay(variant, initial, &out).ok()?;
        out.push(s.moves.iter().find(|m| m.wire == *t || m.notation == *t)?.wire.clone());
    }
    Some(out)
}

/// One opening of a file of opening lines: a set-up position (FEN, SFEN; an EPD line's
/// operations after `;` left out), or moves from the start.
fn line(variant: &str, text: &str) -> Option<Opening> {
    let text = text.split(';').next()?.trim();
    if bragd_games::replay::<&str>(variant, Some(text), &[]).is_ok() {
        return Some(Opening { initial: Some(text.into()), moves: vec![] });
    }
    // Move numbers (`1.`, `1...`) are left out.
    let tokens: Vec<&str> = text.split_whitespace().filter(|t| !t.trim_end_matches('.').chars().all(|c| c.is_ascii_digit())).collect();
    Some(Opening { initial: None, moves: entries(variant, None, &tokens)? })
}

/// The openings of a match: a function from game pair to opening.
pub fn openings(setup: &Setup) -> Result<Box<dyn Fn(u32) -> Opening + Send + Sync>, String> {
    let (variant, seed, depth) = (setup.variant.clone(), setup.seed, setup.book_depth);
    let Some(book) = setup.book.clone() else {
        if variant == "chess960" {
            return Ok(Box::new(move |n| Opening { initial: Some(bragd_games::chess960_start((mix(seed ^ n as u64) % 960) as u16)), moves: vec![] }));
        }
        return Ok(Box::new(|_| Opening::default()));
    };
    if polyglot::is_book(&book) {
        if variant != "chess" {
            return Err(format!("{}: a Polyglot book has chess openings only", book.display()));
        }
        return Ok(Box::new(move |n| book_line(&book, depth, mix(seed ^ n as u64))));
    }
    let text = std::fs::read_to_string(&book).map_err(|e| format!("{}: {e}", book.display()))?;
    let mut lines = vec![];
    for (i, l) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#')) {
        lines.push(line(&variant, l).ok_or_else(|| format!("{}, line {}: not an opening of {variant}", book.display(), i + 1))?);
    }
    if lines.is_empty() {
        return Err(format!("{}: no openings in it", book.display()));
    }
    // Taken in turn, starting where the seed says.
    let start = (seed % lines.len() as u64) as usize;
    Ok(Box::new(move |n| lines[(start + n as usize) % lines.len()].clone()))
}

/// A line out of the Polyglot book at `path`, up to `depth` moves deep, each move chosen by its
/// weight with the random numbers `r` starts.
fn book_line(path: &Path, depth: u32, mut r: u64) -> Opening {
    let mut moves: Vec<String> = vec![];
    for _ in 0..depth {
        let Ok(state) = bragd_games::replay("chess", None, &moves) else { break };
        let choices = polyglot::moves_at(path, &state.position);
        let total: u64 = choices.iter().map(|m| m.weight as u64).sum();
        if total == 0 {
            break;
        }
        r = mix(r);
        let mut pick = r % total;
        let Some(m) = choices.iter().find(|m| {
            let here = pick < m.weight as u64;
            pick = pick.saturating_sub(m.weight as u64);
            here
        }) else {
            break;
        };
        let Some(e) = state.moves.iter().find(|l| l.notation == m.san) else { break };
        moves.push(e.wire.clone());
    }
    Opening { initial: None, moves }
}

/// What the tablebases say about a chess position, given the engine's last score (centipawns
/// from the first Side's view). Tables in the folder are read by the engines themselves, so a
/// position they hold is decided by the engine's score: level is a draw, a won score a win.
/// Otherwise the Lichess tablebase answers, when it is switched on.
pub fn tablebase(tb: &Tablebases, variant: &str, state: &State, score: Option<i32>) -> Option<Ended> {
    if variant != "chess" {
        return None;
    }
    let fen = &state.position;
    let first_to_move = fen.split_whitespace().nth(1) != Some("b");
    if tb.covers(fen) {
        return match score? {
            0 => Some(Ended::Draw),
            c if c >= MATE_CP => Some(Ended::First),
            c if c <= -MATE_CP => Some(Ended::Second),
            _ => None,
        };
    }
    let l = lichess::lookup(tb, &bragd_engines::fetch::Http, fen).ok()??;
    let mover_wins = match l.category.as_str() {
        "win" => true,
        "loss" => false,
        "draw" | "cursed-win" | "blessed-loss" => return Some(Ended::Draw),
        _ => return None,
    };
    Some(if mover_wins == first_to_move { Ended::First } else { Ended::Second })
}

/// The tablebases as Settings has them.
pub fn tablebases(cfg: &Config) -> Tablebases {
    Tablebases { folders: cfg.tablebase_folder().into_iter().collect(), online: cfg.tablebases.online && !bragd_core::offline() }
}

/// The engine names a match's Games are recorded under.
pub fn names(setup: &Setup) -> Vec<String> {
    setup.engines.iter().map(|k| bragd_engines::known::known(k).map_or_else(|| k.clone(), |k| k.name.to_string())).collect()
}

/// Game `index` of a match as it goes into a Library.
pub fn record(setup: &Setup, names: &[String], index: usize, game: &matches::Game, played: &Played, date: &str) -> Result<NewGame, String> {
    let family = bragd_games::variant(&setup.variant).map_err(|e| e.to_string())?.family().key();
    let n = played.history.len().min(played.entries.len());
    let positions: Vec<String> = (0..=n).map(|k| bragd_games::replay(&setup.variant, played.initial.as_deref(), &played.entries[..k]).map(|s| s.position)).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
    let mut headers: Vec<(String, String)> = vec![
        ("Event".into(), "Engine match".into()),
        ("Site".into(), "Bragd".into()),
        ("Date".into(), date.into()),
        ("Round".into(), format!("{}.{}", game.opening + 1, index % 2 + 1)),
        ("White".into(), names[game.first].clone()),
        ("Black".into(), names[game.second].clone()),
        ("Result".into(), played.result.clone()),
        ("Termination".into(), played.reason.clone()),
        ("Variant".into(), setup.variant.clone()),
    ];
    if let Some(i) = &played.initial {
        headers.push(("FEN".into(), i.clone()));
    }
    Ok(NewGame::played(family, &setup.variant, played.initial.clone(), played.history[..n].to_vec(), &positions, headers, &played.result))
}

/// Plays the match `setup` on from `results`: the results at the end. `stop` is asked between
/// moves; `done` hears each Game with its record and the results so far.
pub fn play(setup: &Setup, results: String, stop: &(dyn Fn() -> bool + Sync), done: &(dyn Fn(usize, NewGame, &str) + Sync)) -> Result<String, String> {
    let cfg = Config::load().unwrap_or_default();
    if setup.engines.len() < 2 {
        return Err("A match needs two engines or more.".into());
    }
    let chosen = setup.engines.iter().map(|k| local::resolve(&cfg, k, &setup.variant)).collect::<Result<Vec<_>, _>>()?;
    let opening = openings(setup)?;
    let games = matches::schedule(&setup.kind, setup.engines.len(), setup.rounds);
    let names = names(setup);
    let tb = tablebases(&cfg);
    let date = crate::utc(bragd_core::fs::now())[..10].replace('-', ".");
    let start = |n: usize| -> Result<Player, String> {
        let (e, notation) = local::start(&cfg, &chosen[n], &setup.variant, stop)?;
        Ok(Player { session: Box::new(e), notation })
    };
    let table = |s: &State, score: Option<i32>| tablebase(&tb, &setup.variant, s, score);
    let heard = |i: usize, p: &Played, so_far: &str| match record(setup, &names, i, &games[i], p, &date) {
        Ok(g) => done(i, g, so_far),
        Err(e) => crate::note(format_args!("a match Game could not be recorded: {e}")),
    };
    let results = if results.len() == games.len() { results } else { matches::unplayed(games.len()) };
    matches::run(&Run { setup, games: &games, opening: &*opening, start: &start, table: &table, stop, done: &heard }, results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_lines_of_any_kind() {
        let o = line("chess", "1. e4 e5 2. Nf3").unwrap();
        assert_eq!(o.moves, ["e2e4", "e7e5", "g1f3"]);
        assert_eq!(line("chess", "e2e4 c7c5").unwrap().moves, ["e2e4", "c7c5"]);
        let fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";
        assert_eq!(line("chess", &format!("{fen} ; id \"x\"")).unwrap().initial.as_deref(), Some(fen));
        assert!(line("chess", "e4 e4").is_none());
        let xq = bragd_games::replay::<&str>("xiangqi", None, &[]).unwrap().moves[0].wire.clone();
        assert_eq!(line("xiangqi", &xq).unwrap().moves, [xq]);
    }

    #[test]
    fn a_book_of_lines_is_taken_in_turn_and_checked() {
        let d = crate::tests::Dir::new("book");
        let p = d.0.join("openings.txt");
        std::fs::write(&p, "# mine\ne4 e5\n\nd4 d5\n").unwrap();
        let mut s = Setup { variant: "chess".into(), engines: vec![], kind: matches::Kind::RoundRobin, rounds: 1, limit: bragd_engines::Limit::Depth(1), parallel: 1, book: Some(p.clone()), book_depth: 8, adjudication: Default::default(), library: "x".into(), seed: 1 };
        let o = openings(&s).unwrap();
        assert_eq!([o(0).moves[0].as_str(), o(1).moves[0].as_str(), o(2).moves[0].as_str()], ["d2d4", "e2e4", "d2d4"]);
        std::fs::write(&p, "e4 e5\nnot a move\n").unwrap();
        assert!(openings(&s).err().unwrap().contains("line 2"));
        s.book = None;
        s.variant = "chess960".into();
        let o = openings(&s).unwrap();
        assert_eq!(o(3), o(3));
        assert_ne!((0..20).map(|n| o(n).initial).collect::<std::collections::BTreeSet<_>>().len(), 1);
    }

    #[test]
    fn tables_in_the_folder_decide_by_the_engines_score() {
        let d = crate::tests::Dir::new("tb");
        std::fs::write(d.0.join("KQvK.rtbw"), b"x").unwrap();
        let tb = Tablebases { folders: vec![d.0.clone()], online: false };
        let s = bragd_games::replay::<&str>("chess", Some("8/8/8/4k3/8/8/8/KQ6 w - - 0 1"), &[]).unwrap();
        assert_eq!(tablebase(&tb, "chess", &s, Some(20_000)), Some(Ended::First));
        assert_eq!(tablebase(&tb, "chess", &s, Some(0)), Some(Ended::Draw));
        assert_eq!(tablebase(&tb, "chess", &s, Some(300)), None);
        assert_eq!(tablebase(&tb, "chess", &s, None), None);
        // Not in the folder, and Lichess off: nothing known.
        let s = bragd_games::replay::<&str>("chess", Some("8/8/8/4k3/8/8/8/KR6 w - - 0 1"), &[]).unwrap();
        assert_eq!(tablebase(&tb, "chess", &s, Some(20_000)), None);
        assert_eq!(tablebase(&tb, "xiangqi", &s, Some(20_000)), None);
    }
}
