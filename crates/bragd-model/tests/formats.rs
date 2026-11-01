//! `translations.sqlite`: today's schema is the one committed for its version, and every
//! committed version opens with today's code and keeps its translations
//! (`docs/reference/file-formats.md`).

use std::path::PathBuf;

use bragd_core::format;
use bragd_model::store::{FORMAT, Translation, Translations};

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bragd-model-formats-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn kept() -> [(&'static str, &'static str, Translation); 2] {
    let t = |text: &str, time| Translation { text: text.into(), model: "small-model".into(), time };
    [("Gespielt in der Oper.", "en", t("Played at the opera.", 1_800_000_000)), ("Gespielt in der Oper.", "da", t("Spillet i operaen.", 1_800_000_001))]
}

#[test]
fn the_current_format_is_the_one_committed() {
    let p = dir("current").join("translations.sqlite");
    let s = Translations::open(&p).unwrap();
    for (original, lang, t) in kept() {
        s.put(original, lang, &t).unwrap();
    }
    drop(s);
    let c = rusqlite::Connection::open(&p).unwrap();
    format::snapshot("translations", FORMAT.version(), "schema.sql", &format::sqlite_schema(&c).unwrap());
    format::snapshot("translations", FORMAT.version(), "data.sql", &format::sqlite_data(&c).unwrap());
}

#[test]
fn every_committed_version_opens_with_its_translations() {
    let all = format::versions("translations");
    assert_eq!(all.last().map(|v| v.0), Some(FORMAT.version()), "the current version has no fixture");
    for (v, fixture) in all {
        let p = dir(&format!("v{v}")).join("translations.sqlite");
        format::sqlite_fixture(&fixture, v, &p).unwrap();
        let s = Translations::open(&p).unwrap();
        for (original, lang, t) in kept() {
            assert_eq!(s.get(original, lang).unwrap(), Some(t), "v{v}");
        }
    }
}
