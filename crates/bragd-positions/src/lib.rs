//! The Position store: one SQLite file per machine that keeps Evaluations by position, for
//! every Library, Foreign database and Game played (ADR 0004). Deleting a Library does not
//! touch it.
//!
//! A row is keyed by Family, Variant, position key and Edition. The position key is the
//! Family's own: the Polyglot key for chess, a hash of the FEN or SFEN for draughts, xiangqi and
//! shogi, and `rules_go::zobrist` for go. Callers compute it; the store only keeps it.
//!
//! **Every row is a fact about one Edition.** Merging two stores (an import, and Peers from
//! 0.5.0) never conflicts: for one position and one Edition the store keeps the row that is
//! greater by `(depth, time, kind, score, lead, line)`, so the deeper one wins. That order is
//! total, so a merge gives the same store in any order and any number of times (`put`).
//!
//! What is shown for a position and an Engine is its newest Edition's row, then the deepest,
//! then the latest (`shown`). A new Edition never re-queues anything by itself.
//!
//! An Evaluation keeps the Engine's second choice too (schema 3, MultiPV 2), when it gave one:
//! a move is `!` or `!!` by how much worse that second choice is, so a position from the store
//! can earn them. It comes last in the merge order: `(depth, time, kind, score, lead, line,
//! second)`, a row without one less than a row with one.
//!
//! Annotations (schema 2) are kept the same way, keyed also by the move, and merged by the same
//! rule: the greater by `(depth, time, mark, better, motifs)` wins (`put_annotation`).

pub mod annotation;
pub mod edition;
pub mod parquet;
pub mod queue;
pub mod sync;

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

pub use annotation::{Annotation, Mark, Motif};
pub use edition::Edition;

#[derive(Debug)]
pub enum Error {
    Sql(rusqlite::Error),
    Parquet(::parquet::errors::ParquetError),
    Io(std::io::Error),
    /// A store from a newer Bragd, or of a version no migration leads from: left as it is.
    Format(bragd_core::format::Error),
    /// A row that is not an Evaluation (a score that is not a number, a kind not known).
    Bad(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Sql(e) => write!(f, "{e}"),
            Error::Parquet(e) => write!(f, "{e}"),
            Error::Io(e) => write!(f, "{e}"),
            Error::Format(e) => write!(f, "{e}"),
            Error::Bad(why) => write!(f, "not an Evaluation: {why}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Error {
        Error::Sql(e)
    }
}

impl From<bragd_core::format::Error> for Error {
    fn from(e: bragd_core::format::Error) -> Error {
        match e {
            bragd_core::format::Error::Sql(e) => Error::Sql(e),
            e => Error::Format(e),
        }
    }
}

impl From<::parquet::errors::ParquetError> for Error {
    fn from(e: ::parquet::errors::ParquetError) -> Error {
        Error::Parquet(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// A score, from the first Side's view (White, Red; Black in shogi and go), as engines report it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Score {
    /// Centipawns (chess, xiangqi, shogi; draughts in hundredths of a man).
    Cp(i32),
    /// Mate in so many moves; minus: the other side mates.
    Mate(i32),
    /// Go: the chance to win, 0 to 1, and the lead in points.
    WinRate { win: f64, lead: f64 },
    /// Backgammon: the equity, in points per game.
    Equity(f64),
}

impl Score {
    /// How the store writes it: its kind, its number, and go's lead (0 for the others).
    fn columns(self) -> (&'static str, f64, f64) {
        match self {
            Score::Cp(c) => ("cp", c as f64, 0.0),
            Score::Mate(m) => ("mate", m as f64, 0.0),
            Score::WinRate { win, lead } => ("winrate", win, lead),
            Score::Equity(e) => ("equity", e, 0.0),
        }
    }

    pub(crate) fn from_columns(kind: &str, score: f64, lead: f64) -> Result<Score> {
        if !score.is_finite() || !lead.is_finite() {
            return Err(Error::Bad(format!("{kind} {score} {lead}")));
        }
        Ok(match kind {
            "cp" => Score::Cp(score as i32),
            "mate" => Score::Mate(score as i32),
            "winrate" => Score::WinRate { win: score, lead },
            "equity" => Score::Equity(score),
            other => return Err(Error::Bad(format!("score kind {other:?}"))),
        })
    }
}

/// One Edition's verdict on one position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evaluation {
    pub family: String,
    pub variant: String,
    /// The position's key in its Family.
    pub key: u64,
    pub edition: Edition,
    pub score: Score,
    pub depth: u32,
    /// The best line, its moves in the engine's notation, separated by spaces.
    pub line: String,
    /// When it was reached, in seconds since 1970.
    pub time: i64,
    /// The Engine's second choice (MultiPV 2), when it gave one.
    #[serde(default)]
    pub second: Option<Second>,
}

/// The Engine's second-best line in a position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Second {
    /// From the first Side's view, as `Evaluation::score`.
    pub score: Score,
    /// Its moves in the engine's notation, separated by spaces.
    pub line: String,
}

impl Second {
    /// As the store writes it: `kind2` empty, numbers 0 and `line2` empty for none.
    fn columns(s: Option<&Second>) -> (&'static str, f64, f64, &str) {
        match s {
            None => ("", 0.0, 0.0, ""),
            Some(s) => {
                let (kind, score, lead) = s.score.columns();
                (kind, score, lead, &s.line)
            }
        }
    }

    pub(crate) fn from_columns(kind: &str, score: f64, lead: f64, line: String) -> Result<Option<Second>> {
        if kind.is_empty() {
            return Ok(None);
        }
        Ok(Some(Second { score: Score::from_columns(kind, score, lead)?, line }))
    }
}

/// What a merge did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Merged {
    /// Rows read.
    pub read: u64,
    /// Rows that were new, or deeper than the store's.
    pub changed: u64,
}

/// What one Edition takes in the store (`Store::sizes`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Size {
    pub edition: Edition,
    pub rows: u64,
    pub bytes: u64,
}

/// The store's format (`PRAGMA user_version`). Its 1.0 baseline is 3; 4 numbers every change for
/// Peers (`sync`). Commentary comes as a migration, keyed the same way: (family, variant, key)
/// and its own columns.
pub const FORMAT: bragd_core::format::Sqlite = bragd_core::format::Sqlite { file: "positions.sqlite", first: 3, schema: SCHEMA, migrations: &[sync::MIGRATE_4] };

const SCHEMA: &str = "
CREATE TABLE editions (
    n INTEGER PRIMARY KEY,
    -- `Edition::id`, the same on every machine; `n` is this store's own.
    id TEXT NOT NULL UNIQUE,
    engine TEXT NOT NULL,
    version TEXT NOT NULL,
    network TEXT,
    image TEXT
);
-- The second choice is '' and 0 when there is none, not NULL, so the merge rule's row values
-- always compare.
CREATE TABLE evaluations (
    family TEXT NOT NULL,
    variant TEXT NOT NULL,
    key INTEGER NOT NULL,
    edition INTEGER NOT NULL REFERENCES editions (n),
    kind TEXT NOT NULL CHECK (kind IN ('cp', 'mate', 'winrate', 'equity')),
    score REAL NOT NULL,
    lead REAL NOT NULL,
    depth INTEGER NOT NULL,
    line TEXT NOT NULL,
    time INTEGER NOT NULL,
    kind2 TEXT NOT NULL DEFAULT '' CHECK (kind2 IN ('', 'cp', 'mate', 'winrate', 'equity')),
    score2 REAL NOT NULL DEFAULT 0,
    lead2 REAL NOT NULL DEFAULT 0,
    line2 TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (family, variant, key, edition)
) WITHOUT ROWID;
CREATE TABLE annotations (
    family TEXT NOT NULL,
    variant TEXT NOT NULL,
    -- The position the move was played in.
    key INTEGER NOT NULL,
    move TEXT NOT NULL,
    edition INTEGER NOT NULL REFERENCES editions (n),
    -- `Mark::sign`, or empty.
    mark TEXT NOT NULL,
    better TEXT NOT NULL,
    -- `Motif::id`s, separated by spaces.
    motifs TEXT NOT NULL,
    depth INTEGER NOT NULL,
    time INTEGER NOT NULL,
    PRIMARY KEY (family, variant, key, move, edition)
) WITHOUT ROWID;
";

/// How long a write waits for another connection's.
const BUSY: std::time::Duration = std::time::Duration::from_secs(10);

/// The Position store.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Where the store lives: `positions.sqlite` in Bragd's data folder.
    pub fn path() -> Option<PathBuf> {
        Some(bragd_core::dirs::app_data()?.join("positions.sqlite"))
    }

    /// Opens the store at `path`, making it if it is not there. A store written by a newer
    /// Bragd is refused and left as it is.
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        // Turning a new file to WAL while another connection opens it is refused at once, not
        // waited for (seen on Windows), so a busy open is tried again for as long as a write waits.
        let until = std::time::Instant::now() + BUSY;
        loop {
            match Store::open_once(path) {
                Err(Error::Sql(rusqlite::Error::SqliteFailure(e, _))) if e.code == rusqlite::ErrorCode::DatabaseBusy && std::time::Instant::now() < until => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                other => return other,
            }
        }
    }

    fn open_once(path: &Path) -> Result<Store> {
        let mut conn = Connection::open(path)?;
        conn.busy_timeout(BUSY)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        // Another process (a window, the Helper) may be making it at the same moment: it is
        // made under the write lock, by whichever gets it first.
        FORMAT.open(&mut conn)?;
        Ok(Store { conn })
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Merges `e` into the store: kept if the store has no row for its position and Edition,
    /// or if it is greater than that row (deeper, else later, else by its score and line).
    /// True when the store changed.
    pub fn put(&mut self, e: &Evaluation) -> Result<bool> {
        let tx = self.conn.transaction()?;
        let changed = put(&tx, e)?;
        tx.commit()?;
        Ok(changed)
    }

    /// Merges many in one transaction.
    pub fn put_all<'a>(&mut self, all: impl IntoIterator<Item = &'a Evaluation>) -> Result<Merged> {
        self.merge(all, [])
    }

    /// Merges Evaluations and Annotations in one transaction: nothing when one is refused.
    pub fn merge<'a>(&mut self, evaluations: impl IntoIterator<Item = &'a Evaluation>, annotations: impl IntoIterator<Item = &'a Annotation>) -> Result<Merged> {
        let tx = self.conn.transaction()?;
        let m = merge(&tx, evaluations, annotations)?;
        tx.commit()?;
        Ok(m)
    }

    /// Merges `a`: kept if the store has none for its position, move and Edition, or if it is
    /// greater than that one by `(depth, time, mark, better, motifs)`. True when the store changed.
    pub fn put_annotation(&mut self, a: &Annotation) -> Result<bool> {
        let tx = self.conn.transaction()?;
        let changed = put_annotation(&tx, a)?;
        tx.commit()?;
        Ok(changed)
    }

    /// What Bragd shows for a move and an Engine: as `shown`, its newest Edition's Annotation,
    /// the deepest of the equally new, then the latest.
    pub fn annotation(&self, family: &str, variant: &str, key: u64, mv: &str, engine: &str) -> Result<Option<Annotation>> {
        let all = self.select_annotations("WHERE a.family = ?1 AND a.variant = ?2 AND a.key = ?3 AND a.move = ?4 AND d.engine = ?5", params![family, variant, key as i64, mv, engine])?;
        Ok(all.into_iter().max_by(|a, b| a.edition.newer(&b.edition).unwrap_or(Ordering::Equal).then(a.depth.cmp(&b.depth)).then(a.time.cmp(&b.time))))
    }

    /// Every Annotation in the store, by position, move and Edition id.
    pub fn annotations(&self) -> Result<Vec<Annotation>> {
        self.select_annotations("ORDER BY a.family, a.variant, a.key, a.move, d.id", [])
    }

    fn select_annotations(&self, rest: &str, p: impl rusqlite::Params) -> Result<Vec<Annotation>> {
        Ok(self.select_annotations_seq(rest, p)?.into_iter().map(|(_, a)| a).collect())
    }

    /// As `select_annotations`, each with its change number (`sync`).
    pub(crate) fn select_annotations_seq(&self, rest: &str, p: impl rusqlite::Params) -> Result<Vec<(u64, Annotation)>> {
        let mut st = self.conn.prepare(&format!(
            "SELECT a.family, a.variant, a.key, a.move, d.engine, d.version, d.network, d.image, a.mark, a.better, a.motifs, a.depth, a.time, a.seq \
             FROM annotations a JOIN editions d ON d.n = a.edition {rest}"
        ))?;
        let rows = st.query_map(p, |r| {
            Ok((
                Annotation {
                    family: r.get(0)?,
                    variant: r.get(1)?,
                    key: r.get::<_, i64>(2)? as u64,
                    mv: r.get(3)?,
                    edition: Edition { engine: r.get(4)?, version: r.get(5)?, network: r.get(6)?, image: r.get(7)? },
                    mark: None,
                    better: r.get(9)?,
                    motifs: vec![],
                    depth: r.get(11)?,
                    time: r.get(12)?,
                },
                r.get::<_, String>(8)?,
                r.get::<_, String>(10)?,
                r.get::<_, i64>(13)? as u64,
            ))
        })?;
        rows.map(|row| {
            let (mut a, mark, motifs, seq) = row?;
            (a.mark, a.motifs) = annotation_columns(&mark, &motifs)?;
            Ok((seq, a))
        })
        .collect()
    }

    /// Every Edition's Evaluation of one position.
    pub fn evaluations(&self, family: &str, variant: &str, key: u64) -> Result<Vec<Evaluation>> {
        self.select("WHERE v.family = ?1 AND v.variant = ?2 AND v.key = ?3", params![family, variant, key as i64])
    }

    /// What Bragd shows for a position and an Engine: its newest Edition's Evaluation, the
    /// deepest of the equally new, then the latest.
    pub fn shown(&self, family: &str, variant: &str, key: u64, engine: &str) -> Result<Option<Evaluation>> {
        let all = self.evaluations(family, variant, key)?;
        Ok(all
            .into_iter()
            .filter(|e| e.edition.engine == engine)
            .max_by(|a, b| a.edition.newer(&b.edition).unwrap_or(Ordering::Equal).then(a.depth.cmp(&b.depth)).then(a.time.cmp(&b.time))))
    }

    /// Every Evaluation in the store, by position and Edition id.
    pub fn all(&self) -> Result<Vec<Evaluation>> {
        self.select("ORDER BY v.family, v.variant, v.key, d.id", [])
    }

    /// Whether the store already answers `spot` of `job` well enough (`queue::Job::answered`).
    pub fn answers(&self, job: &queue::Job, spot: &queue::Spot) -> Result<bool> {
        Ok(self.evaluations(&job.family, &job.variant, spot.key)?.iter().any(|e| job.answered(spot, e)))
    }

    /// What each Edition takes: its Evaluations, and its share of the file's bytes by the size
    /// of its rows, its Annotations' included. By Engine, then Edition id.
    pub fn sizes(&self) -> Result<Vec<Size>> {
        let file: i64 = self.conn.query_row("SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()", [], |r| r.get(0))?;
        let mut st = self.conn.prepare(
            "SELECT d.engine, d.version, d.network, d.image, \
             (SELECT count(*) FROM evaluations v WHERE v.edition = d.n), \
             (SELECT coalesce(sum(length(v.line) + length(v.line2) + length(v.family) + length(v.variant) + 64), 0) FROM evaluations v WHERE v.edition = d.n) \
             + (SELECT coalesce(sum(length(a.move) + length(a.better) + length(a.motifs) + length(a.family) + length(a.variant) + 40), 0) FROM annotations a WHERE a.edition = d.n) \
             FROM editions d ORDER BY d.engine, d.id",
        )?;
        let rows = st.query_map([], |r| {
            Ok((Edition { engine: r.get(0)?, version: r.get(1)?, network: r.get(2)?, image: r.get(3)? }, r.get::<_, i64>(4)? as u64, r.get::<_, i64>(5)? as u64))
        })?;
        let rows: Vec<_> = rows.collect::<rusqlite::Result<_>>()?;
        let weight: u64 = rows.iter().map(|r| r.2).sum();
        Ok(rows.into_iter().map(|(edition, rows, w)| Size { edition, rows, bytes: if weight == 0 { 0 } else { (file as u128 * w as u128 / weight as u128) as u64 } }).collect())
    }

    /// Deletes the rows of every Edition older than its Engine's newest (`Edition::newer`;
    /// another network or image of the newest version stays), and gives the space back. The
    /// rows deleted.
    pub fn prune_older(&mut self) -> Result<u64> {
        let editions = self.editions()?;
        let old: Vec<i64> = editions.iter().filter(|(_, e)| editions.iter().any(|(_, o)| o.newer(e) == Some(Ordering::Greater))).map(|(n, _)| *n).collect();
        self.delete(&old)
    }

    /// Deletes the rows of the Engines `engines` (by key), and gives the space back. The rows
    /// deleted.
    pub fn prune_engines(&mut self, engines: &[String]) -> Result<u64> {
        let gone: Vec<i64> = self.editions()?.into_iter().filter(|(_, e)| engines.contains(&e.engine)).map(|(n, _)| n).collect();
        self.delete(&gone)
    }

    fn editions(&self) -> Result<Vec<(i64, Edition)>> {
        let mut st = self.conn.prepare("SELECT n, engine, version, network, image FROM editions")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, Edition { engine: r.get(1)?, version: r.get(2)?, network: r.get(3)?, image: r.get(4)? })))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The Evaluations and Annotations of the Editions numbered `ns` and the Editions themselves
    /// gone, then VACUUM. The Evaluations deleted.
    fn delete(&mut self, ns: &[i64]) -> Result<u64> {
        let tx = self.conn.transaction()?;
        let mut rows = 0;
        for n in ns {
            rows += tx.execute("DELETE FROM evaluations WHERE edition = ?1", [n])? as u64;
            tx.execute("DELETE FROM annotations WHERE edition = ?1", [n])?;
            tx.execute("DELETE FROM editions WHERE n = ?1", [n])?;
        }
        tx.commit()?;
        if !ns.is_empty() {
            self.conn.execute_batch("VACUUM")?;
        }
        Ok(rows)
    }

    fn select(&self, rest: &str, p: impl rusqlite::Params) -> Result<Vec<Evaluation>> {
        Ok(self.select_seq(rest, p)?.into_iter().map(|(_, e)| e).collect())
    }

    /// As `select`, each with its change number (`sync`).
    pub(crate) fn select_seq(&self, rest: &str, p: impl rusqlite::Params) -> Result<Vec<(u64, Evaluation)>> {
        let mut st = self.conn.prepare(&format!(
            "SELECT v.family, v.variant, v.key, d.engine, d.version, d.network, d.image, v.kind, v.score, v.lead, v.depth, v.line, v.time, v.kind2, v.score2, v.lead2, v.line2, v.seq \
             FROM evaluations v JOIN editions d ON d.n = v.edition {rest}"
        ))?;
        let rows = st.query_map(p, |r| {
            Ok((
                Evaluation {
                    family: r.get(0)?,
                    variant: r.get(1)?,
                    key: r.get::<_, i64>(2)? as u64,
                    edition: Edition { engine: r.get(3)?, version: r.get(4)?, network: r.get(5)?, image: r.get(6)? },
                    score: Score::Cp(0),
                    depth: r.get(10)?,
                    line: r.get(11)?,
                    time: r.get(12)?,
                    second: None,
                },
                (r.get::<_, String>(7)?, r.get::<_, f64>(8)?, r.get::<_, f64>(9)?),
                (r.get::<_, String>(13)?, r.get::<_, f64>(14)?, r.get::<_, f64>(15)?, r.get::<_, String>(16)?),
                r.get::<_, i64>(17)? as u64,
            ))
        })?;
        rows.map(|row| {
            let (mut e, (kind, score, lead), (kind2, score2, lead2, line2), seq) = row?;
            e.score = Score::from_columns(&kind, score, lead)?;
            e.second = Second::from_columns(&kind2, score2, lead2, line2)?;
            Ok((seq, e))
        })
        .collect()
    }
}

/// An Annotation's mark and motifs as the store writes them: refused when one is not known.
pub(crate) fn annotation_columns(mark: &str, motifs: &str) -> Result<(Option<Mark>, Vec<Motif>)> {
    let mark = match mark {
        "" => None,
        m => Some(Mark::from_sign(m).ok_or_else(|| Error::Bad(format!("mark {m:?}")))?),
    };
    let motifs = motifs.split_whitespace().map(|m| Motif::from_id(m).ok_or_else(|| Error::Bad(format!("motif {m:?}")))).collect::<Result<_>>()?;
    Ok((mark, motifs))
}

/// The store's own number for an Edition, added when it is new.
fn edition_n(tx: &Connection, d: &Edition) -> Result<i64> {
    tx.prepare_cached("INSERT INTO editions (id, engine, version, network, image) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (id) DO NOTHING")?
        .execute(params![d.id(), d.engine, d.version, d.network, d.image])?;
    Ok(tx.prepare_cached("SELECT n FROM editions WHERE id = ?1")?.query_row([d.id()], |r| r.get(0))?)
}

/// Merges Evaluations and Annotations inside `tx`.
fn merge<'a>(tx: &Connection, evaluations: impl IntoIterator<Item = &'a Evaluation>, annotations: impl IntoIterator<Item = &'a Annotation>) -> Result<Merged> {
    let mut m = Merged::default();
    for e in evaluations {
        m.read += 1;
        m.changed += u64::from(put(tx, e)?);
    }
    for a in annotations {
        m.read += 1;
        m.changed += u64::from(put_annotation(tx, a)?);
    }
    Ok(m)
}

/// The merge rule for Annotations: insert, or replace one that is less by
/// `(depth, time, mark, better, motifs)`.
fn put_annotation(tx: &Connection, a: &Annotation) -> Result<bool> {
    let n = edition_n(tx, &a.edition)?;
    let mark = a.mark.map_or("", Mark::sign);
    let motifs: Vec<&str> = a.motifs.iter().map(|m| m.id()).collect();
    let seq = sync::next(tx)?;
    let changed = tx
        .prepare_cached(
            "INSERT INTO annotations (family, variant, key, move, edition, mark, better, motifs, depth, time, seq) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
             ON CONFLICT (family, variant, key, move, edition) DO UPDATE SET \
             mark = excluded.mark, better = excluded.better, motifs = excluded.motifs, depth = excluded.depth, time = excluded.time, seq = excluded.seq \
             WHERE (excluded.depth, excluded.time, excluded.mark, excluded.better, excluded.motifs) \
                 > (annotations.depth, annotations.time, annotations.mark, annotations.better, annotations.motifs)",
        )?
        .execute(params![a.family, a.variant, a.key as i64, a.mv, n, mark, a.better, motifs.join(" "), a.depth, a.time, seq])?;
    sync::taken(tx, seq, changed > 0)
}

/// The merge rule, in one place: insert, or replace a row that is less by
/// `(depth, time, kind, score, lead, line, kind2, score2, lead2, line2)`.
fn put(tx: &Connection, e: &Evaluation) -> Result<bool> {
    let (kind, score, lead) = e.score.columns();
    let (kind2, score2, lead2, line2) = Second::columns(e.second.as_ref());
    if ![score, lead, score2, lead2].iter().all(|x| x.is_finite()) {
        return Err(Error::Bad(format!("{kind} {score} {lead} {kind2} {score2} {lead2}")));
    }
    let n = edition_n(tx, &e.edition)?;
    let seq = sync::next(tx)?;
    let changed = tx
        .prepare_cached(
            "INSERT INTO evaluations (family, variant, key, edition, kind, score, lead, depth, line, time, kind2, score2, lead2, line2, seq) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15) \
             ON CONFLICT (family, variant, key, edition) DO UPDATE SET \
             kind = excluded.kind, score = excluded.score, lead = excluded.lead, depth = excluded.depth, line = excluded.line, time = excluded.time, \
             kind2 = excluded.kind2, score2 = excluded.score2, lead2 = excluded.lead2, line2 = excluded.line2, seq = excluded.seq \
             WHERE (excluded.depth, excluded.time, excluded.kind, excluded.score, excluded.lead, excluded.line, excluded.kind2, excluded.score2, excluded.lead2, excluded.line2) \
                 > (evaluations.depth, evaluations.time, evaluations.kind, evaluations.score, evaluations.lead, evaluations.line, \
                    evaluations.kind2, evaluations.score2, evaluations.lead2, evaluations.line2)",
        )?
        .execute(params![e.family, e.variant, e.key as i64, n, kind, score, lead, e.depth, e.line, e.time, kind2, score2, lead2, line2, seq])?;
    sync::taken(tx, seq, changed > 0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fresh folder of its own for a test, under the system's temporary folder.
    pub(crate) fn dir(name: &str) -> PathBuf {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!("bragd-positions-{name}-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    pub(crate) fn sf(version: &str) -> Edition {
        Edition { engine: "stockfish".into(), version: version.into(), network: None, image: Some("sha256:fc7eee4aada2ef3e3debd59b4bbfb1e3907380062973a2863829410d0c1aa340".into()) }
    }

    pub(crate) fn eval(key: u64, edition: Edition, depth: u32, time: i64, score: Score) -> Evaluation {
        Evaluation { family: "chess".into(), variant: "chess".into(), key, edition, score, depth, line: format!("e2e4 d{depth}"), time, second: None }
    }

    fn store(name: &str) -> (PathBuf, Store) {
        let d = dir(name);
        let s = Store::open(&d.join("positions.sqlite")).unwrap();
        (d, s)
    }

    #[test]
    fn the_deeper_row_wins_and_editions_are_kept_apart() {
        let (_d, mut s) = store("deeper");
        assert!(s.put(&eval(1, sf("Stockfish 17"), 20, 100, Score::Cp(30))).unwrap());
        assert!(!s.put(&eval(1, sf("Stockfish 17"), 18, 200, Score::Cp(10))).unwrap(), "shallower, though later");
        assert!(s.put(&eval(1, sf("Stockfish 17"), 24, 50, Score::Mate(5))).unwrap());
        assert!(s.put(&eval(1, sf("Stockfish 17.1"), 12, 10, Score::Cp(-5))).unwrap());
        let all = s.evaluations("chess", "chess", 1).unwrap();
        assert_eq!(all.len(), 2);
        let old = all.iter().find(|e| e.edition.version == "Stockfish 17").unwrap();
        assert_eq!((old.depth, old.score), (24, Score::Mate(5)));
        // Shown: the newest Edition, though it is shallower.
        assert_eq!(s.shown("chess", "chess", 1, "stockfish").unwrap().unwrap().edition.version, "Stockfish 17.1");
        assert_eq!(s.shown("chess", "chess", 1, "lc0").unwrap(), None);
        assert!(s.evaluations("chess", "chess", 2).unwrap().is_empty());
        assert!(s.evaluations("go", "chess", 1).unwrap().is_empty());
    }

    /// Peers merge stores in whatever order rows arrive: every order, and every row twice,
    /// gives the same store.
    #[test]
    fn a_merge_is_the_same_in_any_order_and_any_number_of_times() {
        let rows = [
            eval(7, sf("Stockfish 17"), 20, 100, Score::Cp(30)),
            eval(7, sf("Stockfish 17"), 20, 100, Score::Cp(31)),
            eval(7, sf("Stockfish 17"), 20, 90, Score::Cp(40)),
            eval(7, sf("Stockfish 17"), 22, 80, Score::Mate(-3)),
            eval(7, sf("Stockfish 17.1"), 15, 80, Score::Cp(0)),
            eval(8, sf("Stockfish 17"), 1, 1, Score::Cp(1)),
            Evaluation { family: "go".into(), variant: "go-19".into(), key: 7, edition: Edition { engine: "katago".into(), version: "KataGo 1.18.2".into(), network: Some("abc".into()), image: None }, score: Score::WinRate { win: 0.6, lead: 2.5 }, depth: 400, line: "Q16 D4".into(), time: 5, second: Some(Second { score: Score::WinRate { win: 0.4, lead: -1.0 }, line: "D4".into() }) },
            Evaluation { family: "backgammon".into(), variant: "backgammon".into(), key: 7, edition: Edition { engine: "gnubg".into(), version: "GNU Backgammon 1.08.003".into(), network: None, image: None }, score: Score::Equity(-0.25), depth: 2, line: "24/18 13/11".into(), time: 5, second: None },
            // The same as row 3 but for the second choice: with one beats without, the greater one wins.
            Evaluation { second: Some(Second { score: Score::Cp(-200), line: "d2d4".into() }), ..eval(7, sf("Stockfish 17"), 22, 80, Score::Mate(-3)) },
            Evaluation { second: Some(Second { score: Score::Mate(-9), line: "c2c4 e7e5".into() }), ..eval(7, sf("Stockfish 17"), 22, 80, Score::Mate(-3)) },
            Evaluation { second: Some(Second { score: Score::Cp(5), line: "g1f3".into() }), ..eval(8, sf("Stockfish 17"), 1, 1, Score::Cp(1)) },
        ];
        let mut want = None;
        // Every rotation and its reverse: each row comes first and last at least once.
        for start in 0..rows.len() {
            for reverse in [false, true] {
                let mut order: Vec<&Evaluation> = rows.iter().cycle().skip(start).take(rows.len()).collect();
                if reverse {
                    order.reverse();
                }
                let (_d, mut s) = store("merge");
                s.put_all(order.iter().copied()).unwrap();
                let once = s.all().unwrap();
                assert_eq!(s.put_all(order.iter().copied()).unwrap().changed, 0, "a second merge changes nothing");
                assert_eq!(s.all().unwrap(), once);
                match &want {
                    None => want = Some(once),
                    Some(w) => assert_eq!(&once, w, "order {start} {reverse}"),
                }
            }
        }
        let want = want.unwrap();
        assert_eq!(want.len(), 5);
        let sf17 = want.iter().find(|e| e.key == 7 && e.edition.version == "Stockfish 17").unwrap();
        assert_eq!((sf17.depth, sf17.score), (22, Score::Mate(-3)));
        assert_eq!(sf17.second, rows[9].second, "of two seconds the greater, by kind first");
        assert!(want.contains(&rows[6]) && want.contains(&rows[7]) && want.contains(&rows[10]));
    }

    #[test]
    fn sizes_by_edition_and_pruning() {
        let (_d, mut s) = store("prune");
        let lc0 = |v: &str| Edition { engine: "lc0".into(), version: v.into(), network: None, image: None };
        let other_image = Edition { image: Some("sha256:00".into()), ..sf("Stockfish 17.1") };
        for k in 0..30 {
            s.put(&eval(k, sf("Stockfish 17"), 20, 1, Score::Cp(1))).unwrap();
        }
        for k in 0..10 {
            s.put(&eval(k, sf("Stockfish 17.1"), 20, 1, Score::Cp(1))).unwrap();
            s.put(&eval(k, other_image.clone(), 20, 1, Score::Cp(1))).unwrap();
            s.put(&eval(k, lc0("Lc0 0.31"), 20, 1, Score::Cp(1))).unwrap();
        }
        // An Annotation by the old Edition goes with it.
        s.put_annotation(&Annotation { family: "chess".into(), variant: "chess".into(), key: 1, mv: "e2e4".into(), edition: sf("Stockfish 17"), mark: None, better: String::new(), motifs: vec![], depth: 20, time: 1 }).unwrap();
        let sizes = s.sizes().unwrap();
        assert_eq!(sizes.iter().map(|z| (z.edition.version.as_str(), z.rows)).collect::<Vec<_>>(), [("Lc0 0.31", 10), ("Stockfish 17.1", 10), ("Stockfish 17.1", 10), ("Stockfish 17", 30)]);
        assert!(sizes[3].bytes > 2 * sizes[1].bytes && sizes[1].bytes > 0, "{sizes:?}");
        // Older Editions: Stockfish 17 goes; both builds of 17.1 and the only Lc0 stay.
        assert_eq!(s.prune_older().unwrap(), 30);
        assert_eq!(s.sizes().unwrap().len(), 3);
        assert_eq!(s.prune_older().unwrap(), 0);
        // A removed Engine.
        assert_eq!(s.prune_engines(&["lc0".into()]).unwrap(), 10);
        assert!(s.sizes().unwrap().iter().all(|z| z.edition.engine == "stockfish"));
        assert_eq!(s.all().unwrap().len(), 20);
    }

    #[test]
    fn a_job_skips_what_the_store_answers() {
        let (_d, mut s) = store("answers");
        let spot = queue::Spot { key: 5, setup: "x".into(), moves: vec![], count: 1 };
        let job = queue::Job::new("chess", "chess", "stockfish", Some("Stockfish 17".into()), queue::Per::Depth(20), vec![spot.clone()], false, "");
        assert!(!s.answers(&job, &spot).unwrap());
        s.put(&eval(5, sf("Stockfish 16"), 30, 1, Score::Cp(1))).unwrap();
        assert!(!s.answers(&job, &spot).unwrap(), "too old");
        s.put(&eval(5, sf("Stockfish 17"), 22, 1, Score::Cp(1))).unwrap();
        assert!(s.answers(&job, &spot).unwrap());
    }

    #[test]
    fn a_score_that_is_not_a_number_is_refused() {
        let (_d, mut s) = store("nan");
        assert!(matches!(s.put(&eval(1, sf("Stockfish 17"), 1, 1, Score::Equity(f64::NAN))), Err(Error::Bad(_))));
        assert!(s.all().unwrap().is_empty());
    }

    #[test]
    fn a_store_from_a_newer_bragd_is_left_alone() {
        let d = dir("newer");
        let p = d.join("positions.sqlite");
        drop(Store::open(&p).unwrap());
        Connection::open(&p).unwrap().execute_batch("PRAGMA user_version = 99").unwrap();
        assert!(matches!(Store::open(&p), Err(Error::Format(bragd_core::format::Error::Newer { version: 99, .. }))));
        // Opened again at the same version, nothing is made twice.
        Connection::open(&p).unwrap().execute_batch(&format!("PRAGMA user_version = {}", FORMAT.version())).unwrap();
        Store::open(&p).unwrap();
    }

    /// Windows and the Helper open the store at once: each gets it whole, made once.
    #[test]
    fn a_new_store_opened_at_once_from_many_threads_is_made_once() {
        let d = dir("at-once");
        let p = d.join("positions.sqlite");
        let all: Vec<_> = (0..16).map(|_| { let p = p.clone(); std::thread::spawn(move || Store::open(&p).map(|_| ())) }).collect();
        for t in all {
            t.join().unwrap().unwrap();
        }
    }

    #[test]
    fn a_second_score_that_is_not_a_number_is_refused() {
        let (_d, mut s) = store("nan2");
        let e = Evaluation { second: Some(Second { score: Score::Equity(f64::INFINITY), line: String::new() }), ..eval(1, sf("Stockfish 17"), 1, 1, Score::Equity(0.0)) };
        assert!(matches!(s.put(&e), Err(Error::Bad(_))));
    }

    pub(crate) fn note(key: u64, mv: &str, edition: Edition, depth: u32, time: i64, mark: Option<Mark>) -> Annotation {
        Annotation { family: "chess".into(), variant: "chess".into(), key, mv: mv.into(), edition, mark, better: if mark.is_some() { "g1f3 b8c6".into() } else { String::new() }, motifs: if mark == Some(Mark::Blunder) { vec![Motif::Hangs, Motif::Fork] } else { vec![] }, depth, time }
    }

    /// Annotations merge as Evaluations do: the deeper wins, every order gives the same store,
    /// and moves and Editions are kept apart.
    #[test]
    fn annotations_merge_by_the_same_rule() {
        let rows = [
            note(5, "e2e4", sf("Stockfish 17"), 20, 100, None),
            note(5, "e2e4", sf("Stockfish 17"), 20, 100, Some(Mark::Good)),
            note(5, "e2e4", sf("Stockfish 17"), 18, 300, Some(Mark::Blunder)),
            note(5, "e2e4", sf("Stockfish 17"), 22, 50, Some(Mark::Mistake)),
            note(5, "d2d4", sf("Stockfish 17"), 10, 50, Some(Mark::Blunder)),
            note(5, "e2e4", sf("Stockfish 17.1"), 5, 50, Some(Mark::Inaccuracy)),
        ];
        let mut want = None;
        for start in 0..rows.len() {
            for reverse in [false, true] {
                let mut order: Vec<&Annotation> = rows.iter().cycle().skip(start).take(rows.len()).collect();
                if reverse {
                    order.reverse();
                }
                let (_d, mut s) = store("notes");
                s.merge([], order.iter().copied()).unwrap();
                let once = s.annotations().unwrap();
                assert_eq!(s.merge([], order.iter().copied()).unwrap(), Merged { read: 6, changed: 0 });
                match &want {
                    None => want = Some(once),
                    Some(w) => assert_eq!(&once, w, "order {start} {reverse}"),
                }
            }
        }
        let want = want.unwrap();
        assert_eq!(want.len(), 3);
        let (_d, mut s) = store("notes-shown");
        s.merge([], &rows).unwrap();
        let e4 = s.annotation("chess", "chess", 5, "e2e4", "stockfish").unwrap().unwrap();
        assert_eq!((e4.edition.version.as_str(), e4.mark), ("Stockfish 17.1", Some(Mark::Inaccuracy)), "the newest Edition shows");
        let old = want.iter().find(|a| a.mv == "e2e4" && a.edition.version == "Stockfish 17").unwrap();
        assert_eq!((old.depth, old.mark), (22, Some(Mark::Mistake)));
        let d4 = want.iter().find(|a| a.mv == "d2d4").unwrap();
        assert_eq!((d4.better.as_str(), d4.motifs.as_slice()), ("g1f3 b8c6", [Motif::Hangs, Motif::Fork].as_slice()));
        assert_eq!(s.annotation("chess", "chess", 5, "c2c4", "stockfish").unwrap(), None);
        assert_eq!(s.annotation("chess", "chess", 5, "e2e4", "lc0").unwrap(), None);
    }

    #[test]
    fn an_unknown_mark_is_refused_on_reading() {
        assert!(matches!(annotation_columns("?!?", ""), Err(Error::Bad(_))));
        assert!(matches!(annotation_columns("", "skewer"), Err(Error::Bad(_))));
        assert_eq!(annotation_columns("!!", "pin fork").unwrap(), (Some(Mark::Brilliant), vec![Motif::Pin, Motif::Fork]));
    }

    #[test]
    fn the_store_lives_in_the_data_folder() {
        let p = Store::path().unwrap();
        assert!(p.starts_with(bragd_core::dirs::app_data().unwrap()));
        assert_eq!(p.file_name().unwrap(), "positions.sqlite");
    }
}
