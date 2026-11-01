//! Strip rules: a filter on a Source that decides which of its Games are kept, such as "both
//! players 2000+" or "slow time controls only". A rule applies three ways: to Games as they
//! stream in (`Library::import_with`: a stripped Game is never stored), to a Library's Games
//! already stored (`Library::prune`), and to the downloads kept on disk (`rewrite`: the file is
//! written again with only the Games kept, and put in place of the old one at once).
//!
//! A rule's parts all have to agree for a Game to be kept. A part left empty keeps every Game.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::chess::Format;
use crate::library::{NewGame, read_game};
use crate::stream;

/// Whose rating a minimum asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RatingOf {
    /// Both players: a Game with a rating missing is stripped.
    #[default]
    Both,
    /// At least one player.
    Either,
    /// The two players' average: both ratings are needed.
    Average,
}

/// A time control class, as Lichess counts them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speed {
    /// Under 3 minutes, UltraBullet and FICS lightning included.
    Bullet,
    Blitz,
    Rapid,
    /// 25 minutes or more, FICS standard included.
    Classical,
    Correspondence,
}

/// The class of a Game's time control: by its `TimeControl` header (`300+3`: the minutes it
/// is expected to last, base plus 40 increments, as Lichess does; `-` or `1/259200`:
/// correspondence), else by the words of its `Event` (`Rated Blitz game`, `FICS rated
/// lightning game`). `None` when neither says, as in most over-the-board downloads.
pub fn speed(time_control: Option<&str>, event: Option<&str>) -> Option<Speed> {
    let tc = time_control.unwrap_or("").trim();
    if tc == "-" || tc.contains('/') {
        return Some(Speed::Correspondence);
    }
    let (base, inc) = tc.split_once('+').unwrap_or((tc, "0"));
    if let (Ok(base), Ok(inc)) = (base.parse::<u64>(), inc.parse::<u64>()) {
        return Some(match base + 40 * inc {
            0..180 => Speed::Bullet,
            180..480 => Speed::Blitz,
            480..1500 => Speed::Rapid,
            _ => Speed::Classical,
        });
    }
    let e = event?.to_ascii_lowercase();
    [
        ("correspondence", Speed::Correspondence),
        ("bullet", Speed::Bullet),
        ("lightning", Speed::Bullet),
        ("blitz", Speed::Blitz),
        ("rapid", Speed::Rapid),
        ("classical", Speed::Classical),
        ("standard", Speed::Classical),
    ]
    .iter()
    .find(|(w, _)| e.contains(w))
    .map(|&(_, s)| s)
}

/// Every Family's usual Variant, by `bragd_games`' id: what a rule's `Standard` names.
const USUAL: [&str; 5] = ["chess", "xiangqi", "shogi", "draughts", "go-19"];

/// Whether a rule's Variant `v` names a Game's `variant`: alike as `library::chess_variant`
/// writes them, and `Standard` names the usual Variant of every Family.
fn names(v: &str, variant: &str) -> bool {
    let (v, variant) = (crate::library::chess_variant(v), crate::library::chess_variant(variant));
    v == variant || (v == "chess" && USUAL.contains(&variant.as_str()))
}

/// Which Games of a Source are kept. Kept in the Helper's state with the Source's options.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StripRule {
    /// The lowest rating kept, by `rating_of`.
    pub min_rating: Option<u16>,
    pub rating_of: RatingOf,
    /// The time control classes kept. A Game whose class cannot be told is kept (`speed`).
    pub speeds: Vec<Speed>,
    /// Strips Games whose Event says casual or unrated. A Game that does not say is rated.
    pub rated_only: bool,
    /// The Variants kept, by name or id (`Standard` or `chess`, `Chess960`, `crazyhouse`, …),
    /// compared as `library::chess_variant` writes them.
    pub variants: Vec<String>,
    /// The first and last dates kept, as PGN writes them (`2024.01.31`); a shorter date
    /// (`2024`, `2024.06`) means all of it. A Game without a known year is stripped.
    pub date_from: String,
    pub date_to: String,
    /// The fewest half-moves kept.
    pub min_plies: Option<u32>,
    /// The results kept (`1-0`, `0-1`, `1/2-1/2`, `*`).
    pub results: Vec<String>,
    /// Keeps only Games with a titled player (GM, IM, FM, …; Lichess's BOT is no title).
    pub titled_only: bool,
    /// Keeps only Games one of these players played, by name as the Source writes it.
    pub players: Vec<String>,
}

impl StripRule {
    /// Whether the rule keeps every Game.
    pub fn is_empty(&self) -> bool {
        *self == StripRule::default()
    }

    /// Whether `g` is kept.
    pub fn keeps(&self, g: &NewGame) -> bool {
        self.keeps_with(|k| g.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str()), &g.variant, g.moves.len(), &g.result)
    }

    /// Whether a Game is kept, `get` giving its headers by name, with its Variant, its length
    /// in half-moves and the result its moves end with.
    pub fn keeps_with<'a>(&self, get: impl Fn(&str) -> Option<&'a str>, variant: &str, plies: usize, result: &str) -> bool {
        let get = |k: &str| get(k).map(str::trim).filter(|v| !v.is_empty());
        if let Some(min) = self.min_rating {
            let elo = |k: &str| get(k).and_then(|v| v.parse::<u16>().ok());
            let (w, b) = (elo("WhiteElo"), elo("BlackElo"));
            let ok = match self.rating_of {
                RatingOf::Both => w.is_some_and(|w| w >= min) && b.is_some_and(|b| b >= min),
                RatingOf::Either => w.is_some_and(|w| w >= min) || b.is_some_and(|b| b >= min),
                RatingOf::Average => matches!((w, b), (Some(w), Some(b)) if (u32::from(w) + u32::from(b)) / 2 >= u32::from(min)),
            };
            if !ok {
                return false;
            }
        }
        let event = get("Event").map(str::to_ascii_lowercase).unwrap_or_default();
        if !self.speeds.is_empty() && speed(get("TimeControl"), Some(&event)).is_some_and(|s| !self.speeds.contains(&s)) {
            return false;
        }
        if self.rated_only && (event.contains("casual") || event.contains("unrated")) {
            return false;
        }
        if !self.variants.is_empty() && !self.variants.iter().any(|v| names(v, variant)) {
            return false;
        }
        if !self.date_from.trim().is_empty() || !self.date_to.trim().is_empty() {
            let date = get("Date").filter(|d| !d.starts_with('?')).or_else(|| get("UTCDate")).unwrap_or("");
            let (from, to) = (self.date_from.trim(), self.date_to.trim());
            let known = date.len() >= 4 && date.as_bytes()[..4].iter().all(u8::is_ascii_digit);
            if !known || (!from.is_empty() && date < from) || (!to.is_empty() && &date[..date.len().min(to.len())] > to) {
                return false;
            }
        }
        if self.min_plies.is_some_and(|m| plies < m as usize) {
            return false;
        }
        let result = get("Result").unwrap_or(result);
        if !self.results.is_empty() && !self.results.iter().any(|r| r.trim() == result) {
            return false;
        }
        if self.titled_only && !["WhiteTitle", "BlackTitle"].iter().any(|k| get(k).is_some_and(|t| !t.eq_ignore_ascii_case("BOT"))) {
            return false;
        }
        if !self.players.is_empty() && !["White", "Black"].iter().any(|k| get(k).is_some_and(|n| self.players.iter().any(|p| p.trim().eq_ignore_ascii_case(n)))) {
            return false;
        }
        true
    }
}

/// What a rule did, or would do, to one download.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rewritten {
    pub path: PathBuf,
    /// Games kept, and Games stripped.
    pub kept: u64,
    pub stripped: u64,
    pub bytes_before: u64,
    /// After it; in a dry run, an estimate by the share of text kept.
    pub bytes_after: u64,
    pub dry_run: bool,
}

fn stopped() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "stopped")
}

/// Writes the download at `path` (`stream::streams`) again with only the Games one of `rules`
/// keeps (a Source's Routes each have one),
/// packed as it was, and puts it in place of the old one at once: until then the old one is
/// untouched, so a stop or a crash leaves it whole. A zip keeps its other members as they are,
/// and so are its packed members. `dry_run`: nothing is written; the counts and the size it
/// would have are reported. `stop` is asked between Games. A packed file stops past
/// `max_ratio` (`stream::games`).
pub fn rewrite(path: &Path, rules: &[&StripRule], dry_run: bool, max_ratio: u64, stop: &dyn Fn() -> bool) -> io::Result<Rewritten> {
    let mut out = Rewritten { path: path.to_path_buf(), bytes_before: fs::metadata(path)?.len(), dry_run, ..Default::default() };
    let (mut kept_bytes, mut all_bytes, mut halted) = (0u64, 0u64, false);
    let mut keep = |f: Format, t: &[u8]| -> Option<bool> {
        if stop() {
            halted = true;
            return None;
        }
        all_bytes += t.len() as u64;
        let game = read_game(f, t);
        let k = rules.iter().any(|r| r.keeps(&game));
        if k {
            out.kept += 1;
            kept_bytes += t.len() as u64;
        } else {
            out.stripped += 1;
        }
        Some(k)
    };
    if dry_run {
        stream::file_games(path, max_ratio, stop, &mut |_, f, t| keep(f, t).is_some()).map_err(|e| if stop() { stopped() } else { e })?;
        if halted {
            return Err(stopped());
        }
        out.bytes_after = if all_bytes == 0 { out.bytes_before } else { (out.bytes_before as u128 * kept_bytes as u128 / all_bytes as u128) as u64 };
        return Ok(out);
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp = path.with_file_name(format!(".{name}.strip-{}.tmp", std::process::id()));
    let written = if stream::is_zip(path) { rewrite_zip(path, &tmp, max_ratio, stop, &mut keep) } else { rewrite_file(path, &tmp, max_ratio, stop, &mut keep) };
    match written {
        Ok(()) if !halted => fs::rename(&tmp, path)?,
        other => {
            let _ = fs::remove_file(&tmp);
            if !stop() {
                other?;
            }
            return Err(stopped());
        }
    }
    out.bytes_after = fs::metadata(path)?.len();
    Ok(out)
}

/// The games of `r` (a file named `name`) that `keep` keeps, written to `w`. `member`: `r` is
/// a zip member of that many packed bytes, unpacked as it is read.
fn filter(name: &str, r: impl io::Read + Send, member: Option<u64>, max_ratio: u64, stop: &dyn Fn() -> bool, keep: &mut dyn FnMut(Format, &[u8]) -> Option<bool>, w: &mut dyn Write) -> io::Result<()> {
    let mut failed = None;
    let mut each = |f, t: &[u8]| match keep(f, t) {
        Some(true) => w.write_all(t).map_err(|e| failed = Some(e)).is_ok(),
        Some(false) => true,
        None => false,
    };
    match member {
        Some(packed) => stream::member_games(name, r, packed, max_ratio, stop, &mut each)?,
        None => stream::games(name, r, max_ratio, stop, &mut each)?,
    };
    failed.map_or(Ok(()), Err)
}

fn rewrite_file(path: &Path, tmp: &Path, max_ratio: u64, stop: &dyn Fn() -> bool, keep: &mut dyn FnMut(Format, &[u8]) -> Option<bool>) -> io::Result<()> {
    let name = path.to_string_lossy();
    let f = packed(&name, File::create(tmp)?, &mut |w| filter(&name, File::open(path)?, None, max_ratio, stop, keep, w))?;
    f.sync_all()
}

fn rewrite_zip(path: &Path, tmp: &Path, max_ratio: u64, stop: &dyn Fn() -> bool, keep: &mut dyn FnMut(Format, &[u8]) -> Option<bool>) -> io::Result<()> {
    let mut src = zip::ZipArchive::new(File::open(path)?).map_err(io::Error::other)?;
    let mut w = zip::ZipWriter::new(File::create(tmp)?);
    for i in 0..src.len() {
        let m = src.by_index(i).map_err(io::Error::other)?;
        let name = m.enclosed_name().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        if m.is_file() && stream::is_text(&name) && stream::unpacked_name(&name) == name {
            w.start_file(m.name().map_err(io::Error::other)?.into_owned(), zip::write::SimpleFileOptions::default()).map_err(io::Error::other)?;
            let packed = m.compressed_size();
            filter(&name, m, Some(packed), max_ratio, stop, keep, &mut w)?;
        } else {
            w.raw_copy_file(m).map_err(io::Error::other)?;
        }
    }
    w.finish().map_err(io::Error::other)?.sync_all()
}

/// `out`, packed as `name` says (`.gz`, `.zst`, `.bz2`, or not), with what `fill` writes.
fn packed(name: &str, out: File, fill: &mut dyn FnMut(&mut dyn Write) -> io::Result<()>) -> io::Result<File> {
    let n = name.to_ascii_lowercase();
    let done = |w: BufWriter<File>| w.into_inner().map_err(|e| e.into_error());
    if n.ends_with(".gz") {
        let mut e = flate2::write::GzEncoder::new(BufWriter::new(out), flate2::Compression::default());
        fill(&mut e)?;
        done(e.finish()?)
    } else if n.ends_with(".bz2") {
        let mut e = bzip2::write::BzEncoder::new(BufWriter::new(out), bzip2::Compression::default());
        fill(&mut e)?;
        done(e.finish()?)
    } else if n.ends_with(".zst") {
        // The encoder pulls its input, so it reads what `fill` writes through a pipe, on a
        // thread of its own. shortcut: its fastest level only (about zstd -1), all ruzstd has; a rewritten file is larger per Game than Lichess's own.
        let (r, w) = io::pipe()?;
        let packer = std::thread::spawn(move || {
            let mut out = BufWriter::new(out);
            ruzstd::encoding::compress(r, &mut out, ruzstd::encoding::CompressionLevel::Fastest);
            done(out)
        });
        let filled = {
            let mut w = BufWriter::new(w);
            fill(&mut w).and_then(|()| w.flush())
        };
        let out = packer.join().map_err(|_| io::Error::other("the zstd packer failed"))??;
        filled.map(|()| out)
    } else {
        let mut w = BufWriter::new(out);
        fill(&mut w)?;
        done(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::tests::{bz2, gz, many, zip_of, zst};

    fn game(tags: &[(&str, &str)], plies: usize) -> NewGame {
        NewGame {
            family: "chess".into(),
            variant: "chess".into(),
            headers: tags.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            moves: vec!["e4".into(); plies],
            result: "*".into(),
            ..Default::default()
        }
    }

    #[test]
    fn time_controls_are_classed_as_lichess_does() {
        assert_eq!(speed(Some("60+0"), None), Some(Speed::Bullet));
        assert_eq!(speed(Some("15+0"), Some("Rated UltraBullet game")), Some(Speed::Bullet));
        assert_eq!(speed(Some("180+2"), None), Some(Speed::Blitz));
        assert_eq!(speed(Some("600+0"), None), Some(Speed::Rapid));
        assert_eq!(speed(Some("1800+20"), None), Some(Speed::Classical));
        assert_eq!(speed(Some("-"), None), Some(Speed::Correspondence));
        assert_eq!(speed(Some("1/259200"), None), Some(Speed::Correspondence));
        assert_eq!(speed(None, Some("Rated Blitz game")), Some(Speed::Blitz));
        assert_eq!(speed(Some("?"), Some("FICS rated lightning game")), Some(Speed::Bullet));
        assert_eq!(speed(None, Some("FICS unrated standard game")), Some(Speed::Classical));
        assert_eq!(speed(None, Some("Tata Steel Masters")), None);
    }

    #[test]
    fn every_part_of_a_rule_strips_what_it_should() {
        let g = game(
            &[
                ("Event", "Rated Blitz game"),
                ("White", "Carlsen"),
                ("Black", "Nakamura"),
                ("WhiteElo", "2900"),
                ("BlackElo", "1900"),
                ("TimeControl", "180+0"),
                ("UTCDate", "2024.06.15"),
                ("Result", "1-0"),
                ("WhiteTitle", "GM"),
            ],
            40,
        );
        let keeps = |r: StripRule| r.keeps(&g);
        assert!(keeps(StripRule::default()) && StripRule::default().is_empty());
        let rated = |min, of| StripRule { min_rating: Some(min), rating_of: of, ..Default::default() };
        assert!(!keeps(rated(2000, RatingOf::Both)) && keeps(rated(1900, RatingOf::Both)));
        assert!(keeps(rated(2800, RatingOf::Either)) && !keeps(rated(2901, RatingOf::Either)));
        assert!(keeps(rated(2400, RatingOf::Average)) && !keeps(rated(2401, RatingOf::Average)));
        assert!(!rated(1000, RatingOf::Either).keeps(&game(&[], 1)), "no rating, no Game");
        let speeds = |s: Vec<Speed>| StripRule { speeds: s, ..Default::default() };
        assert!(keeps(speeds(vec![Speed::Blitz, Speed::Rapid])) && !keeps(speeds(vec![Speed::Classical])));
        assert!(speeds(vec![Speed::Classical]).keeps(&game(&[("Event", "Olympiad")], 1)), "a class not told is kept");
        let rated_only = StripRule { rated_only: true, ..Default::default() };
        assert!(keeps(rated_only.clone()) && !rated_only.keeps(&game(&[("Event", "Casual Blitz game")], 1)));
        assert!(!rated_only.keeps(&game(&[("Event", "FICS unrated blitz game")], 1)) && rated_only.keeps(&game(&[], 1)));
        let variants = |v: &[&str]| StripRule { variants: v.iter().map(|s| s.to_string()).collect(), ..Default::default() };
        assert!(keeps(variants(&["Standard"])) && keeps(variants(&["chess"])) && !keeps(variants(&["Chess960"])));
        let fischer = NewGame { variant: crate::library::chess_variant("Chess960"), ..game(&[], 1) };
        assert!(variants(&["chess 960"]).keeps(&fischer) && !variants(&["Standard"]).keeps(&fischer));
        let go = |v: &str| NewGame { family: "go".into(), variant: v.into(), ..game(&[], 1) };
        assert!(variants(&["Standard"]).keeps(&go("go-19")) && !variants(&["Standard"]).keeps(&go("go-9")) && variants(&["go-9"]).keeps(&go("go-9")));
        let dates = |f: &str, t: &str| StripRule { date_from: f.into(), date_to: t.into(), ..Default::default() };
        assert!(keeps(dates("2024", "2024")) && keeps(dates("2024.06.15", "2024.06")) && !keeps(dates("2024.07", "")) && !keeps(dates("", "2024.05")));
        assert!(!dates("2000", "").keeps(&game(&[("Date", "????.??.??")], 1)), "no year, no Game");
        assert!(dates("2000", "").keeps(&game(&[("Date", "????.??.??"), ("UTCDate", "2020.01.01")], 1)));
        let plies = StripRule { min_plies: Some(40), ..Default::default() };
        assert!(keeps(plies.clone()) && !plies.keeps(&game(&[], 39)));
        let results = |r: &[&str]| StripRule { results: r.iter().map(|s| s.to_string()).collect(), ..Default::default() };
        assert!(keeps(results(&["1-0", "0-1"])) && !keeps(results(&["1/2-1/2"])));
        let titled = StripRule { titled_only: true, ..Default::default() };
        assert!(keeps(titled.clone()) && !titled.keeps(&game(&[("WhiteTitle", "BOT")], 1)));
        let players = |p: &[&str]| StripRule { players: p.iter().map(|s| s.to_string()).collect(), ..Default::default() };
        assert!(keeps(players(&["nakamura "])) && !keeps(players(&["Caruana"])));
        // A rule round-trips through JSON, its parts missing taking their defaults.
        let r: StripRule = serde_json::from_str(r#"{"min_rating":2000,"speeds":["blitz"]}"#).unwrap();
        assert_eq!(r, StripRule { min_rating: Some(2000), speeds: vec![Speed::Blitz], ..Default::default() });
    }

    /// The rule of the tests below: both 2000+, and no casual Games.
    fn strong() -> StripRule {
        StripRule { min_rating: Some(2000), rated_only: true, ..Default::default() }
    }

    fn count(path: &Path) -> (u64, u64) {
        let (mut all, mut kept) = (0, 0);
        crate::stream::file_games(path, 50, &|| false, &mut |_, f, t| {
            all += 1;
            kept += u64::from(strong().keeps(&read_game(f, t)));
            true
        })
        .unwrap();
        (all, kept)
    }

    #[test]
    fn downloads_are_written_again_with_only_the_games_kept() {
        let d = crate::library::tests::Dir::new("rewrite");
        let text = many(600);
        let pgn = d.0.join("x.pgn");
        std::fs::write(&pgn, &text).unwrap();
        let (all, want) = count(&pgn);
        assert!(want > 0 && want < all, "{want} of {all}");
        for (name, bytes) in [("a.pgn", text.clone().into_bytes()), ("a.pgn.gz", gz(&text)), ("a.pgn.zst", zst(&text)), ("a.pgn.bz2", bz2(&text)), ("a.zip", zip_of(&[("a.pgn", text.as_bytes()), ("readme.txt", b"kept")]))] {
            let p = d.0.join(name);
            std::fs::write(&p, &bytes).unwrap();
            let dry = rewrite(&p, &[&strong()], true, 50, &|| false).unwrap();
            assert_eq!((dry.kept, dry.stripped), (want, all - want), "{name}");
            assert!(dry.bytes_after < dry.bytes_before, "{name}");
            assert_eq!(std::fs::read(&p).unwrap(), bytes, "{name}: a dry run changes nothing");
            let done = rewrite(&p, &[&strong()], false, 50, &|| false).unwrap();
            assert_eq!((done.kept, done.stripped), (want, all - want), "{name}");
            assert_eq!(done.bytes_after, std::fs::metadata(&p).unwrap().len());
            assert_eq!(count(&p), (want, want), "{name}: only the Games kept are left, still read as before");
            if name.ends_with(".zip") {
                let mut z = zip::ZipArchive::new(File::open(&p).unwrap()).unwrap();
                let mut s = String::new();
                io::Read::read_to_string(&mut z.by_name("readme.txt").unwrap(), &mut s).unwrap();
                assert_eq!(s, "kept");
            }
            std::fs::remove_file(&p).unwrap();
        }
        // Stopped half-way: the download is as it was, and nothing is left beside it.
        let p = d.0.join("b.pgn.gz");
        std::fs::write(&p, gz(&text)).unwrap();
        let asked = std::cell::Cell::new(0);
        let err = rewrite(&p, &[&strong()], false, 50, &|| {
            asked.set(asked.get() + 1);
            asked.get() > 100
        })
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Interrupted);
        assert_eq!(std::fs::read(&p).unwrap(), gz(&text));
        let names: Vec<_> = std::fs::read_dir(&d.0).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names.len(), 2, "{names:?}");
    }
}
