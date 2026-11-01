//! engine-options.toml: today's form is the one committed for its version, and every committed
//! version reads with today's code and keeps its values (`docs/reference/file-formats.md`).

use std::path::PathBuf;

use bragd_core::format;
use bragd_engines::options::{FORMAT, OptionStore};

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bragd-engines-formats-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn store() -> OptionStore {
    let mut s = OptionStore::default();
    s.0.insert("Stockfish".into(), toml::from_str("Threads = 4\nHash = 256\nUCI_ShowWDL = true\n").unwrap());
    s.0.insert("KataGo".into(), toml::from_str("rules = \"japanese\"\n").unwrap());
    s
}

#[test]
fn the_current_format_is_the_one_committed() {
    let p = dir("current").join("engine-options.toml");
    store().save(&p).unwrap();
    let text = std::fs::read_to_string(&p).unwrap();
    format::snapshot("engine-options", FORMAT.version(), "engine-options.toml", &text);
    format::snapshot("engine-options", FORMAT.version(), "shape.txt", &format::shape(&serde_json::to_value(text.parse::<toml::Table>().unwrap()).unwrap()));
}

#[test]
fn every_committed_version_keeps_its_values() {
    let all = format::versions("engine-options");
    assert_eq!(all.last().map(|v| v.0), Some(FORMAT.version()), "the current version has no fixture");
    for (v, fixture) in all {
        assert_eq!(OptionStore::load(&fixture.join("engine-options.toml")).unwrap(), store(), "v{v}");
    }
}

#[test]
fn a_store_from_a_newer_bragd_is_refused() {
    let p = dir("newer").join("engine-options.toml");
    std::fs::write(&p, format!("format = {}\n", FORMAT.version() + 1)).unwrap();
    assert!(OptionStore::load(&p).is_err());
}
