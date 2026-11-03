//! The KGS archive: the go Games of strong players on the KGS Go Server, as u-go.net publishes
//! them, one zip of SGF files a month (a year before 2016). The page that lists them is read
//! for its zips; each one newer than the last fetched is downloaded, oldest first.

use std::io::{self, Read};
use std::path::PathBuf;

use super::{About, Cx, Kind, Schedule, Source};
use crate::net::{self, Got};

/// The page that lists the zips.
pub const KGS_INDEX: &str = "https://u-go.net/gamerecords/";
/// Where every zip it lists lives: links elsewhere are not followed.
const KGS_FILES: &str = "https://dl.u-go.net/gamerecords/KGS-";
/// The most of the page read: it is about 40 kB.
const MAX_INDEX: u64 = 4 << 20;

pub struct Kgs;

/// A zip the page lists: its period as `yyyymm` (`yyyy00` for a year) and its address.
fn zips(page: &str) -> Vec<(u32, String)> {
    let mut all: Vec<(u32, String)> = page
        .split('"')
        .filter(|s| s.starts_with(KGS_FILES) && s.ends_with(".zip"))
        .filter_map(|url| {
            let name = &url[KGS_FILES.len()..];
            // `2019_04-19-1255-.zip` or `2001-19-2298-.zip`: the period, then the board size.
            if !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')) {
                return None;
            }
            let period = name.split('-').next()?;
            let (year, month) = period.split_once('_').unwrap_or((period, "00"));
            let (year, month): (u32, u32) = (year.parse().ok()?, month.parse().ok()?);
            (year >= 1000 && month <= 12).then(|| (year * 100 + month, url.to_string()))
        })
        .collect();
    all.sort();
    all.dedup_by_key(|(p, _)| *p);
    all
}

impl Source for Kgs {
    fn about(&self) -> About {
        About {
            id: "kgs".into(),
            name: "KGS archive".into(),
            family: "go".into(),
            // The whole archive, which the first fetch takes: 55 zips in November 2026.
            size: 185_000_000,
            schedule: Schedule::Monthly,
            url: KGS_INDEX.into(),
            fetchable: true,
            kind: Kind::Games,
        }
    }

    fn fetch(&self, cx: &mut Cx) -> io::Result<Vec<PathBuf>> {
        let page = match cx.net.get(KGS_INDEX, 0)? {
            Got::Missing => return Err(io::Error::other(format!("{KGS_INDEX} is not there"))),
            Got::Body { body, .. } => {
                let mut s = String::new();
                body.take(MAX_INDEX).read_to_string(&mut s)?;
                s
            }
        };
        let mut files = vec![];
        // `last_issue` holds the period of the last zip fetched.
        let last = cx.memo.last_issue;
        for (period, url) in zips(&page).into_iter().filter(|(p, _)| last.is_none_or(|l| *p > l)) {
            let to = cx.dir.join(&url[url.rfind('/').map_or(0, |i| i + 1)..]);
            if !to.exists() && !net::download(cx.net, &url, &to, cx.cap, cx.stop, cx.progress)? {
                return Err(io::Error::other(format!("{url} is not there")));
            }
            files.push(to);
            cx.memo.last_issue = Some(period);
        }
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::{Status, Work, tests::jobs_in};
    use crate::net::tests::Recorded;
    use crate::sources::tests::zip_of;
    use bragd_library::library::Library;
    use std::sync::Arc;

    /// Rows as u-go.net writes them, with a tar.gz and a link elsewhere that are not taken.
    fn page(periods: &[&str]) -> String {
        let mut s = String::from("<table><tr><td><a href=\"https://example.org/KGS-2030_01-19-1-.zip\">x</a></td></tr>\n");
        for p in periods.iter().rev() {
            s.push_str(&format!("<tr><td>.zip</td><td>1.0 MB</td><td><a href=\"{KGS_FILES}{p}-19-2-.zip\">Download</a></td></tr>\n<tr><td><a href=\"{KGS_FILES}{p}-19-2-.tar.gz\">Download</a></td></tr>\n"));
        }
        s + "</table>"
    }

    /// A Game whose last move, as its players, is its own.
    fn game(black: &str, last: &str) -> String {
        format!("(;FF[4]GM[1]SZ[19]PB[{black}]PW[White]RE[B+R];B[qd];W[dc];B[{last}])")
    }

    fn net(periods: &[&str]) -> Recorded {
        let mut files: std::collections::BTreeMap<String, Vec<u8>> = periods
            .iter()
            .zip(["dd", "ee", "ff"])
            .map(|(p, last)| {
                let a = game(&format!("Black {p}"), last);
                // Each zip also holds a Game the 2001 one has, as one player's Games recur.
                (format!("{KGS_FILES}{p}-19-2-.zip"), zip_of(&[(&format!("kgs/{p}/a.sgf"), a.as_str()), ("kgs/same.sgf", game("Same", "pq").as_str())]))
            })
            .collect();
        files.insert(KGS_INDEX.into(), page(periods).into_bytes());
        Recorded { files, ..Default::default() }
    }

    #[test]
    fn the_page_lists_its_zips_by_period() {
        let got = zips(&page(&["2001", "2016_01", "2019_04"]));
        let periods: Vec<u32> = got.iter().map(|(p, _)| *p).collect();
        assert_eq!(periods, [200100, 201601, 201904], "oldest first, only its own zips");
        assert!(zips("<a href=\"https://dl.u-go.net/gamerecords/KGS-x/../y.zip\">").is_empty());
    }

    #[test]
    fn kgs_fetches_every_zip_once_and_its_games_are_imported_once() {
        let d = crate::tests::Dir::new("kgs");
        let jobs = jobs_in(&d.0, net(&["2001", "2019_03"]), Arc::default());
        let mut o = Kgs.about().defaults();
        o.enabled = true;
        jobs.set_source("kgs", o).unwrap();
        let run = |jobs: &Arc<crate::jobs::Jobs>| {
            let id = jobs.add(Work::Fetch { source: "kgs".into() });
            assert!(jobs.run_next());
            let j = jobs.list().into_iter().find(|j| j.id == id).unwrap();
            assert_eq!((j.status, j.error.as_deref()), (Status::Done, None));
        };
        run(&jobs);
        let lib = d.0.join("lib/main.bragd");
        let count = || Library::open(&lib).unwrap().count().unwrap();
        assert_eq!(count(), 3, "two Games of their own, and the one both zips hold once");
        assert_eq!(jobs.sources().into_iter().find(|(a, _, _)| a.id == "kgs").unwrap().2.last_issue, Some(201903));
        // A month later the page has one more zip: only that one is fetched, not one deleted since.
        let src = d.0.join("sources/kgs");
        std::fs::remove_file(src.join("KGS-2001-19-2-.zip")).unwrap();
        let jobs = jobs_in(&d.0, net(&["2001", "2019_03", "2019_04"]), Arc::default());
        run(&jobs);
        assert_eq!(count(), 4);
        let mut kept: Vec<String> = std::fs::read_dir(&src).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        kept.sort();
        assert_eq!(kept, ["KGS-2019_03-19-2-.zip", "KGS-2019_04-19-2-.zip"]);
    }
}
