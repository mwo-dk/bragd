//! The Lichess database: a `.pgn.zst` a month for each Variant, listed newest first in each
//! Variant's `list.txt`. A standard month is about 30 GB, so it waits for the user's yes
//! (`Memo::too_big`); a download cut short goes on from its `.part` next time.

use std::io;
use std::path::PathBuf;

use super::{About, Cx, Kind, Part, Schedule, Source, month, months_to_fetch, page};
use crate::net;

pub struct Lichess;

pub const BASE: &str = "https://database.lichess.org";

/// The most months one fetch catches up on.
const CATCH_UP: usize = 3;

/// The Variants Bragd has, by Lichess's folder name.
const VARIANTS: [(&str, &str); 4] = [("standard", "chess"), ("chess960", "chess960"), ("kingOfTheHill", "kingofthehill"), ("racingKings", "racingkings")];

fn file(variant: &str, m: u32) -> String {
    format!("lichess_db_{variant}_rated_{}.pgn.zst", super::month_text(m))
}

/// The months a Variant's `list.txt` names.
fn listed(variant: &str, list: &str) -> Vec<u32> {
    let head = format!("lichess_db_{variant}_rated_");
    list.lines().filter_map(|l| month(l.trim().rsplit('/').next()?.strip_prefix(&head)?.strip_suffix(".pgn.zst")?)).collect()
}

impl Source for Lichess {
    fn about(&self) -> About {
        About {
            id: "lichess-db".into(),
            name: "Lichess database".into(),
            family: "chess".into(),
            size: 30_000_000_000,
            schedule: Schedule::Monthly,
            url: BASE.into(),
            fetchable: true,
            kind: Kind::Games,
            parts: VARIANTS.iter().map(|(id, v)| Part::new(id, &format!("variant.{v}"), v, *id == "standard")).collect(),
            months: true,
            first_only: false,
            account: String::new(),
        }
    }

    fn fetch(&self, cx: &mut Cx) -> io::Result<Vec<PathBuf>> {
        let about = self.about();
        let variants: Vec<String> = about.chosen(cx.options).iter().map(|p| p.id.clone()).collect();
        // The months every chosen Variant has, so a month is done for all of them at once.
        let mut there: Option<Vec<u32>> = None;
        for v in &variants {
            let url = format!("{BASE}/{v}/list.txt");
            let months = listed(v, &page(cx.net, &url)?.ok_or_else(|| io::Error::other(format!("{url} is not there")))?);
            there = Some(match there {
                None => months,
                Some(t) => t.into_iter().filter(|m| months.contains(m)).collect(),
            });
        }
        let mut files = vec![];
        for m in months_to_fetch(&there.unwrap_or_default(), cx.memo, cx.options, CATCH_UP) {
            for v in &variants {
                let name = file(v, m);
                let (url, to) = (format!("{BASE}/{v}/{name}"), cx.dir.join(&name));
                if !to.exists() && !net::download(cx.net, &url, &to, cx.cap, cx.stop, cx.progress)? {
                    return Err(io::Error::other(format!("{url} is not there")));
                }
                files.push(to);
            }
            cx.memo.last_issue = Some(m);
        }
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::tests::Recorded;
    use crate::sources::{MAX_DOWNLOAD, Memo, Options};

    fn list(variant: &str, months: &[&str]) -> (String, Vec<u8>) {
        (format!("{BASE}/{variant}/list.txt"), months.iter().map(|m| format!("{BASE}/{variant}/lichess_db_{variant}_rated_{m}.pgn.zst\n")).collect::<String>().into_bytes())
    }

    fn fetch(net: &Recorded, dir: &std::path::Path, memo: &mut Memo, o: &Options) -> io::Result<Vec<PathBuf>> {
        Lichess.fetch(&mut Cx { net, dir, cap: MAX_DOWNLOAD, memo, options: o, stop: &|| false, progress: &mut |_| {} })
    }

    #[test]
    fn the_newest_month_first_then_each_new_one_for_each_variant_chosen() {
        let d = crate::tests::Dir::new("lichess");
        let zst = include_bytes!("../../tests/fixtures/lichess.pgn.zst").to_vec();
        let mut files: std::collections::BTreeMap<String, Vec<u8>> = [list("standard", &["2026-10", "2026-09"]), list("racingKings", &["2026-10", "2026-09"])].into();
        for v in ["standard", "racingKings"] {
            for m in ["2026-09", "2026-10", "2026-11"] {
                files.insert(format!("{BASE}/{v}/lichess_db_{v}_rated_{m}.pgn.zst"), zst.clone());
            }
        }
        let net = Recorded { files, ..Default::default() };
        let mut o = Lichess.about().defaults();
        let mut memo = Memo::default();
        let got = fetch(&net, &d.0, &mut memo, &o).unwrap();
        assert_eq!(got, [d.0.join("lichess_db_standard_rated_2026-10.pgn.zst")], "standard only, by default, and only the newest");
        assert_eq!(memo.last_issue, month("2026-10"));
        assert!(fetch(&net, &d.0, &mut memo, &o).unwrap().is_empty(), "nothing new");
        // A new month, with Racing Kings picked too: both, for that month.
        let mut net = net;
        net.files.extend([list("standard", &["2026-11", "2026-10"]), list("racingKings", &["2026-11", "2026-10"])]);
        o.parts = vec!["standard".into(), "racingKings".into()];
        let got = fetch(&net, &d.0, &mut memo, &o).unwrap();
        assert_eq!(got, [d.0.join("lichess_db_standard_rated_2026-11.pgn.zst"), d.0.join("lichess_db_racingKings_rated_2026-11.pgn.zst")]);
    }

    #[test]
    fn a_month_to_start_from_fetches_each_month_since_a_few_at_a_time() {
        let d = crate::tests::Dir::new("lichess-since");
        let months = ["2025-01", "2025-02", "2025-03", "2025-04", "2025-05"];
        let mut files: std::collections::BTreeMap<String, Vec<u8>> = [list("standard", &months)].into();
        for m in months {
            files.insert(format!("{BASE}/standard/lichess_db_standard_rated_{m}.pgn.zst"), b"x".to_vec());
        }
        let net = Recorded { files, ..Default::default() };
        let mut o = Lichess.about().defaults();
        o.since = "2025-02".into();
        let mut memo = Memo::default();
        let got = fetch(&net, &d.0, &mut memo, &o).unwrap();
        assert_eq!(got.len(), CATCH_UP);
        assert_eq!(memo.last_issue, month("2025-04"));
        assert_eq!(fetch(&net, &d.0, &mut memo, &o).unwrap(), [d.0.join("lichess_db_standard_rated_2025-05.pgn.zst")]);
    }

    #[test]
    fn a_month_too_big_waits_and_is_not_counted_as_fetched() {
        let d = crate::tests::Dir::new("lichess-big");
        let mut files: std::collections::BTreeMap<String, Vec<u8>> = [list("standard", &["2026-10"])].into();
        files.insert(format!("{BASE}/standard/lichess_db_standard_rated_2026-10.pgn.zst"), vec![0; 100]);
        let net = Recorded { files, sizes: true, ..Default::default() };
        let o = Lichess.about().defaults();
        let mut memo = Memo::default();
        let e = Lichess.fetch(&mut Cx { net: &net, dir: &d.0, cap: 10, memo: &mut memo, options: &o, stop: &|| false, progress: &mut |_| {} }).unwrap_err();
        assert_eq!(net::TooBig::of(&e).and_then(|t| t.size), Some(100), "the size, to ask about");
        assert_eq!(memo.last_issue, None);
    }
}

/// The whole way: the Helper fetches a Lichess month and a FICS month holding the same two
/// Games, imports both into the main Library, and keeps each Game once, with both Sources.
#[cfg(test)]
mod import {
    use super::*;
    use crate::jobs::tests::jobs_in;
    use crate::jobs::{Status, Work};
    use crate::net::tests::Recorded;
    use bragd_library::library::Library;

    #[test]
    fn a_lichess_month_and_a_fics_month_import_and_their_duplicates_merge() {
        let d = crate::tests::Dir::new("lichess-import");
        let oct = "2026-10";
        let fics_page = format!("{}/cgi-bin/download.cgi?gametype=4&player=&year=2026&month=10&movetimes=0&download=Download", crate::sources::fics::SITE);
        let net = Recorded {
            files: [
                (format!("{BASE}/standard/list.txt"), format!("{BASE}/standard/lichess_db_standard_rated_{oct}.pgn.zst\n").into_bytes()),
                (format!("{BASE}/standard/lichess_db_standard_rated_{oct}.pgn.zst"), include_bytes!("../../tests/fixtures/lichess.pgn.zst").to_vec()),
                (fics_page, b"<a href=\"/dl/ficsgamesdb_202610_standard2000_nomovetimes_9.pgn.bz2\">".to_vec()),
                (format!("{}/dl/ficsgamesdb_202610_standard2000_nomovetimes_9.pgn.bz2", crate::sources::fics::SITE), include_bytes!("../../tests/fixtures/fics.pgn.bz2").to_vec()),
            ]
            .into(),
            ..Default::default()
        };
        let jobs = jobs_in(&d.0, net, Default::default());
        for id in ["lichess-db", "fics"] {
            let mut o = crate::sources::find(id, "").unwrap().about().defaults();
            o.enabled = true;
            o.since = oct.into();
            jobs.set_source(id, o).unwrap();
            let job = jobs.add(Work::Fetch { source: id.into() });
            assert!(jobs.run_next());
            let j = jobs.list().into_iter().find(|j| j.id == job).unwrap();
            assert_eq!((j.status, j.error), (Status::Done, None), "{id}");
        }
        let lib = Library::open(&d.0.join("lib/main.bragd")).unwrap();
        assert_eq!(lib.count().unwrap(), 2, "each Game once");
        let from: Vec<String> = lib.sources(1).unwrap().into_iter().map(|s| s.0).collect();
        assert_eq!(from, ["lichess-db", "fics"]);
        assert!(d.0.join("sources/lichess-db/lichess_db_standard_rated_2026-10.pgn.zst").exists(), "kept packed, as it came");
    }
}
