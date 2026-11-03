//! Ur's solved tables on disk: one file a rule set, `ur/<rule set>.urt` in Bragd's data folder,
//! made on this machine by the Helper (`rules_ur::solve`) and read a position at a time.
//!
//! The file, version 1, little-endian:
//!
//! | Bytes | What |
//! |---|---|
//! | 0–7 | `BRAGDURT` |
//! | 8–11 | the format version, a `u32` |
//! | 12–27 | the rule set's name (`finkel`), padded with zero bytes |
//! | 28–35 | how many positions, a `u64` |
//! | 36– | each position's chance to win for the side to throw, by `Space::rank`, as a `u16`: the chance × 65,535, rounded |
//!
//! The order of the positions is part of the format: a change to `Space::rank` is a new
//! version (`tests/fixtures/formats/ur-table`).

use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use bragd_positions::Edition;
use rules_ur::solve::{Chances, Solved, Space, Stats};
use rules_ur::Rules;

/// The key the tables show under where Engines are chosen, and their Edition's engine.
pub const KEY: &str = "ur-solved";
/// The file's format version.
pub const VERSION: u32 = 1;
const MAGIC: &[u8; 8] = b"BRAGDURT";
/// The header's length: where the chances start.
pub const HEADER: u64 = 36;

/// Where the table of `rules` lives in the data folder `data`.
pub fn path_in(data: &Path, rules: &Rules) -> PathBuf {
    data.join("ur").join(format!("{}.urt", rules.name))
}

/// Where the table of `rules` lives.
pub fn path(rules: &Rules) -> Option<PathBuf> {
    Some(path_in(&bragd_core::dirs::app_data()?, rules))
}

/// The table's size on disk, in bytes.
pub fn size(rules: &'static Rules) -> u64 {
    HEADER + 2 * Space::new(rules).len()
}

/// The memory a solve takes, in bytes: four for each position while it works.
pub fn memory(rules: &'static Rules) -> u64 {
    4 * Space::new(rules).len()
}

/// The Edition of a table: the engine is the table, its version the rule set and format.
pub fn edition(rules: &Rules) -> Edition {
    Edition { engine: KEY.into(), version: format!("{} v{VERSION}", rules.name), network: None, image: None }
}

/// The header of a table of `rules` with `positions` positions.
pub fn header(rules: &Rules, positions: u64) -> Vec<u8> {
    let mut h = Vec::with_capacity(HEADER as usize);
    h.extend_from_slice(MAGIC);
    h.extend_from_slice(&VERSION.to_le_bytes());
    let mut name = [0u8; 16];
    name[..rules.name.len()].copy_from_slice(rules.name.as_bytes());
    h.extend_from_slice(&name);
    h.extend_from_slice(&positions.to_le_bytes());
    h
}

/// A chance as the file keeps it.
pub fn quantise(chance: f64) -> u16 {
    (chance.clamp(0.0, 1.0) * 65535.0).round() as u16
}

/// A table open for reading, a position at a time.
pub struct Table {
    file: Mutex<File>,
    positions: u64,
}

impl Table {
    /// Opens the table of `rules` at `path`, refusing a file that is not one, is for another
    /// rule set, is cut short, or comes from a newer Bragd.
    pub fn open(path: &Path, rules: &'static Rules) -> io::Result<Table> {
        let bad = |why: &str| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {why}", path.display()));
        let mut file = File::open(path)?;
        let mut h = [0u8; HEADER as usize];
        file.read_exact(&mut h).map_err(|_| bad("not an Ur table"))?;
        if &h[..8] != MAGIC {
            return Err(bad("not an Ur table"));
        }
        let version = u32::from_le_bytes(h[8..12].try_into().expect("four bytes"));
        if version > VERSION {
            return Err(bad("made by a newer Bragd"));
        }
        if h[12..28] != header(rules, 0)[12..28] {
            return Err(bad("for another rule set"));
        }
        let positions = u64::from_le_bytes(h[28..36].try_into().expect("eight bytes"));
        if positions != Space::new(rules).len() || file.metadata()?.len() != HEADER + 2 * positions {
            return Err(bad("cut short or the wrong size"));
        }
        Ok(Table { file: Mutex::new(file), positions })
    }

    /// The table of `rules` in Bragd's data folder, when it is there and whole.
    pub fn installed(rules: &'static Rules) -> Option<Table> {
        Table::open(&path(rules)?, rules).ok()
    }
}

impl Chances for Table {
    fn chance(&self, index: u64) -> Option<f64> {
        if index >= self.positions {
            return None;
        }
        let mut f = self.file.lock().unwrap_or_else(|e| e.into_inner());
        let mut b = [0u8; 2];
        f.seek(SeekFrom::Start(HEADER + 2 * index)).ok()?;
        f.read_exact(&mut b).ok()?;
        Some(u16::from_le_bytes(b) as f64 / 65535.0)
    }
}

/// Writes a solved table to `path`, whole or not at all.
pub fn write(path: &Path, rules: &Rules, solved: &Solved) -> io::Result<()> {
    bragd_core::fs::replace_with(path, false, |f| {
        let mut w = BufWriter::with_capacity(1 << 20, f);
        w.write_all(&header(rules, solved.len()))?;
        for i in 0..solved.len() {
            w.write_all(&quantise(solved.get(i) as f64).to_le_bytes())?;
        }
        w.flush()
    })
}

/// Solves the table of `rules` on `threads` threads and writes it to `path`. `progress` hears
/// the positions done and in all; `stop` is asked between sweeps. Its work, counted.
pub fn compute(rules: &'static Rules, path: &Path, threads: usize, stop: &(dyn Fn() -> bool + Sync), progress: &(dyn Fn(u64, u64) + Sync)) -> Result<Stats, String> {
    let space = Space::new(rules);
    let (solved, stats) = space.solve(threads, stop, progress).ok_or("stopped")?;
    write(path, rules, &solved).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(stats)
}

/// One move of an Ur Game judged by its rule set's table. Chances are the mover's.
#[derive(Clone, Debug, PartialEq)]
pub struct Judged {
    /// Its place among the Game's entries.
    pub index: usize,
    /// The first Side made it.
    pub first: bool,
    /// The mover's chance before the throw it moved with.
    pub before_throw: f64,
    /// The best move for the throw, as an entry, and the chance it keeps.
    pub best: String,
    pub best_chance: f64,
    /// The next best move's chance, when there was another move.
    pub second: Option<f64>,
    /// The move played, and the chance it keeps.
    pub played: String,
    pub chance: f64,
}

/// Each move of an Ur Game judged by `table` (of the Variant's rule set): a throw is not a
/// move, and a throw that allows nothing has none. None when it is not an Ur Game.
pub fn judge(variant: &str, initial: Option<&str>, entries: &[String], table: &dyn Chances) -> Option<Vec<Judged>> {
    let rules = bragd_games::ur_rules(variant)?;
    let space = Space::new(rules);
    let mut game = bragd_games::ur_game::<&str>(variant, initial, &[]).ok()?;
    let mut out = vec![];
    let mut before_throw = None;
    for (index, entry) in entries.iter().enumerate() {
        let position = game.position().clone();
        match game.pending_throw() {
            None => before_throw = space.before_throw(table, &position),
            Some(throw) => {
                let moves = space.moves(table, &position, throw)?;
                let chosen = rules_ur::Entry::parse(rules, entry);
                if let (Some(rules_ur::Entry::Move(chosen)), Some(&(best, best_chance))) = (chosen, moves.first()) {
                    out.push(Judged {
                        index,
                        first: position.to_move() == rules_ur::Side::White,
                        before_throw: before_throw?,
                        best: best.to_wire(rules),
                        best_chance,
                        second: moves.get(1).map(|m| m.1),
                        played: chosen.to_wire(rules),
                        chance: space.after(table, &position, chosen)?,
                    });
                }
            }
        }
        game.play_wire(entry).ok()?;
    }
    Some(out)
}

/// A chance as shown: `51.5 %`.
pub fn shown(chance: f64) -> String {
    format!("{:.1} %", chance * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rules_ur::Throw;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bragd-ur-table-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A table of the right size for Blitz, every chance a known pattern, read back by rank.
    #[test]
    fn a_table_reads_back_what_was_written_and_refuses_the_rest() {
        let d = dir("read");
        let rules = &rules_ur::BLITZ;
        let n = Space::new(rules).len();
        let file = d.join("blitz.urt");
        let mut bytes = header(rules, n);
        bytes.extend((0..n).flat_map(|i| ((i % 65536) as u16).to_le_bytes()));
        std::fs::write(&file, &bytes).unwrap();
        let t = Table::open(&file, rules).unwrap();
        assert_eq!(t.chance(0), Some(0.0));
        assert_eq!(t.chance(65535), Some(1.0));
        assert_eq!(t.chance(n), None);
        assert!(Table::open(&file, &rules_ur::ASEB).is_err(), "another rule set");
        let mut newer = bytes.clone();
        newer[8] = (VERSION + 1) as u8;
        std::fs::write(&file, &newer).unwrap();
        assert!(Table::open(&file, rules).err().unwrap().to_string().contains("newer"));
        std::fs::write(&file, &bytes[..bytes.len() - 2]).unwrap();
        assert!(Table::open(&file, rules).is_err(), "cut short");
        assert_eq!(size(rules), bytes.len() as u64);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// A table that gives each position a chance from its rank: moves differ, and the judged
    /// chances are the ones the space reads for the same positions.
    struct Scattered;
    impl Chances for Scattered {
        fn chance(&self, index: u64) -> Option<f64> {
            Some((index.wrapping_mul(2_654_435_761) % 1000) as f64 / 1000.0)
        }
    }

    #[test]
    fn every_move_is_judged_and_no_throw_is() {
        let mut entries: Vec<String> = vec![];
        let mut moves = 0;
        for i in 0..40 {
            let st = bragd_games::replay("ur", None, &entries).unwrap();
            let next = match st.throws.first() {
                Some(_) => ["1", "2", "3", "4", "2", "0"][i % 6].to_string(),
                None => {
                    moves += 1;
                    st.moves.last().unwrap().wire.clone()
                }
            };
            entries.push(next);
        }
        let judged = judge("ur", None, &entries, &Scattered).unwrap();
        assert_eq!(judged.len(), moves);
        for j in &judged {
            assert!(Throw::parse(&entries[j.index]).is_none(), "{j:?}");
            assert_eq!(j.played, entries[j.index]);
            assert!(j.best_chance >= j.chance && j.second.is_none_or(|s| s <= j.best_chance), "{j:?}");
        }
        assert!(judged.iter().any(|j| j.best != j.played), "the last move is not always the best");
        assert!(judge("chess", None, &[], &Scattered).is_none());
    }

    #[test]
    fn chances_are_kept_to_a_65535th() {
        assert_eq!(quantise(0.0), 0);
        assert_eq!(quantise(1.0), 65535);
        assert_eq!(quantise(0.5), 32768);
        assert_eq!(quantise(-0.1), 0);
    }
}
