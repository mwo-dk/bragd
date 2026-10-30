//! Sources: where Games come from, each fetched again on its schedule. Built in: TWIC, a
//! custom URL, and the big Sources this version lists but does not fetch yet. Every Source is
//! opt-in: none is enabled until the user turns it on. A Source's downloads are kept as they
//! came, packed, and imported from there as a stream (`bragd_library::stream`), so a zip or a
//! `.pgn.zst` is never unpacked to disk.

use std::io;
use std::path::{Path, PathBuf};

use bragd_library::strip::StripRule;
use serde::{Deserialize, Serialize};

use crate::net::{self, Fetcher};

/// How often a Source is fetched again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Schedule {
    Daily,
    Weekly,
    Monthly,
    Off,
}

impl Schedule {
    /// Seconds between fetches; `None` when off.
    pub fn every(self) -> Option<u64> {
        const DAY: u64 = 86_400;
        match self {
            Schedule::Daily => Some(DAY),
            Schedule::Weekly => Some(7 * DAY),
            // shortcut: a month is 30 days; take calendar months when a Source publishes on a date.
            Schedule::Monthly => Some(30 * DAY),
            Schedule::Off => None,
        }
    }
}

/// What the user chose for a Source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Options {
    pub enabled: bool,
    pub schedule: Schedule,
    /// Fetch on schedule only on a connection that is not metered. Where the system cannot tell
    /// (`net::metered` is `None`), the connection counts as unmetered.
    #[serde(default)]
    pub only_unmetered: bool,
    /// The most bytes its downloads may keep on disk; older ones go first.
    #[serde(default)]
    pub disk_cap: Option<u64>,
    /// A custom URL Source's address.
    #[serde(default)]
    pub url: String,
    /// Which of its Games are kept: applied as they come in, and to what is stored already
    /// when asked (`jobs::Work::Strip`). None keeps every Game.
    #[serde(default)]
    pub strip: Option<StripRule>,
    /// What becomes of a download once its Games are in the Library.
    #[serde(default)]
    pub after_import: AfterImport,
    /// What Disk watch may do by itself when a limit this Source counts against is crossed.
    #[serde(default)]
    pub auto: Auto,
}

/// What becomes of a download once its Games are in the Library.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AfterImport {
    /// Kept as it came, within the disk cap.
    #[default]
    Keep,
    /// Written again with only the Games the Strip rule keeps.
    Strip,
    /// Deleted.
    Delete,
}

/// What Disk watch may do by itself for a Source, each only when the user turns it on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Auto {
    /// Apply its Strip rule to its downloads and to the Library (at most once a day).
    pub strip: bool,
    /// Delete its oldest downloads beyond its limit.
    pub delete_oldest: bool,
    /// Stop fetching it until the disk is within the limits again.
    pub pause: bool,
}

/// What the Helper remembers of a Source between fetches.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Memo {
    /// Unix time of the last fetch that worked, and of the last try.
    pub last_fetch: Option<u64>,
    pub last_try: Option<u64>,
    /// TWIC: the last issue fetched.
    pub last_issue: Option<u32>,
    pub last_error: Option<String>,
    /// Disk watch holds its fetches back (`Auto::pause`) while a limit is crossed.
    pub paused: bool,
    /// Unix time Disk watch last queued its Strip rule by itself (`Auto::strip`).
    pub last_auto_strip: Option<u64>,
}

/// A Source as Settings lists it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct About {
    pub id: String,
    pub name: String,
    /// The Game family its Games belong to (`bragd-games`' ids).
    pub family: String,
    /// About how many bytes one fetch downloads: shown before the first one.
    pub size: u64,
    pub schedule: Schedule,
    /// Where it comes from, `{issue}`, `{yyyy}` and `{mm}` standing for what changes.
    pub url: String,
    /// False for the Sources this version lists but cannot fetch yet.
    pub fetchable: bool,
    /// The text key of the notice shown for it, if any.
    pub notice: Option<String>,
}

impl About {
    /// What a Source starts as: off, on its usual schedule.
    pub fn defaults(&self) -> Options {
        Options { enabled: false, schedule: self.schedule, only_unmetered: false, disk_cap: None, url: String::new(), strip: None, after_import: AfterImport::Keep, auto: Auto::default() }
    }
}

/// What a fetch works with: the network, its own folder for downloads, what was remembered
/// (changed by the fetch, kept only once its files are imported) and the user's options.
pub struct Cx<'a> {
    pub net: &'a dyn Fetcher,
    pub dir: &'a Path,
    pub memo: &'a mut Memo,
    pub options: &'a Options,
    pub stop: &'a dyn Fn() -> bool,
    pub progress: &'a mut dyn FnMut(u64),
}

/// A Source the Helper can list and fetch.
pub trait Source: Send + Sync {
    fn about(&self) -> About;
    /// Downloads what is new into `cx.dir`: the files to import, as they came (a zip, a packed
    /// PGN file).
    fn fetch(&self, cx: &mut Cx) -> io::Result<Vec<PathBuf>>;
}

/// The text key of the notice for a Source this version does not fetch.
pub const OUT_OF_SCOPE: &str = "notice.source_out_of_scope";

/// The Week in Chess: a zip of PGN every week, numbered.
pub struct Twic;

pub const TWIC_URL: &str = "https://theweekinchess.com/zips/twic{issue}g.zip";
/// An issue known to be there (July 2023), where the first probe starts.
pub const TWIC_SEED: u32 = 1500;
/// The most issues one fetch catches up on (half a year).
const TWIC_CATCH_UP: u32 = 26;

fn twic_url(issue: u32) -> String {
    TWIC_URL.replace("{issue}", &issue.to_string())
}

/// The last issue there, from `from` (which is there) upwards, issues being numbered without
/// gaps: steps that double until one is missing, then halving between the last there and it.
pub fn latest_issue(exists: &dyn Fn(u32) -> io::Result<bool>, from: u32) -> io::Result<u32> {
    let (mut there, mut step) = (from, 1);
    while exists(there + step)? {
        there += step;
        step *= 2;
    }
    let mut missing = there + step;
    while missing - there > 1 {
        let mid = there + (missing - there) / 2;
        if exists(mid)? { there = mid } else { missing = mid }
    }
    Ok(there)
}

impl Source for Twic {
    fn about(&self) -> About {
        About {
            id: "twic".into(),
            name: "The Week in Chess".into(),
            family: "chess".into(),
            size: 1_500_000,
            schedule: Schedule::Weekly,
            url: TWIC_URL.into(),
            fetchable: true,
            notice: None,
        }
    }

    fn fetch(&self, cx: &mut Cx) -> io::Result<Vec<PathBuf>> {
        let exists = |n: u32| cx.net.exists(&twic_url(n));
        // The first time, only the newest issue: the archive back to 1994 is a Source of its own.
        let issues: Vec<u32> = match cx.memo.last_issue {
            None => {
                if !exists(TWIC_SEED)? {
                    return Err(io::Error::other(format!("{} is not there", twic_url(TWIC_SEED))));
                }
                vec![latest_issue(&exists, TWIC_SEED)?]
            }
            Some(last) => {
                let mut new = vec![];
                while new.len() < TWIC_CATCH_UP as usize && exists(last + 1 + new.len() as u32)? {
                    new.push(last + 1 + new.len() as u32);
                }
                new
            }
        };
        let mut files = vec![];
        for n in issues {
            let zip = cx.dir.join(format!("twic{n}g.zip"));
            if !zip.exists() && !net::download(cx.net, &twic_url(n), &zip, cx.options.disk_cap, cx.stop, cx.progress)? {
                return Err(io::Error::other(format!("{} is not there", twic_url(n))));
            }
            files.push(zip);
            cx.memo.last_issue = Some(n);
        }
        Ok(files)
    }
}

/// A PGN, SGF, KIF, CSA or PDN file (or a zip of them) at an address the user gives.
pub struct CustomUrl {
    pub id: String,
    pub url: String,
}

/// Custom URL Sources have ids that start with this.
pub const CUSTOM: &str = "custom:";

impl Source for CustomUrl {
    fn about(&self) -> About {
        About {
            id: self.id.clone(),
            name: self.id.strip_prefix(CUSTOM).unwrap_or(&self.id).to_string(),
            family: String::new(),
            size: 0,
            schedule: Schedule::Weekly,
            url: self.url.clone(),
            fetchable: true,
            notice: None,
        }
    }

    fn fetch(&self, cx: &mut Cx) -> io::Result<Vec<PathBuf>> {
        let name = self.url.split(['?', '#']).next().unwrap_or("").rsplit('/').next().unwrap_or("");
        let name: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_')).collect();
        let zip = name.to_ascii_lowercase().ends_with(".zip");
        if !(self.url.starts_with("https://") || self.url.starts_with("http://")) || name.starts_with('.') || !(zip || net::importable(&name)) {
            return Err(io::Error::other(format!("{}: not a PGN, SGF, KIF, CSA or PDN file, or a zip of them", self.url)));
        }
        let to = cx.dir.join(&name);
        // Fetched again each time: the file at the address may have grown.
        let _ = std::fs::remove_file(&to);
        if !net::download(cx.net, &self.url, &to, cx.options.disk_cap, cx.stop, cx.progress)? {
            return Err(io::Error::other(format!("{} is not there", self.url)));
        }
        Ok(vec![to])
    }
}

/// A Source this version lists, so the user knows it is coming, but does not fetch.
pub struct Later(pub About);

impl Source for Later {
    fn about(&self) -> About {
        self.0.clone()
    }

    fn fetch(&self, _: &mut Cx) -> io::Result<Vec<PathBuf>> {
        Err(io::Error::other(bragd_core::t!(OUT_OF_SCOPE, "source" => self.0.name)))
    }
}

fn later(id: &str, name: &str, family: &str, size: u64, schedule: Schedule, url: &str) -> Box<dyn Source> {
    Box::new(Later(About {
        id: id.into(),
        name: name.into(),
        family: family.into(),
        size,
        schedule,
        url: url.into(),
        fetchable: false,
        notice: Some(OUT_OF_SCOPE.into()),
    }))
}

/// Every built-in Source, in the order Settings lists them. Sizes are rough, per fetch.
pub fn builtin() -> Vec<Box<dyn Source>> {
    vec![
        Box::new(Twic),
        // When it is fetched, the month's `.pgn.zst` is kept packed and imported from there as a
        // stream (`Library::import_with`, through `bragd_library::stream`), game by game, with the
        // Source's Strip rule applied as the Games stream by: what it strips is never stored, and
        // nothing is unpacked to disk. `AfterImport::Strip` then shrinks the download itself.
        later("lichess-db", "Lichess database", "chess", 30_000_000_000, Schedule::Monthly, "https://database.lichess.org/standard/lichess_db_standard_rated_{yyyy}-{mm}.pgn.zst"),
        // The same goes for FICS: a month's or a year's `.pgn.bz2`, chosen on its download page.
        later("fics", "FICS games database", "chess", 300_000_000, Schedule::Monthly, "https://www.ficsgames.org/download.html"),
        later("pgnmentor", "PGN Mentor", "chess", 200_000_000, Schedule::Monthly, "https://www.pgnmentor.com/files.html"),
        later("lumbras", "LumbrasGigaBase", "chess", 4_000_000_000, Schedule::Monthly, "https://lumbrasgigabase.com/en/download-in-pgn-format-en/"),
        later("ogs", "OGS archive", "go", 1_000_000_000, Schedule::Monthly, "https://online-go.com/"),
        later("kgs", "KGS archive", "go", 100_000_000, Schedule::Monthly, "https://u-go.net/gamerecords/"),
        later("pychess", "pychess exports", "chess", 500_000_000, Schedule::Monthly, "https://www.pychess.org/"),
        later("lishogi", "Lishogi exports", "shogi", 500_000_000, Schedule::Monthly, "https://lishogi.org/"),
    ]
}

/// The Source with `id`: a built-in one, or a custom URL one at `url`.
pub fn find(id: &str, url: &str) -> Option<Box<dyn Source>> {
    if id.starts_with(CUSTOM) {
        return Some(Box::new(CustomUrl { id: id.into(), url: url.into() }));
    }
    builtin().into_iter().find(|s| s.about().id == id)
}

/// Whether a Source is due at `now` (Unix seconds): on, scheduled, its interval gone by since
/// the last fetch that worked, and an hour since the last try, so a failing one is not asked
/// again every minute.
pub fn due(o: &Options, m: &Memo, now: u64) -> bool {
    const RETRY: u64 = 3600;
    let Some(every) = o.schedule.every() else { return false };
    o.enabled && m.last_fetch.is_none_or(|t| now >= t + every) && m.last_try.is_none_or(|t| now >= t + RETRY)
}

/// Whether a scheduled fetch may go now, `metered` being what the system says of the connection.
pub fn may_fetch(o: &Options, metered: Option<bool>) -> bool {
    !(o.only_unmetered && metered == Some(true))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::net::tests::Recorded;
    use std::io::Write;

    /// A zip holding `files` (name, text), deflated.
    pub fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        for (name, text) in files {
            w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(text.as_bytes()).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn twic_finds_the_latest_issue_by_probing_upwards() {
        let asked = std::cell::RefCell::new(vec![]);
        let exists = |n: u32| {
            asked.borrow_mut().push(n);
            Ok(n <= 1663)
        };
        assert_eq!(latest_issue(&exists, TWIC_SEED).unwrap(), 1663);
        // About twice the log of the distance, not one request per issue.
        assert!(asked.borrow().len() < 20, "{:?}", asked.borrow());
        assert_eq!(latest_issue(&|n| Ok(n <= 1500), 1500).unwrap(), 1500);
        assert_eq!(latest_issue(&|n| Ok(n <= 1501), 1500).unwrap(), 1501);
    }

    fn fixture(issues: std::ops::RangeInclusive<u32>) -> Recorded {
        let pgn = crate::jobs::tests::PGN;
        Recorded {
            files: issues.map(|n| (twic_url(n), zip_of(&[(&format!("twic{n}.pgn"), pgn), ("readme.txt", "x")]))).collect(),
            ..Default::default()
        }
    }

    fn fetch(net: &Recorded, dir: &Path, memo: &mut Memo) -> Vec<PathBuf> {
        let o = Twic.about().defaults();
        Twic.fetch(&mut Cx { net, dir, memo, options: &o, stop: &|| false, progress: &mut |_| {} }).unwrap()
    }

    #[test]
    fn twic_fetches_the_newest_first_then_each_new_one() {
        let d = crate::tests::Dir::new("twic");
        let net = fixture(1500..=1663);
        let mut memo = Memo::default();
        let files = fetch(&net, &d.0, &mut memo);
        assert_eq!(files, [d.0.join("twic1663g.zip")], "the newest issue, as it came");
        assert_eq!(memo.last_issue, Some(1663));
        let gets = net.asked.lock().unwrap().iter().filter(|a| a.starts_with("GET")).count();
        assert_eq!(gets, 1, "one download");
        // Two issues later: both, in order, and the probe for the next one finds none.
        let net = fixture(1500..=1665);
        let files = fetch(&net, &d.0, &mut memo);
        assert_eq!(files, [d.0.join("twic1664g.zip"), d.0.join("twic1665g.zip")]);
        assert_eq!(memo.last_issue, Some(1665));
        assert!(net.asked.lock().unwrap().contains(&format!("HEAD {}", twic_url(1666))));
        // Nothing new: nothing fetched.
        let before = memo.clone();
        fetch(&net, &d.0, &mut memo);
        assert_eq!(memo, before);
    }

    #[test]
    fn the_scheduler_is_due_by_the_clock_it_is_given() {
        let day = 86_400;
        let mut o = Twic.about().defaults();
        let mut m = Memo::default();
        assert!(!due(&o, &m, 0), "off until the user turns it on");
        o.enabled = true;
        assert!(due(&o, &m, 0), "never fetched");
        m.last_fetch = Some(1000 * day);
        m.last_try = m.last_fetch;
        assert!(!due(&o, &m, 1006 * day));
        assert!(due(&o, &m, 1007 * day));
        o.schedule = Schedule::Daily;
        assert!(due(&o, &m, 1001 * day));
        o.schedule = Schedule::Monthly;
        assert!(!due(&o, &m, 1029 * day) && due(&o, &m, 1030 * day));
        o.schedule = Schedule::Off;
        assert!(!due(&o, &m, 9999 * day));
        // A failed try waits an hour, whatever the schedule.
        o.schedule = Schedule::Daily;
        m.last_try = Some(1030 * day);
        assert!(!due(&o, &m, 1030 * day + 3599) && due(&o, &m, 1030 * day + 3600));
        // Metered connections wait only when the user asked.
        assert!(may_fetch(&o, Some(true)));
        o.only_unmetered = true;
        assert!(!may_fetch(&o, Some(true)) && may_fetch(&o, Some(false)) && may_fetch(&o, None));
    }

    #[test]
    fn the_registry_lists_every_source_and_fetches_only_some() {
        let ids: Vec<String> = builtin().iter().map(|s| s.about().id).collect();
        assert_eq!(ids, ["twic", "lichess-db", "fics", "pgnmentor", "lumbras", "ogs", "kgs", "pychess", "lishogi"]);
        for s in builtin().iter().skip(1) {
            let a = s.about();
            assert!(!a.fetchable && a.notice.as_deref() == Some(OUT_OF_SCOPE) && a.size > 0, "{}", a.id);
        }
        let d = crate::tests::Dir::new("later");
        let net = Recorded::default();
        let o = Twic.about().defaults();
        let err = find("ogs", "").unwrap().fetch(&mut Cx { net: &net, dir: &d.0, memo: &mut Memo::default(), options: &o, stop: &|| false, progress: &mut |_| {} }).unwrap_err();
        assert!(err.to_string().contains("OGS archive"), "{err}");
        assert!(net.asked.lock().unwrap().is_empty(), "nothing is asked of the network");
        assert!(find("custom:club", "https://example.org/a.pgn").unwrap().about().fetchable);
        assert!(find("nope", "").is_none());
    }

    #[test]
    fn a_custom_url_must_name_a_game_file() {
        let d = crate::tests::Dir::new("custom-bad");
        let net = Recorded::default();
        let o = Twic.about().defaults();
        for url in ["https://example.org/page.html", "file:///etc/passwd.pgn", "https://example.org/"] {
            let s = CustomUrl { id: "custom:x".into(), url: url.into() };
            assert!(s.fetch(&mut Cx { net: &net, dir: &d.0, memo: &mut Memo::default(), options: &o, stop: &|| false, progress: &mut |_| {} }).is_err(), "{url}");
        }
        assert!(net.asked.lock().unwrap().is_empty());
    }
}
