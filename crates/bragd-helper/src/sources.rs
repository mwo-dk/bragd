//! Sources: where Games come from, each fetched again on its schedule. Built in: TWIC, the
//! Lichess database, FICS, PGN Mentor, KGS (each in a file of its own below `sources/`), a custom
//! URL, and the big Sources this version lists but does not fetch yet. Every Source is
//! opt-in: none is enabled until the user turns it on. A Source's downloads are kept as they
//! came, packed, and imported from there as a stream (`bragd_library::stream`), so a zip or a
//! `.pgn.zst` is never unpacked to disk.

use std::io;
use std::path::{Path, PathBuf};

use bragd_library::strip::StripRule;
use serde::{Deserialize, Serialize};

use crate::net::{self, Fetcher};

pub(crate) mod fics;
mod kgs;
mod ogs;
mod lichess;
mod pgnmentor;

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
    /// The most bytes one download may take; none: `MAX_DOWNLOAD`. A larger one waits for
    /// the user's yes (`Memo::too_big`).
    #[serde(default)]
    pub max_download: Option<u64>,
    /// A custom URL Source's address.
    #[serde(default)]
    pub url: String,
    /// Where its Games go: each Route a Strip rule and a Library. A download is read once, and
    /// each Game is stored in every Library whose Route keeps it, and nowhere when none does.
    #[serde(default = "Route::only_main")]
    pub routes: Vec<Route>,
    /// What becomes of a download once its Games are in the Library.
    #[serde(default)]
    pub after_import: AfterImport,
    /// What Disk watch may do by itself when a limit this Source counts against is crossed.
    #[serde(default)]
    pub auto: Auto,
    /// The parts it fetches (`About::parts`, by id); none: those it fetches by default.
    #[serde(default)]
    pub parts: Vec<String>,
    /// A Source of monthly files (`About::months`): the first month to fetch, `yyyy-mm`. Empty:
    /// only the newest the first time.
    #[serde(default)]
    pub since: String,
}

/// The Library a Source's Games go to unless the user says otherwise: `main.bragd`.
pub const MAIN: &str = "main";

/// A Strip rule paired with the Library its Games go to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Route {
    /// Which Games it keeps: applied as they come in, and to what it stored already when asked
    /// (`jobs::Work::Strip`). None keeps every Game.
    #[serde(default)]
    pub strip: Option<StripRule>,
    /// The Library, by name: `main` is `main.bragd` in the Library folder.
    pub library: String,
}

impl Route {
    /// Every Game, into the main Library: what a Source starts with.
    pub fn main() -> Route {
        Route { strip: None, library: MAIN.into() }
    }

    pub fn only_main() -> Vec<Route> {
        vec![Route::main()]
    }

    /// Its rule, when it strips anything.
    pub fn rule(&self) -> Option<&StripRule> {
        self.strip.as_ref().filter(|r| !r.is_empty())
    }
}

/// Whether `name` can name a Library file: letters, digits, `-`, `_` and spaces, not empty.
pub fn library_name(name: &str) -> bool {
    let n = name.trim();
    !n.is_empty() && n == name && n.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '))
}

/// What becomes of a download once its Games are in the Library.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AfterImport {
    /// Kept as it came, within the disk cap.
    #[default]
    Keep,
    /// Written again with only the Games some Route keeps.
    Strip,
    /// Deleted.
    Delete,
}

/// What Disk watch may do by itself for a Source, each only when the user turns it on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Auto {
    /// Apply its Routes' Strip rules to its downloads and their Libraries (at most once a day).
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
    /// TWIC: the last issue fetched. A Source of monthly files: the last month, as `month` has it.
    pub last_issue: Option<u32>,
    /// A Source of many files that seldom change (PGN Mentor): the names fetched already.
    pub got: Vec<String>,
    pub last_error: Option<String>,
    /// Disk watch holds its fetches back (`Auto::pause`) while a limit is crossed.
    pub paused: bool,
    /// Unix time Disk watch last queued its Strip rule by itself (`Auto::strip`).
    pub last_auto_strip: Option<u64>,
    /// The size the server gave for a download over the cap: it waits for the user's yes.
    pub too_big: Option<u64>,
    /// The user's yes: the next download may be this big, once.
    pub allowed: Option<u64>,
}

/// The most bytes one download takes unless its Source says otherwise: 2 GB.
pub const MAX_DOWNLOAD: u64 = 2_000_000_000;

/// The most bytes the next download of a Source may take: what the user allowed once, else
/// its own cap or `MAX_DOWNLOAD`, and never more than its disk cap.
pub fn cap(o: &Options, m: &Memo) -> u64 {
    m.allowed.unwrap_or_else(|| o.max_download.unwrap_or(MAX_DOWNLOAD).min(o.disk_cap.unwrap_or(u64::MAX)))
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
    /// An address a user can open: the site's page, or the file for a Source that is one file.
    pub url: String,
    /// False for the Sources this version lists but cannot fetch yet.
    pub fetchable: bool,
    /// Games for a Library, or a rating list.
    #[serde(default)]
    pub kind: Kind,
    /// What the user can pick of it: Variants, game types, sections. None: it is one thing.
    #[serde(default)]
    pub parts: Vec<Part>,
    /// Whether it is a file a month, so the user can say from which month (`Options::since`).
    #[serde(default)]
    pub months: bool,
    /// `size` is the first fetch, which takes the whole archive; later fetches take only what is
    /// new. False: every fetch is about `size`.
    #[serde(default)]
    pub first_only: bool,
    /// The online server (`bragd_online::server`'s id) whose signed-in account it fetches the
    /// user's own Games through; such a Source also fetches one Game by its address (`Source::one`).
    /// Empty: none.
    #[serde(default)]
    pub account: String,
}

/// A part of a Source the user can pick: a Lichess Variant, a FICS game type, a PGN Mentor
/// section.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub id: String,
    /// Its name's text key.
    pub label: String,
    /// The Variant its Games are (`bragd_games`' key), so it hides with that Variant; empty
    /// for any.
    #[serde(default)]
    pub variant: String,
    /// Fetched while the user has picked none.
    pub default: bool,
}

impl Part {
    pub fn new(id: &str, label: &str, variant: &str, default: bool) -> Part {
        Part { id: id.into(), label: label.into(), variant: variant.into(), default }
    }
}

impl About {
    /// The parts `o` fetches: those picked, or the default ones when none is.
    pub fn chosen<'a>(&'a self, o: &Options) -> Vec<&'a Part> {
        let picked: Vec<&Part> = self.parts.iter().filter(|p| o.parts.contains(&p.id)).collect();
        if picked.is_empty() { self.parts.iter().filter(|p| p.default).collect() } else { picked }
    }
}

/// A month as a number that counts on by one each month: `yyyy * 12 + mm - 1`, from `yyyy-mm`.
pub fn month(s: &str) -> Option<u32> {
    let (y, m) = s.trim().split_once('-')?;
    let (y, m): (u32, u32) = (y.parse().ok()?, m.parse().ok()?);
    (y.to_string().len() == 4 && (1..=12).contains(&m)).then_some(y * 12 + m - 1)
}

/// `month`'s number as `yyyy-mm`.
pub fn month_text(m: u32) -> String {
    format!("{:04}-{:02}", m / 12, m % 12 + 1)
}

/// The month `now` (Unix seconds) is in, UTC.
pub fn month_of(now: u64) -> u32 {
    month(&crate::utc(now)[..7]).unwrap_or(0)
}

/// The months a monthly Source fetches next, out of `there` (the months the server has), in
/// order and at most `most`: those after the last one fetched; the first time those from
/// `Options::since`, or else only the newest.
pub fn months_to_fetch(there: &[u32], memo: &Memo, o: &Options, most: usize) -> Vec<u32> {
    let mut there: Vec<u32> = there.to_vec();
    there.sort_unstable();
    there.dedup();
    let from = match (memo.last_issue, month(&o.since)) {
        (Some(last), _) => last + 1,
        (None, Some(since)) => since,
        (None, None) => return there.last().copied().into_iter().collect(),
    };
    there.into_iter().filter(|&m| m >= from).take(most).collect()
}

/// The text of the page at `url`, at most `MAX_PAGE` bytes; none when the server has no such page.
pub fn page(net: &dyn Fetcher, url: &str) -> io::Result<Option<String>> {
    use std::io::Read;
    const MAX_PAGE: u64 = 8 << 20;
    match net.get(url, 0)? {
        net::Got::Missing => Ok(None),
        net::Got::Body { body, .. } => {
            let mut b = vec![];
            body.take(MAX_PAGE).read_to_end(&mut b)?;
            Ok(Some(String::from_utf8_lossy(&b).into_owned()))
        }
    }
}

/// What a Source's downloads hold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Games, imported into the Libraries its Routes name.
    #[default]
    Games,
    /// Players and their ratings, imported into the rating lists (`bragd_clubs::lists`).
    RatingList,
}

impl About {
    /// What a Source starts as: off, on its usual schedule.
    pub fn defaults(&self) -> Options {
        Options { enabled: false, schedule: self.schedule, only_unmetered: false, disk_cap: None, max_download: None, url: String::new(), routes: Route::only_main(), after_import: AfterImport::Keep, auto: Auto::default(), parts: vec![], since: String::new() }
    }
}

/// What a fetch works with: the network, its own folder for downloads, what was remembered
/// (changed by the fetch, kept only once its files are imported), the user's options and the
/// most bytes one download may take (`cap`).
pub struct Cx<'a> {
    pub net: &'a dyn Fetcher,
    pub dir: &'a Path,
    pub cap: u64,
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
    /// Downloads the one Game `game` names (its address on the site, or its id) into `cx.dir`,
    /// leaving `cx.memo` as it was. Only a Source with an `About::account` has it.
    fn one(&self, _: &mut Cx, game: &str) -> io::Result<PathBuf> {
        Err(io::Error::other(format!("{game}: this Source fetches no single Game")))
    }
}

/// A site's API as the account Bragd signed in to it with, as a `Fetcher`, so `net::download`
/// and `page` read it. It keeps the site's pace: `pace` seconds before every request but the
/// first. A 429 waits as long as the site asks and goes again, twice at most; a 401 or 403 says
/// to sign in again.
pub struct Account<'a> {
    net: &'a dyn Fetcher,
    server: &'static bragd_online::server::Server,
    token: String,
    pace: u64,
    /// What it asks the site's API for (`Accept`), when not empty.
    pub accept: &'static str,
    asked: std::sync::atomic::AtomicBool,
}

impl<'a> Account<'a> {
    /// The account kept for `server` (its token in the OS keychain); none: an error that says
    /// where to sign in.
    pub fn signed_in(net: &'a dyn Fetcher, server: &'static bragd_online::server::Server, pace: u64) -> io::Result<Self> {
        let token = server.token(&server.base()).filter(|t| !t.is_empty()).ok_or_else(|| io::Error::other(bragd_core::t!("source.sign_in", "server" => server.name)))?;
        Ok(Account { net, server, token, pace, accept: "", asked: Default::default() })
    }
}

impl Fetcher for Account<'_> {
    fn get(&self, url: &str, _: u64) -> io::Result<net::Got> {
        self.api(url, "", self.accept)
    }

    fn api(&self, url: &str, _: &str, accept: &str) -> io::Result<net::Got> {
        if self.asked.swap(true, std::sync::atomic::Ordering::Relaxed) {
            self.net.pause(self.pace);
        }
        let mut tries = 0;
        loop {
            match self.net.api(url, &self.token, accept) {
                Err(e) => match net::Refused::of(&e) {
                    Some(r) if r.status == 429 && tries < 2 => {
                        tries += 1;
                        self.net.pause(r.wait);
                    }
                    Some(r) if r.status == 429 => return Err(io::Error::other(bragd_core::t!("source.busy", "server" => self.server.name))),
                    Some(_) => return Err(io::Error::other(bragd_core::t!("source.sign_in_again", "server" => self.server.name))),
                    None => return Err(e),
                },
                got => return got,
            }
        }
    }

    fn offline(&self) -> bool {
        self.net.offline()
    }
}

/// The text key of the error a Source this version does not fetch gives when asked to.
pub const OUT_OF_SCOPE: &str = "notice.source_out_of_scope";

/// The Week in Chess: a zip of PGN every week, numbered.
pub struct Twic;

pub const TWIC_URL: &str = "https://theweekinchess.com/zips/twic{issue}g.zip";
/// The page the weekly zips are listed on: what Settings shows as the Source's address.
const TWIC_PAGE: &str = "https://theweekinchess.com/twic";
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
            url: TWIC_PAGE.into(),
            fetchable: true,
            kind: Kind::Games,
            parts: vec![],
            months: false,
            first_only: false,
            account: String::new(),
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
            if !zip.exists() && !net::download(cx.net, &twic_url(n), &zip, cx.cap, cx.stop, cx.progress)? {
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
            kind: Kind::Games,
            parts: vec![],
            months: false,
            first_only: false,
            account: String::new(),
        }
    }

    fn fetch(&self, cx: &mut Cx) -> io::Result<Vec<PathBuf>> {
        let name = self.url.split(['?', '#']).next().unwrap_or("").rsplit('/').next().unwrap_or("");
        let name: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_')).collect();
        let zip = name.to_ascii_lowercase().ends_with(".zip");
        if !(self.url.starts_with("https://") || self.url.starts_with("http://")) || name.starts_with('.') || !(zip || bragd_library::stream::importable(&name)) {
            return Err(io::Error::other(format!("{}: not a PGN, SGF, KIF, CSA or PDN file, or a zip of them", self.url)));
        }
        let to = cx.dir.join(&name);
        // Fetched again each time, from the start: the file at the address may have changed, and
        // a `.part` of the old one resumed would splice two versions together.
        let _ = std::fs::remove_file(&to);
        let _ = std::fs::remove_file(net::part_of(&to));
        if !net::download(cx.net, &self.url, &to, cx.cap, cx.stop, cx.progress)? {
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

/// A rating list (`bragd_clubs::lists::LISTS`): one file, fetched whole each month.
pub struct RatingList(pub &'static bragd_clubs::lists::List);

impl Source for RatingList {
    fn about(&self) -> About {
        let l = self.0;
        About { id: l.id.into(), name: l.name.into(), family: "chess".into(), size: l.size, schedule: Schedule::Monthly, url: l.url.into(), fetchable: true, kind: Kind::RatingList, parts: vec![], months: false, first_only: false, account: String::new() }
    }

    fn fetch(&self, cx: &mut Cx) -> io::Result<Vec<PathBuf>> {
        let to = cx.dir.join(self.0.file);
        // Fetched whole each time: last month's list is replaced, never resumed into.
        let _ = std::fs::remove_file(&to);
        let _ = std::fs::remove_file(net::part_of(&to));
        if !net::download(cx.net, self.0.url, &to, cx.cap, cx.stop, cx.progress)? {
            return Err(io::Error::other(format!("{} is not there", self.0.url)));
        }
        Ok(vec![to])
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
        kind: Kind::Games,
        parts: vec![],
        months: false,
        first_only: false,
        account: String::new(),
    }))
}

/// Every built-in Source, in the order Settings lists them. Sizes are rough, per fetch.
pub fn builtin() -> Vec<Box<dyn Source>> {
    vec![
        Box::new(Twic) as Box<dyn Source>,
        // Kept packed and imported from there as a stream (`bragd_library::stream`), with the
        // Routes' Strip rules applied as the Games stream by; nothing is unpacked to disk.
        Box::new(lichess::Lichess),
        Box::new(fics::Fics),
        Box::new(pgnmentor::PgnMentor),
        // shortcut: its site starts each download through a one-time script step and sends 7z; wait for a plain address (#314).
        later("lumbras", "LumbrasGigaBase", "chess", 4_000_000_000, Schedule::Monthly, "https://lumbrasgigabase.com/en/download-in-pgn-format-en/"),
        Box::new(ogs::Ogs),
        Box::new(kgs::Kgs),
        later("pychess", "pychess exports", "chess", 500_000_000, Schedule::Monthly, "https://www.pychess.org/"),
        later("lishogi", "Lishogi exports", "shogi", 500_000_000, Schedule::Monthly, "https://lishogi.org/"),
    ]
    .into_iter()
    .chain(bragd_clubs::lists::LISTS.iter().map(|l| Box::new(RatingList(l)) as Box<dyn Source>))
    .collect()
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
    fn no_source_shows_a_url_template() {
        for s in builtin() {
            let url = s.about().url;
            assert!(url.starts_with("https://") && !url.contains(['{', '}']), "{}: {url}", s.about().id);
        }
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
        Twic.fetch(&mut Cx { net, dir, cap: MAX_DOWNLOAD, memo, options: &o, stop: &|| false, progress: &mut |_| {} }).unwrap()
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
        assert_eq!(ids, ["twic", "lichess-db", "fics", "pgnmentor", "lumbras", "ogs", "kgs", "pychess", "lishogi", "fide-ratings", "dsu-ratings", "ecf-ratings", "cfc-ratings", "sshl-ratings", "nzcf-ratings"]);
        let fetched: Vec<String> = builtin().iter().map(|s| s.about()).filter(|a| a.kind == Kind::Games && a.fetchable).map(|a| a.id).collect();
        assert_eq!(fetched, ["twic", "lichess-db", "fics", "pgnmentor", "ogs", "kgs"]);
        assert!(builtin().iter().all(|s| s.about().size > 0));
        let d = crate::tests::Dir::new("later");
        let net = Recorded::default();
        let o = Twic.about().defaults();
        let err = find("lishogi", "").unwrap().fetch(&mut Cx { net: &net, dir: &d.0, cap: MAX_DOWNLOAD, memo: &mut Memo::default(), options: &o, stop: &|| false, progress: &mut |_| {} }).unwrap_err();
        assert!(err.to_string().contains("Lishogi exports"), "{err}");
        assert!(net.asked.lock().unwrap().is_empty(), "nothing is asked of the network");
        assert!(find("custom:club", "https://example.org/a.pgn").unwrap().about().fetchable);
        assert!(find("nope", "").is_none());
    }

    #[test]
    fn months_count_on_by_one() {
        assert_eq!(month("2026-01"), Some(2026 * 12));
        assert_eq!(month("2025-12").map(|m| m + 1), month("2026-01"));
        assert_eq!(month_text(month("2024-07").unwrap()), "2024-07");
        for bad in ["", "2026", "2026-13", "2026-00", "26-01", "2026-1x"] {
            assert_eq!(month(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_custom_url_must_name_a_game_file() {
        let d = crate::tests::Dir::new("custom-bad");
        let net = Recorded::default();
        let o = Twic.about().defaults();
        for url in ["https://example.org/page.html", "file:///etc/passwd.pgn", "https://example.org/"] {
            let s = CustomUrl { id: "custom:x".into(), url: url.into() };
            assert!(s.fetch(&mut Cx { net: &net, dir: &d.0, cap: MAX_DOWNLOAD, memo: &mut Memo::default(), options: &o, stop: &|| false, progress: &mut |_| {} }).is_err(), "{url}");
        }
        assert!(net.asked.lock().unwrap().is_empty());
    }
}
