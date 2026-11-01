//! Editions: one exact build of an Engine. Its version as the engine names itself, its
//! network where it has one, and the image digest when it runs in a container.
//! `bragd_engines::edition::of` reads one from a running engine.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Edition {
    /// The Engine's key (`stockfish`, `katago`), or the program's name for one Bragd does not know.
    pub engine: String,
    /// What the engine says it is (`Stockfish 17.1`, `GNU Go 3.8`).
    pub version: String,
    /// The SHA-256 of its network, hex, when Bragd knows which it runs.
    pub network: Option<String>,
    /// The image's digest (`sha256:…`) for an Engine run in a container.
    pub image: Option<String>,
}

impl Edition {
    /// Its id: the same on every machine for the same build, and different for another
    /// version, network or image. Stored, so its form never changes.
    pub fn id(&self) -> String {
        // 16 hex digits (64 bits) of each digest are plenty within one Engine and version.
        let short = |d: &Option<String>| d.as_deref().map(|d| d.trim_start_matches("sha256:").chars().take(16).collect::<String>()).unwrap_or_default();
        format!("{}|{}|{}|{}", self.engine, self.version, short(&self.network), short(&self.image))
    }

    /// Whether it is newer than `other`, by the numbers in their versions (`17.1` after `17`,
    /// `2026-09-06` after `2025-06-23`). None for two Engines, which are not compared. Equal
    /// for the same version with another network or image.
    pub fn newer(&self, other: &Edition) -> Option<Ordering> {
        (self.engine == other.engine).then(|| numbers(&self.version).cmp(&numbers(&other.version)))
    }
}

/// The runs of digits in a version, as numbers.
// shortcut: a hash in a dev build's name (`dev-20250101-ab12`) counts its digits too; compare
// commit dates instead if dev builds of one Engine are ever told apart this way.
fn numbers(version: &str) -> Vec<u64> {
    version.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap_or(u64::MAX)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(engine: &str, version: &str) -> Edition {
        Edition { engine: engine.into(), version: version.into(), network: None, image: None }
    }

    #[test]
    fn newer_within_one_engine() {
        assert_eq!(e("stockfish", "Stockfish 17.1").newer(&e("stockfish", "Stockfish 17")), Some(Ordering::Greater));
        assert_eq!(e("stockfish", "Stockfish 9").newer(&e("stockfish", "Stockfish 10")), Some(Ordering::Less));
        assert_eq!(e("pikafish", "Pikafish 2026-09-06").newer(&e("pikafish", "Pikafish 2025-06-23")), Some(Ordering::Greater));
        assert_eq!(e("gnubg", "GNU Backgammon 1.08.003").newer(&e("gnubg", "GNU Backgammon 1.8.3")), Some(Ordering::Equal));
        assert_eq!(e("stockfish", "Stockfish 17").newer(&e("lc0", "Lc0 0.32.1")), None);
    }

    #[test]
    fn the_id_tells_builds_apart_and_is_stable() {
        let a = Edition { engine: "lc0".into(), version: "Lc0 0.32.1".into(), network: Some("b1c7047582a8ad37620849bf328935cb46f2ccbbecb7f2696c30c6b913ed690d".into()), image: Some("sha256:d277d133443b0179b55ae7989a73bf6847648fd7648e3f41131c2821457684da".into()) };
        assert_eq!(a.id(), "lc0|Lc0 0.32.1|b1c7047582a8ad37|d277d133443b0179");
        let other_net = Edition { network: Some("aaaa".into()), ..a.clone() };
        let no_image = Edition { image: None, ..a.clone() };
        assert_ne!(a.id(), other_net.id());
        assert_ne!(a.id(), no_image.id());
        assert_eq!(a.newer(&other_net), Some(Ordering::Equal));
    }
}
