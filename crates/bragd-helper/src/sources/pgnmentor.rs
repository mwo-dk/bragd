//! PGN Mentor: a page of files, a zip for each player and opening and a PGN for each event.
//! Each fetch takes the files of the sections picked that it has not taken before.

use std::io;
use std::path::PathBuf;

use super::{About, Cx, Kind, Part, Schedule, Source, page};
use crate::net;

pub struct PgnMentor;

pub const PAGE: &str = "https://www.pgnmentor.com/files.html";
const SITE: &str = "https://www.pgnmentor.com/";
const SECTIONS: [&str; 3] = ["players", "openings", "events"];

/// The files the page links to in `section`, each once, in the page's order: `players/Adams.zip`.
fn files<'a>(page: &'a str, section: &str) -> Vec<&'a str> {
    let head = format!("href=\"{section}/");
    let mut out: Vec<&str> = vec![];
    for (at, _) in page.match_indices(&head) {
        let rest = &page[at + "href=\"".len()..];
        let Some(path) = rest.find('"').map(|end| &rest[..end]) else { continue };
        let name = &path[section.len() + 1..];
        let ok = !name.is_empty() && !name.contains(['/', '\\']) && !name.starts_with('.') && (name.ends_with(".zip") || name.ends_with(".pgn"));
        if ok && !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

impl Source for PgnMentor {
    fn about(&self) -> About {
        About {
            id: "pgnmentor".into(),
            name: "PGN Mentor".into(),
            family: "chess".into(),
            size: 200_000_000,
            schedule: Schedule::Monthly,
            url: PAGE.into(),
            fetchable: true,
            kind: Kind::Games,
            parts: SECTIONS.iter().map(|s| Part::new(s, &format!("source.part.pgnmentor.{s}"), "chess", true)).collect(),
            months: false,
            first_only: true,
            account: String::new(),
        }
    }

    fn fetch(&self, cx: &mut Cx) -> io::Result<Vec<PathBuf>> {
        let text = page(cx.net, PAGE)?.ok_or_else(|| io::Error::other(format!("{PAGE} is not there")))?;
        let mut out = vec![];
        for part in self.about().chosen(cx.options) {
            for path in files(&text, &part.id) {
                if cx.memo.got.iter().any(|g| g == path) {
                    continue;
                }
                let to = cx.dir.join(path.replace('/', "-"));
                let url = format!("{SITE}{path}");
                if !to.exists() && !net::download(cx.net, &url, &to, cx.cap, cx.stop, cx.progress)? {
                    return Err(io::Error::other(format!("{url} is not there")));
                }
                cx.memo.got.push(path.to_string());
                out.push(to);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::tests::Recorded;
    use crate::sources::{MAX_DOWNLOAD, Memo};

    const FIXTURE: &str = "<a href=\"players/Adams.zip\">Adams</a> <a href=\"players/Adams.zip\">Download</a>\n<a href=\"events/Aachen1868.pgn\">x</a> <a href=\"events/Aachen1868/\">View</a>\n<a href=\"openings/Sicilian.zip\">y</a> <a href=\"players/../x.zip\">z</a> <a href=\"crafty.exe\">c</a>";

    #[test]
    fn the_page_names_each_file_once_and_nothing_outside_its_section() {
        assert_eq!(files(FIXTURE, "players"), ["players/Adams.zip"]);
        assert_eq!(files(FIXTURE, "events"), ["events/Aachen1868.pgn"]);
        assert_eq!(files(FIXTURE, "openings"), ["openings/Sicilian.zip"]);
    }

    #[test]
    fn each_file_is_fetched_once_of_the_sections_picked() {
        let d = crate::tests::Dir::new("pgnmentor");
        let pgn = crate::jobs::tests::PGN.as_bytes().to_vec();
        let zip = crate::sources::tests::zip_of(&[("Adams.pgn", crate::jobs::tests::PGN)]);
        let mut net = Recorded {
            files: [(PAGE.to_string(), FIXTURE.as_bytes().to_vec()), (format!("{SITE}players/Adams.zip"), zip.clone()), (format!("{SITE}events/Aachen1868.pgn"), pgn), (format!("{SITE}openings/Sicilian.zip"), zip.clone())].into(),
            ..Default::default()
        };
        let mut o = PgnMentor.about().defaults();
        o.parts = vec!["players".into(), "events".into()];
        let mut memo = Memo::default();
        let fetch = |net: &Recorded, memo: &mut Memo| PgnMentor.fetch(&mut Cx { net, dir: &d.0, cap: MAX_DOWNLOAD, memo, options: &o, stop: &|| false, progress: &mut |_| {} }).unwrap();
        assert_eq!(fetch(&net, &mut memo), [d.0.join("players-Adams.zip"), d.0.join("events-Aachen1868.pgn")]);
        // A new player on the page: only that one, even with the old downloads deleted.
        std::fs::remove_file(d.0.join("players-Adams.zip")).unwrap();
        net.files.insert(PAGE.into(), format!("{FIXTURE} <a href=\"players/Carlsen.zip\">").into_bytes());
        net.files.insert(format!("{SITE}players/Carlsen.zip"), zip);
        assert_eq!(fetch(&net, &mut memo), [d.0.join("players-Carlsen.zip")]);
        assert!(fetch(&net, &mut memo).is_empty());
    }
}
