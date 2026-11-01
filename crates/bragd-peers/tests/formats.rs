//! The Peers folder: `peers.json`, and `key.pk8` (PKCS#8, a standard of its own). Today's form
//! of `peers.json` is the one committed for its version, and every committed version opens with
//! today's code and keeps its Peers and key (`docs/reference/file-formats.md`).

use std::path::PathBuf;

use bragd_core::format;
use bragd_peers::node::FORMAT;
use bragd_peers::{Node, Peer};

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bragd-peers-formats-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn peers() -> Vec<Peer> {
    vec![
        Peer { key: "11".repeat(32), name: "study".into(), addr: "study.local:7385".into(), paired: 1_800_000_000, seen: Some(1_800_000_100), version: Some("1.0.0".into()) },
        Peer { key: "22".repeat(32), name: "laptop".into(), addr: "100.64.0.2:7385".into(), paired: 1_800_000_200, seen: None, version: None },
    ]
}

#[test]
fn the_current_format_is_the_one_committed() {
    let text = serde_json::to_string_pretty(&peers()).unwrap();
    format::snapshot("peers", FORMAT.version(), "peers.json", &text);
    format::snapshot("peers", FORMAT.version(), "shape.txt", &format::shape(&serde_json::from_str(&text).unwrap()));
    // A made-up machine key, and its id: made once, when the version's fixture is written.
    let key = format::fixtures("peers").join(format!("v{}", FORMAT.version()));
    if !key.join("key.pk8").exists() && std::env::var_os("BRAGD_WRITE_FIXTURES").is_some() {
        let me = bragd_peers::keys::Identity::load_or_make(&key).unwrap();
        std::fs::write(key.join("id.txt"), me.id() + "\n").unwrap();
    }
}

#[test]
fn every_committed_version_opens_with_its_key_and_peers() {
    let all = format::versions("peers");
    assert_eq!(all.last().map(|v| v.0), Some(FORMAT.version()), "the current version has no fixture");
    for (v, fixture) in all {
        let d = dir(&format!("v{v}"));
        for f in ["peers.json", "key.pk8"] {
            std::fs::copy(fixture.join(f), d.join(f)).unwrap();
        }
        let node = Node::open(&d, "desk", "1.0.0").unwrap();
        assert_eq!(node.id(), std::fs::read_to_string(fixture.join("id.txt")).unwrap().trim(), "v{v}");
        assert_eq!(node.peers(), peers(), "v{v}");
    }
}

#[test]
fn peers_from_a_newer_bragd_are_refused_and_left_as_they_are() {
    let d = dir("newer");
    let newer = format!(r#"{{"format":{},"peers":[]}}"#, FORMAT.version() + 1);
    std::fs::write(d.join("peers.json"), &newer).unwrap();
    assert!(Node::open(&d, "desk", "1.0.0").is_err());
    assert_eq!(std::fs::read_to_string(d.join("peers.json")).unwrap(), newer);
}
