//! A PGN file's games as a table, for files of millions of games: one pass reads only where
//! each game starts and its headers (on a thread of its own, the table filling as it goes);
//! a game's moves are read when it is opened. `.pgn.gz`, `.pgn.zst` and `.pgn.bz2` are read as
//! they unpack.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use bragd_gamefiles::chess::scan::{self, BLACK, BLACK_ELO, DATE, EVENT, Counted, Head, MAX_GAME, RESULT, WHITE, WHITE_ELO, scan, text};
use bragd_gamefiles::chess::{Format, pgn};
use bragd_gamefiles::Progress;

/// How the file is packed, by its name; a DhtmlXQ game by its first bytes, read as the PGN it
/// is turned into (`xiangqi::dhtml_to_pgn`).
#[derive(Clone, Copy, PartialEq)]
enum Pack {
    None,
    Gz,
    Zst,
    Bz2,
    Dhtml,
}

fn pack(path: &Path) -> Pack {
    let n = path.to_string_lossy().to_ascii_lowercase();
    if n.ends_with(".gz") {
        Pack::Gz
    } else if n.ends_with(".zst") {
        Pack::Zst
    } else if n.ends_with(".bz2") {
        Pack::Bz2
    } else if is_dhtml(path) {
        Pack::Dhtml
    } else {
        Pack::None
    }
}

/// Endings of common files that are never a DhtmlXQ game, whatever their first bytes say.
const NOT_DHTML: &[&str] = &[
    "7z", "aac", "avi", "bin", "bmp", "bz2", "c", "cbh", "cbv", "cpp", "css", "csv", "doc", "docx", "epd", "exe", "fen", "flac", "gif",
    "go", "gz", "h", "heic", "htm", "html", "ico", "iso", "java", "jpeg", "jpg", "js", "json", "kif", "md", "mkv", "mov", "mp3", "mp4",
    "odt", "ogg", "parquet", "pdf", "pdn", "pgn", "png", "ppt", "pptx", "py", "rar", "rs", "sgf", "sh", "si4", "si5", "sqlite", "svg",
    "tar", "tif", "tiff", "toml", "ts", "wav", "webm", "webp", "xls", "xlsx", "xml", "xz", "yaml", "yml", "zip", "zst",
];

/// Whether a file of this name may be a DhtmlXQ game: no ending, `.che`, `.txt`, `.dhtmlxq`, or
/// an "ending" that is no common file type (`001-1.古谱的启示`, `game.abc`).
pub fn dhtml_name(name: &str) -> bool {
    match name.rsplit_once('.') {
        None | Some(("", _)) => true,
        Some((_, e)) => {
            ["che", "txt", "dhtmlxq"].iter().any(|x| e.eq_ignore_ascii_case(x)) || !NOT_DHTML.contains(&e.to_ascii_lowercase().as_str())
        }
    }
}

/// `path` opened for reading only when it is a regular file: never a FIFO or a device.
pub(crate) fn open_regular(path: &Path) -> io::Result<File> {
    let f = File::open(path)?;
    if !f.metadata()?.is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a regular file"));
    }
    Ok(f)
}

/// Whether the file at `path` is a DhtmlXQ game, by its first bytes: only a name such files
/// may have (`dhtml_name`), and only a regular file (a FIFO is never opened).
pub(crate) fn is_dhtml(path: &Path) -> bool {
    let named = path.file_name().is_some_and(|n| dhtml_name(&n.to_string_lossy()));
    if !named || !std::fs::metadata(path).is_ok_and(|m| m.is_file()) {
        return false;
    }
    let mut head = [0u8; 64];
    open_regular(path).and_then(|mut f| read_full(&mut f, &mut head)).is_ok_and(|n| bragd_gamefiles::xiangqi::is_dhtml(&head[..n]))
}

/// What one file may cost: a file of endless tiny games, or a small packed file that unpacks
/// to terabytes, stops here, with the games read so far and a note saying so.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Games in the table (48 bytes each).
    pub games: usize,
    /// Bytes of the names kept (players, events, openings), with what keeping each costs.
    pub names: usize,
    /// How many times its own size a packed file may unpack to: `knowledge.max_unpack_ratio`,
    /// the rule an import keeps (`stream::games`).
    pub ratio: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { games: 10_000_000, names: 256 << 20, ratio: bragd_core::config::MAX_UNPACK_RATIO }
    }
}

/// The file's text, unpacked as it is read (up to `unpacked` bytes); `read` counts the file's
/// own bytes.
pub(crate) fn reader(path: &Path, read: Arc<AtomicU64>, unpacked: u64) -> io::Result<Box<dyn Read + Send>> {
    let f = BufReader::new(Counted { inner: File::open(path)?, read });
    Ok(match pack(path) {
        Pack::None => Box::new(f),
        Pack::Gz | Pack::Zst | Pack::Bz2 => Box::new(crate::stream::unpack(&path.to_string_lossy(), f)?.take(unpacked)),
        Pack::Dhtml => {
            let mut text = vec![];
            f.take(bragd_gamefiles::xiangqi::DHTML_MAX).read_to_end(&mut text)?;
            let pgn = bragd_gamefiles::xiangqi::dhtml_to_pgn(&String::from_utf8_lossy(&text));
            Box::new(io::Cursor::new(pgn.into_bytes()).take(unpacked))
        }
    })
}

fn is_pgn(p: &Path) -> bool {
    p.extension().is_some_and(|e| e == "pgn")
}

/// The PGN file a ChessBase archive (`.cbv`) of a PGN database unpacks to (`cbv::unpacked`,
/// once per archive as it is); None for any other file.
fn archived_pgn(path: &Path) -> Option<PathBuf> {
    if !super::bases::is_cbv(path) {
        return None;
    }
    super::bases::unpacked(path).ok()?.into_iter().next().filter(|p| is_pgn(p))
}

/// The games of a ChessBase database (`cbh`), as the pass over a PGN file sees them: a game's
/// `offset` is its record in the `.cbh` file. `read` is told how far of `size` it is: an
/// archive's (`.cbv`) `size` is twice its own, the first half its unpacking (stopped by `stop`
/// too), the second its `.cbh` file read.
/// `full` as `scan` takes it, for an archive of a PGN database (`archived_pgn`).
fn scan_cbh(path: &Path, stop: &AtomicBool, read: &AtomicU64, size: u64, full: bool, mut each: impl FnMut(&Head) -> bool) -> io::Result<()> {
    let mut head = Head { offset: 0, len: 0, tags: Default::default(), plies: 0, sans: vec![], text: vec![], format: Format::Pgn };
    let share = |n: u64, of: u64, base: u64| read.store(base + (n.min(of) as u128 * (size / 2) as u128 / of.max(1) as u128) as u64, Ordering::Relaxed);
    let cbh_len = if super::bases::is_cbv(path) {
        let mut len = 0;
        let unpacked = super::bases::unpacked_with(path, &|| stop.load(Ordering::Relaxed), &|n, of| share(n, of, 0))?;
        if let Some(pgn) = unpacked.first().filter(|p| is_pgn(p)) {
            let of = std::fs::metadata(pgn)?.len();
            let counted = Arc::new(AtomicU64::new(0));
            let r = BufReader::with_capacity(1 << 16, Counted { inner: File::open(pgn)?, read: counted.clone() });
            return scan(r, stop, Format::Pgn, full, false, |h| {
                share(counted.load(Ordering::Relaxed), of, size / 2);
                each(h)
            });
        }
        for p in unpacked {
            len += std::fs::metadata(&p)?.len();
        }
        Some(len)
    } else {
        None
    };
    let progress = |n: u64| match cbh_len {
        Some(len) => share(n, len, size / 2),
        None => read.store(n, Ordering::Relaxed),
    };
    super::bases::cbh_scan(path, &|| stop.load(Ordering::Relaxed), &progress, |record, g| {
        for t in &mut head.tags {
            t.clear();
        }
        for (k, v) in &g.tags {
            if let Some(i) = scan::WANTED.iter().position(|w| w == k) {
                head.tags[i].clone_from(v);
            }
        }
        head.offset = record;
        head.plies = g.moves * 2;
        each(&head)
    })
}

/// Whose games a PGN-family file holds: draughts for `.pdn`, xiangqi when the first tags of a
/// PGN file say so (`xiangqi::is_xiangqi`), else chess.
pub fn kind(path: &Path) -> Format {
    if super::bases::is_scid(path) {
        return Format::Pgn;
    }
    if let Some(f @ (Format::Pdn | Format::Kif | Format::Csa)) = super::format(path) {
        return f;
    }
    if super::bases::is_cbh(path) {
        return Format::Pgn;
    }
    let mut head = vec![];
    let xiangqi = reader(path, Arc::default(), bragd_gamefiles::xiangqi::SNIFF as u64)
        .and_then(|r| r.take(bragd_gamefiles::xiangqi::SNIFF as u64).read_to_end(&mut head))
        .is_ok_and(|_| bragd_gamefiles::xiangqi::is_xiangqi(&head));
    if xiangqi { Format::Xiangqi } else { Format::Pgn }
}

/// One game in the table: 48 bytes, so a million games take 48 MB.
#[derive(Clone, Copy)]
struct Rec {
    offset: u64,
    len: u32,
    white: u32,
    black: u32,
    event: u32,
    /// The opening's name, interned; `u32::MAX` for none.
    opening: u32,
    /// yyyymmdd, unknown parts 0.
    date: u32,
    white_elo: u16,
    black_elo: u16,
    /// 'A'..'E' as 1..5 times 100 plus the number, 0 for none.
    eco: u16,
    plies: u16,
    /// 1 White won, 2 Black won, 3 a draw, 0 anything else.
    result: u8,
    /// The result as written, interned: draughts writes `2-0`.
    result_name: u32,
}

#[derive(Default)]
struct Shared {
    recs: Vec<Rec>,
    strings: Vec<Box<str>>,
    done: bool,
    /// Stopped at `Limits`: the table has the games read until then.
    capped: bool,
    error: Option<String>,
}

fn result_code(r: &str) -> u8 {
    match r {
        "1-0" | "2-0" => 1,
        "0-1" | "0-2" => 2,
        "1/2-1/2" | "½-½" | "1-1" => 3,
        _ => 0,
    }
}

fn result_text(r: u8) -> &'static str {
    ["*", "1-0", "0-1", "½-½"][r as usize & 3]
}

fn date_code(d: &str) -> u32 {
    let mut parts = d.split(['.', '-', '/']).map(|p| p.trim().parse::<u32>().unwrap_or(0));
    let y = parts.next().unwrap_or(0).min(9999);
    let m = parts.next().unwrap_or(0).min(12);
    let day = parts.next().unwrap_or(0).min(31);
    y * 10000 + m * 100 + day
}

fn date_text(d: u32) -> String {
    let (y, m, day) = (d / 10000, d / 100 % 100, d % 100);
    match (y, m, day) {
        (0, _, _) => String::new(),
        (y, 0, _) => format!("{y}"),
        (y, m, 0) => format!("{y}.{m:02}"),
        (y, m, d) => format!("{y}.{m:02}.{d:02}"),
    }
}

fn eco_code(e: &str) -> u16 {
    let b = e.trim().as_bytes();
    if b.len() == 3 && (b'A'..=b'E').contains(&b[0].to_ascii_uppercase()) && b[1].is_ascii_digit() && b[2].is_ascii_digit() {
        (b[0].to_ascii_uppercase() - b'A' + 1) as u16 * 100 + ((b[1] - b'0') * 10 + b[2] - b'0') as u16
    } else {
        0
    }
}

fn eco_text(e: u16) -> String {
    if e == 0 { String::new() } else { format!("{}{:02}", (b'A' + (e / 100) as u8 - 1) as char, e % 100) }
}

/// Where the games come from when one is read: the file, or an unpacker kept where it stopped,
/// so that reading on through the file does not unpack it again from the start.
struct Source {
    path: PathBuf,
    open: Option<(Box<dyn Read + Send>, u64)>,
}

impl Source {
    fn read(&mut self, offset: u64, len: u32) -> io::Result<Vec<u8>> {
        let mut buf = vec![0; len.min(MAX_GAME) as usize];
        if pack(&self.path) == Pack::None {
            let mut f = File::open(&self.path)?;
            f.seek(SeekFrom::Start(offset))?;
            let n = read_full(&mut f, &mut buf)?;
            buf.truncate(n);
            return Ok(buf);
        }
        // ponytail: a packed file is unpacked from its start for a game before the last one
        // read; seek points every few MB would make going back cheap, if it is ever slow.
        if self.open.as_ref().is_none_or(|(_, at)| *at > offset) {
            self.open = Some((reader(&self.path, Arc::default(), u64::MAX)?, 0));
        }
        let (r, at) = self.open.as_mut().expect("opened above");
        io::copy(&mut r.take(offset - *at), &mut io::sink())?;
        let n = read_full(r, &mut buf)?;
        *at = offset + n as u64;
        buf.truncate(n);
        Ok(buf)
    }
}

fn read_full(r: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

/// What the table shows: which games, in what order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Query {
    /// In White's or Black's name.
    pub player: String,
    /// In the opening's name, or the start of its ECO code.
    pub opening: String,
    /// `1-0`, `0-1`, `1/2-1/2`, or empty for any.
    pub result: String,
    pub from: Option<u16>,
    pub to: Option<u16>,
    /// Both players rated at least this.
    pub elo: Option<u16>,
    pub sort: Sort,
    pub desc: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    #[default]
    Number,
    White,
    Black,
    WhiteElo,
    BlackElo,
    Result,
    Date,
    Event,
    Eco,
    Opening,
    Moves,
}

impl Sort {
    pub const ALL: [Sort; 11] = [
        Sort::Number,
        Sort::White,
        Sort::WhiteElo,
        Sort::Black,
        Sort::BlackElo,
        Sort::Result,
        Sort::Date,
        Sort::Event,
        Sort::Eco,
        Sort::Opening,
        Sort::Moves,
    ];
}

/// A game as the table shows it.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Row {
    /// Its place in the file, from 1.
    pub n: u32,
    pub white: String,
    pub black: String,
    pub white_elo: Option<u16>,
    pub black_elo: Option<u16>,
    pub result: String,
    pub date: String,
    pub event: String,
    pub eco: String,
    pub opening: String,
    /// Full moves.
    pub moves: u32,
}

/// The games of one PGN file.
pub struct Table {
    pub path: PathBuf,
    /// `Pgn`, or `Pdn` for draughts.
    pub format: Format,
    shared: Arc<Mutex<Shared>>,
    read: Arc<AtomicU64>,
    size: u64,
    stop: Arc<AtomicBool>,
    source: Mutex<Source>,
    /// A SCID database's names and files, when the file is one (`scid`).
    scid: Option<Arc<bragd_gamefiles::chess::scid::Base>>,
    /// A ChessBase archive's unpacked files, kept while this reads them (`cbv::hold`).
    _held: Option<super::bases::Held>,
}

impl Drop for Table {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Table {
    /// Starts reading the file's games, on a thread of its own.
    pub fn open(path: &Path) -> io::Result<Table> {
        Table::open_with(path, Limits::default())
    }

    /// `open`, with limits of its own.
    pub fn open_with(path: &Path, limits: Limits) -> io::Result<Table> {
        Table::start(path, limits, Arc::new(AtomicBool::new(false)))
    }

    /// `open_with`, heeding `stop` from the start: a test sets it before the thread runs.
    fn start(path: &Path, limits: Limits, stop: Arc<AtomicBool>) -> io::Result<Table> {
        let size = std::fs::metadata(path)?.len();
        let read = Arc::new(AtomicU64::new(0));
        let shared = Arc::new(Mutex::new(Shared::default()));
        let source = Mutex::new(Source { path: path.to_path_buf(), open: None });
        if super::bases::is_scid(path) {
            let base = Arc::new(super::bases::scid(path)?);
            let (sh, st, b, rd) = (shared.clone(), stop.clone(), base.clone(), read.clone());
            std::thread::Builder::new()
                .name("chess-games".into())
                .spawn(move || index(|each| scan::scid_heads(&b, &st, &rd, each), &sh, Format::Pgn, limits))?;
            return Ok(Table { path: path.to_path_buf(), format: Format::Pgn, shared, read, size, stop, source, scid: Some(base), _held: None });
        }
        if super::bases::is_cbh(path) {
            // An archive is unpacked first, then read: its bar has both (`scan_cbh`).
            let size = if super::bases::is_cbv(path) { size.saturating_mul(2) } else { size };
            let (sh, st, rd, p) = (shared.clone(), stop.clone(), read.clone(), path.to_path_buf());
            std::thread::Builder::new()
                .name("chess-games".into())
                .spawn(move || index(|each| scan_cbh(&p, &st, &rd, size, true, each), &sh, Format::Pgn, limits))?;
            let _held = super::bases::is_cbv(path).then(|| super::bases::hold(path));
            return Ok(Table { path: path.to_path_buf(), format: Format::Pgn, shared, read, size, stop, source, scid: None, _held });
        }
        let r = reader(path, read.clone(), u64::MAX)?;
        let (sh, st, rd) = (shared.clone(), stop.clone(), read.clone());
        let format = kind(path);
        std::thread::Builder::new().name("chess-games".into()).spawn(move || {
            // A stop is heard as the bytes unpack, not only between games: a bomb has none.
            let halt = || st.load(Ordering::Relaxed);
            let r = BufReader::with_capacity(1 << 16, crate::stream::capped(r, limits.ratio, &rd, &halt));
            index(|each| scan(r, &st, format, true, false, each), &sh, format, limits)
        })?;
        Ok(Table { path: path.to_path_buf(), format, shared, read, size, stop, source, scid: None, _held: None })
    }

    pub fn progress(&self) -> Progress {
        let s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        Progress { games: s.recs.len(), done: s.done, capped: s.capped, read: self.read.load(Ordering::Relaxed).min(self.size), size: self.size, error: s.error.clone() }
    }

    /// The games that `q` lets through, in its order: their indices (from 0).
    pub fn query(&self, q: &Query) -> Vec<u32> {
        let s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let player = q.player.trim().to_lowercase();
        let opening = q.opening.trim().to_lowercase();
        // The interned names that match, looked at once each rather than per game.
        let names = |needle: &str| -> Vec<bool> {
            if needle.is_empty() {
                return vec![];
            }
            s.strings.iter().map(|n| n.to_lowercase().contains(needle)).collect()
        };
        let players = names(&player);
        let openings = names(&opening);
        let result = result_code(q.result.trim());
        let hit = |ids: &Vec<bool>, i: u32| ids.get(i as usize).copied().unwrap_or(false);
        let mut ids: Vec<u32> = (0..s.recs.len() as u32)
            .filter(|&i| {
                let r = &s.recs[i as usize];
                (player.is_empty() || hit(&players, r.white) || hit(&players, r.black))
                    && (opening.is_empty()
                        || hit(&openings, r.opening)
                        || eco_text(r.eco).to_lowercase().starts_with(&opening))
                    && (q.result.trim().is_empty() || r.result == result)
                    && q.from.is_none_or(|y| r.date / 10000 >= y as u32)
                    && q.to.is_none_or(|y| r.date / 10000 <= y as u32 && r.date > 0)
                    && q.elo.is_none_or(|e| r.white_elo.min(r.black_elo) >= e)
            })
            .collect();
        // Names sort by their rank among all names, worked out once.
        let rank = |s: &Shared| -> Vec<u32> {
            let mut order: Vec<u32> = (0..s.strings.len() as u32).collect();
            // shortcut: case-folded order, not a language's alphabet; upgrade to a collator when the GUI sorts names.
            let keys: Vec<String> = s.strings.iter().map(|n| n.to_lowercase()).collect();
            order.sort_by(|&a, &b| keys[a as usize].cmp(&keys[b as usize]));
            let mut rank = vec![0; order.len()];
            for (r, &i) in order.iter().enumerate() {
                rank[i as usize] = r as u32;
            }
            rank
        };
        type Key = Box<dyn Fn(&Rec) -> u64>;
        let key: Option<Key> = match q.sort {
            Sort::Number => None,
            Sort::White | Sort::Black | Sort::Event | Sort::Opening => {
                let rank = rank(&s);
                let sort = q.sort;
                Some(Box::new(move |r: &Rec| {
                    let id = match sort {
                        Sort::White => r.white,
                        Sort::Black => r.black,
                        Sort::Event => r.event,
                        _ => r.opening,
                    };
                    // No name sorts last.
                    rank.get(id as usize).map_or(u64::MAX, |&k| k as u64)
                }))
            }
            Sort::WhiteElo => Some(Box::new(|r: &Rec| r.white_elo as u64)),
            Sort::BlackElo => Some(Box::new(|r: &Rec| r.black_elo as u64)),
            Sort::Result => Some(Box::new(|r: &Rec| r.result as u64)),
            Sort::Date => Some(Box::new(|r: &Rec| r.date as u64)),
            Sort::Eco => Some(Box::new(|r: &Rec| r.eco as u64)),
            Sort::Moves => Some(Box::new(|r: &Rec| r.plies as u64)),
        };
        if let Some(key) = key {
            ids.sort_by_key(|&i| key(&s.recs[i as usize]));
        }
        if q.desc {
            ids.reverse();
        }
        ids
    }

    /// The game at index `i`, as the table shows it.
    pub fn row(&self, i: u32) -> Option<Row> {
        let s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let r = s.recs.get(i as usize)?;
        let name = |id: u32| s.strings.get(id as usize).map_or(String::new(), |n| n.to_string());
        Some(Row {
            n: i + 1,
            white: name(r.white),
            black: name(r.black),
            white_elo: (r.white_elo > 0).then_some(r.white_elo),
            black_elo: (r.black_elo > 0).then_some(r.black_elo),
            result: if self.format == Format::Pgn || r.result_name == u32::MAX { result_text(r.result).into() } else { name(r.result_name) },
            date: date_text(r.date),
            event: name(r.event),
            eco: eco_text(r.eco),
            opening: name(r.opening),
            // Shogi counts every move; chess, draughts and xiangqi a move of each side as one.
            moves: if matches!(self.format, Format::Kif | Format::Csa) { r.plies as u32 } else { (r.plies as u32).div_ceil(2) },
        })
    }

    /// The text of the game at index `i`, read now.
    pub fn text(&self, i: u32) -> io::Result<String> {
        let (offset, len) = {
            let s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            let r = s.recs.get(i as usize).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such game"))?;
            (r.offset, r.len)
        };
        if let Some(base) = &self.scid {
            return base.pgn(offset as u32);
        }
        if let Some(pgn) = archived_pgn(&self.path) {
            return Ok(text(&Source { path: pgn, open: None }.read(offset, len)?).into_owned());
        }
        if super::bases::is_cbh(&self.path) {
            return super::bases::cbh_pgn(&self.path, offset);
        }
        let bytes = self.source.lock().unwrap_or_else(|e| e.into_inner()).read(offset, len)?;
        // KIF and CSA files are often in Shift_JIS.
        Ok(match self.format {
            Format::Kif | Format::Csa => bragd_gamefiles::shogi::decode(&bytes),
            _ => text(&bytes).into_owned(),
        })
    }

    /// The game at index `i`, read and played out.
    pub fn game(&self, i: u32) -> io::Result<pgn::Game> {
        let text = self.text(i)?;
        Ok(match self.format {
            Format::Pdn => bragd_gamefiles::draughts::parse(&text),
            Format::Xiangqi => bragd_gamefiles::xiangqi::parse(&text),
            Format::Kif | Format::Csa => bragd_gamefiles::shogi::parse(&text, self.format),
            _ => pgn::parse(&text),
        })
    }
}

/// The pass over the games: their records, handed over in batches so the table fills.
fn index(feed: impl FnOnce(&mut dyn FnMut(&Head) -> bool) -> io::Result<()>, shared: &Mutex<Shared>, format: Format, limits: Limits) {
    let chess = format == Format::Pgn;
    // What the names cost so far: their bytes, twice (the list and the lookup), and the rest.
    let mut names = 0usize;
    let (mut games, mut capped) = (0usize, false);
    let mut ids: HashMap<Box<str>, u32> = HashMap::new();
    let mut fresh: Vec<Box<str>> = vec![];
    let mut batch: Vec<Rec> = vec![];
    let mut base = 0u32;
    let intern = |s: &str, ids: &mut HashMap<Box<str>, u32>, fresh: &mut Vec<Box<str>>, base: u32, names: &mut usize| -> u32 {
        let s = s.trim();
        if s.is_empty() || s == "?" {
            return u32::MAX;
        }
        if let Some(&id) = ids.get(s) {
            return id;
        }
        let id = base + fresh.len() as u32;
        *names += 2 * s.len() + 64;
        ids.insert(s.into(), id);
        fresh.push(s.into());
        id
    };
    let flush = |batch: &mut Vec<Rec>, fresh: &mut Vec<Box<str>>, base: &mut u32| {
        let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
        *base += fresh.len() as u32;
        s.strings.append(fresh);
        s.recs.append(batch);
    };
    let mut last = std::time::Instant::now();
    let res = feed(&mut |h| {
        if games >= limits.games || names >= limits.names {
            capped = true;
            return false;
        }
        games += 1;
        let (eco, opening) = h.opening();
        let rec = Rec {
            offset: h.offset,
            len: h.len.min(MAX_GAME as u64) as u32,
            white: intern(h.tag(WHITE), &mut ids, &mut fresh, base, &mut names),
            black: intern(h.tag(BLACK), &mut ids, &mut fresh, base, &mut names),
            event: intern(h.tag(EVENT), &mut ids, &mut fresh, base, &mut names),
            opening: intern(&opening, &mut ids, &mut fresh, base, &mut names),
            date: date_code(h.tag(DATE)),
            white_elo: h.tag(WHITE_ELO).parse().unwrap_or(0),
            black_elo: h.tag(BLACK_ELO).parse().unwrap_or(0),
            eco: eco_code(&eco),
            plies: h.plies.min(u16::MAX as u32) as u16,
            result: result_code(h.tag(RESULT)),
            result_name: if chess { u32::MAX } else { intern(h.tag(RESULT), &mut ids, &mut fresh, base, &mut names) },
        };
        batch.push(rec);
        if batch.len() >= 4096 && last.elapsed().as_millis() >= 200 {
            flush(&mut batch, &mut fresh, &mut base);
            last = std::time::Instant::now();
        }
        true
    });
    flush(&mut batch, &mut fresh, &mut base);
    let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
    s.done = true;
    s.capped = capped;
    if let Err(e) = res {
        s.error = Some(e.to_string());
    }
}

/// The Polyglot keys of the positions `g` reaches on its main line, each once, sorted.
fn main_line_keys(g: &pgn::Game) -> Vec<u64> {
    let mut keys: Vec<u64> = g
        .main_line()
        .iter()
        .map_while(|&i| g.nodes[i as usize].fen.as_deref().and_then(pgn::board))
        .map(|b| bragd_gamefiles::chess::polyglot::key(&b))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// The most games of one file whose positions `positions` hands over, and the most positions.
pub const POSITION_GAMES: u32 = 100_000;
pub const POSITION_ROWS: usize = 2_000_000;

/// The positions each game of the PGN file at `path` reaches on its main line, as Polyglot keys
/// (`polyglot::key`), each once a game, the start position left out (every game has it):
/// handed to `each` game by game, with the game's index, as the file is read, so a big file
/// never has more than one game in memory. Up to `POSITION_GAMES` games and `POSITION_ROWS`
/// positions; `stop` is asked between games, and true ends it there.
pub fn positions(path: &Path, stop: &dyn Fn() -> bool, mut each: impl FnMut(u32, &[u64])) -> io::Result<()> {
    if super::bases::is_scid(path) {
        // The games as the table lists them (`scan_scid`: deleted ones left out), so a game's
        // index here is its row's, not its place in the SCID index.
        let base = super::bases::scid(path)?;
        let mut games = vec![];
        base.entries(|n, _| {
            games.push(n);
            games.len() < POSITION_GAMES as usize
        })?;
        let mut rows = 0usize;
        for (row, n) in (0u32..).zip(games) {
            if rows >= POSITION_ROWS || stop() {
                break;
            }
            let mut keys = main_line_keys(&pgn::parse(&base.pgn(n)?));
            keys.truncate(POSITION_ROWS - rows);
            rows += keys.len();
            each(row, &keys);
        }
        return Ok(());
    }
    let (mut n, mut rows) = (0u32, 0usize);
    let mut flush = |game: &str, n: &mut u32, rows: &mut usize| {
        let mut keys = main_line_keys(&pgn::parse(game));
        keys.truncate(POSITION_ROWS.saturating_sub(*rows));
        *rows += keys.len();
        each(*n, &keys);
        *n += 1;
    };
    if let Some(pgn) = archived_pgn(path) {
        return positions(&pgn, stop, each);
    }
    if super::bases::is_cbh(path) {
        // The records first, then each game read as PGN: the game's index is its place in the list.
        let mut records = vec![];
        super::bases::cbh_scan(path, stop, &|_| {}, |record, _| {
            records.push(record);
            records.len() < POSITION_GAMES as usize
        })?;
        for r in records {
            if rows >= POSITION_ROWS || stop() {
                break;
            }
            flush(&super::bases::cbh_pgn(path, r).unwrap_or_else(|_| "*\n".into()), &mut n, &mut rows);
        }
        return Ok(());
    }
    // The games as the table cuts them (`scan`), so a game's index here is its row's.
    let r = BufReader::with_capacity(1 << 16, reader(path, Arc::default(), 1 << 30)?);
    scan(r, &AtomicBool::new(false), Format::Pgn, false, true, |h| {
        if n >= POSITION_GAMES || rows >= POSITION_ROWS || stop() {
            return false;
        }
        flush(&text(&h.text), &mut n, &mut rows);
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const TWO: &str = "[Event \"Paris\"]\n[White \"Morphy, Paul\"]\n[Black \"Duke Karl\"]\n[Result \"1-0\"]\n[Date \"1858.11.02\"]\n[WhiteElo \"2690\"]\n\n1.e4 e5 2.Nf3 d6 {Philidor} 3.d4 Bg4 (3...exd4) 4.dxe5 1-0\n\n[Event \"London\"]\n[White \"Anderssen, Adolf\"]\n[Black \"Kieseritzky, Lionel\"]\n[Result \"1-0\"]\n[Date \"1851.06.21\"]\n[ECO \"C33\"]\n\n1.e4 e5 2.f4 exf4 {a\n[not a tag] } 3.Bc4 1-0\n";

    #[test]
    fn positions_game_by_game() {
        let d = tempdir::Dir::new();
        let p = d.0.join("two.pgn");
        std::fs::write(&p, TWO).unwrap();
        let mut seen = vec![];
        positions(&p, &|| false, |n, keys| seen.push((n, keys.to_vec()))).unwrap();
        // Stopped: no game at all.
        let mut none = 0;
        positions(&p, &|| true, |_, _| none += 1).unwrap();
        assert_eq!(none, 0);
        assert_eq!(seen.len(), 2);
        // Both start 1.e4 e5: the position after it is in both, the start position in neither.
        let after = |fen: &str| bragd_gamefiles::chess::polyglot::key(&pgn::board(fen).unwrap());
        let e4e5 = after("rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2");
        assert!(seen.iter().all(|(_, k)| k.contains(&e4e5)));
        assert!(seen.iter().all(|(_, k)| !k.contains(&after(pgn::START))));
        // The first game's 4.dxe5 is in it alone; the comment with a [ line does not split a game.
        assert_eq!((seen[0].1.len(), seen[1].1.len()), (7, 5));
    }

    /// A position names a game by its row: positions are read from the games as the
    /// table cuts them, an unclosed comment and a lone Event tag too.
    #[test]
    fn positions_by_the_tables_games() {
        let text = "[Event \"a\"]\n\n1.e4 {never closed\n\n[Event \"b\"]\n\n1.d4 d5 *\n[Event \"x\"]\n[Event \"c\"]\n\n1.c4 $1 e5 *\n";
        let (l, d) = lib(text.as_bytes(), "odd.pgn");
        let mut seen = vec![];
        positions(&d.0.join("odd.pgn"), &|| false, |n, keys| seen.push((n, keys.len()))).unwrap();
        assert_eq!(l.progress().games, 4);
        assert_eq!(seen, [(0, 1), (1, 2), (2, 0), (3, 2)]);
        assert_eq!((l.row(3).unwrap().moves, l.row(3).unwrap().opening.as_str()), (1, "English Opening: King's English Variation"));
    }

    fn lib(text: &[u8], name: &str) -> (Table, tempdir::Dir) {
        let d = tempdir::Dir::new();
        let p = d.0.join(name);
        std::fs::File::create(&p).unwrap().write_all(text).unwrap();
        let l = Table::open(&p).unwrap();
        while !l.progress().done {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        (l, d)
    }

    mod tempdir {
        pub struct Dir(pub std::path::PathBuf);
        impl Dir {
            pub fn new() -> Dir {
                static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let p = std::env::temp_dir().join(format!("bragd-chess-{}-{n}", std::process::id()));
                std::fs::create_dir_all(&p).unwrap();
                Dir(p)
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn a_byte_order_mark_is_no_game() {
        let text = format!("\u{feff}{TWO}");
        let mut events = vec![];
        scan(text.as_bytes(), &AtomicBool::new(false), Format::Pgn, true, false, |h| {
            events.push((h.offset, h.tags[EVENT].clone()));
            true
        })
        .unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, 3, "the game starts after the mark");
        assert!(!events[0].1.is_empty(), "its Event kept");
    }

    #[test]
    fn a_table_of_games() {
        let (l, _d) = lib(TWO.as_bytes(), "two.pgn");
        assert_eq!(l.progress().games, 2);
        let r = l.row(0).unwrap();
        assert_eq!((r.white.as_str(), r.white_elo, r.result.as_str(), r.date.as_str(), r.moves), ("Morphy, Paul", Some(2690), "1-0", "1858.11.02", 4));
        // The opening by the moves: Philidor (C41); the second by its own ECO code.
        assert_eq!((r.eco.as_str(), r.opening.as_str()), ("C41", "Philidor Defense"));
        let r = l.row(1).unwrap();
        assert_eq!((r.eco.as_str(), r.moves), ("C33", 3));
        assert_eq!(r.opening, "King's Gambit Accepted: Bishop's Gambit");
        // A tag-like line inside a comment is no new game.
        assert!(l.text(1).unwrap().contains("[not a tag]"));
        assert_eq!(l.game(0).unwrap().main_line().len(), 7);
        // Filters and sorting.
        let q = |q: Query| l.query(&q);
        assert_eq!(q(Query { player: "kiesERitz".into(), ..Query::default() }), vec![1]);
        assert_eq!(q(Query { opening: "c4".into(), ..Query::default() }), vec![0]);
        assert_eq!(q(Query { opening: "C".into(), ..Query::default() }), vec![0, 1]);
        assert_eq!(q(Query { opening: "philidor".into(), ..Query::default() }), vec![0]);
        assert_eq!(q(Query { from: Some(1855), ..Query::default() }), vec![0]);
        assert_eq!(q(Query { to: Some(1855), ..Query::default() }), vec![1]);
        assert_eq!(q(Query { elo: Some(2000), ..Query::default() }), Vec::<u32>::new());
        assert_eq!(q(Query { sort: Sort::White, ..Query::default() }), vec![1, 0]);
        assert_eq!(q(Query { sort: Sort::Date, desc: true, ..Query::default() }), vec![0, 1]);
        assert_eq!(q(Query { result: "0-1".into(), ..Query::default() }), Vec::<u32>::new());
    }

    #[test]
    fn packed_files() {
        let mut gz = flate2::write::GzEncoder::new(vec![], flate2::Compression::fast());
        gz.write_all(TWO.as_bytes()).unwrap();
        let (l, d) = lib(&gz.finish().unwrap(), "two.pgn.gz");
        assert_eq!(l.progress().games, 2);
        // Read back to front: the unpacker starts again for the earlier one.
        assert!(l.text(1).unwrap().contains("Anderssen"));
        assert!(l.text(0).unwrap().contains("Morphy"));
        drop(d);
    }

    /// A packed file of a few MB can unpack to an endless line: what is kept stays small.
    #[test]
    fn endless_lines_stay_small() {
        let mut gz = flate2::write::GzEncoder::new(vec![], flate2::Compression::fast());
        gz.write_all(b"[White \"").unwrap();
        let chunk = vec![b'x'; 1 << 20];
        for _ in 0..20 {
            gz.write_all(&chunk).unwrap();
        }
        gz.write_all(b"\"]\n\n1.e4 ").unwrap();
        for _ in 0..20 {
            gz.write_all(&chunk).unwrap();
        }
        gz.write_all(b" *\n\n[White \"B\"]\n\n1.d4 *\n").unwrap();
        let (l, _d) = lib(&gz.finish().unwrap(), "huge.pgn.gz");
        assert_eq!(l.progress().games, 2);
        assert!(l.row(0).unwrap().white.len() <= pgn::MAX_TAG);
        assert!(l.text(0).unwrap().len() <= MAX_GAME as usize);
        assert_eq!(l.row(1).unwrap().white, "B");
    }

    /// A small packed file of endless tiny games stops at the limits, with what it read.
    #[test]
    fn limits_stop_endless_games() {
        let mut zst = vec![];
        let mut gz = flate2::write::GzEncoder::new(&mut zst, flate2::Compression::fast());
        for i in 0..50_000 {
            write!(gz, "[White \"p{i}\"]\n1.e4 *\n").unwrap();
        }
        gz.finish().unwrap();
        let d = tempdir::Dir::new();
        let p = d.0.join("tiny.pgn.gz");
        std::fs::write(&p, &zst).unwrap();
        let wait = |l: &Table| {
            while !l.progress().done {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            l.progress()
        };
        let full = Limits::default();
        let p1 = wait(&Table::open_with(&p, Limits { games: 1000, ..full }).unwrap());
        assert_eq!((p1.games, p1.capped), (1000, true));
        let p2 = wait(&Table::open_with(&p, Limits { names: 64 * 1024, ..full }).unwrap());
        assert!(p2.capped && p2.games < 1000, "{p2:?}");
        let p4 = wait(&Table::open(&p).unwrap());
        assert_eq!((p4.games, p4.capped), (50_000, false));
    }

    /// A file opened in place unpacks as far as an import may (`knowledge.max_unpack_ratio`),
    /// whatever its size: a bomb stops at the ratio with the games read so far, a raised ratio
    /// reads it whole, and a stop is heard while the blank bytes unpack (set before the read
    /// starts, so no thread can finish first).
    #[test]
    fn an_open_file_stops_at_the_unpacking_ratio() {
        let d = tempdir::Dir::new();
        let p = d.0.join("bomb.pgn.gz");
        let mut games = String::new();
        for i in 0..1000 {
            games.push_str(&format!("[White \"p{i}\"]\n1.e4 *\n"));
        }
        // Gzip members one after another: 70 MB of blank lines from a file of some 100 KB.
        let mut bytes = crate::stream::tests::gz(&games);
        let blank = crate::stream::tests::gz(&"\n".repeat(1 << 20));
        for _ in 0..70 {
            bytes.extend_from_slice(&blank);
        }
        std::fs::write(&p, &bytes).unwrap();
        let wait = |l: &Table| {
            while !l.progress().done {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            l.progress()
        };
        let ratio = bragd_core::config::MAX_UNPACK_RATIO;
        let p1 = wait(&Table::open(&p).unwrap());
        // The last game, whose end the blank lines hold, is not listed.
        assert_eq!(p1.games, 999);
        assert!(p1.error.as_deref().is_some_and(|e| e.contains(&format!("more than {ratio} times"))), "{p1:?}");
        let whole = Limits { ratio: u64::MAX, ..Limits::default() };
        let p2 = wait(&Table::open_with(&p, whole).unwrap());
        assert_eq!((p2.games, p2.error, p2.read), (1000, None, bytes.len() as u64));
        let p3 = wait(&Table::start(&p, whole, Arc::new(AtomicBool::new(true))).unwrap());
        assert_eq!(p3.error.as_deref(), Some("stopped"));
        assert!(p3.read < bytes.len() as u64, "{p3:?}");
    }

    #[test]
    fn whatever_the_bytes() {
        // An unclosed comment ends where the next game's tags begin.
        let (l, _d) = lib(b"[Event \"x\"]\n\n1.e4 { never closed\n\n[Event \"y\"]\n\n1.d4 *\n", "brace.pgn");
        assert_eq!(l.progress().games, 2);
        drop(l);
        for text in [&b""[..], b"\n\n", b"[", b"[Event \"x\"]", b"1.e4", b"\xff\xfe[White \"\xe9\"]\n1.e4 *", b"{ never closed\n[Event \"y\"]\n1.d4"] {
            let (l, _d) = lib(text, "x.pgn");
            for i in 0..l.progress().games as u32 {
                let _ = l.game(i);
            }
        }
        // Latin-1 names come out as text.
        let (l, _d) = lib(b"[White \"Bj\xf6rn\"]\n\n1.e4 *\n", "latin.pgn");
        assert_eq!(l.row(0).unwrap().white, "Björn");
    }

    /// A big file, timed: `BRAGD_CHESS_BIG=/path/to/million.pgn cargo test --release -p
    /// bragd-library big_file -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn big_file() {
        let Ok(p) = std::env::var("BRAGD_CHESS_BIG") else { return };
        let t = std::time::Instant::now();
        let l = Table::open(Path::new(&p)).unwrap();
        let mut first = None;
        while !l.progress().done {
            if first.is_none() && l.progress().games > 0 {
                first = Some(t.elapsed());
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let read = t.elapsed();
        let n = l.progress().games;
        let rss = std::fs::read_to_string("/proc/self/status").unwrap_or_default().lines().find(|l| l.starts_with("VmRSS")).unwrap_or("").to_string();
        let time = |q: Query| {
            let t = std::time::Instant::now();
            let ids = l.query(&q);
            (ids.len(), t.elapsed())
        };
        println!("{n} games: first rows after {first:?}, all read in {read:?}; {rss}");
        println!("all: {:?}", time(Query::default()));
        println!("player 'carlsen': {:?}", time(Query { player: "carlsen".into(), ..Query::default() }));
        println!("opening 'gambit' + 1-0 + 1900-1950 + elo 2400: {:?}", time(Query { opening: "gambit".into(), result: "1-0".into(), from: Some(1900), to: Some(1950), elo: Some(2400), ..Query::default() }));
        println!("sort by white: {:?}", time(Query { sort: Sort::White, ..Query::default() }));
        println!("sort by date desc: {:?}", time(Query { sort: Sort::Date, desc: true, ..Query::default() }));
        for i in [0, n as u32 / 2, n as u32 - 1, 10].into_iter().filter(|&i| (i as usize) < n) {
            let t = std::time::Instant::now();
            let g = l.game(i).unwrap();
            println!("game {i}: {} moves, {} nodes, error {:?}, in {:?}", g.main_line().len(), g.nodes.len(), g.error, t.elapsed());
        }
    }

    /// A DhtmlXQ game as the community databases keep it, in brief: a byte order mark, tags on
    /// lines of their own, the pieces from the usual start.
    const DHTML: &str = "\u{feff}[DhtmlXQ]\n[DhtmlXQ_ver]www_vietcotuong_com[/DhtmlXQ_ver]\n[DhtmlXQ_red]布局圣手[/DhtmlXQ_red]\n[DhtmlXQ_black]兰帝[/DhtmlXQ_black]\n[DhtmlXQ_result]红胜[/DhtmlXQ_result]\n[DhtmlXQ_binit]0919293949596979891777062646668600102030405060708012720323436383[/DhtmlXQ_binit]\n[DhtmlXQ_movelist]262563641927[/DhtmlXQ_movelist]\n";

    #[test]
    fn a_dhtml_file_in_the_table() {
        let d = std::env::temp_dir().join(format!("bragd-xqd-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("0001-对兵互进右马局");
        std::fs::write(&p, DHTML).unwrap();
        assert_eq!(super::super::sniff(&p), Some(Format::Pgn));
        assert_eq!(kind(&p), Format::Xiangqi);
        let lib = Table::open(&p).unwrap();
        while !lib.progress().done {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let r = lib.row(0).unwrap();
        assert_eq!((r.white.as_str(), r.black.as_str(), r.moves), ("布局圣手", "兰帝", 2));
        assert_eq!(lib.game(0).unwrap().error, None);
        // A name with a dot that is no file type's ending is sniffed too; a real ending is not.
        for name in ["001-1.古谱的启示", "002.Opening game", "003.a-b", "game.abc", "004.xq2009"] {
            let q = d.join(name);
            std::fs::write(&q, DHTML).unwrap();
            assert_eq!(super::super::sniff(&q), Some(Format::Pgn), "{name}");
        }
        for name in ["game.rs", "game.PNG", "game.json"] {
            let q = d.join(name);
            std::fs::write(&q, DHTML).unwrap();
            assert_eq!(super::super::sniff(&q), None, "{name}");
        }
        assert!(dhtml_name(".hidden") && dhtml_name("G.CHE") && !dhtml_name("x.json"));
        // Any other file with no ending is no game, and a PGN file stays one.
        let other = d.join("README");
        std::fs::write(&other, "[DhtmX").unwrap();
        assert_eq!(super::super::sniff(&other), None);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// A draughts file's rows keep its own results (`2-0`), which the search finds as chess's.
    #[test]
    fn a_draughts_file_in_the_table() {
        let d = std::env::temp_dir().join(format!("bragd-pdn-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("games.pdn");
        std::fs::write(&p, "[Event \"Club\"]\n[White \"Anna\"]\n[Black \"Bram\"]\n[Result \"2-0\"]\n\n1. 32-28 19-23 2. 28x19 14x23 2-0\n\n[GameType \"21\"]\n[White \"Carl\"]\n[Black \"Dora\"]\n[Result \"0-1\"]\n\n1. 11-15 23-19 0-1\n").unwrap();
        let lib = Table::open(&p).unwrap();
        while !lib.progress().done {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!((lib.format, lib.progress().games), (Format::Pdn, 2));
        let r = lib.row(0).unwrap();
        assert_eq!((r.white.as_str(), r.result.as_str(), r.moves, r.eco.as_str(), r.opening.as_str()), ("Anna", "2-0", 2, "", ""));
        assert_eq!(lib.query(&Query { result: "1-0".into(), ..Default::default() }), vec![0]);
        assert_eq!(lib.game(1).unwrap().error, None);
        std::fs::remove_dir_all(&d).unwrap();
    }
}
