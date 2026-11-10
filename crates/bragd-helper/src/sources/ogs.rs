//! OGS: the user's own go Games on online-go.com, through the account Bragd signed in to OGS
//! with for online play. The account's Games are listed newest ended first, and each Game that
//! ended since the last fetch is downloaded as SGF, one request every `PACE` seconds, oldest
//! first. A Game still being played is taken once it has ended. One Game can also be fetched
//! by its address or its number.

use std::io;
use std::path::PathBuf;

use bragd_online::server::OGS;

use super::{About, Account, Cx, Kind, Schedule, Source, page};
use crate::net::{self, Fetcher, TooBig};

pub struct Ogs;

/// Seconds between two requests: OGS asks clients to go gently and states no rate.
const PACE: u64 = 2;
/// Games in one page of the list.
const PAGE: u32 = 50;
/// About what one Game's SGF takes: a 19×19 Game with its chat.
const SGF_BYTES: u64 = 3_000;

fn sgf_url(base: &str, id: u64) -> String {
    format!("{base}/api/v1/games/{id}/sgf")
}

/// The Game's number in `game`: the number itself, or an OGS address that ends in it
/// (`https://online-go.com/game/12345`).
fn game_id(game: &str, base: &str) -> Option<u64> {
    let g = game.trim().trim_end_matches('/');
    if g.contains("://") && !(g.starts_with(base) || g.starts_with(OGS.url)) {
        return None;
    }
    let last = g.rsplit('/').next()?;
    last.parse().ok().filter(|&n| n > 0)
}

impl Source for Ogs {
    fn about(&self) -> About {
        About {
            id: "ogs".into(),
            name: "OGS".into(),
            family: "go".into(),
            // A thousand Games the first time; later fetches take only those ended since.
            size: 1000 * SGF_BYTES,
            schedule: Schedule::Daily,
            url: format!("{}/", OGS.url),
            fetchable: true,
            kind: Kind::Games,
            parts: vec![],
            months: false,
            first_only: true,
            account: OGS.id.into(),
        }
    }

    fn fetch(&self, cx: &mut Cx) -> io::Result<Vec<PathBuf>> {
        let site = Account::signed_in(cx.net, &OGS, PACE)?;
        let base = OGS.base();
        // `last_issue`: when the last Game fetched ended, in seconds since 1970.
        let last = cx.memo.last_issue.map(u64::from);
        let mut url = format!("{base}/api/v1/me/games/?ordering=-ended&page_size={PAGE}");
        let mut ended: Vec<(u64, u64)> = vec![];
        'pages: loop {
            let text = page(&site, &url)?.ok_or_else(|| io::Error::other(format!("{url} is not there")))?;
            let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| io::Error::other(format!("{url}: {e}")))?;
            // Games still being played come first (no end yet): they wait for a later fetch.
            for g in v["results"].as_array().into_iter().flatten() {
                let (Some(id), Some(t)) = (g["id"].as_u64(), g["ended"].as_str().and_then(crate::iso_secs)) else { continue };
                if last.is_some_and(|l| t < l) {
                    break 'pages;
                }
                ended.push((id, t));
            }
            match v["next"].as_str() {
                Some(next) if next.starts_with(&base) => url = next.into(),
                _ => break,
            }
        }
        let size = ended.len() as u64 * SGF_BYTES;
        if size > cx.cap {
            return Err(io::Error::other(TooBig { url: format!("{base}/api/v1/me/games/"), size: Some(size), cap: cx.cap }));
        }
        let mut files = vec![];
        for (id, t) in ended.into_iter().rev() {
            let to = cx.dir.join(format!("ogs-{id}.sgf"));
            if to.exists() || net::download(&site, &sgf_url(&base, id), &to, cx.cap, cx.stop, cx.progress)? {
                files.push(to);
            }
            cx.memo.last_issue = Some(u32::try_from(t).unwrap_or(u32::MAX));
        }
        Ok(files)
    }

    fn one(&self, cx: &mut Cx, game: &str) -> io::Result<PathBuf> {
        let base = OGS.base();
        let id = game_id(game, &base).ok_or_else(|| io::Error::other(bragd_core::t!("source.not_a_game", "game" => game.trim(), "server" => OGS.name)))?;
        let to = cx.dir.join(format!("ogs-{id}.sgf"));
        // An SGF is public: the account is asked for only when Bragd is signed in.
        let site = Account::signed_in(cx.net, &OGS, PACE).ok();
        let net: &dyn Fetcher = site.as_ref().map_or(cx.net, |s| s);
        if !net::download(net, &sgf_url(&base, id), &to, cx.cap, cx.stop, cx.progress)? {
            return Err(io::Error::other(format!("{} is not there", sgf_url(&base, id))));
        }
        Ok(to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::{Status, Work, tests::jobs_in};
    use crate::net::tests::Recorded;
    use bragd_library::library::Library;
    use std::sync::Arc;

    const LIST: &str = "https://online-go.com/api/v1/me/games/?ordering=-ended&page_size=50";

    /// A Game as OGS's SGF has it (recorded from online-go.com, shortened).
    fn sgf(id: u64, last: &str) -> String {
        format!("(;FF[4]\nCA[UTF-8]\nGM[1]\nDT[2026-07-18]\nPC[OGS: https://online-go.com/game/{id}]\nPB[Black{id}]\nPW[anoek]\nRE[W+R]\nSZ[9]\nKM[6.5]\nRU[Japanese]\n;B[cf]\n(;W[gd]\n(;B[{last}]\n)))")
    }

    /// One page of the list as OGS gives it: `(id, ended)`, none for a Game being played.
    fn list(games: &[(u64, Option<&str>)], next: Option<&str>) -> String {
        let results: Vec<serde_json::Value> = games.iter().map(|(id, ended)| serde_json::json!({ "id": id, "ended": ended, "width": 9, "height": 9 })).collect();
        serde_json::json!({ "count": games.len(), "next": next, "previous": null, "results": results }).to_string()
    }

    fn site(pages: &[(String, String)], games: &[u64]) -> Recorded {
        let mut files: std::collections::BTreeMap<String, Vec<u8>> = pages.iter().map(|(u, b)| (u.clone(), b.clone().into_bytes())).collect();
        for (i, id) in games.iter().enumerate() {
            files.insert(sgf_url("https://online-go.com", *id), sgf(*id, ["dd", "ee", "ff", "gg"][i % 4]).into_bytes());
        }
        Recorded { files, ..Default::default() }
    }

    #[test]
    fn a_game_is_named_by_its_number_or_its_address() {
        let base = "https://online-go.com";
        assert_eq!(game_id("88868859", base), Some(88868859));
        assert_eq!(game_id(" https://online-go.com/game/88868859/ ", base), Some(88868859));
        assert_eq!(game_id("https://example.org/game/1", base), None, "not an OGS address");
        assert_eq!(game_id("https://online-go.com/game/view", base), None);
        assert_eq!(game_id("0", base), None);
    }

    /// Signed out, then signed in: the list and each new Game as the account, paced, with a
    /// 429 waited out; a later fetch takes only what ended since; one Game by its address.
    #[test]
    fn ogs_fetches_the_accounts_ended_games_once_each() {
        let _one = KEYCHAIN.lock().unwrap_or_else(|e| e.into_inner());
        bragd_model::key::mock();
        let base = OGS.base();
        let d = crate::tests::Dir::new("ogs");
        let page2 = format!("{base}/api/v1/me/games/?ordering=-ended&page=2&page_size=50");
        let pages = vec![
            (LIST.to_string(), list(&[(30, None), (20, Some("2026-07-29T10:09:45.69Z"))], Some(&page2))),
            (page2.clone(), list(&[(10, Some("2026-07-01T08:00:00Z"))], None)),
        ];
        let jobs = jobs_in(&d.0, site(&pages, &[10, 20]), Arc::default());
        let mut o = Ogs.about().defaults();
        o.enabled = true;
        jobs.set_source("ogs", o.clone()).unwrap();
        let run = |jobs: &Arc<crate::jobs::Jobs>, work: Work| {
            let id = jobs.add(work);
            assert!(jobs.run_next());
            jobs.list().into_iter().find(|j| j.id == id).unwrap()
        };
        let fetch = || Work::Fetch { source: "ogs".into() };
        let j = run(&jobs, fetch());
        assert_eq!(j.status, Status::Failed);
        assert_eq!(j.error.as_deref(), Some(bragd_core::t!("source.sign_in", "server" => "OGS").as_str()), "signed out: says where to sign in");
        bragd_model::key::set_in(bragd_online::session::SERVICE, &base, "tok").unwrap();
        let net = site(&pages, &[10, 20]);
        net.busy.lock().unwrap().push(sgf_url(&base, 20));
        let jobs = jobs_in(&d.0, net, Arc::default());
        let j = run(&jobs, fetch());
        assert_eq!((j.status, j.error.as_deref()), (Status::Done, None));
        let lib = d.0.join("lib/main.bragd");
        let count = || Library::open(&lib).unwrap().count().unwrap();
        assert_eq!(count(), 2, "the two ended Games, not the one being played");
        let memo = jobs.sources().into_iter().find(|(a, _, _)| a.id == "ogs").unwrap().2;
        assert_eq!(memo.last_issue.map(u64::from), crate::iso_secs("2026-07-29T10:09:45Z"));
        // A day later one more Game has ended: only it is downloaded.
        let pages = vec![(LIST.to_string(), list(&[(30, Some("2026-08-02T12:00:00Z")), (20, Some("2026-07-29T10:09:45.69Z")), (10, Some("2026-07-01T08:00:00Z"))], None))];
        std::fs::remove_file(d.0.join("sources/ogs/ogs-20.sgf")).unwrap();
        let jobs = jobs_in(&d.0, site(&pages, &[10, 20, 30]), Arc::default());
        let j = run(&jobs, fetch());
        assert_eq!((j.status, j.error.as_deref()), (Status::Done, None));
        assert_eq!(count(), 3, "the new Game, and the one ended at the same second is not stored twice");
        // One Game by its address, signed in or not, leaves what the Source remembers.
        let before = jobs.sources().into_iter().find(|(a, _, _)| a.id == "ogs").unwrap().2;
        let mut one = site(&[], &[10, 20, 30, 40]);
        one.files.insert(sgf_url(&base, 40), sgf(40, "hh").into_bytes());
        let jobs = jobs_in(&d.0, one, Arc::default());
        let j = run(&jobs, Work::FetchGame { source: "ogs".into(), game: format!("{base}/game/40") });
        assert_eq!((j.status, j.error.as_deref()), (Status::Done, None));
        assert_eq!(count(), 4);
        assert_eq!(jobs.sources().into_iter().find(|(a, _, _)| a.id == "ogs").unwrap().2, before);
        let j = run(&jobs, Work::FetchGame { source: "ogs".into(), game: "https://example.org/game/40".into() });
        assert_eq!(j.status, Status::Failed);
        bragd_model::key::set_in(bragd_online::session::SERVICE, &base, "").unwrap();
    }

    #[test]
    fn ogs_asks_as_the_account_at_its_pace_and_waits_out_a_429() {
        let _one = KEYCHAIN.lock().unwrap_or_else(|e| e.into_inner());
        bragd_model::key::mock();
        let base = OGS.base();
        let d = crate::tests::Dir::new("ogs-pace");
        let pages = vec![(LIST.to_string(), list(&[(20, Some("2026-07-29T10:09:45Z")), (10, Some("2026-07-01T08:00:00Z"))], None))];
        let net = site(&pages, &[10, 20]);
        net.busy.lock().unwrap().push(sgf_url(&base, 10));
        let mut memo = crate::sources::Memo::default();
        let o = Ogs.about().defaults();
        let fetched = {
            let _token = Token::set(&base);
            Ogs.fetch(&mut Cx { net: &net, dir: &d.0, cap: crate::sources::MAX_DOWNLOAD, memo: &mut memo, options: &o, stop: &|| false, progress: &mut |_| {} }).unwrap()
        };
        assert_eq!(fetched, [d.0.join("ogs-10.sgf"), d.0.join("ogs-20.sgf")], "oldest first");
        let asked = net.asked.lock().unwrap().clone();
        assert!(asked.iter().all(|a| a.starts_with("API ") && a.ends_with(" tok")), "{asked:?}");
        assert_eq!(asked.len(), 4, "the list, Game 10 twice (once refused), Game 20");
        assert_eq!(*net.paused.lock().unwrap(), [PACE, 60, PACE], "a pause before each request but the first, and the minute a 429 asks for");
        // Over the cap: nothing downloaded, the size said first.
        let mut memo = crate::sources::Memo::default();
        let _token = Token::set(&base);
        let e = Ogs.fetch(&mut Cx { net: &net, dir: &d.0, cap: SGF_BYTES, memo: &mut memo, options: &o, stop: &|| false, progress: &mut |_| {} }).unwrap_err();
        assert_eq!(TooBig::of(&e).and_then(|t| t.size), Some(2 * SGF_BYTES));
    }

    /// One test at a time signs in to OGS in the keychain all tests share.
    static KEYCHAIN: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// The test account's token in the keychain while it lives.
    struct Token(String);

    impl Token {
        fn set(base: &str) -> Token {
            bragd_model::key::set_in(bragd_online::session::SERVICE, base, "tok").unwrap();
            Token(base.into())
        }
    }

    impl Drop for Token {
        fn drop(&mut self) {
            let _ = bragd_model::key::set_in(bragd_online::session::SERVICE, &self.0, "");
        }
    }
}
