//! The Game families and their Variants as Settings, the sidebar and the wizard name them.
//! Their names are the `family.<id>` and `variant.<id>` texts.

/// A Game family: its id (its key in `[families.family]`), its Variants' ids (their keys in
/// `[families.variant]`), and whether an outside Engine plays it.
#[derive(Debug, PartialEq, Eq)]
pub struct Family {
    pub id: &'static str,
    pub variants: &'static [&'static str],
    pub has_engine: bool,
}

const fn family(id: &'static str, variants: &'static [&'static str], has_engine: bool) -> Family {
    Family {
        id,
        variants,
        has_engine,
    }
}

/// Every Game family, in the order the sidebar lists them.
pub const FAMILIES: &[Family] = &[
    family(
        "chess",
        &[
            "chess",
            "chess960",
            "shatranj",
            "chaturanga",
            "kingofthehill",
            "racingkings",
            "seirawan",
            "capablanca",
        ],
        true,
    ),
    family("xiangqi", &["xiangqi", "janggi"], true),
    family("shogi", &["shogi"], true),
    family("go", &["go-9", "go-13", "go-19"], true),
    family(
        "draughts",
        &[
            "checkers",
            "draughts",
            "draughts-brazilian",
            "draughts-russian",
        ],
        true,
    ),
    family(
        "tafl",
        &["hnefatafl", "hnefatafl-fetlar", "tablut", "brandubh"],
        false,
    ),
    family("backgammon", &["backgammon"], true),
    family("ur", &["ur"], false),
    family("senet", &["senet"], false),
];

/// The Family `id`.
pub fn find(id: &str) -> Option<&'static Family> {
    FAMILIES.iter().find(|f| f.id == id)
}

/// The Family a Variant belongs to.
pub fn of_variant(variant: &str) -> Option<&'static Family> {
    FAMILIES.iter().find(|f| f.variants.contains(&variant))
}

/// Every Variant id, in order.
pub fn variants() -> impl Iterator<Item = &'static str> {
    FAMILIES.iter().flat_map(|f| f.variants.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_hold_all_25_variants_once() {
        let mut all: Vec<&str> = variants().collect();
        assert_eq!(all.len(), 25);
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 25, "a Variant in two Families");
        assert_eq!(of_variant("capablanca").map(|f| f.id), Some("chess"));
        assert_eq!(of_variant("janggi").map(|f| f.id), Some("xiangqi"));
    }
}
