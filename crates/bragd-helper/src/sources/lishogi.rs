//! Lishogi: the user's own shogi Games on lishogi.org, through the account Bragd signed in to
//! Lishogi with for online play. One request lists every Game the account played since the
//! last fetch, newest first, each with its KIF; the shogi Games among them are kept as one zip,
//! a KIF file a Game (a KIF file holds one Game), and imported from it. Lishogi sends them at its own pace, and a 429 waits the full minute
//! its API asks for. One Game can also be fetched by its address or its id.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use bragd_online::server::LISHOGI;

use super::{About, Account, Cx, Kind, Schedule, Source};
use crate::net::{self, Fetcher, Got};

pub struct Lishogi;

/// Seconds between two requests of one fetch: a fetch asks twice, the account and its Games.
const PACE: u64 = 1;
/// About what one Game's KIF takes.
const KIF_BYTES: u64 = 2_000;
/// The longest line read: one Game's JSON with its KIF.
const MAX_LINE: u64 = 4 << 20;

/// The Game's id in `game`: the eight letters and digits themselves, or a Lishogi address
/// that starts with them (`https://lishogi.org/KYFlsX8h`, `…/KYFlsX8hgote`).
fn game_id(game: &str, base: &str) -> Option<String> {
    let g = game.trim().trim_end_matches('/');
    if g.contains("://") && !(g.starts_with(base) || g.starts_with(LISHOGI.url)) {
        return None;
    }
    let last = g.rsplit('/').next()?;
    let id = last.get(..8)?;
    // After the id, a player's view of it: `sente` or `gote`.
    (id.chars().all(|c| c.is_ascii_alphanumeric()) && ["", "sente", "gote"].contains(&&last[8..])).then(|| id.to_string())
}

/// The KIF of a Game as Lishogi's JSON has it, when it is one of standard shogi: Bragd plays
/// no other of Lishogi's Variants.
fn kif(g: &serde_json::Value) -> Option<&str> {
    (g["variant"].as_str() == Some("standard")).then(|| g["notation"].as_str()).flatten()
}

impl Source for Lishogi {
    fn about(&self) -> About {
        About {
            id: "lishogi".into(),
            name: "Lishogi".into(),
            family: "shogi".into(),
            // A thousand Games the first time; later fetches take only those since.
            size: 1000 * KIF_BYTES,
            schedule: Schedule::Daily,
            url: format!("{}/", LISHOGI.url),
            fetchable: true,
            kind: Kind::Games,
            parts: vec![],
            months: false,
            first_only: true,
            account: LISHOGI.id.into(),
        }
    }

    fn fetch(&self, cx: &mut Cx) -> io::Result<Vec<PathBuf>> {
        let mut site = Account::signed_in(cx.net, &LISHOGI, PACE)?;
        let base = LISHOGI.base();
        let me = super::page(&site, &format!("{base}/api/account"))?.ok_or_else(|| io::Error::other(format!("{base}/api/account is not there")))?;
        let me: serde_json::Value = serde_json::from_str(&me).map_err(|e| io::Error::other(format!("{base}/api/account: {e}")))?;
        let name = me["username"].as_str().filter(|n| n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))).ok_or_else(|| io::Error::other(format!("{base}/api/account: no username")))?;
        // `last_issue`: when the newest Game fetched was created, in seconds since 1970;
        // Lishogi counts `since` in milliseconds, from a Game's creation.
        let since = cx.memo.last_issue.map_or(String::new(), |t| format!("&since={}", u64::from(t) * 1000));
        let url = format!("{base}/api/games/user/{name}?notationInJson=true&clocks=false&evals=false{since}");
        site.accept = "application/x-ndjson";
        let body = match site.get(&url, 0)? {
            Got::Missing => return Err(io::Error::other(format!("{url} is not there"))),
            Got::Body { body, .. } => body,
        };
        let stamp = cx.memo.last_issue.unwrap_or(0);
        let to = cx.dir.join(format!("lishogi-{name}-{stamp}.zip"));
        let part = net::part_of(&to);
        let mut out = zip::ZipWriter::new(io::BufWriter::new(std::fs::File::create(&part)?));
        let (mut newest, mut written) = (cx.memo.last_issue, 0u64);
        let mut lines = io::BufReader::new(body);
        let mut line = String::new();
        loop {
            if (cx.stop)() {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "stopped"));
            }
            line.clear();
            if io::Read::take(&mut lines, MAX_LINE).read_line(&mut line)? == 0 {
                break;
            }
            let Ok(g) = serde_json::from_str::<serde_json::Value>(line.trim()) else { continue };
            let created = g["createdAt"].as_u64().map(|ms| u32::try_from(ms / 1000).unwrap_or(u32::MAX));
            newest = newest.max(created);
            let (Some(kif), Some(id)) = (kif(&g), g["id"].as_str().filter(|id| id.len() == 8 && id.chars().all(|c| c.is_ascii_alphanumeric()))) else { continue };
            written += kif.len() as u64;
            if written > cx.cap {
                drop(out);
                let _ = std::fs::remove_file(&part);
                return Err(io::Error::other(net::TooBig { url, size: None, cap: cx.cap }));
            }
            out.start_file(format!("{id}.kif"), zip::write::SimpleFileOptions::default()).map_err(io::Error::other)?;
            out.write_all(kif.as_bytes())?;
            (cx.progress)(written);
        }
        out.finish().map_err(io::Error::other)?.flush()?;
        if written == 0 {
            let _ = std::fs::remove_file(&part);
            cx.memo.last_issue = newest;
            return Ok(vec![]);
        }
        std::fs::rename(&part, &to)?;
        // The next fetch asks from the second after: a Game made in the same second as the
        // newest here comes again, and is stored once.
        cx.memo.last_issue = newest;
        Ok(vec![to])
    }

    fn one(&self, cx: &mut Cx, game: &str) -> io::Result<PathBuf> {
        let base = LISHOGI.base();
        let id = game_id(game, &base).ok_or_else(|| io::Error::other(bragd_core::t!("source.not_a_game", "game" => game.trim(), "server" => LISHOGI.name)))?;
        let url = format!("{base}/game/export/{id}?notationInJson=true&clocks=false&evals=false");
        let text = match Account::signed_in(cx.net, &LISHOGI, PACE) {
            Ok(mut site) => {
                site.accept = "application/json";
                super::page(&site, &url)?
            }
            // A finished Game is public: the account is asked for only when Bragd is signed in.
            Err(_) => super::page(cx.net, &url)?,
        }
        .ok_or_else(|| io::Error::other(format!("{url} is not there")))?;
        let g: serde_json::Value = serde_json::from_str(&text).map_err(|e| io::Error::other(format!("{url}: {e}")))?;
        let kif = kif(&g).ok_or_else(|| io::Error::other(bragd_core::t!("source.not_shogi", "game" => id)))?;
        let to = cx.dir.join(format!("lishogi-{id}.kif"));
        std::fs::write(&to, kif)?;
        Ok(to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::{Status, Work, tests::jobs_in};
    use crate::net::tests::Recorded;
    use crate::sources::tests::KEYCHAIN;
    use bragd_library::library::Library;
    use std::sync::Arc;

    /// Two Games as Lishogi's API sent them (recorded from lishogi.org, the players renamed),
    /// newest first.
    const GAMES: &str = include_str!("../../tests/fixtures/lishogi-games.ndjson");

    /// The recorded Games, and a minishogi Game Bragd does not play, as one answer.
    fn answer() -> String {
        let mini = serde_json::json!({ "id": "MiniMini", "variant": "minishogi", "createdAt": 1_794_309_000_000u64, "notation": "手合割：5五将棋\n" });
        format!("{GAMES}{mini}\n")
    }

    fn site(since: Option<u64>, body: &str) -> Recorded {
        let base = LISHOGI.base();
        let since = since.map_or(String::new(), |t| format!("&since={t}"));
        let files = [
            (format!("{base}/api/account"), br#"{"id":"tester","username":"tester"}"#.to_vec()),
            (format!("{base}/api/games/user/tester?notationInJson=true&clocks=false&evals=false{since}"), body.as_bytes().to_vec()),
        ];
        Recorded { files: files.into(), ..Default::default() }
    }

    #[test]
    fn a_game_is_named_by_its_id_or_its_address() {
        let base = "https://lishogi.org";
        assert_eq!(game_id("KYFlsX8h", base).as_deref(), Some("KYFlsX8h"));
        assert_eq!(game_id("https://lishogi.org/KYFlsX8h/", base).as_deref(), Some("KYFlsX8h"));
        assert_eq!(game_id("https://lishogi.org/KYFlsX8hgote", base).as_deref(), Some("KYFlsX8h"), "a player's view of it");
        assert_eq!(game_id("https://example.org/KYFlsX8h", base), None, "not a Lishogi address");
        assert_eq!(game_id("KYFls", base), None);
        assert_eq!(game_id("../../etc", base), None);
    }

    /// Signed out, then signed in: the account's Games since the last fetch, as the account,
    /// the shogi ones only, into one KIF file; the next fetch asks from the newest one's time.
    #[test]
    fn lishogi_fetches_the_accounts_games_since_the_last_fetch() {
        let _one = KEYCHAIN.lock().unwrap_or_else(|e| e.into_inner());
        bragd_model::key::mock();
        let base = LISHOGI.base();
        let d = crate::tests::Dir::new("lishogi");
        let run = |jobs: &Arc<crate::jobs::Jobs>, work: Work| {
            let id = jobs.add(work);
            assert!(jobs.run_next());
            jobs.list().into_iter().find(|j| j.id == id).unwrap()
        };
        let jobs = jobs_in(&d.0, site(None, &answer()), Arc::default());
        let mut o = Lishogi.about().defaults();
        o.enabled = true;
        jobs.set_source("lishogi", o).unwrap();
        let fetch = || Work::Fetch { source: "lishogi".into() };
        let j = run(&jobs, fetch());
        assert_eq!(j.status, Status::Failed);
        assert_eq!(j.error.as_deref(), Some(bragd_core::t!("source.sign_in", "server" => "Lishogi").as_str()));
        bragd_model::key::set_in(bragd_online::session::SERVICE, &base, "tok").unwrap();
        let net = site(None, &answer());
        let jobs = jobs_in(&d.0, net, Arc::default());
        let j = run(&jobs, fetch());
        assert_eq!((j.status, j.error.as_deref()), (Status::Done, None));
        let lib = d.0.join("lib/main.bragd");
        let count = || Library::open(&lib).unwrap().count().unwrap();
        assert_eq!(count(), 2, "the two shogi Games, not the minishogi one");
        let memo = jobs.sources().into_iter().find(|(a, _, _)| a.id == "lishogi").unwrap().2;
        assert_eq!(memo.last_issue, Some(1_794_309_360), "when the newest Game was made");
        // The next fetch asks from then; the newest Game comes again and is stored once.
        let again: String = GAMES.lines().next().unwrap().to_string() + "\n";
        let jobs = jobs_in(&d.0, site(Some(1_794_309_360_000), &again), Arc::default());
        let j = run(&jobs, fetch());
        assert_eq!((j.status, j.error.as_deref()), (Status::Done, None));
        assert_eq!(count(), 2);
        bragd_model::key::set_in(bragd_online::session::SERVICE, &base, "").unwrap();
    }

    #[test]
    fn lishogi_asks_as_the_account_for_its_games_as_json_lines() {
        let _one = KEYCHAIN.lock().unwrap_or_else(|e| e.into_inner());
        bragd_model::key::mock();
        let base = LISHOGI.base();
        bragd_model::key::set_in(bragd_online::session::SERVICE, &base, "tok").unwrap();
        let d = crate::tests::Dir::new("lishogi-ask");
        let net = site(None, &answer());
        let games = format!("{base}/api/games/user/tester?notationInJson=true&clocks=false&evals=false");
        net.busy.lock().unwrap().push(games.clone());
        let mut memo = crate::sources::Memo::default();
        let o = Lishogi.about().defaults();
        let files = Lishogi.fetch(&mut Cx { net: &net, dir: &d.0, cap: crate::sources::MAX_DOWNLOAD, memo: &mut memo, options: &o, stop: &|| false, progress: &mut |_| {} }).unwrap();
        bragd_model::key::set_in(bragd_online::session::SERVICE, &base, "").unwrap();
        assert_eq!(files.len(), 1);
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&files[0]).unwrap()).unwrap();
        assert_eq!(zip.len(), 2, "both shogi Games, not the minishogi one");
        let mut kif = String::new();
        io::Read::read_to_string(&mut zip.by_name("KYFlsX8h.kif").unwrap(), &mut kif).unwrap();
        assert!(kif.starts_with("開始日時"));
        let asked = net.asked.lock().unwrap().clone();
        assert_eq!(asked, [format!("API {base}/api/account tok"), format!("API {games} tok application/x-ndjson"), format!("API {games} tok application/x-ndjson")]);
        assert_eq!(*net.paused.lock().unwrap(), [PACE, 60], "its pace, then the minute a 429 asks for");
    }

    #[test]
    fn one_lishogi_game_by_its_address_signed_out() {
        let d = crate::tests::Dir::new("lishogi-one");
        let base = LISHOGI.base();
        let first = GAMES.lines().next().unwrap();
        let mut files = std::collections::BTreeMap::new();
        files.insert(format!("{base}/game/export/KYFlsX8h?notationInJson=true&clocks=false&evals=false"), first.as_bytes().to_vec());
        files.insert(format!("{base}/game/export/MiniMini?notationInJson=true&clocks=false&evals=false"), br#"{"id":"MiniMini","variant":"minishogi","notation":"x"}"#.to_vec());
        let net = Recorded { files, ..Default::default() };
        let mut memo = crate::sources::Memo::default();
        let o = Lishogi.about().defaults();
        let mut cx = Cx { net: &net, dir: &d.0, cap: crate::sources::MAX_DOWNLOAD, memo: &mut memo, options: &o, stop: &|| false, progress: &mut |_| {} };
        let _one = KEYCHAIN.lock().unwrap_or_else(|e| e.into_inner());
        bragd_model::key::mock();
        let f = Lishogi.one(&mut cx, &format!("{base}/KYFlsX8hsente")).unwrap();
        assert!(std::fs::read_to_string(f).unwrap().starts_with("開始日時"));
        let e = Lishogi.one(&mut cx, "MiniMini").unwrap_err();
        assert_eq!(e.to_string(), bragd_core::t!("source.not_shogi", "game" => "MiniMini"));
        assert!(Lishogi.one(&mut cx, "https://example.org/KYFlsX8h").is_err());
    }
}
