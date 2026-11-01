//! A player's summary over a Library: their Games, results by side, score, rating over time,
//! most played openings, best wins and worst losses, and first and last Game, for each Game
//! family they play in. Read through the White and Black indexes (`games_white`, `games_black`),
//! so the work grows with the player's Games, not with the Library.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use serde::Serialize;

use crate::library::{Library, Result};

/// How many openings, wins and losses a summary lists.
pub const TOP: usize = 5;

/// Wins, draws and losses, and Games with no result yet (`*`, or one Bragd does not read).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Record {
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub unfinished: u32,
}

/// One Game, as the player's summary names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameRef {
    pub id: i64,
    pub date: String,
    pub opponent: String,
    pub opponent_elo: Option<i64>,
    pub result: String,
    /// Whether the player had the `white` column's side (White, Red, Sente: the first named).
    pub first_side: bool,
}

/// An ECO code the player played, with the opening name most often given for it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Opening {
    pub eco: String,
    pub name: String,
    pub record: Record,
}

/// What the player did in one Game family. A list or value the Family's Games do not give
/// (ratings, ECO codes, dates) is empty or None, so the page leaves it out.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilySummary {
    pub family: String,
    pub games: u32,
    /// As the side of the `white` column (White, Red, Sente), and of the `black` column.
    pub first_side: Record,
    pub second_side: Record,
    /// Points in percent of the finished Games (a win 1, a draw ½); None when none finished.
    pub score: Option<f64>,
    /// The player's rating by date, oldest first: Games with a rating and a known year.
    pub ratings: Vec<(String, i64)>,
    /// The ECO codes played most, at most `TOP`, as each side, most Games first, then by code;
    /// chess only.
    pub openings_first: Vec<Opening>,
    pub openings_second: Vec<Opening>,
    /// Wins against the highest rated, and losses against the lowest rated: at most `TOP` each,
    /// of Games whose opponent has a rating.
    pub best_wins: Vec<GameRef>,
    pub worst_losses: Vec<GameRef>,
    /// The first and last Game of a known year.
    pub first: Option<GameRef>,
    pub last: Option<GameRef>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlayerSummary {
    pub name: String,
    /// By Family id; none when the name plays no Game here.
    pub families: Vec<FamilySummary>,
}

/// The player's Games, each once: as White, then as Black (not their own as White again).
/// Columns: id, family, date, white, black, white_elo, black_elo, eco, opening, result.
pub(crate) const GAMES: &str = "SELECT id, family, date, white, black, white_elo, black_elo, eco, opening, result FROM games WHERE white = ?1
     UNION ALL
     SELECT id, family, date, white, black, white_elo, black_elo, eco, opening, result FROM games WHERE black = ?1 AND white != ?1";


/// Who won: Some(true) the `white` column's side, Some(false) the other, None a draw; Err for
/// a Game with no result Bragd reads. PGN and PDN scores, and go's `B+…` and `W+…`.
#[allow(clippy::result_unit_err, reason = "an unread result is one case, with nothing to say about it")]
pub fn winner(result: &str) -> std::result::Result<Option<bool>, ()> {
    match result {
        "1-0" | "2-0" => Ok(Some(true)),
        "0-1" | "0-2" => Ok(Some(false)),
        "1/2-1/2" | "½-½" | "1-1" | "0" => Ok(None),
        r if r.eq_ignore_ascii_case("draw") || r.eq_ignore_ascii_case("jigo") => Ok(None),
        r if r.starts_with("W+") => Ok(Some(true)),
        r if r.starts_with("B+") => Ok(Some(false)),
        _ => Err(()),
    }
}

/// A Game's headers as a player's summary reads them, from a Library or a Foreign database.
/// `id` is what opens it again.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seen {
    pub id: i64,
    pub family: String,
    pub date: String,
    pub white: String,
    pub black: String,
    pub white_elo: Option<i64>,
    pub black_elo: Option<i64>,
    pub eco: String,
    pub opening: String,
    pub result: String,
}

/// The summary of `name` over `games`: those they played, as either side, by Game family.
pub fn summary(name: &str, games: impl IntoIterator<Item = Seen>) -> PlayerSummary {
    let mut by_family: BTreeMap<String, Vec<Played>> = BTreeMap::new();
    for g in games.into_iter().filter(|g| g.white == name || g.black == name) {
        let first_side = g.white == name;
        let (elo, opponent_elo, opponent) = if first_side { (g.white_elo, g.black_elo, g.black) } else { (g.black_elo, g.white_elo, g.white) };
        let p = Played { id: g.id, family: g.family, date: g.date, opponent, opponent_elo, elo, eco: g.eco, opening: g.opening, result: g.result, first_side };
        by_family.entry(p.family.clone()).or_default().push(p);
    }
    let families = by_family.into_iter().map(|(family, games)| family_summary(family, games)).collect();
    PlayerSummary { name: name.to_string(), families }
}

/// One of the player's Games. `elo` and `opponent_elo` are the player's
/// and the opponent's rating.
struct Played {
    id: i64,
    family: String,
    date: String,
    opponent: String,
    opponent_elo: Option<i64>,
    elo: Option<i64>,
    eco: String,
    opening: String,
    result: String,
    first_side: bool,
}

impl Played {
    fn game_ref(&self) -> GameRef {
        GameRef {
            id: self.id,
            date: self.date.clone(),
            opponent: self.opponent.clone(),
            opponent_elo: self.opponent_elo,
            result: self.result.clone(),
            first_side: self.first_side,
        }
    }

    fn known_year(&self) -> bool {
        self.date.get(..4).is_some_and(|y| y.bytes().all(|b| b.is_ascii_digit()))
    }
}

fn count(r: &Record) -> u32 {
    r.wins + r.draws + r.losses + r.unfinished
}

/// Adds a Game to a side's record; `first_side` is whether the player had that side.
fn add_result(rec: &mut Record, won: std::result::Result<Option<bool>, ()>, first_side: bool) {
    match won {
        Err(()) => rec.unfinished += 1,
        Ok(None) => rec.draws += 1,
        Ok(Some(w)) if w == first_side => rec.wins += 1,
        Ok(Some(_)) => rec.losses += 1,
    }
}

/// The ECO codes one side played, most Games first, then by code, at most `TOP`.
fn openings(games: &[Played], first_side: bool) -> Vec<Opening> {
    let mut by_eco: BTreeMap<&str, (Record, Vec<(&str, u32)>)> = BTreeMap::new();
    for g in games.iter().filter(|g| g.first_side == first_side && !g.eco.is_empty()) {
        let (rec, names) = by_eco.entry(&g.eco).or_default();
        add_result(rec, winner(&g.result), g.first_side);
        if !g.opening.is_empty() {
            match names.iter_mut().find(|(n, _)| *n == g.opening) {
                Some((_, c)) => *c += 1,
                None => names.push((&g.opening, 1)),
            }
        }
    }
    let mut list: Vec<Opening> = by_eco
        .into_iter()
        .map(|(eco, (record, names))| {
            // max_by_key keeps the last of equal maxima, so walk the names backwards to keep the first seen.
            let name = names.iter().rev().max_by_key(|n| n.1).map(|n| n.0).unwrap_or_default();
            Opening { eco: eco.to_string(), name: name.to_string(), record }
        })
        .collect();
    list.sort_by(|a, b| count(&b.record).cmp(&count(&a.record)).then_with(|| a.eco.cmp(&b.eco)));
    list.truncate(TOP);
    list
}

fn family_summary(family: String, mut games: Vec<Played>) -> FamilySummary {
    games.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
    let (mut first_side, mut second_side) = (Record::default(), Record::default());
    for g in &games {
        let rec = if g.first_side { &mut first_side } else { &mut second_side };
        add_result(rec, winner(&g.result), g.first_side);
    }
    let wins = first_side.wins + second_side.wins;
    let draws = first_side.draws + second_side.draws;
    let losses = first_side.losses + second_side.losses;
    let finished = wins + draws + losses;
    let score = (finished > 0).then(|| (wins as f64 + draws as f64 / 2.0) / finished as f64 * 100.0);

    let known: Vec<&Played> = games.iter().filter(|g| g.known_year()).collect();
    let ratings = known.iter().filter_map(|g| g.elo.map(|e| (g.date.clone(), e))).collect();

    let mut best_wins: Vec<&Played> = games.iter().filter(|g| g.opponent_elo.is_some() && winner(&g.result) == Ok(Some(g.first_side))).collect();
    best_wins.sort_by_key(|g| Reverse(g.opponent_elo));
    let mut worst_losses: Vec<&Played> = games.iter().filter(|g| g.opponent_elo.is_some() && winner(&g.result) == Ok(Some(!g.first_side))).collect();
    worst_losses.sort_by_key(|g| g.opponent_elo);

    let chess = family == "chess";
    FamilySummary {
        family,
        games: games.len() as u32,
        first_side,
        second_side,
        score,
        ratings,
        openings_first: if chess { openings(&games, true) } else { Vec::new() },
        openings_second: if chess { openings(&games, false) } else { Vec::new() },
        best_wins: best_wins.iter().take(TOP).map(|g| g.game_ref()).collect(),
        worst_losses: worst_losses.iter().take(TOP).map(|g| g.game_ref()).collect(),
        first: known.first().map(|g| g.game_ref()),
        last: known.last().map(|g| g.game_ref()),
    }
}

impl Library {
    /// The summary of the player named exactly `name`, over every Family the Library holds.
    pub fn player(&self, name: &str) -> Result<PlayerSummary> {
        let mut st = self.conn().prepare(GAMES)?;
        let games = st.query_map([name], |r| {
            Ok(Seen {
                id: r.get(0)?,
                family: r.get(1)?,
                date: r.get(2)?,
                white: r.get(3)?,
                black: r.get(4)?,
                white_elo: r.get(5)?,
                black_elo: r.get(6)?,
                eco: r.get(7)?,
                opening: r.get(8)?,
                result: r.get(9)?,
            })
        })?;
        Ok(summary(name, games.collect::<rusqlite::Result<Vec<_>>>()?))
    }

    /// The players with a word of their name starting with each word of `text`, in any case
    /// and with or without accents ("garry" finds "Kasparov, Garry"), with how many Games each
    /// has here, most Games first, at most `limit`. The first search after Games came or went
    /// makes the name index again.
    pub fn players(&self, text: &str, limit: usize) -> Result<Vec<(String, u32)>> {
        let words: Vec<String> = text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| format!("\"{w}\"*")).collect();
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let fresh: bool = self.conn().query_row("SELECT EXISTS (SELECT 1 FROM player_names_fresh)", [], |r| r.get(0))?;
        if !fresh {
            self.conn().execute_batch(NAMES_AGAIN)?;
        }
        let mut st = self.conn().prepare_cached(FIND)?;
        let found = st.query_map(rusqlite::params![words.join(" "), limit as i64], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(found.collect::<rusqlite::Result<_>>()?)
    }
}

/// The names whose words match `?1`, with their Games, most Games first, at most `?2`.
pub(crate) const FIND: &str = "SELECT n.name, n.games FROM player_words w JOIN player_names n ON n.rowid = w.rowid WHERE player_words MATCH ?1 ORDER BY n.games DESC, n.name LIMIT ?2";

/// Every name in the `white` and `black` columns with its Games, and the index of their words.
const NAMES_AGAIN: &str = "BEGIN;
    DELETE FROM player_names;
    INSERT INTO player_names (name, games) SELECT name, count(*) FROM (
        SELECT white AS name FROM games
        UNION ALL
        SELECT black FROM games WHERE black != white
    ) WHERE name != '' GROUP BY name;
    INSERT INTO player_words (player_words) VALUES ('rebuild');
    DELETE FROM player_names_fresh;
    INSERT INTO player_names_fresh VALUES (1);
    COMMIT;";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::pgn;
    use crate::library::NewGame;
    use crate::library::tests::{Dir, pgn as pgn_text};
    use rusqlite::StatementStatus;

    fn add(lib: &mut Library, tags: &[(&str, &str)], moves: &str, result: &str) {
        let g = pgn::parse(&pgn_text(tags, moves, result));
        let n = lib.count().unwrap() + 1;
        lib.add(&NewGame::from_chess(&g), "test", &format!("t#{n}")).unwrap();
    }

    /// Kasparov's Games: five chess, of which one unfinished, and one go Game of his.
    fn library(d: &Dir) -> Library {
        let mut lib = Library::open(&d.0.join("lib.bragd")).unwrap();
        let k = "Kasparov, Garry";
        add(&mut lib, &[("White", k), ("Black", "Karpov, Anatoly"), ("ECO", "B44"), ("Opening", "Sicilian"), ("Date", "1985.10.15"), ("WhiteElo", "2700"), ("BlackElo", "2720")], "1. e4 c5", "1-0");
        add(&mut lib, &[("White", "Karpov, Anatoly"), ("Black", k), ("ECO", "D85"), ("Opening", "Grünfeld"), ("Date", "1990.10.08"), ("WhiteElo", "2730"), ("BlackElo", "2800")], "1. d4 Nf6", "1/2-1/2");
        add(&mut lib, &[("White", k), ("Black", "Deep Blue"), ("ECO", "B44"), ("Opening", "Sicilian, Taimanov"), ("Date", "1996.02.10"), ("WhiteElo", "2795")], "1. e4 c5 2. Nf3", "0-1");
        add(&mut lib, &[("White", "Topalov, Veselin"), ("Black", k), ("ECO", "B06"), ("Date", "1999.01.20"), ("WhiteElo", "2700"), ("BlackElo", "2812")], "1. e4 d6", "0-1");
        add(&mut lib, &[("White", k), ("Black", "Short, Nigel"), ("Date", "????.??.??")], "1. c4", "*");
        add(&mut lib, &[("White", "Anand, Viswanathan"), ("Black", "Short, Nigel"), ("ECO", "C65"), ("Date", "1992")], "1. e4 e5", "1-0");
        let go = NewGame::played("go", "go-19", None, vec!["B[pd]".into()], &["a".into(), "b".into()], vec![("Black".into(), k.into()), ("White".into(), "Cho, Chikun".into()), ("Date".into(), "2000-01-01".into())], "B+R");
        lib.add(&go, "test", "go#1").unwrap();
        lib
    }

    #[test]
    fn results_are_read_in_every_family() {
        assert_eq!(winner("1-0"), Ok(Some(true)));
        assert_eq!(winner("0-1"), Ok(Some(false)));
        assert_eq!(winner("1/2-1/2"), Ok(None));
        assert_eq!(winner("½-½"), Ok(None), "as a Foreign database lists it");
        assert_eq!(winner("2-0"), Ok(Some(true)));
        assert_eq!(winner("0-2"), Ok(Some(false)));
        assert_eq!(winner("1-1"), Ok(None));
        assert_eq!(winner("W+7.5"), Ok(Some(true)));
        assert_eq!(winner("B+R"), Ok(Some(false)));
        assert_eq!(winner("0"), Ok(None));
        assert_eq!(winner("Draw"), Ok(None));
        assert_eq!(winner("*"), Err(()));
        assert_eq!(winner(""), Err(()));
    }

    #[test]
    fn a_players_summary_says_what_the_games_say() {
        let d = Dir::new("player");
        let lib = library(&d);
        let s = lib.player("Kasparov, Garry").unwrap();
        assert_eq!(s.families.iter().map(|f| f.family.as_str()).collect::<Vec<_>>(), ["chess", "go"]);
        let c = &s.families[0];
        assert_eq!(c.games, 5);
        assert_eq!(c.first_side, Record { wins: 1, draws: 0, losses: 1, unfinished: 1 });
        assert_eq!(c.second_side, Record { wins: 1, draws: 1, losses: 0, unfinished: 0 });
        assert_eq!(c.score, Some(62.5));
        assert_eq!(c.ratings, [("1985.10.15".to_string(), 2700), ("1990.10.08".into(), 2800), ("1996.02.10".into(), 2795), ("1999.01.20".into(), 2812)]);
        assert_eq!(c.openings_first.len(), 1);
        assert_eq!((c.openings_first[0].eco.as_str(), c.openings_first[0].record.wins, c.openings_first[0].record.losses), ("B44", 1, 1));
        assert_eq!(c.openings_second.iter().map(|o| o.eco.as_str()).collect::<Vec<_>>(), ["B06", "D85"]);
        assert_eq!(c.openings_second.iter().find(|o| o.eco == "D85").unwrap().name, "Grünfeld");
        // Wins, against the highest rated first; the loss to Deep Blue has no opponent rating.
        assert_eq!(c.best_wins.iter().map(|g| g.opponent.as_str()).collect::<Vec<_>>(), ["Karpov, Anatoly", "Topalov, Veselin"]);
        assert!(c.worst_losses.is_empty());
        assert_eq!(c.first.as_ref().map(|g| g.date.as_str()), Some("1985.10.15"));
        assert_eq!(c.last.as_ref().map(|g| (g.opponent.as_str(), g.first_side)), Some(("Topalov, Veselin", false)));
        // Go: Kasparov had Black and won; go has no ECO and no Elo here.
        let g = &s.families[1];
        assert_eq!((g.games, g.second_side.wins, g.score), (1, 1, Some(100.0)));
        assert!(g.openings_first.is_empty() && g.openings_second.is_empty() && g.ratings.is_empty());
        // Nobody of that name: nothing.
        assert!(lib.player("Kasparov").unwrap().families.is_empty());
    }

    #[test]
    fn players_are_found_by_the_start_of_any_word_of_their_name() {
        let d = Dir::new("player-find");
        let mut lib = library(&d);
        assert_eq!(lib.players("Garry", 10).unwrap(), [("Kasparov, Garry".to_string(), 6)]);
        assert_eq!(lib.players("gar kasp", 10).unwrap(), [("Kasparov, Garry".to_string(), 6)]);
        assert!(lib.players("Garry Karpov", 10).unwrap().is_empty(), "every word must match");
        assert_eq!(lib.players("visw", 10).unwrap(), [("Anand, Viswanathan".to_string(), 1)]);
        // Games that come after the first search are found, and counted.
        add(&mut lib, &[("White", "Polgár, Judit"), ("Black", "Short, Nigel")], "1. e4", "1-0");
        assert_eq!(lib.players("polgar", 10).unwrap(), [("Polgár, Judit".to_string(), 1)], "accents aside");
        assert_eq!(lib.players("nigel", 10).unwrap(), [("Short, Nigel".to_string(), 3)]);
        // A name changed by merging headers again is found by its new name.
        lib.conn().execute("UPDATE games SET white = 'Polgar, Judit' WHERE white = 'Polgár, Judit'", []).unwrap();
        assert_eq!(lib.players("judit", 10).unwrap(), [("Polgar, Judit".to_string(), 1)]);
        assert!(lib.players("\"*", 10).unwrap().is_empty(), "search syntax is only text");
        assert_eq!(lib.players("Kas", 10).unwrap(), [("Kasparov, Garry".to_string(), 6)]);
        assert_eq!(lib.players("kas", 10).unwrap(), [("Kasparov, Garry".to_string(), 6)]);
        assert_eq!(lib.players("Short", 10).unwrap(), [("Short, Nigel".to_string(), 3)]);
        assert_eq!(lib.players("K", 1).unwrap().len(), 1);
        assert_eq!(lib.players("K", 10).unwrap().iter().map(|p| p.0.as_str()).collect::<Vec<_>>(), ["Kasparov, Garry", "Karpov, Anatoly"]);
        assert!(lib.players("", 10).unwrap().is_empty());
        assert!(lib.players("Zz", 10).unwrap().is_empty());
    }

    /// The SQLite steps a statement takes for `args`.
    fn steps(lib: &Library, sql: &str, args: &[&dyn rusqlite::ToSql]) -> i32 {
        let mut st = lib.conn().prepare(sql).unwrap();
        let n = st.query(args).unwrap().mapped(|_| Ok(())).count();
        assert!(n > 0);
        st.get_status(StatementStatus::VmStep)
    }

    /// The work is the player's Games: 300 Games of others add no step, and no table is scanned.
    #[test]
    fn a_summary_reads_only_the_players_games() {
        let d = Dir::new("player-work");
        let mut lib = library(&d);
        let summary = |lib: &Library| steps(lib, GAMES, &[&"Kasparov, Garry"]);
        let names = |lib: &Library| {
            lib.players("Kas", 10).unwrap();
            steps(lib, FIND, &[&"\"Kas\"*", &10])
        };
        let before = (summary(&lib), names(&lib));
        // 300 Games of moves no other Game has, so none is merged as a Duplicate.
        let w = ["a3", "a4", "b3", "b4", "c3", "c4", "d3", "d4", "e3", "e4", "f3", "f4", "g3", "g4", "h3", "h4", "Nc3", "Nf3", "Na3", "Nh3"];
        let b = ["a6", "a5", "b6", "b5", "c6", "c5", "d6", "d5", "e6", "e5", "f6", "f5", "g6", "g5", "h6", "h5", "Nc6", "Nf6", "Na6", "Nh6"];
        let others = (0..400).map(|i| (w[i % 20], b[i / 20])).filter(|(w, _)| !matches!(*w, "e4" | "d4")).take(300);
        for (i, (w, b)) in others.enumerate() {
            add(&mut lib, &[("White", &format!("Other {i}")), ("Black", &format!("Rival {i}"))], &format!("1. {w} {b}"), "1-0");
        }
        assert_eq!(lib.count().unwrap(), 307);
        assert_eq!((summary(&lib), names(&lib)), before);
        let mut st = lib.conn().prepare(GAMES).unwrap();
        assert_eq!(st.query(["Kasparov, Garry"]).unwrap().mapped(|_| Ok(())).count(), 6);
        assert_eq!(st.get_status(StatementStatus::FullscanStep), 0);
    }
}
