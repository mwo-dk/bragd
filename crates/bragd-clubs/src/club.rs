//! A Club file: one SQLite file (`*.bragd-club`) with the Club's name, its rating system, its
//! members with their roles, IDs and club ratings, and the audit log of every rating change.
//! Copying the file shares the Club.
//!
//! A lock stops two directors working on it at once: `<file>.lock` beside it names who has it
//! open, and on which machine. It is a file of its own, so a copy of the Club carries no lock.
//! Someone else's lock opens the Club read-only; *Take over* replaces it, and the audit log says
//! so. A lock of the same user on the same machine is this user's own, left by a window that
//! closed without letting go.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use bragd_core::t;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::rating::{self, Game, Rating, System};
use crate::league::League;
use crate::tournament::{self, Tournament};

/// A Club file's extension.
pub const EXT: &str = "bragd-club";
/// The file format this Bragd writes (`PRAGMA user_version`). A newer one is not opened.
const FORMAT: i64 = 1;

/// A member's role in the Club.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Director,
    Arbiter,
    Member,
}

impl Role {
    fn id(self) -> &'static str {
        match self {
            Role::Director => "director",
            Role::Arbiter => "arbiter",
            Role::Member => "member",
        }
    }

    fn of(s: &str) -> Role {
        match s {
            "director" => Role::Director,
            "arbiter" => Role::Arbiter,
            _ => Role::Member,
        }
    }
}

/// What a director enters for a member.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Details {
    pub name: String,
    pub role: Option<Role>,
    /// The FIDE ID: digits.
    pub fide_id: String,
    /// The national federation's ID, and the federation's three-letter code (`DEN`).
    pub national_id: String,
    pub federation: String,
    /// The Lichess username.
    pub lichess: String,
}

/// A member as the Club page shows them.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Member {
    pub id: i64,
    pub name: String,
    pub role: Role,
    pub fide_id: String,
    pub national_id: String,
    pub federation: String,
    pub lichess: String,
    pub rating: Rating,
}

/// An entry of the audit log: who, when, the old value, the new value and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Unix time.
    pub at: u64,
    pub who: String,
    /// The member it concerns, by name as it was then; empty for the Club itself. In a
    /// Tournament's log: the player or the board.
    pub member: String,
    /// `rating`, `system` or `lock`; in a Tournament's log `pairing`, `result` or `status`.
    pub what: String,
    pub old: String,
    pub new: String,
    pub why: String,
}

/// Who holds a Club's lock.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holder {
    pub user: String,
    pub host: String,
    /// Unix time it was taken.
    pub since: u64,
}

impl Holder {
    /// This user on this machine, now.
    pub fn me() -> Holder {
        Holder { user: user(), host: host().to_string(), since: bragd_core::fs::now() }
    }

    fn same(&self, other: &Holder) -> bool {
        self.user == other.user && self.host == other.host
    }
}

fn user() -> String {
    std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default()
}

/// The machine's name, asked once.
fn host() -> &'static str {
    static HOST: OnceLock<String> = OnceLock::new();
    HOST.get_or_init(|| {
        #[cfg(unix)]
        {
            let mut buf = [0u8; 256];
            // SAFETY: gethostname writes at most `buf.len()` bytes into `buf`.
            if unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } == 0 {
                let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
                return String::from_utf8_lossy(&buf[..end]).into_owned();
            }
            String::new()
        }
        #[cfg(not(unix))]
        std::env::var("COMPUTERNAME").unwrap_or_default()
    })
}

/// Why a change was refused.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// Someone else has the Club open.
    ReadOnly(Holder),
    /// Only a director may do this.
    NotDirector,
    /// The Club would have no director left.
    LastDirector,
    /// A field is not valid: its text key.
    Invalid(&'static str),
    /// The file was written by a newer Bragd.
    Newer,
    Exists,
    NoMember,
    Db(String),
    Io(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Error::ReadOnly(h) => t!("clubs.error.read_only", "user" => h.user, "host" => h.host),
            Error::NotDirector => t!("clubs.error.not_director"),
            Error::LastDirector => t!("clubs.error.last_director"),
            Error::Invalid(key) => t!(key),
            Error::Newer => t!("clubs.error.newer"),
            Error::Exists => t!("clubs.error.exists"),
            Error::NoMember => t!("clubs.error.no_member"),
            Error::Db(e) | Error::Io(e) => e.clone(),
        };
        f.write_str(&s)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Error {
        Error::Db(e.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::Io(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// The lock file of the Club at `path`.
pub fn lock_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".lock");
    PathBuf::from(s)
}

fn read_lock(path: &Path) -> Option<Holder> {
    serde_json::from_slice(&std::fs::read(lock_path(path)).ok()?).ok()
}

fn write_lock(path: &Path, h: &Holder) -> Result<()> {
    bragd_core::fs::replace(&lock_path(path), &serde_json::to_vec(h).map_err(|e| Error::Io(e.to_string()))?, false)?;
    Ok(())
}

/// Takes the lock of the Club at `path`, unless someone else holds it: then that holder.
fn take_lock(path: &Path, me: &Holder) -> Result<Option<Holder>> {
    use std::io::Write;
    loop {
        match std::fs::OpenOptions::new().write(true).create_new(true).open(lock_path(path)) {
            Ok(mut f) => {
                f.write_all(&serde_json::to_vec(me).map_err(|e| Error::Io(e.to_string()))?)?;
                return Ok(None);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => match read_lock(path) {
                Some(h) if !h.same(me) => return Ok(Some(h)),
                Some(_) => {
                    write_lock(path, me)?;
                    return Ok(None);
                }
                // Being written this very moment, or unreadable: one that cannot be read is
                // nobody's, and goes.
                None => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    if read_lock(path).is_none() {
                        let _ = std::fs::remove_file(lock_path(path));
                    }
                }
            },
            Err(e) => return Err(e.into()),
        }
    }
}

/// A Club file, open.
pub struct Club {
    path: PathBuf,
    db: Connection,
    me: Holder,
    /// Someone else's lock: the Club is read-only.
    held_by: Option<Holder>,
}

/// A Club as its page shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct View {
    pub path: PathBuf,
    pub name: String,
    pub system: System,
    /// Someone else has it open: it is read-only until taken over.
    pub held_by: Option<Holder>,
    pub members: Vec<Member>,
    /// Newest first.
    pub log: Vec<Entry>,
    pub tournaments: Vec<Listed>,
    pub leagues: Vec<LeagueListed>,
}

/// A League as the Club's list shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LeagueListed {
    pub id: i64,
    pub name: String,
    pub date: String,
    pub teams: u32,
    pub rounds: u32,
    /// Rounds paired so far.
    pub paired: u32,
}

/// A Tournament as the Club's list shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Listed {
    pub id: i64,
    pub name: String,
    pub date: String,
    pub system: tournament::System,
    pub rounds: u32,
    /// Rounds paired so far.
    pub paired: u32,
    pub players: u32,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS club (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS members (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    role TEXT NOT NULL,
    fide_id TEXT NOT NULL DEFAULT '',
    national_id TEXT NOT NULL DEFAULT '',
    federation TEXT NOT NULL DEFAULT '',
    lichess TEXT NOT NULL DEFAULT '',
    rating REAL NOT NULL,
    deviation REAL NOT NULL,
    volatility REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS log (
    id INTEGER PRIMARY KEY,
    at INTEGER NOT NULL,
    who TEXT NOT NULL,
    member TEXT NOT NULL,
    what TEXT NOT NULL,
    old TEXT NOT NULL,
    new TEXT NOT NULL,
    why TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS tournaments (id INTEGER PRIMARY KEY, data TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS leagues (id INTEGER PRIMARY KEY, data TEXT NOT NULL);
";

fn valid(d: &Details) -> Result<()> {
    let ok = |s: &str, max: usize, f: fn(char) -> bool| s.chars().count() <= max && s.chars().all(f);
    if d.name.trim().is_empty() || d.name.chars().count() > 100 {
        return Err(Error::Invalid("clubs.error.name"));
    }
    if !ok(&d.fide_id, 12, |c| c.is_ascii_digit()) {
        return Err(Error::Invalid("clubs.error.fide_id"));
    }
    if !ok(&d.national_id, 20, |c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(Error::Invalid("clubs.error.national_id"));
    }
    if !(d.federation.is_empty() || d.federation.len() == 3 && d.federation.chars().all(|c| c.is_ascii_uppercase())) {
        return Err(Error::Invalid("clubs.error.federation"));
    }
    // Lichess usernames: 2 to 30 letters, digits, `_` and `-`.
    if !(d.lichess.is_empty() || d.lichess.len() >= 2 && ok(&d.lichess, 30, |c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))) {
        return Err(Error::Invalid("clubs.error.lichess"));
    }
    Ok(())
}

fn round(x: f64) -> String {
    format!("{x:.0}")
}

impl Club {
    /// A new Club at `path`, with `director` its first member and director. The file must not
    /// exist yet.
    pub fn create(path: &Path, name: &str, system: System, director: &str) -> Result<Club> {
        if name.trim().is_empty() {
            return Err(Error::Invalid("clubs.error.name"));
        }
        valid(&Details { name: director.into(), ..Details::default() })?;
        if path.exists() {
            return Err(Error::Exists);
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let me = Holder::me();
        // A stale lock beside a file that is gone is nobody's.
        let _ = std::fs::remove_file(lock_path(path));
        take_lock(path, &me)?;
        let db = Connection::open(path)?;
        db.execute_batch(SCHEMA)?;
        db.pragma_update(None, "user_version", FORMAT)?;
        db.execute("INSERT INTO club (key, value) VALUES ('name', ?1), ('system', ?2)", params![name.trim(), system_id(system)])?;
        let club = Club { path: path.into(), db, me, held_by: None };
        club.insert(&Details { name: director.trim().into(), role: Some(Role::Director), ..Details::default() }, Rating::default())?;
        Ok(club)
    }

    /// The Club at `path`: for editing when its lock is free or this user's own, otherwise
    /// read-only, naming who has it.
    pub fn open(path: &Path) -> Result<Club> {
        if !path.is_file() {
            return Err(Error::Io(t!("clubs.error.missing", "path" => path.display())));
        }
        let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > FORMAT {
            return Err(Error::Newer);
        }
        let me = Holder::me();
        let held_by = take_lock(path, &me)?;
        if held_by.is_none() {
            // A Club from before Tournaments gets their table.
            db.execute_batch(SCHEMA)?;
        }
        Ok(Club { path: path.into(), db, me, held_by })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read_only(&self) -> bool {
        self.held_by.is_some()
    }

    fn value(&self, key: &str) -> Result<String> {
        Ok(self.db.query_row("SELECT value FROM club WHERE key = ?1", [key], |r| r.get(0)).optional()?.unwrap_or_default())
    }

    pub fn name(&self) -> Result<String> {
        self.value("name")
    }

    pub fn system(&self) -> Result<System> {
        Ok(if self.value("system")? == "elo" { System::Elo } else { System::Glicko2 })
    }

    pub fn members(&self) -> Result<Vec<Member>> {
        let mut q = self.db.prepare("SELECT id, name, role, fide_id, national_id, federation, lichess, rating, deviation, volatility FROM members ORDER BY name COLLATE NOCASE, id")?;
        let rows = q.query_map([], |r| {
            Ok(Member {
                id: r.get(0)?,
                name: r.get(1)?,
                role: Role::of(&r.get::<_, String>(2)?),
                fide_id: r.get(3)?,
                national_id: r.get(4)?,
                federation: r.get(5)?,
                lichess: r.get(6)?,
                rating: Rating { rating: r.get(7)?, deviation: r.get(8)?, volatility: r.get(9)? },
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    fn member(&self, id: i64) -> Result<Member> {
        self.members()?.into_iter().find(|m| m.id == id).ok_or(Error::NoMember)
    }

    /// The audit log, newest first.
    pub fn log(&self) -> Result<Vec<Entry>> {
        let mut q = self.db.prepare("SELECT at, who, member, what, old, new, why FROM log ORDER BY id DESC")?;
        let rows = q.query_map([], |r| Ok(Entry { at: r.get::<_, i64>(0)? as u64, who: r.get(1)?, member: r.get(2)?, what: r.get(3)?, old: r.get(4)?, new: r.get(5)?, why: r.get(6)? }))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn view(&self) -> Result<View> {
        Ok(View { path: self.path.clone(), name: self.name()?, system: self.system()?, held_by: self.held_by.clone(), members: self.members()?, log: self.log()?, tournaments: self.tournaments()?, leagues: self.leagues()? })
    }

    /// The Club's Tournaments, newest first.
    pub fn tournaments(&self) -> Result<Vec<Listed>> {
        // A Club from before Tournaments, open read-only, has no table for them.
        let has: bool = self.db.query_row("SELECT count(*) > 0 FROM sqlite_master WHERE name = 'tournaments'", [], |r| r.get(0))?;
        if !has {
            return Ok(vec![]);
        }
        let mut q = self.db.prepare("SELECT id, data FROM tournaments ORDER BY id DESC")?;
        let rows = q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = vec![];
        for row in rows {
            let (id, data) = row?;
            let t = parse(&data)?;
            out.push(Listed { id, name: t.name, date: t.date, system: t.system, rounds: t.rounds, paired: t.played.len() as u32, players: t.players.len() as u32 });
        }
        Ok(out)
    }

    pub fn tournament(&self, id: i64) -> Result<Tournament> {
        let data: String = self.db.query_row("SELECT data FROM tournaments WHERE id = ?1", [id], |r| r.get(0)).optional()?.ok_or(Error::Invalid("tournaments.error.missing"))?;
        parse(&data)
    }

    /// Adds a Tournament, by a director or an arbiter; its id.
    pub fn add_tournament(&self, by: i64, t: &Tournament) -> Result<i64> {
        let who = self.official(by)?;
        let mut t = t.clone();
        if t.arbiter.is_empty() {
            t.arbiter = who;
        }
        self.db.execute("INSERT INTO tournaments (data) VALUES (?1)", [json(&t)?])?;
        Ok(self.db.last_insert_rowid())
    }

    /// Runs `f` on Tournament `id` as the director or arbiter `by`, whose name it is given for
    /// the log, and keeps what it changed. Nothing is kept when `f` fails.
    pub fn change_tournament<R>(&self, by: i64, id: i64, f: impl FnOnce(&mut Tournament, &str) -> Result<R>) -> Result<R> {
        let who = self.official(by)?;
        let mut t = self.tournament(id)?;
        let r = f(&mut t, &who)?;
        self.db.execute("UPDATE tournaments SET data = ?2 WHERE id = ?1", params![id, json(&t)?])?;
        Ok(r)
    }

    /// Rates the members from the games they played each other in Tournament `id`, as one
    /// rating period, once; the audit log names the Tournament.
    pub fn rate_tournament(&self, by: i64, id: i64) -> Result<()> {
        let who = self.director(by)?;
        let mut t = self.tournament(id)?;
        if t.rated {
            return Err(Error::Invalid("tournaments.error.rated"));
        }
        if t.played.iter().any(|r| !r.complete()) {
            return Err(Error::Invalid("tournaments.error.results_missing"));
        }
        let tx = self.db.unchecked_transaction()?;
        self.rate_period(&who, &t.member_games(), &t.name)?;
        t.rated = true;
        self.db.execute("UPDATE tournaments SET data = ?2 WHERE id = ?1", params![id, json(&t)?])?;
        tx.commit()?;
        Ok(())
    }

    /// A director replaces the last Round's pairing of Tournament `id` by hand, with a reason;
    /// the warnings it was kept with.
    pub fn set_pairing(&self, by: i64, id: i64, pairs: &[(usize, usize)], bye: Option<usize>, why: &str) -> Result<Vec<tournament::Warning>> {
        self.director(by)?;
        self.change_tournament(by, id, |t, who| t.set_pairing(who, pairs, bye, why))
    }

    /// Deletes a Tournament; only a director may.
    /// The Club's Leagues, newest first.
    pub fn leagues(&self) -> Result<Vec<LeagueListed>> {
        // A Club from before Leagues, open read-only, has no table for them.
        let has: bool = self.db.query_row("SELECT count(*) > 0 FROM sqlite_master WHERE name = 'leagues'", [], |r| r.get(0))?;
        if !has {
            return Ok(vec![]);
        }
        let mut q = self.db.prepare("SELECT id, data FROM leagues ORDER BY id DESC")?;
        let rows = q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = vec![];
        for row in rows {
            let (id, data) = row?;
            let l = parse_league(&data)?;
            out.push(LeagueListed { id, rounds: l.rounds(), name: l.name, date: l.date, teams: l.teams.len() as u32, paired: l.played.len() as u32 });
        }
        Ok(out)
    }

    pub fn league(&self, id: i64) -> Result<League> {
        let data: String = self.db.query_row("SELECT data FROM leagues WHERE id = ?1", [id], |r| r.get(0)).optional()?.ok_or(Error::Invalid("leagues.error.missing"))?;
        parse_league(&data)
    }

    /// A new League, set up by a director or an arbiter.
    pub fn add_league(&self, by: i64, l: &League) -> Result<i64> {
        self.official(by)?;
        self.db.execute("INSERT INTO leagues (data) VALUES (?1)", [json(l)?])?;
        Ok(self.db.last_insert_rowid())
    }

    /// Changes a League as `f` says, by a director or an arbiter (named to `f` for the log);
    /// nothing is kept when `f` fails.
    pub fn change_league<R>(&self, by: i64, id: i64, f: impl FnOnce(&mut League, &str) -> Result<R>) -> Result<R> {
        let who = self.official(by)?;
        let mut l = self.league(id)?;
        let r = f(&mut l, &who)?;
        self.db.execute("UPDATE leagues SET data = ?2 WHERE id = ?1", params![id, json(&l)?])?;
        Ok(r)
    }

    pub fn remove_league(&self, by: i64, id: i64) -> Result<()> {
        self.director(by)?;
        self.db.execute("DELETE FROM leagues WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn remove_tournament(&self, by: i64, id: i64) -> Result<()> {
        self.director(by)?;
        self.db.execute("DELETE FROM tournaments WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Like `director`, but an arbiter may also run a Tournament.
    fn official(&self, by: i64) -> Result<String> {
        match self.director(by) {
            Err(Error::NotDirector) => match self.member(by) {
                Ok(m) if m.role == Role::Arbiter => self.director_free().map(|_| m.name),
                _ => Err(Error::NotDirector),
            },
            r => r,
        }
    }

    /// Checks that the Club is not someone else's to change.
    fn director_free(&self) -> Result<()> {
        if let Some(h) = &self.held_by {
            return Err(Error::ReadOnly(h.clone()));
        }
        if let Some(h) = read_lock(&self.path).filter(|h| !h.same(&self.me)) {
            return Err(Error::ReadOnly(h));
        }
        Ok(())
    }

    /// Checks that the Club may be changed, by the member `by` acting as a director; their name.
    fn director(&self, by: i64) -> Result<String> {
        // The lock may have been taken over since this window opened the Club.
        self.director_free()?;
        let m = self.member(by).map_err(|_| Error::NotDirector)?;
        if m.role != Role::Director {
            return Err(Error::NotDirector);
        }
        Ok(m.name)
    }

    fn insert(&self, d: &Details, r: Rating) -> Result<i64> {
        self.db.execute(
            "INSERT INTO members (name, role, fide_id, national_id, federation, lichess, rating, deviation, volatility) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![d.name.trim(), d.role.unwrap_or(Role::Member).id(), d.fide_id, d.national_id, d.federation, d.lichess, r.rating, r.deviation, r.volatility],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    fn write_log(&self, who: &str, member: &str, what: &str, old: &str, new: &str, why: &str) -> Result<()> {
        self.db.execute(
            "INSERT INTO log (at, who, member, what, old, new, why) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![bragd_core::fs::now() as i64, who, member, what, old, new, why],
        )?;
        Ok(())
    }

    /// A new member, starting at `rating` (a rating list's, say) or the system's default.
    pub fn add(&self, by: i64, d: &Details, rating: Option<f64>) -> Result<i64> {
        self.director(by)?;
        valid(d)?;
        let mut r = Rating::default();
        if let Some(x) = rating {
            check_rating(x)?;
            r.rating = x;
        }
        self.insert(d, r)
    }

    /// A member's name, role and IDs.
    pub fn update(&self, by: i64, id: i64, d: &Details) -> Result<()> {
        self.director(by)?;
        valid(d)?;
        let m = self.member(id)?;
        let role = d.role.unwrap_or(m.role);
        if m.role == Role::Director && role != Role::Director && self.directors()? == 1 {
            return Err(Error::LastDirector);
        }
        self.db.execute(
            "UPDATE members SET name = ?2, role = ?3, fide_id = ?4, national_id = ?5, federation = ?6, lichess = ?7 WHERE id = ?1",
            params![id, d.name.trim(), role.id(), d.fide_id, d.national_id, d.federation, d.lichess],
        )?;
        Ok(())
    }

    pub fn remove(&self, by: i64, id: i64) -> Result<()> {
        self.director(by)?;
        if self.member(id)?.role == Role::Director && self.directors()? == 1 {
            return Err(Error::LastDirector);
        }
        self.db.execute("DELETE FROM members WHERE id = ?1", [id])?;
        Ok(())
    }

    fn directors(&self) -> Result<i64> {
        Ok(self.db.query_row("SELECT count(*) FROM members WHERE role = 'director'", [], |r| r.get(0))?)
    }

    /// A director sets a member's rating by hand, saying why; the audit log keeps it.
    pub fn set_rating(&self, by: i64, id: i64, rating: f64, why: &str) -> Result<()> {
        let who = self.director(by)?;
        check_rating(rating)?;
        if why.trim().is_empty() {
            return Err(Error::Invalid("clubs.error.why"));
        }
        let m = self.member(id)?;
        let tx = self.db.unchecked_transaction()?;
        self.db.execute("UPDATE members SET rating = ?2 WHERE id = ?1", params![id, rating])?;
        self.write_log(&who, &m.name, "rating", &round(m.rating.rating), &round(rating), why.trim())?;
        tx.commit()?;
        Ok(())
    }

    /// A director changes how the Club rates; the ratings stay as they are.
    pub fn set_system(&self, by: i64, system: System, why: &str) -> Result<()> {
        let who = self.director(by)?;
        let old = self.system()?;
        if old == system {
            return Ok(());
        }
        let tx = self.db.unchecked_transaction()?;
        self.db.execute("UPDATE club SET value = ?1 WHERE key = 'system'", [system_id(system)])?;
        self.write_log(&who, "", "system", system_id(old), system_id(system), why.trim())?;
        tx.commit()?;
        Ok(())
    }

    /// One rating period: each game is (white, black, white's score), every rating worked out
    /// from the ratings before the period. Logged as `who` did it, for `why` (a Tournament).
    pub fn rate(&self, by: i64, games: &[(i64, i64, f64)], why: &str) -> Result<()> {
        let who = self.director(by)?;
        let tx = self.db.unchecked_transaction()?;
        self.rate_period(&who, games, why)?;
        tx.commit()?;
        Ok(())
    }

    /// `rate` inside a transaction the caller holds.
    fn rate_period(&self, who: &str, games: &[(i64, i64, f64)], why: &str) -> Result<()> {
        let system = self.system()?;
        let before: std::collections::HashMap<i64, Member> = self.members()?.into_iter().map(|m| (m.id, m)).collect();
        let mut played: std::collections::BTreeMap<i64, Vec<Game>> = std::collections::BTreeMap::new();
        for &(w, b, s) in games {
            let (Some(mw), Some(mb)) = (before.get(&w), before.get(&b)) else { return Err(Error::NoMember) };
            if !(0.0..=1.0).contains(&s) || w == b {
                return Err(Error::Invalid("clubs.error.result"));
            }
            played.entry(w).or_default().push(Game { opponent: mb.rating, score: s });
            played.entry(b).or_default().push(Game { opponent: mw.rating, score: 1.0 - s });
        }
        for (id, gs) in &played {
            let m = &before[id];
            let r = rating::rate(system, m.rating, gs);
            self.db.execute("UPDATE members SET rating = ?2, deviation = ?3, volatility = ?4 WHERE id = ?1", params![id, r.rating, r.deviation, r.volatility])?;
            self.write_log(who, &m.name, "rating", &round(m.rating.rating), &round(r.rating), why)?;
        }
        Ok(())
    }

    /// Takes the lock from whoever holds it, the member `by` (a director) saying so in the
    /// audit log. Then the Club can be changed here.
    pub fn take_over(&mut self, by: i64) -> Result<()> {
        let Some(old) = read_lock(&self.path).filter(|h| !h.same(&self.me)).or_else(|| self.held_by.clone()) else {
            return Ok(());
        };
        let m = self.member(by).map_err(|_| Error::NotDirector)?;
        if m.role != Role::Director {
            return Err(Error::NotDirector);
        }
        self.me = Holder::me();
        write_lock(&self.path, &self.me)?;
        self.held_by = None;
        self.write_log(&m.name, "", "lock", &format!("{}@{}", old.user, old.host), &format!("{}@{}", self.me.user, self.me.host), &t!("clubs.took_over"))?;
        Ok(())
    }
}

impl Drop for Club {
    /// Lets go of the lock, if it is still this user's.
    fn drop(&mut self) {
        if self.held_by.is_none() && read_lock(&self.path).is_some_and(|h| h.same(&self.me)) {
            let _ = std::fs::remove_file(lock_path(&self.path));
        }
    }
}

fn parse(data: &str) -> Result<Tournament> {
    let t: Tournament = serde_json::from_str(data).map_err(|e| Error::Db(e.to_string()))?;
    // A Club file is shared by copying it: one edited by hand must not crash the page.
    if t.sound() { Ok(t) } else { Err(Error::Invalid("tournaments.error.damaged")) }
}

fn parse_league(data: &str) -> Result<League> {
    let l: League = serde_json::from_str(data).map_err(|e| Error::Db(e.to_string()))?;
    if l.sound() { Ok(l) } else { Err(Error::Invalid("leagues.error.damaged")) }
}

fn json(t: &impl Serialize) -> Result<String> {
    serde_json::to_string(t).map_err(|e| Error::Db(e.to_string()))
}

fn system_id(s: System) -> &'static str {
    match s {
        System::Glicko2 => "glicko2",
        System::Elo => "elo",
    }
}

fn check_rating(x: f64) -> Result<()> {
    if x.is_finite() && (0.0..=4000.0).contains(&x) { Ok(()) } else { Err(Error::Invalid("clubs.error.rating")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Club file in a folder of its own in the temporary folder, gone when dropped.
    pub struct Tmp(pub PathBuf);

    impl Tmp {
        pub fn new(name: &str) -> Tmp {
            static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let d = std::env::temp_dir().join(format!("bragd-clubs-{name}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Tmp(d)
        }

        fn club(&self) -> PathBuf {
            self.0.join("club.bragd-club")
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn someone_else(path: &Path) {
        write_lock(path, &Holder { user: "anna".into(), host: "club-pc".into(), since: 1 }).unwrap();
    }

    #[test]
    fn a_club_keeps_its_members_roles_and_ids() {
        let d = Tmp::new("members");
        let c = Club::create(&d.club(), "Skakklubben", System::default(), "Mette").unwrap();
        let me = c.members().unwrap()[0].id;
        let id = c.add(me, &Details { name: "Bo".into(), fide_id: "1401234".into(), national_id: "12345".into(), federation: "DEN".into(), lichess: "bo_k".into(), ..Details::default() }, Some(1820.0)).unwrap();
        drop(c);
        let c = Club::open(&d.club()).unwrap();
        let v = c.view().unwrap();
        assert_eq!((v.name.as_str(), v.system, v.held_by), ("Skakklubben", System::Glicko2, None));
        let bo = v.members.iter().find(|m| m.id == id).unwrap();
        assert_eq!((bo.role, bo.fide_id.as_str(), bo.federation.as_str(), bo.lichess.as_str(), bo.rating.rating), (Role::Member, "1401234", "DEN", "bo_k", 1820.0));
        for bad in [
            Details { name: " ".into(), ..Details::default() },
            Details { name: "X".into(), fide_id: "14a".into(), ..Details::default() },
            Details { name: "X".into(), federation: "den".into(), ..Details::default() },
            Details { name: "X".into(), lichess: "a b".into(), ..Details::default() },
        ] {
            assert!(matches!(c.add(me, &bad, None), Err(Error::Invalid(_))), "{bad:?}");
        }
        c.update(me, id, &Details { name: "Bo K".into(), role: Some(Role::Arbiter), ..Details::default() }).unwrap();
        assert_eq!(c.member(id).unwrap().role, Role::Arbiter);
        // Only a director changes the Club, and it always keeps one.
        assert_eq!(c.add(id, &Details { name: "Y".into(), ..Details::default() }, None), Err(Error::NotDirector));
        assert_eq!(c.remove(me, me), Err(Error::LastDirector));
        assert_eq!(c.update(me, me, &Details { name: "Mette".into(), role: Some(Role::Member), ..Details::default() }), Err(Error::LastDirector));
        c.remove(me, id).unwrap();
        assert_eq!(c.members().unwrap().len(), 1);
        assert_eq!(Club::create(&d.club(), "Again", System::Elo, "X").err(), Some(Error::Exists));
    }

    #[test]
    fn a_rating_edit_is_logged_with_who_when_old_new_and_why() {
        let d = Tmp::new("log");
        let c = Club::create(&d.club(), "K", System::Elo, "Mette").unwrap();
        let me = c.members().unwrap()[0].id;
        let bo = c.add(me, &Details { name: "Bo".into(), ..Details::default() }, Some(1600.0)).unwrap();
        assert_eq!(c.set_rating(me, bo, 1650.0, "  "), Err(Error::Invalid("clubs.error.why")));
        assert_eq!(c.set_rating(bo, bo, 2000.0, "mine"), Err(Error::NotDirector));
        assert_eq!(c.set_rating(me, bo, f64::NAN, "x"), Err(Error::Invalid("clubs.error.rating")));
        let before = bragd_core::fs::now();
        c.set_rating(me, bo, 1650.0, "Rated games at the open").unwrap();
        let log = c.log().unwrap();
        assert_eq!(log.len(), 1);
        let e = &log[0];
        assert_eq!((e.who.as_str(), e.member.as_str(), e.what.as_str(), e.old.as_str(), e.new.as_str(), e.why.as_str()), ("Mette", "Bo", "rating", "1600", "1650", "Rated games at the open"));
        assert!(e.at >= before);
        assert_eq!(c.member(bo).unwrap().rating.rating, 1650.0);
    }

    #[test]
    fn a_rating_period_rates_every_player_from_the_ratings_before_it() {
        let d = Tmp::new("rate");
        let c = Club::create(&d.club(), "K", System::Elo, "Mette").unwrap();
        let me = c.members().unwrap()[0].id;
        let bo = c.add(me, &Details { name: "Bo".into(), ..Details::default() }, Some(1500.0)).unwrap();
        c.rate(me, &[(me, bo, 1.0), (bo, me, 1.0)], "Club evening").unwrap();
        // One win each between equals: back where they started.
        assert_eq!(c.member(me).unwrap().rating.rating, 1500.0);
        assert_eq!(c.member(bo).unwrap().rating.rating, 1500.0);
        c.rate(me, &[(me, bo, 1.0)], "Club evening").unwrap();
        assert_eq!(c.member(me).unwrap().rating.rating, 1510.0);
        assert_eq!(c.member(bo).unwrap().rating.rating, 1490.0);
        assert_eq!(c.log().unwrap().len(), 4);
        assert_eq!(c.rate(me, &[(me, 999, 1.0)], "x"), Err(Error::NoMember));
        c.set_system(me, System::Glicko2, "Move to Glicko-2").unwrap();
        assert_eq!((c.system().unwrap(), c.log().unwrap()[0].what.as_str()), (System::Glicko2, "system"));
    }

    #[test]
    fn someone_elses_lock_opens_the_club_read_only_until_taken_over() {
        let d = Tmp::new("lock");
        let path = d.club();
        let c = Club::create(&path, "K", System::Glicko2, "Mette").unwrap();
        let me = c.members().unwrap()[0].id;
        assert!(lock_path(&path).exists(), "the open Club is locked");
        // The same user on the same machine: a second window opens it too.
        let c2 = Club::open(&path).unwrap();
        assert!(!c2.read_only());
        drop(c2);
        drop(c);
        assert!(!lock_path(&path).exists(), "closing lets go of the lock");
        someone_else(&path);
        let mut c = Club::open(&path).unwrap();
        let held = c.view().unwrap().held_by.unwrap();
        assert_eq!((held.user.as_str(), held.host.as_str()), ("anna", "club-pc"));
        assert!(matches!(c.set_rating(me, me, 1600.0, "x"), Err(Error::ReadOnly(_))));
        c.take_over(me).unwrap();
        assert!(!c.read_only());
        c.set_rating(me, me, 1600.0, "x").unwrap();
        let log = c.log().unwrap();
        assert_eq!((log[1].what.as_str(), log[1].old.as_str(), log[1].who.as_str()), ("lock", "anna@club-pc", "Mette"));
        // Taken over by someone else in turn while open here: no more changes from here.
        someone_else(&path);
        assert!(matches!(c.set_rating(me, me, 1700.0, "x"), Err(Error::ReadOnly(_))));
        drop(c);
        assert!(lock_path(&path).exists(), "another's lock is left alone");
    }

    #[test]
    fn leagues_are_kept_in_the_club_file_and_run_by_directors_and_arbiters() {
        use crate::league::Team;
        let d = Tmp::new("leagues");
        let c = Club::create(&d.club(), "K", System::Glicko2, "Mette").unwrap();
        let me = c.members().unwrap()[0].id;
        let arb = c.add(me, &Details { name: "Arne".into(), role: Some(Role::Arbiter), ..Details::default() }, None).unwrap();
        let bo = c.add(me, &Details { name: "Bo".into(), ..Details::default() }, None).unwrap();
        let l = League::new("Division 1", 1, false, "2026-11-05").unwrap();
        assert_eq!(c.add_league(bo, &l), Err(Error::NotDirector));
        let id = c.add_league(arb, &l).unwrap();
        c.change_league(arb, id, |l, _| {
            for n in ["K1", "K2"] {
                l.add_team(Team { name: n.into(), players: vec![tournament::Player { name: format!("{n} a"), ..tournament::Player::default() }] })?;
            }
            l.pair_next()
        })
        .unwrap();
        assert!(c.change_league(arb, id, |l, _| l.pair_next()).is_err(), "nothing kept from a failed step");
        c.change_league(arb, id, |l, who| l.set_result(who, 0, 0, 0, Some(tournament::Outcome::White), "")).unwrap();
        assert_eq!(c.remove_league(arb, id), Err(Error::NotDirector));
        drop(c);
        let c = Club::open(&d.club()).unwrap();
        let listed = c.view().unwrap().leagues;
        assert_eq!((listed.len(), listed[0].name.as_str(), listed[0].teams, listed[0].rounds, listed[0].paired), (1, "Division 1", 2, 1, 1));
        assert_eq!(c.league(id).unwrap().standings()[0].match_points, 4);
        c.remove_league(me, id).unwrap();
        assert_eq!(c.league(id), Err(Error::Invalid("leagues.error.missing")));
    }

    #[test]
    fn tournaments_are_kept_in_the_club_file_and_run_by_directors_and_arbiters() {
        use crate::tournament::{self, Status};
        let d = Tmp::new("tournaments");
        let c = Club::create(&d.club(), "K", System::Glicko2, "Mette").unwrap();
        let me = c.members().unwrap()[0].id;
        let arb = c.add(me, &Details { name: "Arne".into(), role: Some(Role::Arbiter), ..Details::default() }, None).unwrap();
        let bo = c.add(me, &Details { name: "Bo".into(), ..Details::default() }, None).unwrap();
        let t = Tournament::new("Autumn Swiss", tournament::System::Dutch, 5, "2026-11-05").unwrap();
        assert_eq!(c.add_tournament(bo, &t), Err(Error::NotDirector));
        let id = c.add_tournament(arb, &t).unwrap();
        // An arbiter runs the night; nothing is kept when a step fails.
        c.change_tournament(arb, id, |t, _| {
            for n in ["Ann", "Bob"] {
                let p = t.register(tournament::Player { name: n.into(), ..tournament::Player::default() })?;
                t.set_status("", p, Status::Here)?;
            }
            t.pair_next()
        })
        .unwrap();
        assert!(c.change_tournament(arb, id, |t, _| t.pair_next()).is_err());
        assert_eq!(c.change_tournament(arb, id, |t, who| t.set_result(who, 0, 0, Some(tournament::Outcome::Draw), "").map(|_| who.to_string())).unwrap(), "Arne");
        assert_eq!(c.remove_tournament(arb, id), Err(Error::NotDirector));
        assert_eq!(c.set_pairing(arb, id, &[(1, 0)], None, "x"), Err(Error::NotDirector));
        drop(c);
        let c = Club::open(&d.club()).unwrap();
        let listed = c.view().unwrap().tournaments;
        assert_eq!((listed.len(), listed[0].name.as_str(), listed[0].paired, listed[0].players), (1, "Autumn Swiss", 1, 2));
        assert_eq!(c.tournament(id).unwrap().played[0].games[0].result, Some(tournament::Outcome::Draw));
        // Rated once, from the one game between members.
        let ann = c.add(me, &Details { name: "Ann".into(), ..Details::default() }, Some(1500.0)).unwrap();
        let bob = c.add(me, &Details { name: "Bob".into(), ..Details::default() }, Some(1500.0)).unwrap();
        c.change_tournament(me, id, |t, _| {
            t.players[0].member = Some(ann);
            t.players[1].member = Some(bob);
            Ok(())
        })
        .unwrap();
        assert_eq!(c.rate_tournament(arb, id), Err(Error::NotDirector));
        c.rate_tournament(me, id).unwrap();
        assert_eq!(c.log().unwrap()[0].why, "Autumn Swiss");
        assert_eq!(c.rate_tournament(me, id), Err(Error::Invalid("tournaments.error.rated")));
        c.remove_tournament(me, id).unwrap();
        assert!(c.tournaments().unwrap().is_empty());
    }

    #[test]
    fn a_club_from_before_tournaments_opens_read_only_without_them() {
        let d = Tmp::new("old");
        let c = Club::create(&d.club(), "K", System::Glicko2, "Mette").unwrap();
        drop(c);
        Connection::open(d.club()).unwrap().execute_batch("DROP TABLE tournaments").unwrap();
        someone_else(&d.club());
        assert_eq!(Club::open(&d.club()).unwrap().tournaments().unwrap(), []);
    }

    #[test]
    fn a_copy_of_a_club_carries_no_lock_and_a_newer_file_is_not_opened() {
        let d = Tmp::new("copy");
        let c = Club::create(&d.club(), "K", System::Glicko2, "Mette").unwrap();
        let copy = d.0.join("copy.bragd-club");
        std::fs::copy(d.club(), &copy).unwrap();
        assert!(!lock_path(&copy).exists());
        drop(c);
        Connection::open(&copy).unwrap().pragma_update(None, "user_version", FORMAT + 1).unwrap();
        assert_eq!(Club::open(&copy).err(), Some(Error::Newer));
    }
}
