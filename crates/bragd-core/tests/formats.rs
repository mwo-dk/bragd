//! config.toml and state.json: today's shape is the one committed for each version, and every
//! committed version reads with today's code and keeps its values
//! (`docs/reference/file-formats.md`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use bragd_core::config::{self, Config};
use bragd_core::format;
use bragd_core::state::{self, AppState};

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bragd-core-formats-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn read(dir: &std::path::Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap()
}

#[test]
fn the_current_config_format_is_the_one_committed() {
    let v = config::FORMAT.version();
    let (_, current) = format::versions("config").pop().expect("config has no fixture");
    let text = read(&current, "config.toml");
    assert_eq!(config::FORMAT.upgrade(&text).unwrap(), text, "the fixture is of the current version");
    let c = Config::parse(&text).unwrap();
    format::snapshot("config", v, "shape.txt", &format::shape(&serde_json::to_value(&c).unwrap()));
}

#[test]
fn every_committed_config_version_keeps_its_settings() {
    let all = format::versions("config");
    assert_eq!(all.last().map(|v| v.0), Some(config::FORMAT.version()), "the current version has no fixture");
    for (v, fixture) in all {
        let p = dir(&format!("config-v{v}")).join("config.toml");
        std::fs::copy(fixture.join("config.toml"), &p).unwrap();
        // Read as Bragd reads it, then saved once, so its migration runs as it would.
        let c = Config::save_value_at(&p, &["look", "look"], "".into()).unwrap();
        assert!(c.refused.is_empty(), "v{v}: {:?}", c.refused);
        assert_eq!(c.language, "da", "v{v}");
        assert_eq!((c.look.theme.as_str(), c.look.board_theme.as_str()), ("Night", "walnut"), "v{v}");
        assert_eq!(c.families.family.get("go"), Some(&false), "v{v}");
        assert_eq!(c.families.variant.get("checkers"), Some(&false), "v{v}");
        assert!(c.engines.containers, "v{v}");
        assert_eq!(c.engines.paths["stockfish"], "/opt/stockfish/stockfish", "v{v}");
        assert_eq!(c.engines.default["chess"], "stockfish", "v{v}");
        assert_eq!((c.disk.total, c.disk.items["library:main.bragd"]), (50_000_000_000, 20_000_000_000), "v{v}");
        assert_eq!(c.themes["Night"].bg, "#101418", "v{v}");
        let saved = std::fs::read_to_string(&p).unwrap();
        assert!(saved.starts_with(&format!("format = {}\n# Bragd's settings", config::FORMAT.version())), "v{v}: {saved}");
    }
}

#[test]
fn a_config_from_a_newer_bragd_is_refused_and_never_saved_over() {
    let p = dir("config-newer").join("config.toml");
    let newer = format!("format = {}\nlanguage = \"da\"\n", config::FORMAT.version() + 1);
    std::fs::write(&p, &newer).unwrap();
    assert!(Config::save_value_at(&p, &["language"], "sv".into()).is_err());
    assert!(Config::remove_value_at(&p, &["language"]).is_err());
    assert_eq!(std::fs::read_to_string(&p).unwrap(), newer);
}

fn state() -> AppState {
    AppState {
        guide_seen: true,
        seen_version: "1.0.0".into(),
        update_checked: 1_800_000_000,
        latest_version: "1.0.1".into(),
        latest_from: "github".into(),
        images_used: BTreeMap::from([("ghcr.io/example/stockfish:17".into(), 1_800_000_001)]),
        images_checked: 1_800_000_002,
        hints_shown: BTreeMap::from([("sidebar".into(), 2)]),
        notices_dismissed: vec!["containers-off".into()],
        feature_uses: BTreeMap::from([("analyse".into(), 5)]),
        tip_shown: 1_800_000_003,
        tip_last: "analyse".into(),
        tips_silenced: vec!["clubs".into()],
        upgraded: 1_800_000_004,
        upgraded_from: "0.9.0".into(),
        clubs: vec!["/home/anna/Skakklubben K.bragd-club".into()],
        unreadable: false,
    }
}

#[test]
fn the_current_state_format_is_the_one_committed() {
    let p = dir("state-current").join("state.json");
    state().save_to(&p).unwrap();
    let text = std::fs::read_to_string(&p).unwrap();
    format::snapshot("state", state::FORMAT.version(), "state.json", &text);
    format::snapshot("state", state::FORMAT.version(), "shape.txt", &format::shape(&serde_json::from_str(&text).unwrap()));
}

#[test]
fn every_committed_state_version_keeps_its_values() {
    let all = format::versions("state");
    assert_eq!(all.last().map(|v| v.0), Some(state::FORMAT.version()), "the current version has no fixture");
    for (v, fixture) in all {
        // A copy: a file that does not read is moved aside.
        let p = dir(&format!("state-v{v}")).join("state.json");
        std::fs::copy(fixture.join("state.json"), &p).unwrap();
        assert_eq!(AppState::load_from(&p), state(), "v{v}");
    }
}
