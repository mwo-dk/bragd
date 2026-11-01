//! Slices: named queries over a Library, such as "Grünfeld" or "both players 2500+". A Slice is
//! evaluated live each time it is asked for. As an option it is materialised: its Games, their
//! Sources, positions and Near-duplicates copied into a Library file of its own, to edit apart.

use std::io;
use std::path::{Path, PathBuf};

use rusqlite::params;
use rusqlite::types::Value;
use serde::{Deserialize, Serialize};

use crate::library::{Library, Result};

/// What a Slice lets through. Every field set must hold; an empty field lets everything through.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Query {
    /// Games with any of these players, as White or Black (a part of the name, any case).
    pub players: Vec<String>,
    /// ECO codes from and to, both included: `D70` to `D99` is the Grünfeld.
    pub eco_from: Option<String>,
    pub eco_to: Option<String>,
    /// Dates from and to, both included, as far as given: `1990`, `1990.06`, `1990.06.15`.
    /// Games of no known year are left out when either is set.
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    /// Both players rated at least this.
    pub rating_min: Option<u32>,
    /// `1-0`, `0-1`, `1/2-1/2`, …
    pub result: Option<String>,
    pub family: Option<String>,
    pub variant: Option<String>,
    /// Games that reach this position (its key: Polyglot for chess, `library::NewGame::keys`).
    pub position: Option<u64>,
}

/// A Slice as a Library keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slice {
    pub name: String,
    pub query: Query,
    pub materialised: bool,
    /// The Library file it was copied to, when materialised.
    pub path: Option<PathBuf>,
}

/// `q` as SQL over `games`: the condition, and its values in order.
fn condition(q: &Query) -> (String, Vec<Value>) {
    let mut parts: Vec<String> = vec![];
    let mut args: Vec<Value> = vec![];
    let names: Vec<&str> = q.players.iter().map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
    if !names.is_empty() {
        let any: Vec<&str> = names.iter().map(|_| "(white LIKE ? ESCAPE '\\' OR black LIKE ? ESCAPE '\\')").collect();
        parts.push(format!("({})", any.join(" OR ")));
        for n in names {
            let like = format!("%{}%", n.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
            args.extend([Value::Text(like.clone()), Value::Text(like)]);
        }
    }
    let mut text = |sql: &str, v: &Option<String>| {
        if let Some(v) = v.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            parts.push(sql.to_string());
            args.push(Value::Text(v.to_string()));
        }
    };
    text("(eco != '' AND eco >= ?)", &q.eco_from);
    text("(eco != '' AND eco <= ?)", &q.eco_to);
    text("result = ?", &q.result);
    text("family = ?", &q.family);
    text("variant = ?", &q.variant);
    // A date bound compares as far as it is given, so `1999` takes in all of 1999. From a
    // date, a plain `date >= ?` says the same and can use the index.
    if let Some(v) = q.date_from.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        parts.push("(date GLOB '[0-9][0-9][0-9][0-9]*' AND date >= ?)".into());
        args.push(Value::Text(v.to_string()));
    }
    if let Some(v) = q.date_to.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        parts.push(format!("(date GLOB '[0-9][0-9][0-9][0-9]*' AND substr(date, 1, {}) <= ?)", v.chars().count()));
        args.push(Value::Text(v.to_string()));
    }
    if let Some(min) = q.rating_min {
        parts.push("(white_elo >= ? AND black_elo >= ?)".into());
        args.extend([Value::Integer(min as i64), Value::Integer(min as i64)]);
    }
    if let Some(key) = q.position {
        parts.push("id IN (SELECT game_id FROM positions WHERE hash = ?)".into());
        args.push(Value::Integer(key as i64));
    }
    let sql = if parts.is_empty() { "1".to_string() } else { parts.join(" AND ") };
    (sql, args)
}

impl Library {
    /// The Games `q` lets through, by id.
    pub fn find(&self, q: &Query) -> Result<Vec<i64>> {
        let (cond, args) = condition(q);
        let mut st = self.conn().prepare(&format!("SELECT id FROM games WHERE {cond} ORDER BY id"))?;
        let ids = st.query_map(rusqlite::params_from_iter(args), |r| r.get(0))?;
        Ok(ids.collect::<rusqlite::Result<_>>()?)
    }

    /// Keeps the Slice `name` (a new query for an old name replaces it, live again).
    pub fn save_slice(&self, name: &str, q: &Query) -> Result<()> {
        let json = serde_json::to_string(q).map_err(io::Error::other)?;
        self.conn().execute(
            "INSERT INTO slices (name, query, materialised, path) VALUES (?1, ?2, 0, NULL)
             ON CONFLICT (name) DO UPDATE SET query = ?2, materialised = 0, path = NULL",
            params![name, json],
        )?;
        Ok(())
    }

    /// Every Slice, by name.
    pub fn slices(&self) -> Result<Vec<Slice>> {
        let mut st = self.conn().prepare("SELECT name, query, materialised, path FROM slices ORDER BY name")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, bool>(2)?, r.get::<_, Option<String>>(3)?)))?;
        let mut out = vec![];
        for row in rows {
            let (name, query, materialised, path) = row?;
            let query = serde_json::from_str(&query).map_err(io::Error::other)?;
            out.push(Slice { name, query, materialised, path: path.map(PathBuf::from) });
        }
        Ok(out)
    }

    /// The Slice `name`, if there is one.
    pub fn slice(&self, name: &str) -> Result<Option<Slice>> {
        Ok(self.slices()?.into_iter().find(|s| s.name == name))
    }

    /// The Games of the Slice `name` as the Library is now.
    pub fn slice_games(&self, name: &str) -> Result<Vec<i64>> {
        let s = self.slice(name)?.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no Slice named {name}")))?;
        self.find(&s.query)
    }

    /// Copies the Games of the Slice `name` into a new Library at `out`, with their Sources,
    /// positions and the Near-duplicates among them, and notes the Slice as kept there. `out`
    /// must not exist yet.
    pub fn materialise(&self, name: &str, out: &Path) -> Result<Library> {
        let s = self.slice(name)?.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no Slice named {name}")))?;
        if out.exists() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} exists", out.display())).into());
        }
        // Made with its tables, then filled from here.
        drop(Library::open(out)?);
        let (cond, args) = condition(&s.query);
        let c = self.conn();
        c.execute("DROP TABLE IF EXISTS temp.slice_ids", [])?;
        c.execute(&format!("CREATE TEMP TABLE slice_ids AS SELECT id FROM games WHERE {cond}"), rusqlite::params_from_iter(args))?;
        c.execute("ATTACH DATABASE ? AS slice_out", [out.to_string_lossy()])?;
        let copied = c.execute_batch(
            "BEGIN;
             INSERT INTO slice_out.games SELECT * FROM main.games WHERE id IN (SELECT id FROM temp.slice_ids);
             INSERT INTO slice_out.game_sources SELECT * FROM main.game_sources WHERE game_id IN (SELECT id FROM temp.slice_ids);
             INSERT INTO slice_out.positions SELECT * FROM main.positions WHERE game_id IN (SELECT id FROM temp.slice_ids);
             INSERT INTO slice_out.near_duplicates SELECT * FROM main.near_duplicates
                 WHERE short_id IN (SELECT id FROM temp.slice_ids) AND long_id IN (SELECT id FROM temp.slice_ids);
             COMMIT;",
        );
        if copied.is_err() {
            let _ = c.execute_batch("ROLLBACK");
        }
        let _ = c.execute("DROP TABLE IF EXISTS temp.slice_ids", []);
        c.execute("DETACH DATABASE slice_out", [])?;
        copied?;
        c.execute("UPDATE slices SET materialised = 1, path = ? WHERE name = ?", params![out.to_string_lossy(), name])?;
        Library::open(out)
    }
}

/// A position the Games of a Slice reach, for background analysis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frequent {
    /// Its Polyglot key, as the `positions` table has it.
    pub key: u64,
    pub variant: String,
    pub fen: String,
    /// How often the Games reach it.
    pub count: u32,
}

impl Library {
    /// The chess positions the Games `ids` reach most often, at most `top`, the most frequent
    /// first, counted in the `positions` table; the start position is not among them.
    // shortcut: chess only, as only chess Games are kept in moves Bragd can replay here; the
    // other Families join when their stored moves can be replayed (`stored_replay`).
    pub fn frequent(&self, ids: &[i64], top: usize) -> Result<Vec<Frequent>> {
        let c = self.conn();
        c.execute_batch("CREATE TEMP TABLE IF NOT EXISTS picked (id INTEGER PRIMARY KEY); DELETE FROM temp.picked;")?;
        {
            let mut put = c.prepare_cached("INSERT OR IGNORE INTO temp.picked (id) VALUES (?1)")?;
            for id in ids {
                put.execute([id])?;
            }
        }
        let mut st = c.prepare(
            "SELECT p.hash, g.variant, count(*) AS n, min(p.game_id) FROM positions p JOIN temp.picked k ON k.id = p.game_id JOIN games g ON g.id = p.game_id \
             WHERE g.family = 'chess' GROUP BY p.hash, g.variant ORDER BY n DESC, p.hash LIMIT ?1",
        )?;
        let rows: Vec<(i64, String, u32, i64)> = st.query_map([top as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut ply = c.prepare_cached("SELECT ply FROM positions WHERE hash = ?1 AND game_id = ?2 LIMIT 1")?;
        let mut out = Vec::with_capacity(rows.len());
        for (hash, variant, count, game) in rows {
            let n: usize = ply.query_row(params![hash, game], |r| r.get::<_, i64>(0))? as usize;
            let g = self.stored(game)?;
            let mut b = g.start.as_deref().and_then(crate::chess::pgn::board).unwrap_or_else(rules_chess::Position::starting);
            for san in g.moves.iter().take(n) {
                let Some(mv) = crate::chess::pgn::san_move(&b, san) else { break };
                b = b.make_move(mv);
            }
            out.push(Frequent { key: hash as u64, variant, fen: crate::chess::pgn::fen(&b), count });
        }
        c.execute_batch("DELETE FROM temp.picked;")?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::{pgn, polyglot};
    use crate::library::NewGame;
    use crate::library::tests::{CENTURY, Dir, pgn as pgn_text};

    fn add(lib: &mut Library, tags: &[(&str, &str)], moves: &str, result: &str) -> i64 {
        let g = pgn::parse(&pgn_text(tags, moves, result));
        match lib.add(&NewGame::from_chess(&g), "twic", &format!("test#{}", lib.count().unwrap() + 1)).unwrap() {
            crate::library::Added::New(id) => id,
            other => panic!("{other:?}"),
        }
    }

    /// Four games: Byrne – Fischer (D92, 1956), a Grünfeld of strong players (D85, 1990), a
    /// Sicilian (B90, 1999) and a game of no known date.
    fn library(d: &Dir) -> Library {
        let mut lib = Library::open(&d.0.join("lib.bragd")).unwrap();
        add(&mut lib, &[("White", "Byrne, Donald"), ("Black", "Fischer, Robert James"), ("ECO", "D92"), ("Date", "1956.10.17"), ("WhiteElo", "2400"), ("BlackElo", "2300")], CENTURY, "0-1");
        add(&mut lib, &[("White", "Karpov, Anatoly"), ("Black", "Kasparov, Garry"), ("ECO", "D85"), ("Date", "1990.10.08"), ("WhiteElo", "2730"), ("BlackElo", "2800")], "1. d4 Nf6 2. c4 g6 3. Nc3 d5 4. cxd5 Nxd5", "1/2-1/2");
        add(&mut lib, &[("White", "Anand, Viswanathan"), ("Black", "Kasparov, Garry"), ("ECO", "B90"), ("Date", "1999.??.??"), ("WhiteElo", "2780"), ("BlackElo", "2810")], "1. e4 c5 2. Nf3 d6 3. d4 cxd4 4. Nxd4 Nf6 5. Nc3 a6", "1-0");
        add(&mut lib, &[("White", "Unknown"), ("Black", "Player")], "1. e4 e5", "*");
        lib
    }

    /// The positions a Slice's Games reach most often come first, each with its FEN, which is
    /// the position its key names.
    #[test]
    fn a_slices_most_frequent_positions() {
        let d = Dir::new("frequent");
        let lib = library(&d);
        let all = lib.find(&Query::default()).unwrap();
        let f = lib.frequent(&all, 3).unwrap();
        assert_eq!(f.len(), 3);
        // 1. e4 is in two Games; nothing else is in more than one.
        assert_eq!(f.iter().map(|p| p.count).collect::<Vec<_>>(), [2, 1, 1]);
        for p in &f {
            assert_eq!(polyglot::key(&pgn::board(&p.fen).unwrap()), p.key, "{}", p.fen);
            assert_eq!(p.variant, "chess");
        }
        assert!(f[0].fen.starts_with("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b"), "{}", f[0].fen);
        // Only the Games asked about.
        let one = lib.frequent(&all[3..], 10).unwrap();
        assert_eq!((one.len(), one[0].count), (2, 1));
        assert!(lib.frequent(&[], 10).unwrap().is_empty());
    }

    /// A Slice's filters use the Library's indexes.
    #[test]
    fn slice_filters_use_their_indexes() {
        let d = Dir::new("slice-index");
        let lib = library(&d);
        let plan = |q: Query| {
            let (cond, args) = condition(&q);
            let mut st = lib.conn().prepare(&format!("EXPLAIN QUERY PLAN SELECT id FROM games WHERE {cond}")).unwrap();
            st.query_map(rusqlite::params_from_iter(args), |r| r.get::<_, String>(3)).unwrap().map(|r| r.unwrap()).collect::<Vec<_>>().join(" ")
        };
        assert!(plan(Query { eco_from: Some("D70".into()), ..Default::default() }).contains("games_eco"));
        assert!(plan(Query { date_from: Some("1990".into()), ..Default::default() }).contains("games_date"));
        assert!(plan(Query { rating_min: Some(2500), ..Default::default() }).contains("_elo"));
        assert_eq!(lib.find(&Query { date_from: Some("1990".into()), ..Default::default() }).unwrap(), [2, 3]);
    }

    #[test]
    fn queries_find_what_they_say() {
        let d = Dir::new("slice-find");
        let lib = library(&d);
        let find = |q: Query| lib.find(&q).unwrap();
        assert_eq!(find(Query::default()), [1, 2, 3, 4]);
        assert_eq!(find(Query { players: vec!["kasparov".into()], ..Default::default() }), [2, 3]);
        assert_eq!(find(Query { players: vec!["Fischer".into(), "Anand".into()], ..Default::default() }), [1, 3]);
        let grunfeld = Query { eco_from: Some("D70".into()), eco_to: Some("D99".into()), ..Default::default() };
        assert_eq!(find(grunfeld), [1, 2]);
        assert_eq!(find(Query { date_from: Some("1990".into()), ..Default::default() }), [2, 3]);
        assert_eq!(find(Query { date_to: Some("1990".into()), ..Default::default() }), [1, 2]);
        assert_eq!(find(Query { date_from: Some("1990.10.09".into()), date_to: Some("1999".into()), ..Default::default() }), [3]);
        assert_eq!(find(Query { rating_min: Some(2500), ..Default::default() }), [2, 3]);
        assert_eq!(find(Query { result: Some("1-0".into()), ..Default::default() }), [3]);
        assert_eq!(find(Query { variant: Some("chess960".into()), ..Default::default() }), Vec::<i64>::new());
        assert_eq!(find(Query { family: Some("chess".into()), variant: Some("chess".into()), ..Default::default() }).len(), 4);
        // The position after 1.e4 c5 2.Nf3: only the Sicilian reaches it.
        let b = pgn::board("rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2").unwrap();
        assert_eq!(find(Query { position: Some(polyglot::key(&b)), ..Default::default() }), [3]);
        // A wildcard in a name is a plain character.
        assert_eq!(find(Query { players: vec!["%".into()], ..Default::default() }), Vec::<i64>::new());
    }

    #[test]
    fn a_slice_is_live_and_can_be_kept_apart() {
        let d = Dir::new("slice-keep");
        let mut lib = library(&d);
        let strong = Query { rating_min: Some(2500), ..Default::default() };
        lib.save_slice("Both players 2500+", &strong).unwrap();
        assert_eq!(lib.slice_games("Both players 2500+").unwrap(), [2, 3]);
        // Live: a game added later is in it.
        let new = add(&mut lib, &[("White", "Carlsen, Magnus"), ("Black", "Caruana, Fabiano"), ("WhiteElo", "2850"), ("BlackElo", "2820")], "1. e4 e5 2. Nf3 Nc6", "1/2-1/2");
        assert_eq!(lib.slice_games("Both players 2500+").unwrap(), [2, 3, new]);
        // Kept apart: its own Library, with the Games' Sources and positions.
        let out = d.0.join("strong.bragd");
        let kept = lib.materialise("Both players 2500+", &out).unwrap();
        assert_eq!(kept.count().unwrap(), 3);
        assert_eq!(kept.sources(2).unwrap().len(), 1);
        let positions: i64 = kept.conn().query_row("SELECT count(*) FROM positions", [], |r| r.get(0)).unwrap();
        assert_eq!(positions, 8 + 10 + 4);
        let s = lib.slice("Both players 2500+").unwrap().unwrap();
        assert!(s.materialised && s.path.as_deref() == Some(out.as_path()));
        assert_eq!(s.query, strong);
        // Edited apart: the Library it came from is untouched.
        kept.conn().execute("DELETE FROM games WHERE id = 2", []).unwrap();
        assert_eq!(lib.count().unwrap(), 5);
        assert!(lib.materialise("Both players 2500+", &out).is_err(), "never over a file");
        assert!(lib.slice_games("nothing").is_err());
    }
}
