//! The FICS games database: a month of one game type as a `.pgn.bz2`, made by its download
//! page on request. The page answers with a link to the file, or with none while the month is
//! not out yet (on the 2nd of the next month).

use std::io;
use std::path::PathBuf;

use super::{About, Cx, Kind, Part, Schedule, Source, month, month_of, month_text, page};
use crate::net;

pub struct Fics;

pub const SITE: &str = "https://www.ficsgames.org";

/// The most months one fetch catches up on.
const CATCH_UP: u32 = 12;

/// The game types, by the number the download page gives each.
const TYPES: [(&str, u32); 8] = [("standard2000", 4), ("standard", 3), ("blitz2000", 6), ("blitz", 5), ("lightning2000", 8), ("lightning", 7), ("chess2000", 2), ("chess", 1)];

/// The page that makes the file of game type `n` for month `m`.
fn asked(n: u32, m: u32) -> String {
    format!("{SITE}/cgi-bin/download.cgi?gametype={n}&player=&year={}&month={}&movetimes=0&download=Download", m / 12, m % 12 + 1)
}

/// The file the page links to: `/dl/<name>.pgn.bz2`.
fn link(page: &str) -> Option<&str> {
    let at = page.find("href=\"/dl/")? + "href=\"".len();
    let path = &page[at..at + page[at..].find('"')?];
    path.ends_with(".pgn.bz2").then_some(path)
}

impl Fics {
    /// Fetches as at `now` (Unix seconds), which says what the newest month can be.
    pub fn fetch_at(&self, cx: &mut Cx, now: u64) -> io::Result<Vec<PathBuf>> {
        let newest = month_of(now) - 1;
        let first = match (cx.memo.last_issue, month(&cx.options.since)) {
            (Some(last), _) => last + 1,
            (None, Some(since)) => since,
            (None, None) => newest,
        };
        let types: Vec<(String, u32)> = self.about().chosen(cx.options).iter().filter_map(|p| TYPES.iter().find(|t| t.0 == p.id).map(|t| (p.id.clone(), t.1))).collect();
        let mut files = vec![];
        for m in (first..=newest).take(CATCH_UP as usize) {
            let mut month_files = vec![];
            for (id, n) in &types {
                let to = cx.dir.join(format!("ficsgamesdb_{}_{id}.pgn.bz2", month_text(m).replace('-', "")));
                if !to.exists() {
                    // No link: the month is not out yet, and nothing after it is either.
                    let Some(path) = page(cx.net, &asked(*n, m))?.as_deref().and_then(link).map(String::from) else { return Ok(files) };
                    let url = format!("{SITE}{path}");
                    if !net::download(cx.net, &url, &to, cx.cap, cx.stop, cx.progress)? {
                        return Err(io::Error::other(format!("{url} is not there")));
                    }
                }
                month_files.push(to);
            }
            files.append(&mut month_files);
            cx.memo.last_issue = Some(m);
        }
        Ok(files)
    }
}

impl Source for Fics {
    fn about(&self) -> About {
        About {
            id: "fics".into(),
            name: "FICS games database".into(),
            family: "chess".into(),
            size: 300_000,
            schedule: Schedule::Monthly,
            url: format!("{SITE}/download.html"),
            fetchable: true,
            kind: Kind::Games,
            parts: TYPES.iter().map(|(id, _)| Part::new(id, &format!("source.part.fics.{id}"), "chess", *id == "standard2000")).collect(),
            months: true,
            first_only: false,
            account: String::new(),
        }
    }

    fn fetch(&self, cx: &mut Cx) -> io::Result<Vec<PathBuf>> {
        self.fetch_at(cx, bragd_core::fs::now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::tests::Recorded;
    use crate::sources::{MAX_DOWNLOAD, Memo, Options};
    use std::collections::BTreeMap;

    /// 2026-11-03.
    const NOW: u64 = 1_793_664_000;

    fn answer(file: &str) -> Vec<u8> {
        format!("<p>The requested games can be downloaded from here: <a href=\"/dl/{file}\">{file}</a></p>").into_bytes()
    }

    /// The site with the months `out`, of game type 4 (Standard, average rating over 2000).
    fn site(out: &[u32]) -> Recorded {
        let bz2 = include_bytes!("../../tests/fixtures/fics.pgn.bz2").to_vec();
        let mut files = BTreeMap::new();
        for &m in out {
            let name = format!("ficsgamesdb_{}_standard2000_nomovetimes_{m}.pgn.bz2", month_text(m).replace('-', ""));
            files.insert(asked(4, m), answer(&name));
            files.insert(format!("{SITE}/dl/{name}"), bz2.clone());
        }
        // A month not out yet: the page, without a link.
        files.insert(asked(4, out.iter().max().unwrap() + 1), b"<p>Download not available</p>".to_vec());
        Recorded { files, ..Default::default() }
    }

    fn fetch(net: &Recorded, dir: &std::path::Path, memo: &mut Memo, o: &Options, now: u64) -> Vec<PathBuf> {
        Fics.fetch_at(&mut Cx { net, dir, cap: MAX_DOWNLOAD, memo, options: o, stop: &|| false, progress: &mut |_| {} }, now).unwrap()
    }

    #[test]
    fn the_last_month_first_then_each_month_once_it_is_out() {
        let d = crate::tests::Dir::new("fics");
        assert_eq!(month_text(month_of(NOW)), "2026-11");
        let oct = month("2026-10").unwrap();
        let net = site(&[oct]);
        let o = Fics.about().defaults();
        let mut memo = Memo::default();
        assert_eq!(fetch(&net, &d.0, &mut memo, &o, NOW), [d.0.join("ficsgamesdb_202610_standard2000.pgn.bz2")]);
        assert_eq!(memo.last_issue, Some(oct));
        // On 1 December November is not out: nothing, and it is asked again next time.
        let dec1 = NOW + 28 * 86_400;
        assert!(fetch(&net, &d.0, &mut memo, &o, dec1).is_empty());
        assert_eq!(memo.last_issue, Some(oct));
        let net = site(&[oct, oct + 1]);
        assert_eq!(fetch(&net, &d.0, &mut memo, &o, dec1 + 86_400), [d.0.join("ficsgamesdb_202611_standard2000.pgn.bz2")]);
    }

    #[test]
    fn a_month_to_start_from_and_the_game_types_picked() {
        let d = crate::tests::Dir::new("fics-since");
        let (aug, sep, oct) = (month("2026-08").unwrap(), month("2026-09").unwrap(), month("2026-10").unwrap());
        let mut net = site(&[aug, sep, oct]);
        let name = "ficsgamesdb_202608_blitz_nomovetimes_1.pgn.bz2";
        net.files.insert(asked(5, aug), answer(name));
        net.files.insert(format!("{SITE}/dl/{name}"), b"x".to_vec());
        let mut o: Options = Fics.about().defaults();
        o.since = "2026-08".into();
        o.parts = vec!["blitz".into()];
        let mut memo = Memo::default();
        // Blitz is not there for September: the fetch stops at August.
        assert_eq!(fetch(&net, &d.0, &mut memo, &o, NOW), [d.0.join("ficsgamesdb_202608_blitz.pgn.bz2")]);
        assert_eq!(memo.last_issue, Some(aug));
        o.parts = vec![];
        assert_eq!(fetch(&net, &d.0, &mut memo, &o, NOW).len(), 2, "September and October of the default type");
    }

    #[test]
    fn the_link_is_read_from_the_page() {
        assert_eq!(link("x <a href=\"/dl/ficsgamesdb_200001_standard2000_nomovetimes_4355249.pgn.bz2\">"), Some("/dl/ficsgamesdb_200001_standard2000_nomovetimes_4355249.pgn.bz2"));
        assert_eq!(link("<a href=\"/dl/x.html\">"), None);
        assert_eq!(link("<a href=\"/download.html\">"), None);
    }
}
