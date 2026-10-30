//! A Library: Bragd's own store of Games, one SQLite file. Games are imported from Foreign
//! databases (`chess::Table`, `go::Collection`) in batches, each batch one transaction, and each
//! Game is kept once with every Source it came from.
//!
//! A Duplicate is the same moves from the same start position, whatever its headers say. It is
//! found by `zobrist_chain_hash`: a hash over the chain of the positions' keys, ply by ply,
//! after the Family, the Variant and the start position. A Duplicate is merged on import: the
//! Game is kept once, the new Source is recorded on it, and where the Sources' headers disagree
//! the Source ranked first in `priority` wins (`DEFAULT_PRIORITY`; unknown Sources rank after).
//!
//! A Near-duplicate is a Game whose moves are a strict prefix of another's (a copy cut off
//! early). It goes into `near_duplicates` for review and is never merged.
//!
//! Position keys: chess uses the Polyglot Zobrist key (`chess::polyglot::key`), as the
//! `positions` table does for position search. Draughts, xiangqi and shogi use a hash of the
//! position's FEN or SFEN. Go has no position key here: its key after a move is the key of the
//! move sequence so far, so go's Duplicates and Near-duplicates are still exact.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Map, Value};

use crate::chess::{Format, Table, pgn, polyglot};
use crate::go;

/// Sources whose headers win a conflict, first first. Sources not named rank after these.
pub const DEFAULT_PRIORITY: [&str; 2] = ["twic", "lichess"];

/// Games written in one transaction during an import.
pub const DEFAULT_BATCH: usize = 1000;

/// What can go wrong with a Library.
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Sql(rusqlite::Error),
    Parquet(parquet::errors::ParquetError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => e.fmt(f),
            Error::Sql(e) => e.fmt(f),
            Error::Parquet(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sql(e)
    }
}

impl From<parquet::errors::ParquetError> for Error {
    fn from(e: parquet::errors::ParquetError) -> Self {
        Error::Parquet(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS games (
    id INTEGER PRIMARY KEY,
    family TEXT NOT NULL,
    variant TEXT NOT NULL,
    -- Every header, merged from the Sources, as a JSON object.
    headers TEXT NOT NULL,
    -- The headers Slices filter on, from `headers`.
    white TEXT NOT NULL DEFAULT '',
    black TEXT NOT NULL DEFAULT '',
    white_elo INTEGER,
    black_elo INTEGER,
    date TEXT NOT NULL DEFAULT '',
    event TEXT NOT NULL DEFAULT '',
    site TEXT NOT NULL DEFAULT '',
    eco TEXT NOT NULL DEFAULT '',
    opening TEXT NOT NULL DEFAULT '',
    result TEXT NOT NULL,
    -- The main line, moves apart by spaces; chess in SAN as Bragd writes it.
    moves TEXT NOT NULL,
    -- The start position; NULL for the Variant's usual one.
    start TEXT,
    ply_count INTEGER NOT NULL,
    zobrist_final INTEGER NOT NULL,
    zobrist_chain_hash INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS games_chain ON games (zobrist_chain_hash);
CREATE TABLE IF NOT EXISTS game_sources (
    game_id INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    source TEXT NOT NULL,
    source_ref TEXT NOT NULL,
    fetched_at INTEGER NOT NULL,
    -- The headers as this Source has them, for merging them again.
    headers TEXT NOT NULL,
    PRIMARY KEY (game_id, source, source_ref)
);
CREATE TABLE IF NOT EXISTS positions (
    hash INTEGER NOT NULL,
    game_id INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    ply INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS positions_hash ON positions (hash);
CREATE INDEX IF NOT EXISTS positions_game ON positions (game_id);
CREATE TABLE IF NOT EXISTS near_duplicates (
    short_id INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    long_id INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    PRIMARY KEY (short_id, long_id)
);
CREATE TABLE IF NOT EXISTS slices (
    name TEXT PRIMARY KEY,
    query TEXT NOT NULL,
    materialised INTEGER NOT NULL DEFAULT 0,
    path TEXT
);
";

/// A Game as it goes into a Library, from any reader.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NewGame {
    /// `chess`, `draughts`, `xiangqi`, `shogi` or `go`.
    pub family: String,
    /// `standard`, `chess960`, `international`, `english`, …
    pub variant: String,
    pub headers: Vec<(String, String)>,
    /// The main line's moves.
    pub moves: Vec<String>,
    /// The start position, `None` for the Variant's usual one.
    pub start: Option<String>,
    /// The result the moves end with, when no header says.
    pub result: String,
    /// The start position's key, then the key after each move (`keys.len() == moves.len()`).
    pub start_key: u64,
    pub keys: Vec<u64>,
}

/// A stable 64-bit mix (SplitMix64's finaliser). Stored in Libraries: never change it.
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A stable key of a text (FNV-1a, mixed). Stored in Libraries: never change it.
pub fn text_key(s: &str) -> u64 {
    mix(s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)))
}

impl NewGame {
    /// The chain hash after each ply: `[0]` before any move, `[n]` after the last. A prefix of
    /// the moves has a prefix of the chain, which finds Near-duplicates.
    pub fn chain(&self) -> Vec<u64> {
        let mut h = mix(text_key(&self.family) ^ text_key(&self.variant).rotate_left(17) ^ self.start_key.rotate_left(31));
        let mut out = Vec::with_capacity(self.keys.len() + 1);
        out.push(h);
        for &k in &self.keys {
            h = mix(h.rotate_left(5) ^ k);
            out.push(h);
        }
        out
    }

    /// A chess game, its moves written again as Bragd writes SAN so that two files' spellings
    /// of a move (`Ng1f3`, `Nf3`) are one. The main line stops at a move that cannot be played.
    pub fn from_chess(g: &pgn::Game) -> NewGame {
        let first = pgn::board(&g.start).unwrap_or_default();
        let (mut b, mut moves, mut keys) = (first.clone(), vec![], vec![]);
        for i in g.main_line() {
            let Some(mv) = pgn::san_move(&b, &g.nodes[i as usize].san) else { break };
            moves.push(pgn::san(&b, mv));
            b.play_unchecked(mv);
            keys.push(polyglot::key(&b));
        }
        let variant = g.tag("Variant").map(|v| v.trim().to_ascii_lowercase()).filter(|v| !v.is_empty() && v != "standard" && v != "from position");
        NewGame {
            family: "chess".into(),
            variant: variant.unwrap_or_else(|| "standard".into()),
            headers: g.tags.clone(),
            moves,
            start: (pgn::fen(&first) != pgn::fen(&cozy_chess::Board::default())).then(|| pgn::fen(&first)),
            result: g.result.clone(),
            start_key: polyglot::key(&first),
            keys,
        }
    }

    /// A draughts, xiangqi or shogi game: their moves as the file writes them, each position's
    /// key a hash of its FEN (SFEN for shogi). The main line stops at a move that cannot be played.
    /// `usual_start`: the Variant's start position; empty when only a FEN tag says it is another.
    pub fn from_family(g: &pgn::Game, family: &str, variant: &str, usual_start: &str) -> NewGame {
        let (mut moves, mut keys) = (vec![], vec![]);
        for i in g.main_line() {
            let n = &g.nodes[i as usize];
            let Some(fen) = &n.fen else { break };
            moves.push(n.san.clone());
            keys.push(text_key(fen));
        }
        NewGame {
            family: family.into(),
            variant: variant.into(),
            headers: g.tags.clone(),
            moves,
            start: if usual_start.is_empty() { g.tag("FEN").map(|_| g.start.clone()) } else { (g.start != usual_start).then(|| g.start.clone()) },
            result: g.result.clone(),
            start_key: text_key(&g.start),
            keys,
        }
    }

    /// A go game: its moves as SGF writes them (`B[qd]`, `W[]` a pass), the start its board size and
    /// the stones set up before the first move.
    pub fn from_go(g: &go::Game) -> NewGame {
        let coord = |(x, y): (u8, u8)| format!("{}{}", (b'a' + x) as char, (b'a' + y) as char);
        let mut start = format!("SZ[{}:{}]", g.info.width, g.info.height);
        let mut at = g.nodes.first().map(|_| 0usize);
        let mut setup: Vec<String> = g.nodes.first().map(|n| n.setup.iter().map(|&(x, y, c)| format!("{}[{}]", ["AE", "AB", "AW"][c.min(2) as usize], coord((x, y)))).collect()).unwrap_or_default();
        setup.sort();
        start.push_str(&setup.concat());
        let (mut moves, mut keys) = (vec![], vec![]);
        let mut k = text_key(&start);
        while let Some(i) = at {
            if let Some(m) = g.nodes[i].mv {
                let mv = format!("{}[{}]", if m.black { "B" } else { "W" }, m.at.map(coord).unwrap_or_default());
                k = mix(k.rotate_left(5) ^ text_key(&mv));
                moves.push(mv);
                keys.push(k);
            }
            at = g.nodes[i].children.first().map(|&c| c as usize);
        }
        let i = &g.info;
        let headers = [
            ("Black", &i.black),
            ("White", &i.white),
            ("BlackRank", &i.black_rank),
            ("WhiteRank", &i.white_rank),
            ("Result", &i.result),
            ("Date", &i.date),
            ("Event", &i.event),
            ("Round", &i.round),
            ("Site", &i.place),
            ("Name", &i.name),
            ("Komi", &i.komi),
            ("Handicap", &i.handicap),
            ("Rules", &i.rules),
        ];
        NewGame {
            family: "go".into(),
            variant: "standard".into(),
            headers: headers.iter().filter(|h| !h.1.is_empty()).map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            moves,
            start: (start != "SZ[19:19]").then_some(start.clone()),
            result: if i.result.is_empty() { "*".into() } else { i.result.clone() },
            start_key: text_key(&start),
            keys,
        }
    }
}

/// How an import is going, and how it ended.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// Games read from the Foreign database.
    pub read: usize,
    /// New Games added to the Library.
    pub added: usize,
    /// Duplicates merged into a Game already there.
    pub merged: usize,
    /// Near-duplicate pairs found.
    pub near_duplicates: usize,
    /// Games that could not be read, left out.
    pub skipped: usize,
    /// Games written for good (whole batches).
    pub committed: usize,
    pub cancelled: bool,
}

/// What `add` did with a Game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Added {
    New(i64),
    /// A Duplicate of this Game, merged into it.
    Merged(i64),
}

/// A Foreign database as a stream of Games.
enum Reader {
    Table(Table),
    Go(go::Collection),
}

impl Reader {
    fn open(path: &Path) -> io::Result<Reader> {
        if go::is_sgf(path) { Ok(Reader::Go(go::Collection::open(path)?)) } else { Ok(Reader::Table(Table::open(path)?)) }
    }

    /// Game `i` (from 0), once the reader has listed it; `None` past the last.
    fn game(&self, i: usize) -> Option<io::Result<NewGame>> {
        match self {
            Reader::Go(c) => {
                c.wait(i + 1);
                c.info(i)?;
                Some(c.game(i).map(|g| NewGame::from_go(&g)))
            }
            Reader::Table(t) => {
                while t.progress().games <= i {
                    if t.progress().done {
                        return None;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                let row = t.row(i as u32)?;
                Some(t.game(i as u32).map(|g| {
                    let mut new = match t.format {
                        Format::Pdn => {
                            let v = crate::draughts::variant(&g);
                            let name = match (v.kind, v.width) {
                                (20, 10) => "international".to_string(),
                                (21, 8) => "english".to_string(),
                                (k, _) => format!("gametype-{k}"),
                            };
                            NewGame::from_family(&g, "draughts", &name, "")
                        }
                        Format::Xiangqi => NewGame::from_family(&g, "xiangqi", "standard", crate::xiangqi::START),
                        Format::Kif | Format::Csa => NewGame::from_family(&g, "shogi", "standard", crate::shogi::START),
                        _ => NewGame::from_chess(&g),
                    };
                    // The table's reading of the headers fills what the tags leave out: an opening
                    // named by its moves, xiangqi's Red as White, SCID's and ChessBase's names.
                    let has = |new: &NewGame, k: &str| new.headers.iter().any(|(n, v)| n.eq_ignore_ascii_case(k) && !v.trim().is_empty());
                    let elo = |e: Option<u16>| e.map(|e| e.to_string()).unwrap_or_default();
                    for (k, v) in [
                        ("White", row.white),
                        ("Black", row.black),
                        ("WhiteElo", elo(row.white_elo)),
                        ("BlackElo", elo(row.black_elo)),
                        ("Date", row.date),
                        ("Event", row.event),
                        ("ECO", row.eco),
                        ("Opening", row.opening),
                    ] {
                        if !v.is_empty() && !has(&new, k) {
                            new.headers.push((k.into(), v));
                        }
                    }
                    new
                }))
            }
        }
    }
}

/// A header value that says nothing.
fn blank(v: &str) -> bool {
    let v = v.trim();
    v.is_empty() || v.chars().all(|c| matches!(c, '?' | '.' | '-'))
}

/// One Library file, open.
pub struct Library {
    conn: Connection,
    path: PathBuf,
    /// Sources whose headers win a conflict, first first (lower case).
    priority: Vec<String>,
    /// Games written in one transaction during an import.
    pub batch: usize,
}

impl Library {
    /// Opens the Library at `path`, made with its tables when it is new.
    pub fn open(path: &Path) -> Result<Library> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Library { conn, path: path.to_path_buf(), priority: DEFAULT_PRIORITY.map(String::from).to_vec(), batch: DEFAULT_BATCH })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Sets which Sources win a header conflict, first first. It applies to the Games merged from now on.
    pub fn set_priority(&mut self, sources: &[&str]) {
        self.priority = sources.iter().map(|s| s.trim().to_lowercase()).collect();
    }

    /// How many Games the Library holds.
    pub fn count(&self) -> Result<u64> {
        Ok(self.conn.query_row("SELECT count(*) FROM games", [], |r| r.get::<_, i64>(0))? as u64)
    }

    /// Adds one Game from `source`, merged into its Duplicate when there is one, in a
    /// transaction of its own.
    pub fn add(&mut self, game: &NewGame, source: &str, source_ref: &str) -> Result<Added> {
        let tx = self.conn.transaction()?;
        let (added, _) = add_in(&tx, &self.priority, game, source, source_ref)?;
        tx.commit()?;
        Ok(added)
    }

    /// Imports every Game of the Foreign database at `path` as coming from `source`, `batch`
    /// Games a transaction. `progress` hears after each batch; `stop` is asked between Games,
    /// and true ends the import there: the batch under way is rolled back, the ones before stay.
    pub fn import(&mut self, path: &Path, source: &str, stop: &dyn Fn() -> bool, progress: &mut dyn FnMut(&Progress)) -> Result<Progress> {
        let reader = Reader::open(path)?;
        let mut p = Progress::default();
        let batch = self.batch.max(1);
        let mut i = 0usize;
        'batches: loop {
            let tx = self.conn.transaction()?;
            let mut done = Progress { committed: p.committed, ..p.clone() };
            for _ in 0..batch {
                if stop() {
                    p.cancelled = true;
                    // Dropped, the transaction rolls back.
                    break 'batches;
                }
                let Some(game) = reader.game(i) else {
                    tx.commit()?;
                    done.committed = done.added + done.merged;
                    p = done;
                    break 'batches;
                };
                i += 1;
                done.read += 1;
                let Ok(game) = game else {
                    done.skipped += 1;
                    continue;
                };
                let source_ref = format!("{}#{i}", path.display());
                match add_in(&tx, &self.priority, &game, source, &source_ref)? {
                    (Added::New(_), near) => {
                        done.added += 1;
                        done.near_duplicates += near;
                    }
                    (Added::Merged(_), _) => done.merged += 1,
                }
            }
            tx.commit()?;
            done.committed = done.added + done.merged;
            p = done;
            progress(&p);
        }
        progress(&p);
        Ok(p)
    }

    /// The Sources a Game came from: source, its reference, when it was fetched (Unix seconds).
    pub fn sources(&self, id: i64) -> Result<Vec<(String, String, i64)>> {
        let mut st = self.conn.prepare("SELECT source, source_ref, fetched_at FROM game_sources WHERE game_id = ? ORDER BY rowid")?;
        let rows = st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// A Game's merged headers.
    pub fn headers(&self, id: i64) -> Result<Map<String, Value>> {
        let json: String = self.conn.query_row("SELECT headers FROM games WHERE id = ?", [id], |r| r.get(0))?;
        Ok(serde_json::from_str(&json).unwrap_or_default())
    }

    /// Merges the headers of every Game with more than one Source again, by the priority set
    /// now: what "Dedupe now" does after the Source priority changed. `stop` is asked between
    /// Games; the Games merged before it said stop stay merged. How many Games were merged.
    pub fn merge_again(&mut self, stop: &dyn Fn() -> bool, progress: &mut dyn FnMut(usize, usize)) -> Result<usize> {
        let ids: Vec<i64> = {
            let mut st = self.conn.prepare("SELECT game_id FROM game_sources GROUP BY game_id HAVING count(*) > 1 ORDER BY game_id")?;
            st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
        };
        let mut done = 0;
        for chunk in ids.chunks(self.batch.max(1)) {
            let tx = self.conn.transaction()?;
            for id in chunk {
                if stop() {
                    tx.commit()?;
                    return Ok(done);
                }
                let result: String = tx.query_row("SELECT result FROM games WHERE id = ?", [id], |r| r.get(0))?;
                merge_headers(&tx, &self.priority, *id, &result)?;
                done += 1;
            }
            tx.commit()?;
            progress(done, ids.len());
        }
        Ok(done)
    }

    /// The Near-duplicates found, for review: the shorter Game's id, then the longer's.
    pub fn near_duplicates(&self) -> Result<Vec<(i64, i64)>> {
        let mut st = self.conn.prepare("SELECT short_id, long_id FROM near_duplicates ORDER BY short_id, long_id")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

/// `Library::add` inside the transaction `tx`; with the Near-duplicate pairs it found.
fn add_in(tx: &Transaction, priority: &[String], g: &NewGame, source: &str, source_ref: &str) -> Result<(Added, usize)> {
    let chain = g.chain();
    let hash = *chain.last().expect("the chain has its start") as i64;
    let moves = g.moves.join(" ");
    let n = g.moves.len();
    let fetched = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let own: Map<String, Value> = g.headers.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect();
    let own = Value::Object(own).to_string();
    let same: Option<i64> = tx
        .query_row(
            "SELECT id FROM games WHERE zobrist_chain_hash = ?1 AND family = ?2 AND variant = ?3 AND moves = ?4 AND start IS ?5",
            params![hash, g.family, g.variant, moves, g.start],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(id) = same {
        tx.execute("INSERT OR IGNORE INTO game_sources (game_id, source, source_ref, fetched_at, headers) VALUES (?, ?, ?, ?, ?)", params![id, source, source_ref, fetched, own])?;
        merge_headers(tx, priority, id, &g.result)?;
        return Ok((Added::Merged(id), 0));
    }
    let last = *g.keys.last().unwrap_or(&g.start_key);
    tx.execute(
        "INSERT INTO games (family, variant, headers, result, moves, start, ply_count, zobrist_final, zobrist_chain_hash) VALUES (?, ?, '{}', ?, ?, ?, ?, ?, ?)",
        params![g.family, g.variant, g.result, moves, g.start, n as i64, last as i64, hash],
    )?;
    let id = tx.last_insert_rowid();
    tx.execute("INSERT INTO game_sources (game_id, source, source_ref, fetched_at, headers) VALUES (?, ?, ?, ?, ?)", params![id, source, source_ref, fetched, own])?;
    let mut put = tx.prepare_cached("INSERT INTO positions (hash, game_id, ply) VALUES (?, ?, ?)")?;
    for (ply, &k) in g.keys.iter().enumerate() {
        put.execute(params![k as i64, id, ply as i64 + 1])?;
    }
    merge_headers(tx, priority, id, &g.result)?;
    let near = if n == 0 { 0 } else { near_duplicates(tx, g, id, &moves, &chain)? };
    Ok((Added::New(id), near))
}

/// Records the Near-duplicates of the new Game `id`: longer Games it is a prefix of, and shorter
/// Games that are a prefix of it. Candidates come from the indexes; the moves decide.
fn near_duplicates(tx: &Transaction, g: &NewGame, id: i64, moves: &str, chain: &[u64]) -> Result<usize> {
    let n = g.moves.len();
    let mut pairs: HashSet<(i64, i64)> = HashSet::new();
    let mut longer = tx.prepare_cached(
        "SELECT g.id, g.moves FROM positions p JOIN games g ON g.id = p.game_id
         WHERE p.hash = ?1 AND p.ply = ?2 AND g.ply_count > ?2 AND g.family = ?3 AND g.variant = ?4 AND g.start IS ?5",
    )?;
    let rows = longer.query_map(params![g.keys[n - 1] as i64, n as i64, g.family, g.variant, g.start], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (other, theirs) = row?;
        if theirs.strip_prefix(moves).is_some_and(|rest| rest.starts_with(' ')) {
            pairs.insert((id, other));
        }
    }
    // The chain after 1 … n-1 plies: a Game whose whole chain is one of them is a shorter prefix.
    for part in chain[1..n].chunks(500) {
        let marks = vec!["?"; part.len()].join(",");
        let sql = format!("SELECT id, moves FROM games WHERE zobrist_chain_hash IN ({marks}) AND family = ? AND variant = ? AND start IS ? AND id != ?");
        let mut st = tx.prepare(&sql)?;
        let mut args: Vec<rusqlite::types::Value> = part.iter().map(|&h| (h as i64).into()).collect();
        args.extend([g.family.clone().into(), g.variant.clone().into(), g.start.clone().into(), id.into()]);
        let rows = st.query_map(rusqlite::params_from_iter(args), |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (other, theirs) = row?;
            if moves.strip_prefix(theirs.as_str()).is_some_and(|rest| rest.starts_with(' ')) {
                pairs.insert((other, id));
            }
        }
    }
    let mut put = tx.prepare_cached("INSERT OR IGNORE INTO near_duplicates (short_id, long_id) VALUES (?, ?)")?;
    let mut found = 0;
    for (short, long) in pairs {
        found += put.execute(params![short, long])?;
    }
    Ok(found)
}

/// Game `id`'s headers merged again from all its Sources: for each header, the value of the
/// Source ranked first in `priority` that has one (unknown Sources after, in the order they came).
fn merge_headers(tx: &Transaction, priority: &[String], id: i64, moves_result: &str) -> Result<()> {
    let mut st = tx.prepare_cached("SELECT source, headers FROM game_sources WHERE game_id = ? ORDER BY rowid")?;
    let mut sources: Vec<(String, String)> = st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    let rank = |s: &str| priority.iter().position(|p| p.eq_ignore_ascii_case(s.trim())).unwrap_or(priority.len());
    sources.sort_by_key(|(s, _)| rank(s));
    let mut merged = Map::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (_, json) in &sources {
        let Ok(Value::Object(h)) = serde_json::from_str::<Value>(json) else { continue };
        for (k, v) in h {
            if v.as_str().is_some_and(|s| !blank(s)) && seen.insert(k.to_ascii_lowercase()) {
                merged.insert(k, v);
            }
        }
    }
    let get = |names: &[&str]| -> String {
        names.iter().find_map(|n| merged.iter().find(|(k, _)| k.eq_ignore_ascii_case(n)).and_then(|(_, v)| v.as_str())).unwrap_or("").trim().to_string()
    };
    let elo = |n: &str| get(&[n]).parse::<i64>().ok().filter(|&e| e > 0);
    let result = Some(get(&["Result"])).filter(|r| !r.is_empty()).unwrap_or_else(|| moves_result.to_string());
    tx.execute(
        "UPDATE games SET headers = ?, white = ?, black = ?, white_elo = ?, black_elo = ?, date = ?, event = ?, site = ?, eco = ?, opening = ?, result = ? WHERE id = ?",
        params![
            Value::Object(merged.clone()).to_string(),
            get(&["White", "Red"]),
            get(&["Black"]),
            elo("WhiteElo"),
            elo("BlackElo"),
            get(&["Date", "UTCDate"]),
            get(&["Event"]),
            get(&["Site"]),
            get(&["ECO"]),
            get(&["Opening"]),
            result,
            id
        ],
    )?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A folder in the temporary folder, gone when dropped.
    pub struct Dir(pub PathBuf);

    impl Dir {
        pub fn new(name: &str) -> Dir {
            static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!("bragd-library-{name}-{}-{n}", std::process::id()));
            std::fs::create_dir_all(&p).unwrap();
            Dir(p)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The Game of The Century (Byrne – Fischer, 1956), its first 41 moves and the result.
    pub const CENTURY: &str = "1. Nf3 Nf6 2. c4 g6 3. Nc3 Bg7 4. d4 O-O 5. Bf4 d5 6. Qb3 dxc4 7. Qxc4 c6 8. e4 Nbd7 9. Rd1 Nb6 10. Qc5 Bg4 11. Bg5 Na4 12. Qa3 Nxc3 13. bxc3 Nxe4 14. Bxe7 Qb6 15. Bc4 Nxc3 16. Bc5 Rfe8+ 17. Kf1 Be6 18. Bxb6 Bxc4+ 19. Kg1 Ne2+ 20. Kf1 Nxd4+ 21. Kg1 Ne2+ 22. Kf1 Nc3+ 23. Kg1 axb6 24. Qb4 Ra4 25. Qxb6 Nxd1 26. h3 Rxa2 27. Kh2 Nxf2 28. Re1 Rxe1 29. Qd8+ Bf8 30. Nxe1 Bd5 31. Nf3 Ne4 32. Qb8 b5 33. h4 h5 34. Ne5 Kg7 35. Kg1 Bc5+ 36. Kf1 Ng3+ 37. Ke1 Bb4+ 38. Kd1 Bb3+ 39. Kc1 Ne2+ 40. Kb1 Nc3+ 41. Kc1 Rc2#";

    pub fn pgn(tags: &[(&str, &str)], moves: &str, result: &str) -> String {
        let mut s: String = tags.iter().map(|(k, v)| format!("[{k} \"{v}\"]\n")).collect();
        s.push_str(&format!("[Result \"{result}\"]\n\n{moves} {result}\n\n"));
        s
    }

    fn import(lib: &mut Library, path: &Path, source: &str) -> Progress {
        lib.import(path, source, &|| false, &mut |_| {}).unwrap()
    }

    #[test]
    fn the_same_game_from_two_sources_is_kept_once() {
        let d = Dir::new("dupe");
        let twic = d.0.join("twic.pgn");
        let lichess = d.0.join("lichess.pgn");
        std::fs::write(&twic, pgn(&[("White", "Byrne, Donald"), ("Black", "Fischer, Robert James"), ("Event", "Rosenwald Memorial"), ("Date", "1956.10.17")], CENTURY, "0-1")).unwrap();
        // Lichess spells the names its own way, writes moves in long form, and has a header TWIC has not.
        let long = CENTURY.replacen("1. Nf3 Nf6", "1. Ng1f3 Ng8f6", 1);
        std::fs::write(&lichess, pgn(&[("White", "D. Byrne"), ("Black", "R. Fischer"), ("Event", "?"), ("Date", "1956.??.??"), ("WhiteElo", "2400")], &long, "0-1")).unwrap();
        let mut lib = Library::open(&d.0.join("lib.bragd")).unwrap();
        assert_eq!(import(&mut lib, &lichess, "lichess").added, 1);
        let p = import(&mut lib, &twic, "twic");
        assert_eq!((p.added, p.merged), (0, 1));
        assert_eq!(lib.count().unwrap(), 1);
        let sources: Vec<String> = lib.sources(1).unwrap().into_iter().map(|s| s.0).collect();
        assert_eq!(sources, ["lichess", "twic"]);
        // TWIC ranks first: its spelling wins, though it came second; Lichess fills the gap.
        let h = lib.headers(1).unwrap();
        assert_eq!(h["White"], "Byrne, Donald");
        assert_eq!(h["Date"], "1956.10.17");
        assert_eq!(h["Event"], "Rosenwald Memorial");
        assert_eq!(h["WhiteElo"], "2400");
        let (white, elo): (String, i64) = lib.conn().query_row("SELECT white, white_elo FROM games", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((white.as_str(), elo), ("Byrne, Donald", 2400));
        // Importing the same file again records nothing new.
        assert_eq!(import(&mut lib, &twic, "twic").merged, 1);
        assert_eq!(lib.sources(1).unwrap().len(), 2);
        // With Lichess ranked first, merging again gives Lichess's spelling.
        lib.set_priority(&["lichess", "twic"]);
        assert_eq!(lib.merge_again(&|| false, &mut |_, _| {}).unwrap(), 1);
        assert_eq!(lib.headers(1).unwrap()["White"], "D. Byrne");
    }

    #[test]
    fn source_priority_is_configurable() {
        let d = Dir::new("priority");
        let a = d.0.join("a.pgn");
        let b = d.0.join("b.pgn");
        std::fs::write(&a, pgn(&[("White", "Byrne, Donald")], CENTURY, "0-1")).unwrap();
        std::fs::write(&b, pgn(&[("White", "D. Byrne")], CENTURY, "0-1")).unwrap();
        let mut lib = Library::open(&d.0.join("lib.bragd")).unwrap();
        lib.set_priority(&["lichess", "twic"]);
        import(&mut lib, &a, "twic");
        import(&mut lib, &b, "lichess");
        assert_eq!(lib.headers(1).unwrap()["White"], "D. Byrne");
        // An unknown Source ranks after the named ones.
        let c = d.0.join("c.pgn");
        std::fs::write(&c, pgn(&[("White", "Byrne D")], CENTURY, "0-1")).unwrap();
        import(&mut lib, &c, "club");
        assert_eq!(lib.headers(1).unwrap()["White"], "D. Byrne");
    }

    #[test]
    fn a_game_cut_at_move_40_is_a_near_duplicate() {
        let d = Dir::new("near");
        let cut = &CENTURY[..CENTURY.find(" 41.").unwrap()];
        let full = d.0.join("full.pgn");
        let short = d.0.join("short.pgn");
        std::fs::write(&full, pgn(&[("White", "Byrne, Donald")], CENTURY, "0-1")).unwrap();
        std::fs::write(&short, pgn(&[("White", "Byrne, Donald")], cut, "*")).unwrap();
        // Either order finds it.
        for (first, second) in [(&full, &short), (&short, &full)] {
            let mut lib = Library::open(&d.0.join(format!("{}.bragd", first.file_stem().unwrap().to_string_lossy()))).unwrap();
            import(&mut lib, first, "twic");
            let p = import(&mut lib, second, "twic");
            assert_eq!((p.added, p.merged, p.near_duplicates), (1, 0, 1));
            assert_eq!(lib.count().unwrap(), 2, "never merged");
            let plies: Vec<i64> = lib.near_duplicates().unwrap().iter().map(|&(s, _)| lib.conn().query_row("SELECT ply_count FROM games WHERE id = ?", [s], |r| r.get(0)).unwrap()).collect();
            assert_eq!(plies, [80], "the shorter one is the one cut after move 40");
            let long: i64 = lib.conn().query_row("SELECT ply_count FROM games WHERE id = ?", [lib.near_duplicates().unwrap()[0].1], |r| r.get(0)).unwrap();
            assert_eq!(long, 82, "every move of the whole game played");
        }
    }

    #[test]
    fn different_games_do_not_merge() {
        let d = Dir::new("different");
        let p = d.0.join("games.pgn");
        // Same opening, then apart at the last move; and a game by transposition to the same position.
        let other_end = CENTURY.replace("41. Kc1 Rc2#", "41. Kc1 Ra1+");
        let text = [
            pgn(&[("White", "Byrne, Donald")], CENTURY, "0-1"),
            pgn(&[("White", "Byrne, Donald")], &other_end, "*"),
            pgn(&[], "1. e4 e5 2. Nf3 Nc6", "*"),
            pgn(&[], "1. Nf3 Nc6 2. e4 e5", "*"),
        ]
        .concat();
        std::fs::write(&p, text).unwrap();
        let mut lib = Library::open(&d.0.join("lib.bragd")).unwrap();
        let got = import(&mut lib, &p, "twic");
        assert_eq!((got.added, got.merged, got.near_duplicates), (4, 0, 0));
        // The two transposed games share their last position, not their chain.
        let finals: Vec<(i64, i64)> = lib.conn().prepare("SELECT zobrist_final, zobrist_chain_hash FROM games WHERE id IN (3, 4)").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect();
        assert_eq!(finals[0].0, finals[1].0);
        assert_ne!(finals[0].1, finals[1].1);
    }

    #[test]
    fn imports_go_in_batches_and_stop_when_asked() {
        let d = Dir::new("batches");
        let p = d.0.join("many.pgn");
        // 25 games, each a knight dance one longer than the last.
        let text: String = (0..25).map(|i| pgn(&[("White", &format!("p{i}"))], &format!("1. a3 a6 {}", "Nf3 Nf6 Ng1 Ng8 ".repeat(i)), "*")).collect();
        std::fs::write(&p, text).unwrap();
        let mut lib = Library::open(&d.0.join("lib.bragd")).unwrap();
        lib.batch = 10;
        let mut heard = vec![];
        let asked = std::cell::Cell::new(0);
        let got = lib
            .import(&p, "twic", &|| {
                asked.set(asked.get() + 1);
                asked.get() > 15
            }, &mut |p| heard.push(p.committed))
            .unwrap();
        assert!(got.cancelled);
        assert_eq!(got.committed, 10, "the second batch was rolled back");
        assert_eq!(lib.count().unwrap(), 10);
        assert_eq!(heard[0], 10);
        // Whole, with progress after each batch.
        let mut heard = vec![];
        let mut lib = Library::open(&d.0.join("all.bragd")).unwrap();
        lib.batch = 10;
        let got = lib.import(&p, "twic", &|| false, &mut |p| heard.push(p.read)).unwrap();
        assert_eq!((got.read, got.cancelled), (25, false));
        assert_eq!(heard, [10, 20, 25]);
        assert_eq!(lib.count().unwrap(), 25);
    }

    #[test]
    fn every_family_imports() {
        let d = Dir::new("families");
        let files = [
            ("g.sgf", "(;FF[4]GM[1]SZ[19]PB[Shusaku]PW[Gennan]RE[B+2];B[qd];W[dc];B[pq])(;FF[4]GM[1]SZ[19]PB[Shusaku]PW[Gennan]RE[B+2];B[qd];W[dc];B[pq])".to_string(), "go"),
            ("g.pdn", "[White \"Anna\"]\n[Black \"Bram\"]\n[Result \"2-0\"]\n\n1. 32-28 19-23 2. 28x19 14x23 2-0\n".to_string(), "draughts"),
            ("g.csa", "N+Habu\nN-Sato\nPI\n+\n+7776FU\n-3334FU\n%TORYO\n".to_string(), "shogi"),
            ("x.pgn", "[Game \"Chinese Chess\"]\n[Red \"Hu\"]\n[Black \"Lin\"]\n[Result \"1-0\"]\n\n1. h2e2 h9g7 2. h0g2 1-0\n".to_string(), "xiangqi"),
        ];
        let mut lib = Library::open(&d.0.join("lib.bragd")).unwrap();
        for (name, text, family) in files {
            let p = d.0.join(name);
            std::fs::write(&p, text).unwrap();
            let got = import(&mut lib, &p, "club");
            assert!(got.added == 1 && got.skipped == 0, "{name}: {got:?}");
            let (f, plies, white): (String, i64, String) = lib.conn().query_row("SELECT family, ply_count, white FROM games ORDER BY id DESC LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
            assert_eq!(f, family);
            assert!(plies >= 2 && !white.is_empty(), "{name}: {plies} {white}");
        }
        // The go file's two games are one Game with the same Source twice.
        assert_eq!(lib.sources(1).unwrap().len(), 2);
    }

    /// A real ChessBase database, from the fixtures: it goes in through the same table.
    #[test]
    fn a_chessbase_database_imports() {
        let d = Dir::new("cbh");
        let cbh = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/chess/cbh-test/Games.cbh");
        let mut lib = Library::open(&d.0.join("lib.bragd")).unwrap();
        let got = import(&mut lib, &cbh, "chessbase");
        assert!(got.added >= 1 && got.skipped == 0, "{got:?}");
        let positions: i64 = lib.conn().query_row("SELECT count(*) FROM positions", [], |r| r.get(0)).unwrap();
        assert!(positions > 0);
    }

    #[test]
    fn chess_positions_are_polyglot_keys() {
        let g = pgn::parse("1. e4 *");
        let new = NewGame::from_chess(&g);
        // The Polyglot specification's key after 1.e4.
        assert_eq!(new.keys, [0x823c_9b50_fd11_4196]);
        assert_eq!(new.start_key, 0x463b_9618_1691_fc9c);
        assert_eq!(new.start, None);
    }
}
