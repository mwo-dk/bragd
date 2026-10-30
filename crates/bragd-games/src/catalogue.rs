//! The nine Game families and their 25 Variants, as data.

use serde::{Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Family {
    Chess,
    Xiangqi,
    Shogi,
    Go,
    Draughts,
    Tafl,
    Backgammon,
    Ur,
    Senet,
}

impl Family {
    pub const ALL: [Family; 9] = [
        Family::Chess,
        Family::Xiangqi,
        Family::Shogi,
        Family::Go,
        Family::Draughts,
        Family::Tafl,
        Family::Backgammon,
        Family::Ur,
        Family::Senet,
    ];

    /// The stable key, also the prefix of every sprite id the Family draws.
    pub fn key(self) -> &'static str {
        match self {
            Family::Chess => "chess",
            Family::Xiangqi => "xiangqi",
            Family::Shogi => "shogi",
            Family::Go => "go",
            Family::Draughts => "draughts",
            Family::Tafl => "tafl",
            Family::Backgammon => "backgammon",
            Family::Ur => "ur",
            Family::Senet => "senet",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Family::Chess => "Chess",
            Family::Xiangqi => "Xiangqi",
            Family::Shogi => "Shogi",
            Family::Go => "Go",
            Family::Draughts => "Draughts",
            Family::Tafl => "Tafl",
            Family::Backgammon => "Backgammon",
            Family::Ur => "Ur",
            Family::Senet => "Senet",
        }
    }

    pub fn variants(self) -> impl Iterator<Item = &'static VariantInfo> {
        VARIANTS.iter().filter(move |info| info.family == self)
    }

    /// The Family `key`.
    pub fn from_key(key: &str) -> Option<Family> {
        Family::ALL.into_iter().find(|f| f.key() == key)
    }

    /// Whether an outside Engine plays it; tafl, Ur and senet have only built-in Bots.
    pub fn has_engine(self) -> bool {
        !matches!(self, Family::Tafl | Family::Ur | Family::Senet)
    }
}

/// How the board is built, which decides how a front end draws it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Geometry {
    /// Pieces stand inside squares: chess, shogi, draughts, tafl.
    #[default]
    Squares,
    /// Pieces stand on the crossings of lines: xiangqi, janggi, Go.
    Intersections,
    /// Twenty-four points, a bar and two trays: backgammon.
    Points,
    /// A path of squares raced along: Ur and senet.
    Track,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Variant {
    Chess,
    Chess960,
    Shatranj,
    Chaturanga,
    KingOfTheHill,
    RacingKings,
    Seirawan,
    Capablanca,
    Xiangqi,
    Janggi,
    Shogi,
    Go9,
    Go13,
    Go19,
    Checkers,
    International,
    Brazilian,
    Russian,
    Hnefatafl,
    Fetlar,
    Tablut,
    Brandubh,
    Backgammon,
    Ur,
    Senet,
}

impl Serialize for Variant {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.key())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantInfo {
    #[serde(skip)]
    pub variant: Variant,
    /// Stable: stored with every Game.
    pub key: &'static str,
    pub name: &'static str,
    pub family: Family,
    /// The two Sides, the first one first: White and Black, Attackers and Defenders.
    pub sides: [&'static str; 2],
    pub geometry: Geometry,
    pub files: u8,
    pub ranks: u8,
    /// Whether the squares alternate dark and light.
    pub chequered: bool,
}

#[allow(clippy::too_many_arguments, reason = "one argument per catalogue column keeps each row on one line")]
const fn row(
    variant: Variant,
    key: &'static str,
    name: &'static str,
    family: Family,
    sides: [&'static str; 2],
    geometry: Geometry,
    files: u8,
    ranks: u8,
    chequered: bool,
) -> VariantInfo {
    VariantInfo {
        variant,
        key,
        name,
        family,
        sides,
        geometry,
        files,
        ranks,
        chequered,
    }
}

const WB: [&str; 2] = ["White", "Black"];
const TAFL: [&str; 2] = ["Attackers", "Defenders"];
const GO: [&str; 2] = ["Black", "White"];
use Geometry::{Intersections, Points, Squares, Track};

/// Every Variant, Family by Family, in the order the spec lists them.
pub const VARIANTS: [VariantInfo; 25] = [
    row(Variant::Chess, "chess", "Chess", Family::Chess, WB, Squares, 8, 8, true),
    row(Variant::Chess960, "chess960", "Chess960", Family::Chess, WB, Squares, 8, 8, true),
    row(Variant::Shatranj, "shatranj", "Shatranj", Family::Chess, WB, Squares, 8, 8, true),
    row(Variant::Chaturanga, "chaturanga", "Chaturanga", Family::Chess, WB, Squares, 8, 8, true),
    row(Variant::KingOfTheHill, "kingofthehill", "King of the Hill", Family::Chess, WB, Squares, 8, 8, true),
    row(Variant::RacingKings, "racingkings", "Racing Kings", Family::Chess, WB, Squares, 8, 8, true),
    row(Variant::Seirawan, "seirawan", "Seirawan chess", Family::Chess, WB, Squares, 8, 8, true),
    row(Variant::Capablanca, "capablanca", "Capablanca chess", Family::Chess, WB, Squares, 10, 8, true),
    row(Variant::Xiangqi, "xiangqi", "Xiangqi", Family::Xiangqi, ["Red", "Black"], Intersections, 9, 10, false),
    row(Variant::Janggi, "janggi", "Janggi", Family::Xiangqi, ["Cho", "Han"], Intersections, 9, 10, false),
    row(Variant::Shogi, "shogi", "Shogi", Family::Shogi, ["Sente", "Gote"], Squares, 9, 9, false),
    row(Variant::Go9, "go-9", "Go 9×9", Family::Go, GO, Intersections, 9, 9, false),
    row(Variant::Go13, "go-13", "Go 13×13", Family::Go, GO, Intersections, 13, 13, false),
    row(Variant::Go19, "go-19", "Go 19×19", Family::Go, GO, Intersections, 19, 19, false),
    row(Variant::Checkers, "checkers", "English checkers", Family::Draughts, WB, Squares, 8, 8, true),
    row(Variant::International, "draughts", "International draughts", Family::Draughts, WB, Squares, 10, 10, true),
    row(Variant::Brazilian, "draughts-brazilian", "Brazilian draughts", Family::Draughts, WB, Squares, 8, 8, true),
    row(Variant::Russian, "draughts-russian", "Russian draughts", Family::Draughts, WB, Squares, 8, 8, true),
    row(Variant::Hnefatafl, "hnefatafl", "Hnefatafl (Copenhagen)", Family::Tafl, TAFL, Squares, 11, 11, false),
    row(Variant::Fetlar, "hnefatafl-fetlar", "Hnefatafl (Fetlar)", Family::Tafl, TAFL, Squares, 11, 11, false),
    row(Variant::Tablut, "tablut", "Tablut", Family::Tafl, TAFL, Squares, 9, 9, false),
    row(Variant::Brandubh, "brandubh", "Brandubh", Family::Tafl, TAFL, Squares, 7, 7, false),
    row(Variant::Backgammon, "backgammon", "Backgammon", Family::Backgammon, WB, Points, 12, 2, false),
    row(Variant::Ur, "ur", "The Royal Game of Ur", Family::Ur, ["Light", "Dark"], Track, 8, 3, false),
    row(Variant::Senet, "senet", "Senet", Family::Senet, ["Evens", "Odds"], Track, 10, 3, false),
];

impl Variant {
    pub fn from_key(key: &str) -> Option<Variant> {
        VARIANTS
            .iter()
            .find(|info| info.key == key)
            .map(|info| info.variant)
    }

    pub fn info(self) -> &'static VariantInfo {
        VARIANTS
            .iter()
            .find(|info| info.variant == self)
            .expect("every Variant has a catalogue row")
    }

    pub fn key(self) -> &'static str {
        self.info().key
    }

    pub fn family(self) -> Family {
        self.info().family
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_has_one_row_and_its_key_finds_it() {
        for info in &VARIANTS {
            assert_eq!(Variant::from_key(info.key), Some(info.variant));
            assert_eq!(info.variant.info().key, info.key);
        }
        let mut keys: Vec<_> = VARIANTS.iter().map(|info| info.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), 25);
    }

    #[test]
    fn every_family_has_a_variant() {
        let counts: Vec<usize> = Family::ALL.iter().map(|f| f.variants().count()).collect();
        assert_eq!(counts, [8, 2, 1, 3, 4, 4, 1, 1, 1]);
    }
}
