//! An Ur solved table's format (`bragd_engines::ur_table`): its header and the place of each
//! position in it, which is `rules_ur::solve::Space::rank`. Today's are the ones committed for
//! its version, so a change to either needs a new version (`docs/reference/file-formats.md`).

use bragd_core::format;
use bragd_engines::ur_table::{self, VERSION};
use rules_ur::solve::Space;
use rules_ur::{ALL, Game, Needed, Side, Throw};

/// Each rule set's header, then some positions of a Game played by the Bot and their places.
fn snapshot() -> String {
    let mut out = String::new();
    for rules in ALL {
        let space = Space::new(rules);
        let hex: String = ur_table::header(rules, space.len()).iter().map(|b| format!("{b:02x}")).collect();
        out += &format!("{} header {hex}\n", rules.name);
        let mut game = Game::new(rules, Side::White);
        let mut seed: u32 = 0x1357_9bdf;
        for _ in 0..120 {
            if let (Needed::Throw { .. }, Some(index)) = (game.needs(), space.index(game.position())) {
                out += &format!("{} {} {index}\n", rules.name, game.position().to_picture());
            }
            match game.needs() {
                Needed::Throw { .. } => {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    let marks = (seed & ((1 << rules.dice) - 1)).count_ones() as u8;
                    game.throw(Throw::new(marks).unwrap()).unwrap();
                }
                Needed::Move { .. } => {
                    let chosen = rules_ur::bot::choose(game.position(), game.pending_throw().unwrap()).unwrap();
                    game.play(chosen).unwrap();
                }
                Needed::Nothing => break,
            }
        }
    }
    out
}

#[test]
fn the_current_format_is_the_one_committed() {
    format::snapshot("ur-table", VERSION as i64, "shape.txt", &snapshot());
}
