//! The Club file, the rating lists file and a Tournament bundle's `tournament.json`: today's
//! schema is the one committed for each version, and every committed version opens with
//! today's code and keeps its data (`docs/reference/file-formats.md`).

use std::io::Write;
use std::path::PathBuf;

use bragd_clubs::club::{self, Club, Details};
use bragd_clubs::export;
use bragd_clubs::league::League;
use bragd_clubs::lists::{self, Player as Listed};
use bragd_clubs::rating::System as Rating;
use bragd_clubs::tournament::{Outcome, Player, Status, System, Tournament};
use bragd_core::format;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bragd-clubs-formats-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A Tournament two rounds in, its log's times 0 so it is the same each time.
fn tournament() -> Tournament {
    let mut t = Tournament::new("Autumn Swiss", System::Dutch { baku: false }, 3, "2026-11-05").unwrap();
    t.place = "Aarhus".into();
    for (n, r, id) in [("Anna Berg", 2100, "1401234"), ("Bo", 1900, ""), ("Carl", 2000, "")] {
        let p = t.register(Player { name: n.into(), rating: r, fide_id: id.into(), ..Player::default() }).unwrap();
        t.set_status("Mette", p, Status::Here).unwrap();
    }
    t.pair_next().unwrap();
    t.set_result("Mette", 0, 0, Some(Outcome::Draw), "").unwrap();
    t.pair_next().unwrap();
    t.set_result("Mette", 1, 0, Some(Outcome::WhiteForfeit), "Did not come").unwrap();
    t.log.iter_mut().for_each(|e| e.at = 0);
    t
}

fn league() -> League {
    League::new("Division 1", 2, false, "2026-11-05").unwrap()
}

fn sqlite(name: &str, dump: &rusqlite::Connection, version: i64) {
    format::snapshot(name, version, "schema.sql", &format::sqlite_schema(dump).unwrap());
    format::snapshot(name, version, "data.sql", &format::sqlite_data(dump).unwrap());
}

#[test]
fn the_current_club_format_is_the_one_committed() {
    let p = dir("club-current").join("k.bragd-club");
    let c = Club::create(&p, "Skakklubben K", Rating::Glicko2, "Mette").unwrap();
    let anna = c.add(1, &Details { name: "Anna Berg".into(), fide_id: "1401234".into(), federation: "DEN".into(), ..Details::default() }, Some(1850.0)).unwrap();
    c.set_rating(1, anna, 1900.0, "Club champion").unwrap();
    c.add_tournament(1, &tournament()).unwrap();
    c.add_league(1, &league()).unwrap();
    drop(c);
    let db = rusqlite::Connection::open(&p).unwrap();
    // When each change was made is the clock's: the fixture's is 0.
    db.execute("UPDATE log SET at = 0", []).unwrap();
    sqlite("club", &db, club::FORMAT.version());
}

#[test]
fn every_committed_club_version_opens_with_its_members_and_tournaments() {
    let all = format::versions("club");
    assert_eq!(all.last().map(|v| v.0), Some(club::FORMAT.version()), "the current version has no fixture");
    for (v, fixture) in all {
        let p = dir(&format!("club-v{v}")).join("k.bragd-club");
        format::sqlite_fixture(&fixture, v, &p).unwrap();
        let c = Club::open(&p).unwrap();
        assert_eq!(c.name().unwrap(), "Skakklubben K", "v{v}");
        assert_eq!(c.system().unwrap(), Rating::Glicko2, "v{v}");
        let members: Vec<_> = c.members().unwrap().into_iter().map(|m| (m.name, m.fide_id, m.federation, m.rating.rating.round())).collect();
        assert_eq!(members, [("Anna Berg".into(), "1401234".into(), "DEN".into(), 1900.0), ("Mette".into(), String::new(), String::new(), 1500.0)], "v{v}");
        assert_eq!(c.log().unwrap().iter().filter(|e| e.why == "Club champion").count(), 1, "v{v}");
        let listed = c.tournaments().unwrap();
        assert_eq!(listed.len(), 1, "v{v}");
        // The Club names its director as the arbiter.
        assert_eq!(c.tournament(listed[0].id).unwrap(), Tournament { arbiter: "Mette".into(), ..tournament() }, "v{v}");
        let leagues = c.leagues().unwrap();
        assert_eq!(c.league(leagues[0].id).unwrap(), league(), "v{v}");
    }
}

fn listed() -> Vec<Listed> {
    vec![
        Listed { id: "1401234".into(), name: "Berg, Anna".into(), federation: "DEN".into(), title: "WFM".into(), rating: Some(2100), rapid: Some(2050), blitz: None, born: Some(1999), ..Listed::default() },
        Listed { id: "77".into(), name: "Ørsted, Bo".into(), club: "Skakklubben K".into(), rating: Some(1900), fide_id: "1405555".into(), ..Listed::default() },
    ]
}

#[test]
fn the_current_rating_lists_format_is_the_one_committed() {
    let p = dir("lists-current").join("rating-lists.sqlite");
    lists::import(&p, "fide-std", listed().into_iter().take(1)).unwrap();
    lists::import(&p, "dsu", listed().into_iter().skip(1)).unwrap();
    let db = rusqlite::Connection::open(&p).unwrap();
    db.execute("UPDATE lists SET at = 0", []).unwrap();
    sqlite("rating-lists", &db, lists::FORMAT.version());
}

#[test]
fn every_committed_rating_lists_version_opens_with_its_players() {
    let all = format::versions("rating-lists");
    assert_eq!(all.last().map(|v| v.0), Some(lists::FORMAT.version()), "the current version has no fixture");
    for (v, fixture) in all {
        let p = dir(&format!("lists-v{v}")).join("rating-lists.sqlite");
        format::sqlite_fixture(&fixture, v, &p).unwrap();
        let imported: Vec<_> = lists::lists(&p).unwrap().into_iter().map(|l| (l.list, l.players)).collect();
        assert_eq!(imported, [("dsu".into(), 1), ("fide-std".into(), 1)], "v{v}");
        let found = lists::search(&p, "orsted").unwrap();
        assert_eq!((found.len(), found[0].list.as_str(), &found[0].player), (1, "dsu", &listed()[1]), "v{v}");
        assert_eq!(lists::search(&p, "1401234").unwrap()[0].player, listed()[0], "v{v}");
    }
}

#[test]
fn the_current_tournament_format_is_the_one_committed() {
    let mut v = serde_json::to_value(tournament()).unwrap();
    export::FORMAT.stamp(&mut v);
    format::snapshot("tournament", export::FORMAT.version(), "tournament.json", &(serde_json::to_string_pretty(&v).unwrap() + "\n"));
    format::snapshot("tournament", export::FORMAT.version(), "shape.txt", &format::shape(&v));
}

#[test]
fn every_committed_tournament_version_reads_back() {
    let all = format::versions("tournament");
    assert_eq!(all.last().map(|v| v.0), Some(export::FORMAT.version()), "the current version has no fixture");
    for (v, fixture) in all {
        let p = dir(&format!("tournament-v{v}")).join("t.bragd-tournament");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
        z.start_file("tournament.json", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(&std::fs::read(fixture.join("tournament.json")).unwrap()).unwrap();
        z.finish().unwrap();
        assert_eq!(export::read_bundle(&p).unwrap(), tournament(), "v{v}");
    }
}

#[test]
fn a_bundle_from_a_newer_bragd_is_refused() {
    let p = dir("tournament-newer").join("t.bragd-tournament");
    let mut z = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
    z.start_file("tournament.json", zip::write::SimpleFileOptions::default()).unwrap();
    let mut v = serde_json::to_value(tournament()).unwrap();
    v["format"] = (export::FORMAT.version() + 1).into();
    z.write_all(&serde_json::to_vec(&v).unwrap()).unwrap();
    z.finish().unwrap();
    assert!(matches!(export::read_bundle(&p), Err(club::Error::Io(_))));
}
