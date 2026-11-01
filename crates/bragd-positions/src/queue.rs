//! A job of background analysis, as the Helper's queue keeps it (`docs/spec.md`, *Position
//! store*). It carries its positions, the Engine and the oldest Edition that may answer, never
//! a Library's path, so a Peer that does not have the Slice can work on it too (0.5.0).

use serde::{Deserialize, Serialize};

use crate::{Edition, Evaluation};

/// How long the engine thinks about each position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Per {
    /// So many half-moves deep.
    Depth(u32),
    /// So many milliseconds.
    Millis(u32),
}

/// One position to analyse.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spot {
    /// Its key in its Family (`crate` docs).
    pub key: u64,
    /// The position in the Family's notation (a FEN for chess).
    pub setup: String,
    /// Moves after `setup`, as the Family's entries, for engines that take no set-up position
    /// (GTP); empty when `setup` is the position itself.
    #[serde(default)]
    pub moves: Vec<String>,
    /// How often it occurs where the job came from.
    pub count: u32,
}

/// A job: these positions, by this Engine, each to `per`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub family: String,
    pub variant: String,
    /// The Engine's key (`stockfish`), or the name of one the user added.
    pub engine: String,
    /// The oldest version that may answer, as the Engine names itself (`Stockfish 17.1`); None
    /// for any. Each result records its exact Edition.
    #[serde(default)]
    pub min_version: Option<String>,
    pub per: Per,
    /// The most frequent first (`Job::new`).
    pub spots: Vec<Spot>,
    /// From a Game the user opened: runs after the jobs asked for.
    #[serde(default)]
    pub low: bool,
    /// What it is about, for the queue's list: a Slice's name, a Game's players.
    #[serde(default)]
    pub about: String,
}

impl Job {
    /// The job, its positions the most frequent first (in their order where equally frequent).
    #[allow(clippy::too_many_arguments, reason = "one argument per field, each named where it is called")]
    pub fn new(family: &str, variant: &str, engine: &str, min_version: Option<String>, per: Per, mut spots: Vec<Spot>, low: bool, about: &str) -> Job {
        spots.sort_by_key(|s| std::cmp::Reverse(s.count));
        let min_version = min_version.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
        Job { family: family.into(), variant: variant.into(), engine: engine.into(), min_version, per, spots, low, about: about.into() }
    }

    /// Whether `e` may answer this job: its Engine, and no older than `min_version`.
    pub fn accepts(&self, e: &Edition) -> bool {
        e.engine == self.engine
            && self.min_version.as_ref().is_none_or(|m| {
                let min = Edition { engine: self.engine.clone(), version: m.clone(), network: None, image: None };
                e.newer(&min).is_some_and(|o| o.is_ge())
            })
    }

    /// Whether `e` already answers `spot` well enough: an accepted Edition, and as deep as asked
    /// (any depth for a time limit).
    pub fn answered(&self, spot: &Spot, e: &Evaluation) -> bool {
        e.family == self.family
            && e.variant == self.variant
            && e.key == spot.key
            && self.accepts(&e.edition)
            && match self.per {
                Per::Depth(d) => e.depth >= d,
                Per::Millis(_) => true,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Score;
    use crate::tests::{eval, sf};

    fn spot(key: u64, count: u32) -> Spot {
        Spot { key, setup: format!("fen {key}"), moves: vec![], count }
    }

    #[test]
    fn the_most_frequent_positions_go_first() {
        let j = Job::new("chess", "chess", "stockfish", Some(" ".into()), Per::Depth(20), vec![spot(1, 2), spot(2, 9), spot(3, 2), spot(4, 5)], false, "Slice");
        assert_eq!(j.spots.iter().map(|s| s.key).collect::<Vec<_>>(), [2, 4, 1, 3]);
        assert_eq!(j.min_version, None, "a blank minimum is none");
    }

    #[test]
    fn only_a_new_enough_edition_of_the_engine_answers() {
        let j = Job::new("chess", "chess", "stockfish", Some("Stockfish 17.1".into()), Per::Depth(20), vec![spot(7, 1)], false, "");
        assert!(j.accepts(&sf("Stockfish 17.1")));
        assert!(j.accepts(&sf("Stockfish 18")));
        assert!(!j.accepts(&sf("Stockfish 17")));
        assert!(!j.accepts(&Edition { engine: "lc0".into(), ..sf("Stockfish 18") }));
        let s = &j.spots[0].clone();
        assert!(j.answered(s, &eval(7, sf("Stockfish 18"), 20, 1, Score::Cp(0))));
        assert!(!j.answered(s, &eval(7, sf("Stockfish 18"), 19, 1, Score::Cp(0))), "too shallow");
        assert!(!j.answered(s, &eval(8, sf("Stockfish 18"), 30, 1, Score::Cp(0))), "another position");
        let timed = Job { per: Per::Millis(500), min_version: None, ..j };
        assert!(timed.answered(s, &eval(7, sf("Stockfish 9"), 1, 1, Score::Cp(0))));
    }
}
