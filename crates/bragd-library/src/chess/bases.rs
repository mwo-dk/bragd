//! SCID and ChessBase databases on the disk: the files beside one another under one name,
//! handed to the readers (`bragd_gamefiles::chess::{scid, cbh}`) as `Files`. ChessBase archives
//! (`.cbv`) are unpacked into this process's own folder in the temporary folder, once per
//! archive as it is, and read from there; the archive is never changed.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use bragd_gamefiles::chess::{cbh, cbv, scid};
use bragd_gamefiles::{Files, Source};

/// How many unpacked archives stay in the cache at a time.
const KEPT: usize = 4;

/// Whether `path` is a SCID index file by its name.
pub fn is_scid(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("si4") || e.eq_ignore_ascii_case("si5"))
}

/// The SCID database whose index is at `path` (`x.si4`, beside `x.sg4` and `x.sn4`).
pub fn scid(path: &Path) -> io::Result<scid::Base> {
    let p = path.to_path_buf();
    let v5 = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("si5"));
    scid::Base::open(Box::new(move |ext| Ok(Box::new(File::open(p.with_extension(ext))?) as Box<dyn Source>)), v5)
}

/// Whether `path` is a ChessBase database's header file, or an archive of one (`.cbv`), by its
/// name.
pub fn is_cbh(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("cbh")) || is_cbv(path)
}

/// Whether `path` is a ChessBase archive, by its name.
pub fn is_cbv(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("cbv"))
}

/// The `.cbh` files to read for `path`: itself, or an archive's unpacked ones (one a database
/// it holds). A game's record names its database in its high 32 bits (`cbh_scan`).
fn headers(path: &Path) -> io::Result<Vec<PathBuf>> {
    if is_cbv(path) { unpacked(path) } else { Ok(vec![path.to_path_buf()]) }
}

/// The file beside `cbh` with the ending `ext`: the same case first, else any case.
fn sibling(cbh: &Path, ext: &str) -> PathBuf {
    let exact = cbh.with_extension(ext);
    if exact.exists() {
        return exact;
    }
    let upper = cbh.with_extension(ext.to_ascii_uppercase());
    if upper.exists() { upper } else { exact }
}

/// The ChessBase database whose `.cbh` file is `cbh`, as the reader takes it.
fn cbh_files(cbh: &Path) -> Box<Files> {
    let p = cbh.to_path_buf();
    Box::new(move |ext| Ok(Box::new(File::open(sibling(&p, ext))?) as Box<dyn Source>))
}

/// The games of the database at `path` (an archive's databases one after the other), in order,
/// each handed to `each` with its record (its database above, its `.cbh` record below) until it
/// says to stop; `read` is told the bytes of the `.cbh` files read so far.
pub(crate) fn cbh_scan(path: &Path, stop: &dyn Fn() -> bool, read: &dyn Fn(u64), mut each: impl FnMut(u64, &cbh::Listed) -> bool) -> io::Result<()> {
    let mut base = 0u64;
    for (db, path) in headers(path)?.iter().enumerate() {
        let (mut more, end) = (true, std::cell::Cell::new(0));
        cbh::scan(&*cbh_files(path), stop, &|n| {
            end.set(end.get().max(n));
            read(base + n)
        }, |g| {
            more = each((db as u64) << 32 | g.record as u64, g);
            more
        })?;
        if !more || stop() {
            return Ok(());
        }
        base += end.get();
    }
    Ok(())
}

/// Game `record` (as `cbh_scan` lists it) of the database at `path` as PGN.
pub fn cbh_pgn(path: &Path, record: u64) -> io::Result<String> {
    let cbh = headers(path)?.into_iter().nth((record >> 32) as usize).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such game"))?;
    cbh::pgn(&*cbh_files(&cbh), record as u32)
}

/// The archives a `Table` reads now, once per reader: their unpacking is never evicted.
static IN_USE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// An archive in use for as long as this lives (`hold`).
pub(crate) struct Held(PathBuf);

/// Marks `cbv` in use until the `Held` is dropped: its unpacked files are kept meanwhile.
pub(crate) fn hold(cbv: &Path) -> Held {
    IN_USE.lock().unwrap_or_else(|e| e.into_inner()).push(cbv.to_path_buf());
    Held(cbv.to_path_buf())
}

impl Drop for Held {
    fn drop(&mut self) {
        let mut in_use = IN_USE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = in_use.iter().position(|p| *p == self.0) {
            in_use.swap_remove(i);
        }
    }
}

/// This process's folder for unpacked archives, in the temporary folder, made on first use.
/// shortcut: never swept after a crash; move it under Bragd's cache with a sweep when the Helper owns caches.
fn run_folder() -> io::Result<PathBuf> {
    let run = std::env::temp_dir().join(format!("bragd-cbv-run-{}", std::process::id()));
    private_dir(&run)?;
    Ok(run)
}

/// A folder only its owner can read, made when missing. One already there (in the shared
/// temporary folder, someone else may have made it first, or put a link there) must be a real
/// folder of ours: only its owner can set its mode.
fn private_dir(dir: &Path) -> io::Result<()> {
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
    b.create(dir)?;
    if !std::fs::symlink_metadata(dir)?.is_dir() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "the folder for unpacked archives is a link"));
    }
    #[cfg(unix)]
    std::fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(0o700))?;
    Ok(())
}

/// Archives unpacked in this run: by path, size and time, the `.cbh` files in the cache.
type Unpacked = (PathBuf, u64, Option<std::time::SystemTime>, Vec<PathBuf>);
static UNPACKED: Mutex<Vec<Unpacked>> = Mutex::new(Vec::new());

/// The `.cbh` files (or, for an archive of PGN databases, the `.pgn` files) of the databases
/// archive `cbv` holds, in the archive's order, unpacked into this run's folder (with the files
/// beside each its games need) when they are not there yet as the archive is now.
pub fn unpacked(cbv: &Path) -> io::Result<Vec<PathBuf>> {
    unpacked_with(cbv, &|| false, &|_, _| {})
}

/// `unpacked`, asking `stop` between blocks and telling `progress` the packed bytes read of all.
pub(crate) fn unpacked_with(cbv: &Path, stop: &dyn Fn() -> bool, progress: &dyn Fn(u64, u64)) -> io::Result<Vec<PathBuf>> {
    let meta = std::fs::metadata(cbv)?;
    let (size, time) = (meta.len(), meta.modified().ok());
    let current = |done: &[Unpacked]| done.iter().find(|d| d.0 == cbv && d.1 == size && d.2 == time && d.3.iter().all(|p| p.is_file())).map(|d| d.3.clone());
    // The lock is held only to look and to note: other archives open while this one unpacks.
    if let Some(out) = current(&UNPACKED.lock().unwrap_or_else(|e| e.into_inner())) {
        return Ok(out);
    }
    let mut f = File::open(cbv)?;
    let plan = cbv::plan(&mut f)?;
    let run = run_folder()?;
    let dir = run.join(format!("{:016x}", {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (cbv, size, time).hash(&mut h);
        h.finish()
    }));
    // Unpacked into a folder of its own first, so two opens of the same archive never write
    // into one folder, and a folder in the list is always whole.
    static PART: AtomicU64 = AtomicU64::new(0);
    let part = run.join(format!("part-{}", PART.fetch_add(1, Ordering::Relaxed)));
    private_dir(&part)?;
    let mut read = 0u64;
    let written = plan.members.iter().try_for_each(|(name, m)| {
        let to = File::create(part.join(name))?;
        cbv::unpack(&mut f, m, to, stop, &mut |n| progress(read + n, plan.packed))?;
        read += m.packed;
        Ok(())
    });
    if let Err(e) = written {
        let _ = std::fs::remove_dir_all(&part);
        return Err(e);
    }
    let mut done = UNPACKED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(out) = current(&done) {
        let _ = std::fs::remove_dir_all(&part);
        return Ok(out);
    }
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(e) = std::fs::rename(&part, &dir) {
        let _ = std::fs::remove_dir_all(&part);
        return Err(e);
    }
    // This archive's older unpacking goes, then the oldest not in use past the four last kept
    // (a reader of many archives keeps no more than that in the cache). When all
    // are in use, there are more for a while.
    let mut gone: Vec<Unpacked> = vec![];
    done.retain(|d| d.0 != cbv || {
        gone.push(d.clone());
        false
    });
    let in_use = IN_USE.lock().unwrap_or_else(|e| e.into_inner());
    while done.len() >= KEPT {
        let Some(i) = done.iter().position(|d| !in_use.contains(&d.0)) else { break };
        gone.push(done.remove(i));
    }
    drop(in_use);
    for old in gone {
        if let Some(d) = old.3.first().and_then(|p| p.parent())
            && d != dir
        {
            let _ = std::fs::remove_dir_all(d);
        }
    }
    let out: Vec<PathBuf> = (0..plan.databases).map(|n| dir.join(format!("db{n}.{}", plan.ending))).collect();
    done.push((cbv.to_path_buf(), size, time, out.clone()));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::{Table, games};
    use super::*;
    use bragd_gamefiles::chess::{pgn, polyglot};

    /// The readers' test databases.
    fn fixtures(dir: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../bragd-gamefiles/src/chess").join(dir)
    }

    fn done(lib: &Table) {
        while !lib.progress().done {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// The table and the positions read a SCID base as they read a PGN file.
    #[test]
    fn the_table_reads_a_scid_base() {
        for name in ["test.si4", "test.si5"] {
            let p = fixtures("scid-test").join(name);
            let lib = Table::open(&p).unwrap();
            done(&lib);
            assert_eq!((lib.progress().games, lib.progress().error.clone()), (30, None));
            let r = lib.row(0).unwrap();
            assert_eq!((r.white.as_str(), r.black.as_str(), r.eco.as_str(), r.date.as_str(), r.result.as_str()), ("Morphy, Paul", "Duke Karl / Count Isouard", "C41", "1858.11.02", "1-0"));
            assert_eq!((r.white_elo, r.moves, r.event.as_str()), (Some(2690), 17, "Paris"));
            assert_eq!(lib.game(0).unwrap().main_line().len(), 33);
            let mut seen = vec![];
            games::positions(&p, &|| false, |n, keys| seen.push((n, keys.len()))).unwrap();
            assert_eq!(seen.len(), 30);
            assert!(seen[0].1 > 30);
        }
    }

    /// A game SCID 5 marked deleted is no row, and the positions number games by their rows.
    #[test]
    fn a_deleted_scid5_game_is_no_row() {
        let d = std::env::temp_dir().join(format!("bragd-scid5-table-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        for ext in ["si5", "sg5", "sn5"] {
            std::fs::copy(fixtures("scid-test").join("test").with_extension(ext), d.join("t").with_extension(ext)).unwrap();
        }
        let si5 = d.join("t.si5");
        let mut b = std::fs::read(&si5).unwrap();
        b[2 * 56 + 28] |= 0x08;
        std::fs::write(&si5, b).unwrap();
        let base = scid(&si5).unwrap();
        let lib = Table::open(&si5).unwrap();
        done(&lib);
        assert_eq!(lib.progress().games, 29);
        // The deleted game's entry counts as read: the bar is full.
        assert_eq!(lib.progress().read, lib.progress().size);
        assert_eq!(lib.row(2).unwrap().event, base.event(&base.entry(3).unwrap()));
        // Row 2 is SCID's game 3, and the deleted game's positions are not there.
        let mut found = vec![];
        games::positions(&si5, &|| false, |n, keys| found.push((n, keys.to_vec()))).unwrap();
        assert_eq!(found.iter().map(|f| f.0).collect::<Vec<_>>(), (0..29).collect::<Vec<_>>());
        let keys_of = |n: u32| {
            let g = pgn::parse(&base.pgn(n).unwrap());
            let mut k: Vec<u64> = g.main_line().iter().map_while(|&i| g.nodes[i as usize].fen.as_deref().and_then(pgn::board)).map(|b| polyglot::key(&b)).collect();
            k.sort_unstable();
            k.dedup();
            k
        };
        assert_eq!(found[2].1, keys_of(3));
        assert_ne!(keys_of(2), keys_of(3));
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// The table, a game and the positions of a ChessBase database whose files are named in
    /// capitals beside a `.cbh` in lower case.
    #[test]
    fn the_table_reads_a_chessbase_database() {
        let d = std::env::temp_dir().join(format!("bragd-cbh-table-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        for ext in ["cbh", "cbg", "cba", "cbp", "cbt"] {
            let to = if ext == "cbh" { "Games.cbh".to_string() } else { format!("Games.{}", ext.to_ascii_uppercase()) };
            std::fs::copy(fixtures("cbh-test").join(format!("Games.{ext}")), d.join(to)).unwrap();
        }
        let p = d.join("Games.cbh");
        let lib = Table::open(&p).unwrap();
        done(&lib);
        assert_eq!(lib.progress().error, None);
        let row = lib.row(0).unwrap();
        assert!(!row.white.is_empty() && row.moves > 0, "{row:?}");
        let main = lib.game(0).unwrap().main_line().len();
        assert!(main > 0);
        let mut keys = vec![];
        games::positions(&p, &|| false, |n, k| keys.push((n, k.len()))).unwrap();
        assert_eq!(keys.len(), lib.progress().games);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_place_of_the_unpack_folder_is_refused() {
        let d = std::env::temp_dir().join(format!("bragd-cbv-link-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("elsewhere")).unwrap();
        std::os::unix::fs::symlink(d.join("elsewhere"), d.join("run")).unwrap();
        assert!(private_dir(&d.join("run")).is_err());
        std::fs::set_permissions(d.join("elsewhere"), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        private_dir(&d.join("elsewhere")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(d.join("elsewhere")).unwrap().permissions().mode() & 0o777, 0o700, "made private again");
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Held by the tests that unpack archives: the archives kept (`KEPT`) are counted over all
    /// of them, so one test's eviction must not meet another's archives.
    static UNPACKING: Mutex<()> = Mutex::new(());

    /// An archive of `files` (name, bytes), each in stored blocks of 1000 bytes.
    fn archive(files: &[(String, Vec<u8>)]) -> Vec<u8> {
        let rec = 173usize;
        let mut out = vec![8u8, 0, files.len() as u8, 0, rec as u8, 0, 3, 0];
        let streams: Vec<Vec<u8>> = files
            .iter()
            .map(|(_, b)| {
                let mut s = vec![];
                for c in b.chunks(1000) {
                    s.extend(((c.len() + 1) as u16).to_le_bytes());
                    s.extend([0, 0, 0]);
                    s.extend(c);
                }
                s
            })
            .collect();
        let mut at = 8 + files.len() * rec;
        for ((name, b), st) in files.iter().zip(&streams) {
            let mut r = vec![0u8; rec];
            r[..name.len()].copy_from_slice(name.as_bytes());
            r[128..132].copy_from_slice(&(at as u32).to_le_bytes());
            r[132..136].copy_from_slice(&(st.len() as u32).to_le_bytes());
            r[136..140].copy_from_slice(&(b.len() as u32).to_le_bytes());
            out.extend(r);
            at += st.len();
        }
        out.extend(streams.concat());
        out
    }

    /// The test database's files under `name`, for an archive.
    fn database(name: &str) -> Vec<(String, Vec<u8>)> {
        ["cbh", "cbg", "cba", "cbp", "cbt"].iter().map(|ext| (format!("{name}.{ext}"), std::fs::read(fixtures("cbh-test").join(format!("Games.{ext}"))).unwrap())).collect()
    }

    /// An archive of two databases lists the games of both, the second's after the first's,
    /// and each opens from its own files.
    #[test]
    fn every_database_of_an_archive_is_read() {
        let _one = UNPACKING.lock().unwrap_or_else(|e| e.into_inner());
        let d = std::env::temp_dir().join(format!("bragd-cbv-two-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let one = Table::open(&fixtures("cbh-test").join("Games.cbh")).unwrap();
        done(&one);
        let n = one.progress().games;
        let cbv = d.join("Two.cbv");
        std::fs::write(&cbv, archive(&[database("Games"), database("More")].concat())).unwrap();
        assert_eq!(unpacked(&cbv).unwrap().len(), 2);
        let lib = Table::open(&cbv).unwrap();
        done(&lib);
        assert_eq!(lib.progress().games, 2 * n);
        assert_eq!(lib.text(n as u32).unwrap(), one.text(0).unwrap());
        assert_eq!(lib.text(0).unwrap(), one.text(0).unwrap());
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// An archive of a PGN database (ChessBase backs those up as `.cbv` too: the `.pgn`, its
    /// `.ini` and `.pgi`) opens as the PGN file: the table, a game, its positions.
    #[test]
    fn an_archive_of_a_pgn_database_opens_as_it() {
        let _one = UNPACKING.lock().unwrap_or_else(|e| e.into_inner());
        let d = std::env::temp_dir().join(format!("bragd-cbv-pgn-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let pgn = "[White \"Morphy, Paul\"]\n[Black \"Anderssen, Adolf\"]\n[Result \"1-0\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 1-0\n\n[White \"Tal, Mikhail\"]\n[Black \"Botvinnik, Mikhail\"]\n[Result \"0-1\"]\n\n1. d4 Nf6 0-1\n";
        let files: Vec<(String, Vec<u8>)> =
            vec![("Games.pgn".into(), pgn.into()), ("Games.ini".into(), b"[x]".to_vec()), ("Games.pgi".into(), vec![0; 40])];
        let cbv = d.join("Games.cbv");
        std::fs::write(&cbv, archive(&files)).unwrap();
        let lib = Table::open(&cbv).unwrap();
        done(&lib);
        assert_eq!(lib.progress().error, None);
        assert_eq!(lib.progress().games, 2);
        assert_eq!(lib.row(1).unwrap().white, "Tal, Mikhail");
        assert_eq!(lib.game(0).unwrap().main_line().len(), 5);
        assert_eq!(lib.game(1).unwrap().main_line().len(), 2);
        assert_eq!(lib.row(1).unwrap().black, "Botvinnik, Mikhail");
        let mut seen = vec![];
        games::positions(&cbv, &|| false, |n, keys| seen.push((n, keys.len()))).unwrap();
        assert_eq!(seen, [(0, 5), (1, 2)]);
        drop(lib);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// A database packed into an archive (stored blocks, a picture in a folder of its own beside
    /// it) opens as the database; the archive is left as it was, and the unpacked archives kept
    /// are the four last, never one still read.
    #[test]
    fn an_archive_opens_as_its_database() {
        let _one = UNPACKING.lock().unwrap_or_else(|e| e.into_inner());
        let d = std::env::temp_dir().join(format!("bragd-cbv-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let one = Table::open(&fixtures("cbh-test").join("Games.cbh")).unwrap();
        done(&one);
        let mut files: Vec<(String, Vec<u8>)> = vec![("Games.cbh\\pic.bmp".into(), vec![1, 2, 3])];
        files.extend(database("Games"));
        let out = archive(&files);
        let cbv = d.join("Games.cbv");
        std::fs::write(&cbv, &out).unwrap();
        let lib = Table::open(&cbv).unwrap();
        done(&lib);
        assert_eq!(lib.row(0).unwrap().white, one.row(0).unwrap().white);
        assert_eq!(lib.text(0).unwrap(), one.text(0).unwrap());
        assert_eq!(std::fs::read(&cbv).unwrap(), out);
        // Cut short: an error, and nothing half unpacked is used.
        let cut = d.join("Cut.cbv");
        std::fs::write(&cut, &out[..out.len() - 10]).unwrap();
        assert!(unpacked(&cut).is_err());
        // Five newer archives do not take the files of one still read: the oldest unused go.
        let mine = unpacked(&cbv).unwrap();
        let others: Vec<PathBuf> = (0..5).map(|i| d.join(format!("Other{i}.cbv"))).collect();
        let theirs: Vec<Vec<PathBuf>> = others
            .iter()
            .map(|o| {
                std::fs::write(o, &out).unwrap();
                unpacked(o).unwrap()
            })
            .collect();
        assert!(mine[0].is_file(), "in use, kept");
        assert_eq!(lib.text(0).unwrap(), one.text(0).unwrap());
        assert!(!theirs[0][0].exists() && theirs[4][0].is_file(), "the oldest unused went");
        // Read no more: its files go when the next archive comes.
        drop(lib);
        std::fs::write(&others[0], [&out[..], &[0]].concat()).unwrap();
        unpacked(&others[0]).unwrap();
        assert!(!mine[0].exists(), "no longer in use, evicted");
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Every game of a real database or archive (`BRAGD_CBV=/path/to/x.cbv` or `.cbh`) read to
    /// its end. No real archive is in the repository.
    #[test]
    #[ignore]
    fn a_real_archive_reads_whole() {
        let p = PathBuf::from(std::env::var("BRAGD_CBV").unwrap());
        let (mut games, mut cut) = (0, vec![]);
        cbh_scan(&p, &|| false, &|_| {}, |record, _| {
            let text = cbh_pgn(&p, record).unwrap();
            games += 1;
            if text.contains(cbh::CUT_NOTE) {
                cut.push(record);
            }
            true
        })
        .unwrap();
        eprintln!("{games} games, cut: {cut:?}");
        assert!(games > 0 && cut.is_empty());
    }
}
