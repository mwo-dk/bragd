//! Big downloads read as a stream, game by game: a PGN or PDN file, packed or not (`.gz`,
//! `.zst`, `.bz2`), or the game files inside a zip, unpacked as they are read and never written
//! out whole. The pass is `chess::games::scan`'s, so one game's text is held at a time (at most
//! `games::MAX_GAME` bytes) and one line (`lex::MAX_LINE`), whatever the size of the download:
//! a month of the Lichess database or a year of FICS goes in straight from the packed file.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;
use std::sync::atomic::AtomicBool;

use crate::chess::{Format, games};

/// `r`, the bytes of a file named `name`, unpacked as they are read when the name says it is
/// packed (`.gz`, `.zst`, `.bz2`).
pub fn unpack<'a>(name: &str, r: impl Read + Send + 'a) -> io::Result<Box<dyn Read + Send + 'a>> {
    let n = name.to_ascii_lowercase();
    Ok(if n.ends_with(".gz") {
        Box::new(flate2::read::MultiGzDecoder::new(r))
    } else if n.ends_with(".zst") {
        Box::new(ruzstd::decoding::StreamingDecoder::new(r).map_err(io::Error::other)?)
    } else if n.ends_with(".bz2") {
        Box::new(bzip2::read::MultiBzDecoder::new(r))
    } else {
        Box::new(r)
    })
}

/// `name` without the ending that says how it is packed.
pub fn unpacked_name(name: &str) -> &str {
    [".gz", ".zst", ".bz2"].iter().find_map(|e| name.len().checked_sub(e.len()).filter(|&i| name.is_char_boundary(i) && name[i..].eq_ignore_ascii_case(e)).map(|i| &name[..i])).unwrap_or(name)
}

/// Whether a file named `name` is read as a stream: PGN (chess or xiangqi) or PDN, packed or not.
pub fn is_text(name: &str) -> bool {
    let n = unpacked_name(name).to_ascii_lowercase();
    n.ends_with(".pgn") || n.ends_with(".pdn")
}

pub fn is_zip(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"))
}

/// Whether the file at `path` is read as a stream: a game file `is_text` names, or a zip.
pub fn streams(path: &Path) -> bool {
    is_zip(path) || is_text(&path.to_string_lossy())
}

/// What a pass read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub games: u64,
    /// Bytes of text, unpacked.
    pub unpacked: u64,
    /// The most bytes of one game held at once.
    pub peak: usize,
}

impl Stats {
    fn add(&mut self, o: Stats) {
        self.games += o.games;
        self.unpacked += o.unpacked;
        self.peak = self.peak.max(o.peak);
    }
}

/// Counts what is read through it, and ends the read when `stop` says so: a packed stream can
/// unpack to terabytes with no game in it, and games are the only other place a stop is heard.
struct Count<'s, R> {
    r: R,
    n: u64,
    stop: &'s dyn Fn() -> bool,
}

impl<R: Read> Read for Count<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if (self.stop)() {
            // Not `Interrupted`, which readers above this one take as "try again".
            return Err(io::Error::other("stopped"));
        }
        let n = self.r.read(buf)?;
        self.n += n as u64;
        Ok(n)
    }
}

/// The games of `r`, the bytes of a file named `name`, each handed to `each` (whose games they
/// are, and the game's text) until it says false. `stop` is asked as the bytes come, and a stop
/// ends the read with an error.
pub fn games(name: &str, r: impl Read + Send, stop: &dyn Fn() -> bool, each: &mut dyn FnMut(Format, &[u8]) -> bool) -> io::Result<Stats> {
    let mut buf = BufReader::with_capacity(1 << 16, Count { r: unpack(name, r)?, n: 0, stop });
    let format = if unpacked_name(name).to_ascii_lowercase().ends_with(".pdn") {
        Format::Pdn
    } else if crate::xiangqi::is_xiangqi(buf.fill_buf()?) {
        Format::Xiangqi
    } else {
        Format::Pgn
    };
    let mut stats = Stats::default();
    games::scan(&mut buf, &AtomicBool::new(false), format, false, true, |h| {
        stats.games += 1;
        stats.peak = stats.peak.max(h.text.len());
        each(format, &h.text)
    })?;
    stats.unpacked = buf.get_ref().n;
    Ok(stats)
}

/// The games of the file at `path` (`streams`), each handed to `each` with the zip member it is
/// in (`""` for a file of its own) until it says false. Members of a zip that are not game text
/// are left out. `stop` as for `games`.
pub fn file_games(path: &Path, stop: &dyn Fn() -> bool, each: &mut dyn FnMut(&str, Format, &[u8]) -> bool) -> io::Result<Stats> {
    if !is_zip(path) {
        return games(&path.to_string_lossy(), File::open(path)?, stop, &mut |f, t| each("", f, t));
    }
    let mut zip = zip::ZipArchive::new(File::open(path)?).map_err(io::Error::other)?;
    let mut stats = Stats::default();
    for i in 0..zip.len() {
        let member = zip.by_index(i).map_err(io::Error::other)?;
        let Some(name) = member.enclosed_name().map(|p| p.to_string_lossy().into_owned()) else { continue };
        if !member.is_file() || !is_text(&name) {
            continue;
        }
        let mut go = true;
        stats.add(games(&name, member, stop, &mut |f, t| {
            go = each(&name, f, t);
            go
        })?);
        if !go {
            break;
        }
    }
    Ok(stats)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    /// `n` different short games, each with ratings, a time control and a date that change
    /// with its number.
    pub fn many(n: usize) -> String {
        let firsts = ["e4 e5", "d4 d5", "c4 e5", "Nf3 d5", "e4 c5", "d4 Nf6"];
        let mut s = String::new();
        for i in 0..n {
            let speed = ["60+0", "180+2", "600+0", "1800+20", "-"][i % 5];
            let event = if i % 7 == 0 { "Casual Blitz game" } else { "Rated game" };
            s.push_str(&format!(
                "[Event \"{event}\"]\n[White \"w{}\"]\n[Black \"b{}\"]\n[WhiteElo \"{}\"]\n[BlackElo \"{}\"]\n[TimeControl \"{speed}\"]\n[UTCDate \"20{:02}.01.01\"]\n[Result \"1-0\"]\n\n",
                i % 97,
                i % 89,
                1000 + i * 7 % 2000,
                1100 + i * 13 % 1900,
                10 + i % 15
            ));
            // The knight dance's length and the first moves make each game its own.
            s.push_str(&format!("1. {} 2. Nc3 Nc6 {} 1-0\n\n", firsts[i % 6], "3. Nb1 Nb8 4. Nc3 Nc6 ".repeat(i / 6 % 40)));
        }
        s
    }

    pub fn gz(text: &str) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(vec![], flate2::Compression::fast());
        e.write_all(text.as_bytes()).unwrap();
        e.finish().unwrap()
    }

    pub fn bz2(text: &str) -> Vec<u8> {
        let mut e = bzip2::write::BzEncoder::new(vec![], bzip2::Compression::fast());
        e.write_all(text.as_bytes()).unwrap();
        e.finish().unwrap()
    }

    pub fn zst(text: &str) -> Vec<u8> {
        ruzstd::encoding::compress_to_vec(text.as_bytes(), ruzstd::encoding::CompressionLevel::Fastest)
    }

    pub fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        for (name, bytes) in files {
            w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(bytes).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    /// 20,000 games from each kind of packed download: every game is read once, the text is
    /// unpacked once from start to end, no more than one game is held at a time, and nothing
    /// is written beside the download.
    #[test]
    fn big_packed_downloads_stream_game_by_game() {
        let d = crate::library::tests::Dir::new("stream");
        let text = many(20_000);
        let biggest = text.split("\n\n[Event").map(str::len).max().unwrap();
        let mid = text.len() / 2 + text[text.len() / 2..].find("[Event").unwrap();
        let files = [
            ("month.pgn.gz", gz(&text)),
            ("month.pgn.zst", zst(&text)),
            ("month.pgn.bz2", bz2(&text)),
            ("month.pgn", text.clone().into_bytes()),
            ("issue.zip", zip_of(&[("a.pgn", &text.as_bytes()[..mid]), ("readme.txt", b"x"), ("b.pgn", &text.as_bytes()[mid..])])),
        ];
        for (name, bytes) in files {
            let p = d.0.join(name);
            std::fs::write(&p, &bytes).unwrap();
            let mut seen = 0u64;
            let mut members = std::collections::BTreeSet::new();
            let s = file_games(&p, &|| false, &mut |m, f, t| {
                assert_eq!(f, Format::Pgn);
                assert!(t.starts_with(b"[Event"), "{name}: a game starts at its first tag");
                members.insert(m.to_string());
                seen += 1;
                true
            })
            .unwrap();
            assert_eq!((s.games, seen), (20_000, 20_000), "{name}");
            assert_eq!(s.unpacked, text.len() as u64, "{name}: unpacked once, all of it");
            assert!(s.peak <= biggest + 8 && s.peak < 2000, "{name}: {} bytes held for one game", s.peak);
            if name.ends_with(".zip") {
                assert_eq!(members.into_iter().collect::<Vec<_>>(), ["a.pgn", "b.pgn"]);
            }
            std::fs::remove_file(&p).unwrap();
            assert_eq!(std::fs::read_dir(&d.0).unwrap().count(), 0, "{name}: nothing unpacked to disk");
        }
    }

    #[test]
    fn a_stream_stops_when_asked_and_knows_its_names() {
        let d = crate::library::tests::Dir::new("stream-stop");
        let p = d.0.join("a.pgn.bz2");
        std::fs::write(&p, bz2(&many(50))).unwrap();
        let mut n = 0;
        let s = file_games(&p, &|| false, &mut |_, _, _| {
            n += 1;
            n < 10
        })
        .unwrap();
        assert_eq!((n, s.games), (10, 10));
        // A stream with no game in it (a bomb of blank lines) still hears a stop.
        let blank = d.0.join("blank.pgn.bz2");
        std::fs::write(&blank, bz2(&"\n".repeat(1 << 20))).unwrap();
        let asked = std::cell::Cell::new(0);
        let e = file_games(&blank, &|| {
            asked.set(asked.get() + 1);
            asked.get() > 3
        }, &mut |_, _, _| true)
        .unwrap_err();
        assert_eq!((e.to_string().as_str(), asked.get()), ("stopped", 4));
        assert!(is_text("x.PGN.BZ2") && is_text("a.pdn.gz") && !is_text("a.sgf") && !is_text("a.bz2"));
        assert_eq!(unpacked_name("Lichess.pgn.ZST"), "Lichess.pgn");
        assert!(streams(Path::new("/a/twic1600g.ZIP")) && !streams(Path::new("a.si4")));
    }
}
