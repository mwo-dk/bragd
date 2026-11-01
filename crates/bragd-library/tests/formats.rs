//! The Library format: today's schema is the one committed for its version, and every committed
//! version opens with today's code and keeps its Games (`docs/reference/file-formats.md`).

use std::path::PathBuf;

use bragd_core::format;
use bragd_library::chess::pgn;
use bragd_library::library::{FORMAT, Library, NewGame, Note};
use bragd_library::slice::Query;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bragd-library-formats-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const ANNA: &str = "[Event \"Club\"]\n[White \"Anna Berg\"]\n[Black \"Carl Dahl\"]\n[WhiteElo \"2010\"]\n[Date \"1999.04.01\"]\n[Result \"1-0\"]\n\n{ Opening } 1. e4 e5 2. Nf3 $1 { The usual. } Nc6 3. Bb5 1-0\n";
const DORA: &str = "[White \"Dora Eng\"]\n[Black \"Anna Berg\"]\n[Result \"0-1\"]\n\n1. d4 d5 2. c4 0-1\n";

fn players(lib: &Library) -> Vec<(String, u32)> {
    lib.players("anna", 10).unwrap()
}

#[test]
fn the_current_format_is_the_one_committed() {
    let p = dir("current").join("lib.bragd");
    let mut lib = Library::open(&p).unwrap();
    lib.add(&NewGame::from_chess(&pgn::parse(ANNA)), "mine", "a.pgn#1").unwrap();
    lib.add(&NewGame::from_chess(&pgn::parse(ANNA)), "twic", "twic1500.pgn#7").unwrap();
    lib.add(&NewGame::from_chess(&pgn::parse(DORA)), "mine", "a.pgn#2").unwrap();
    lib.save_slice("Anna", &Query { players: vec!["Anna".into()], ..Query::default() }).unwrap();
    drop(lib);
    let c = rusqlite::Connection::open(&p).unwrap();
    // When a Source was read is the clock's: the fixture's is 0.
    c.execute("UPDATE game_sources SET fetched_at = 0", []).unwrap();
    format::snapshot("library", FORMAT.version(), "schema.sql", &format::sqlite_schema(&c).unwrap());
    format::snapshot("library", FORMAT.version(), "data.sql", &format::sqlite_data(&c).unwrap());
}

#[test]
fn every_committed_version_opens_with_its_games() {
    let all = format::versions("library");
    assert_eq!(all.last().map(|v| v.0), Some(FORMAT.version()), "the current version has no fixture");
    for (v, fixture) in all {
        let p = dir(&format!("v{v}")).join("lib.bragd");
        format::sqlite_fixture(&fixture, v, &p).unwrap();
        let lib = Library::open(&p).unwrap();
        assert_eq!(lib.count().unwrap(), 2, "v{v}");
        let anna = lib.stored(1).unwrap();
        assert_eq!((anna.moves.join(" "), anna.result.as_str()), ("e4 e5 Nf3 Nc6 Bb5".into(), "1-0"), "v{v}");
        assert_eq!(anna.headers["White"], "Anna Berg", "v{v}");
        assert_eq!(anna.notes, [Note { ply: 0, comment: "Opening".into(), nags: vec![] }, Note { ply: 3, comment: "The usual.".into(), nags: vec![1] }], "v{v}");
        assert_eq!(lib.sources(1).unwrap(), [("mine".into(), "a.pgn#1".into(), 0), ("twic".into(), "twic1500.pgn#7".into(), 0)], "v{v}");
        assert_eq!(lib.slice_games("Anna").unwrap(), [1, 2], "v{v}");
        assert_eq!(lib.find(&Query { date_from: Some("1999".into()), ..Query::default() }).unwrap(), [1], "v{v}");
        assert_eq!(players(&lib), [("Anna Berg".into(), 2)], "v{v}");
    }
}
