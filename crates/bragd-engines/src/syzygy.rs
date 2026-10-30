//! Syzygy tablebases: the folders that hold them, passed to every engine that reads them
//! (`SyzygyPath`); a plan of what a download of a set fetches and its size, before anything is
//! fetched; and the download itself. 3–5 pieces is what the wizard recommends (about 1 GB), 6
//! pieces is offered when asked (about 160 GB), and 7 pieces (about 17 TB) is a folder only.
//!
//! The files and their sizes come from the official mirror's `bytes.tsv`, one request.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::fetch::Fetch;

/// The official mirror, run by Lichess.
pub const MIRROR: &str = "https://tablebase.lichess.ovh/tables/standard/";

/// The tablebase settings: the folders that hold tables, and whether the Lichess tablebase API
/// answers for positions they do not cover.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tablebases {
    pub folders: Vec<PathBuf>,
    pub online: bool,
}

impl Tablebases {
    /// `SyzygyPath` for an engine: the folders that exist, joined as the engines read them
    /// (`:`, or `;` on Windows). None when there are none.
    pub fn syzygy_path(&self) -> Option<String> {
        let sep = if cfg!(windows) { ";" } else { ":" };
        let here: Vec<String> = self.folders.iter().filter(|f| f.is_dir()).map(|f| f.display().to_string()).collect();
        (!here.is_empty()).then(|| here.join(sep))
    }

    /// Whether a table for the material of the chess position `fen` is in one of the folders
    /// (its WDL file, the stronger side named either way round).
    pub fn covers(&self, fen: &str) -> bool {
        let Some((white, black)) = material(fen) else { return false };
        let names = [format!("{white}v{black}.rtbw"), format!("{black}v{white}.rtbw")];
        self.folders.iter().any(|f| names.iter().any(|n| f.join(n).is_file()))
    }
}

/// The pieces of each side of a FEN, as tables name them: `KQR`, `KRP`.
pub fn material(fen: &str) -> Option<(String, String)> {
    let board = fen.split_whitespace().next()?;
    let order = |c: char| "KQRBNP".find(c.to_ascii_uppercase());
    let side = |white: bool| {
        let mut v: Vec<char> = board.chars().filter(|c| c.is_ascii_alphabetic() && c.is_ascii_uppercase() == white).map(|c| c.to_ascii_uppercase()).collect();
        v.sort_by_key(|c| order(*c));
        v.into_iter().collect::<String>()
    };
    let (w, b) = (side(true), side(false));
    (w.starts_with('K') && b.starts_with('K')).then_some((w, b))
}

/// How many pieces a table holds: `KQRvKR.rtbw` holds 5.
pub fn pieces(name: &str) -> usize {
    name.split('.').next().unwrap_or("").chars().filter(|c| c.is_ascii_uppercase()).count()
}

/// A set to download.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Set {
    /// 3, 4 and 5 pieces: what the wizard recommends.
    UpTo5,
    /// 6 pieces, only when asked.
    Six,
    /// 7 pieces: too large to download here; a folder only.
    Seven,
}

/// A file of a plan.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Planned {
    pub name: String,
    pub url: String,
    pub size: u64,
}

/// What a download of a set fetches, and how much.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Plan {
    pub set: Set,
    pub files: Vec<Planned>,
    pub total: u64,
}

/// The mirror's `bytes.tsv`: a size and a name per line.
pub fn parse_sizes(tsv: &str) -> Vec<(String, u64)> {
    tsv.lines()
        .filter_map(|l| {
            let (size, name) = l.split_once('\t')?;
            let name = name.trim();
            // Pieces and `v` only, so no name (`C:KQvK.rtbw`, `../x`) can lead out of the folder.
            let ok = name.strip_suffix(".rtbw").or_else(|| name.strip_suffix(".rtbz")).is_some_and(|p| !p.is_empty() && p.chars().all(|c| "KQRBNPv".contains(c)));
            ok.then(|| Some((name.to_string(), size.trim().parse().ok()?))).flatten()
        })
        .collect()
}

/// The plan for `set` from the mirror's `bytes.tsv`. Err for 7 pieces, which is a folder only.
pub fn plan(set: Set, tsv: &str) -> Result<Plan, String> {
    let (count, wdl, dtz) = match set {
        Set::UpTo5 => (3..=5, "3-4-5-wdl", "3-4-5-dtz"),
        Set::Six => (6..=6, "6-wdl", "6-dtz"),
        Set::Seven => return Err("7-piece tablebases are about 17 TB: download them yourself and choose their folder.".into()),
    };
    let files: Vec<Planned> = parse_sizes(tsv)
        .into_iter()
        .filter(|(name, _)| count.contains(&pieces(name)))
        .map(|(name, size)| {
            let dir = if name.ends_with(".rtbw") { wdl } else { dtz };
            Planned { url: format!("{MIRROR}{dir}/{name}"), name, size }
        })
        .collect();
    if files.is_empty() {
        return Err("The mirror lists no tables for this set.".into());
    }
    Ok(Plan { set, total: files.iter().map(|f| f.size).sum(), files })
}

/// The plan for `set`, from the mirror.
pub fn plan_from(fetch: &dyn Fetch, set: Set) -> Result<Plan, String> {
    if set == Set::Seven {
        return plan(set, "");
    }
    let tsv = fetch.get(&format!("{MIRROR}bytes.tsv"), 1 << 20).map_err(|e| format!("The tablebase mirror could not be reached: {e}"))?;
    plan(set, &String::from_utf8_lossy(&tsv))
}

/// What a download did.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Downloaded {
    pub fetched: usize,
    /// Already there, with the right size.
    pub kept: usize,
}

/// Downloads `plan` into `folder`: a file already there with its size is kept; each other goes
/// to `<name>.part` and is renamed when its size is right. `progress` is told the bytes done of
/// the plan's total (false stops it, keeping the files done).
pub fn download(plan: &Plan, folder: &Path, fetch: &dyn Fetch, progress: &mut dyn FnMut(u64, u64) -> bool) -> Result<Downloaded, String> {
    std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let mut done = Downloaded::default();
    let mut bytes = 0u64;
    for f in &plan.files {
        let to = folder.join(&f.name);
        if std::fs::metadata(&to).is_ok_and(|m| m.len() == f.size) {
            bytes += f.size;
            done.kept += 1;
            continue;
        }
        let part = folder.join(format!("{}.part", f.name));
        let mut out = std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
        // Stopped as soon as it passes its size, so a server that keeps sending cannot fill the disk.
        let got = fetch.download(&f.url, &mut out, &mut |n| n <= f.size && progress(bytes + n, plan.total));
        drop(out);
        match got {
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted && std::fs::metadata(&part).is_ok_and(|m| m.len() > f.size) => {
                let _ = std::fs::remove_file(&part);
                return Err(format!("{}: more bytes came than the {} expected.", f.name, f.size));
            }
            Ok(n) if n == f.size => std::fs::rename(&part, &to).map_err(|e| format!("{}: {e}", to.display()))?,
            Ok(n) => {
                let _ = std::fs::remove_file(&part);
                return Err(format!("{}: {n} bytes came, {} were expected.", f.name, f.size));
            }
            Err(e) => {
                let _ = std::fs::remove_file(&part);
                return Err(if e.kind() == std::io::ErrorKind::Interrupted { "Download stopped.".into() } else { format!("{}: {e}", f.name) });
            }
        }
        bytes += f.size;
        done.fetched += 1;
    }
    Ok(done)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    /// Lines of the mirror's `bytes.tsv` as recorded on 2026-10-30.
    pub const SIZES: &str = "208\tKRvK.rtbw\n7632\tKRvK.rtbz\n272\tKQvK.rtbw\n5392\tKQvK.rtbz\n179408\tKRvKP.rtbw\n193424\tKRvKP.rtbz\n279824\tKQRvKR.rtbw\n3421968\tKQRvKR.rtbz\n175112784\tKQRvKRP.rtbw\n405163472\tKQRvKRP.rtbz\n15078224\tKQRBvKR.rtbw\n33470096\tKQQQQQvK.rtbw\n12\t../evil.rtbw\n12\tC:KQvK.rtbw\n12\t.rtbz\n";

    /// A fetcher that serves `files` by URL, as many bytes as asked, and writes down what it
    /// was asked.
    pub struct Fake {
        pub files: BTreeMap<String, Vec<u8>>,
        pub asked: Mutex<Vec<String>>,
    }

    impl Fetch for Fake {
        fn get(&self, url: &str, max: u64) -> std::io::Result<Vec<u8>> {
            self.asked.lock().unwrap().push(url.to_string());
            let b = self.files.get(url).cloned().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "404"))?;
            if b.len() as u64 > max { Err(std::io::Error::other("too large")) } else { Ok(b) }
        }

        fn download(&self, url: &str, to: &mut dyn std::io::Write, progress: &mut dyn FnMut(u64) -> bool) -> std::io::Result<u64> {
            let b = self.get(url, u64::MAX)?;
            to.write_all(&b)?;
            if !progress(b.len() as u64) {
                return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "stopped"));
            }
            Ok(b.len() as u64)
        }
    }

    #[test]
    fn a_plan_lists_the_set_and_its_size() {
        let p = plan(Set::UpTo5, SIZES).unwrap();
        assert_eq!(p.files.len(), 8);
        assert_eq!(p.total, 208 + 7632 + 272 + 5392 + 179408 + 193424 + 279824 + 3421968);
        assert_eq!(p.files[0], Planned { name: "KRvK.rtbw".into(), url: format!("{MIRROR}3-4-5-wdl/KRvK.rtbw"), size: 208 });
        assert_eq!(p.files[1].url, format!("{MIRROR}3-4-5-dtz/KRvK.rtbz"));
        assert!(!p.files.iter().any(|f| f.name.contains("..") || f.name.contains(':') || f.name.starts_with('.')), "a name with a path is refused");
        assert_eq!(parse_sizes(SIZES).len(), 12, "only the twelve real names");
        let six = plan(Set::Six, SIZES).unwrap();
        assert_eq!((six.files.len(), six.files[0].url.as_str()), (3, "https://tablebase.lichess.ovh/tables/standard/6-wdl/KQRvKRP.rtbw"));
        assert!(plan(Set::Seven, SIZES).is_err(), "a folder only");
        assert_eq!((pieces("KQRvKR.rtbw"), pieces("KQQQQQvK.rtbz")), (5, 7));
    }

    #[test]
    fn a_download_fetches_what_is_missing_and_checks_its_size() {
        let d = crate::test_dir("syzygy");
        let mut files = BTreeMap::new();
        files.insert(format!("{MIRROR}bytes.tsv"), "3\tKRvK.rtbw\n4\tKRvK.rtbz\n6\tKQvK.rtbw\n".as_bytes().to_vec());
        files.insert(format!("{MIRROR}3-4-5-wdl/KRvK.rtbw"), b"abc".to_vec());
        files.insert(format!("{MIRROR}3-4-5-dtz/KRvK.rtbz"), b"abcd".to_vec());
        files.insert(format!("{MIRROR}3-4-5-wdl/KQvK.rtbw"), b"short".to_vec());
        let fake = Fake { files, asked: Mutex::new(vec![]) };
        let p = plan_from(&fake, Set::UpTo5).unwrap();
        assert_eq!(p.total, 13);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("KRvK.rtbz"), b"abcd").unwrap();
        let mut seen = vec![];
        let e = download(&p, &d, &fake, &mut |n, of| {
            seen.push((n, of));
            true
        })
        .unwrap_err();
        assert!(e.contains("KQvK.rtbw") && e.contains("expected"), "{e}");
        assert_eq!(std::fs::read(d.join("KRvK.rtbw")).unwrap(), b"abc");
        assert!(!d.join("KQvK.rtbw").exists() && !d.join("KQvK.rtbw.part").exists(), "a wrong size is not kept");
        assert!(!fake.asked.lock().unwrap().iter().any(|u| u.ends_with("KRvK.rtbz")), "a file there is not fetched again");
        assert_eq!(seen[0], (3, 13));
        // Then with the right file: done, nothing fetched twice.
        let mut p = p;
        p.files[2].size = 5;
        let done = download(&p, &d, &fake, &mut |_, _| true).unwrap();
        assert_eq!(done, Downloaded { fetched: 1, kept: 2 });
        let tb = Tablebases { folders: vec![d.clone(), d.join("missing")], online: false };
        assert_eq!(tb.syzygy_path(), Some(d.display().to_string()));
        assert!(tb.covers("8/8/8/4k3/8/8/8/KQ6 w - - 0 1") && tb.covers("8/8/8/4K3/8/8/8/kq6 w - - 0 1"));
        assert!(!tb.covers("8/8/8/4k3/8/8/8/KN6 w - - 0 1"));
        // Stopped: the file under way is not kept.
        std::fs::remove_file(d.join("KQvK.rtbw")).unwrap();
        assert_eq!(download(&p, &d, &fake, &mut |_, _| false).unwrap_err(), "Download stopped.");
        // A server sending more than the size: stopped there, nothing kept.
        p.files[2].size = 4;
        let e = download(&p, &d, &fake, &mut |_, _| true).unwrap_err();
        assert!(e.contains("more bytes"), "{e}");
        assert!(!d.join("KQvK.rtbw").exists() && !d.join("KQvK.rtbw.part").exists());
        assert!(!d.join("KQvK.rtbw.part").exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn material_as_tables_name_it() {
        assert_eq!(material("8/8/8/4k3/8/8/1r6/KQ6 w - - 0 1"), Some(("KQ".into(), "KR".into())));
        assert_eq!(material("8/8/8/4k3/8/8/8/K7 w - - 0 1"), Some(("K".into(), "K".into())));
        assert_eq!(material("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap().0, "KQRRBBNNPPPPPPPP");
        assert_eq!(material("nonsense"), None);
    }
}
