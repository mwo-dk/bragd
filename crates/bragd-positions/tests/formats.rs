//! The Position store's format: today's schema is the one committed for its version, and every
//! committed version opens with today's code and keeps its rows (`docs/reference/file-formats.md`).

use std::path::PathBuf;

use bragd_core::format;
use bragd_positions::{Annotation, Edition, Evaluation, Mark, Motif, Score, Second, Store, FORMAT};

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bragd-positions-formats-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn edition(version: &str, image: Option<&str>) -> Edition {
    Edition { engine: "stockfish".into(), version: version.into(), network: Some("ab12cd34ef56ab12cd34".into()), image: image.map(String::from) }
}

/// The rows every fixture holds, in the order the store gives them.
fn evaluations() -> Vec<Evaluation> {
    let e = |key, variant: &str, ed, score, second| Evaluation { family: "chess".into(), variant: variant.into(), key, edition: ed, score, depth: 24, line: "e2e4 e7e5".into(), time: 1_800_000_000, second };
    vec![
        e(7, "chess", edition("Stockfish 17.1", Some("sha256:00ff")), Score::Mate(-3), None),
        e(7, "chess", edition("Stockfish 17", None), Score::Cp(31), Some(Second { score: Score::Cp(-12), line: "d2d4".into() })),
        Evaluation { family: "go".into(), variant: "go-19".into(), key: u64::MAX, edition: Edition { engine: "katago".into(), version: "KataGo 1.16".into(), network: None, image: None }, score: Score::WinRate { win: 0.625, lead: 3.5 }, depth: 400, line: "Q16".into(), time: 1_800_000_001, second: None },
    ]
}

fn annotations() -> Vec<Annotation> {
    vec![Annotation { family: "chess".into(), variant: "chess".into(), key: 7, mv: "g1f3".into(), edition: edition("Stockfish 17", None), mark: Some(Mark::Blunder), better: "e2e4 e7e5".into(), motifs: vec![Motif::Fork, Motif::Hangs], depth: 20, time: 1_800_000_002 }]
}

#[test]
fn the_current_format_is_the_one_committed() {
    let p = dir("current").join("positions.sqlite");
    let mut s = Store::open(&p).unwrap();
    s.merge(&evaluations(), &annotations()).unwrap();
    drop(s);
    let c = rusqlite::Connection::open(&p).unwrap();
    format::snapshot("positions", FORMAT.version(), "schema.sql", &format::sqlite_schema(&c).unwrap());
    format::snapshot("positions", FORMAT.version(), "data.sql", &format::sqlite_data(&c).unwrap());
}

#[test]
fn every_committed_version_opens_with_its_rows() {
    let all = format::versions("positions");
    assert_eq!(all.last().map(|v| v.0), Some(FORMAT.version()), "the current version has no fixture");
    for (v, fixture) in all {
        let p = dir(&format!("v{v}")).join("positions.sqlite");
        format::sqlite_fixture(&fixture, v, &p).unwrap();
        let s = Store::open(&p).unwrap();
        assert_eq!(s.all().unwrap(), evaluations(), "v{v}");
        assert_eq!(s.annotations().unwrap(), annotations(), "v{v}");
    }
}

/// The Parquet export: its columns as Parquet writes them.
fn parquet_schema(path: &std::path::Path) -> String {
    use parquet::file::reader::FileReader;
    let r = parquet::file::reader::SerializedFileReader::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut out = Vec::new();
    parquet::schema::printer::print_schema(&mut out, r.metadata().file_metadata().schema());
    String::from_utf8(out).unwrap()
}

#[test]
fn the_current_parquet_format_is_the_one_committed() {
    let d = dir("parquet-current");
    let mut s = Store::open(&d.join("positions.sqlite")).unwrap();
    s.merge(&evaluations(), &annotations()).unwrap();
    let out = d.join("positions.parquet");
    bragd_positions::parquet::to_parquet(&s, &out).unwrap();
    let v = bragd_positions::parquet::FORMAT;
    format::snapshot("positions-parquet", v, "schema.txt", &parquet_schema(&out));
    // The file itself, made once when the version's fixture is written: its bytes name the
    // Parquet library's version, so they are not compared.
    let fixture = format::fixtures("positions-parquet").join(format!("v{v}")).join("positions.parquet");
    if !fixture.exists() && std::env::var_os("BRAGD_WRITE_FIXTURES").is_some() {
        std::fs::copy(&out, &fixture).unwrap();
    }
}

#[test]
fn every_committed_parquet_version_imports_with_its_rows() {
    let all = format::versions("positions-parquet");
    assert_eq!(all.last().map(|v| v.0), Some(bragd_positions::parquet::FORMAT), "the current version has no fixture");
    for (v, fixture) in all {
        let mut s = Store::open(&dir(&format!("parquet-v{v}")).join("positions.sqlite")).unwrap();
        bragd_positions::parquet::from_parquet(&mut s, &fixture.join("positions.parquet")).unwrap();
        assert_eq!(s.all().unwrap(), evaluations(), "v{v}");
        assert_eq!(s.annotations().unwrap(), annotations(), "v{v}");
    }
}
