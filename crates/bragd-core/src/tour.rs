//! Tour cards: after an upgrade, one card for each feature newer than the version the user last
//! saw, in the same frame as the install wizard. A card's texts are `tour.<id>.title` and
//! `tour.<id>.text`; its feature is the F1 entry of the same id.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Card {
    /// The version the feature came in.
    pub version: &'static str,
    pub id: &'static str,
}

const fn card(version: &'static str, id: &'static str) -> Card {
    Card { version, id }
}

/// Every card, oldest version first. A card may carry the next minor version, the release
/// being prepared: the version is raised only by the release (`packaging/release-pr.py`).
pub const CARDS: &[Card] = &[
    card("0.2.0", "families"),
    card("0.2.0", "themes"),
    card("0.2.0", "databases"),
    card("0.2.0", "parquet"),
    card("0.2.0", "engines"),
    card("0.2.0", "helper"),
    card("0.2.0", "disk"),
];

/// A card as the app shows it, in the current language.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct View {
    pub id: &'static str,
    pub version: &'static str,
    pub title: String,
    pub text: String,
}

pub fn view(c: &Card) -> View {
    View {
        id: c.id,
        version: c.version,
        title: crate::i18n::tr(&format!("tour.{}.title", c.id), &[]),
        text: crate::i18n::tr(&format!("tour.{}.text", c.id), &[]),
    }
}

/// What's new in the sidebar: every version, newest first, with its cards.
pub fn by_version() -> Vec<(&'static str, Vec<View>)> {
    let mut out: Vec<(&'static str, Vec<View>)> = vec![];
    for c in CARDS.iter().rev() {
        match out.last_mut() {
            Some((v, list)) if *v == c.version => list.push(view(c)),
            _ => out.push((c.version, vec![view(c)])),
        }
    }
    out
}

/// The cards newer than `seen_version`. None on a first start (no version seen): the install
/// wizard shows instead.
pub fn cards_since(seen_version: &str) -> Vec<Card> {
    since(CARDS, seen_version)
}

fn since(cards: &[Card], seen: &str) -> Vec<Card> {
    if seen.is_empty() {
        return vec![];
    }
    cards
        .iter()
        .filter(|c| crate::update::is_newer(seen, c.version))
        .copied()
        .collect()
}

/// The cards have been seen, up to this version or the newest card, whichever is later.
pub fn seen(state: &mut crate::state::AppState) {
    state.seen_version = newest(CARDS).to_string();
}

fn newest(cards: &[Card]) -> &'static str {
    cards
        .iter()
        .map(|c| c.version)
        .fold(crate::update::VERSION, |a, b| {
            if crate::update::is_newer(a, b) { b } else { a }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tour_cards_since_the_version_seen() {
        let cards = [card("0.1.0", "a"), card("0.2.0", "b"), card("0.10.0", "c")];
        let ids = |seen: &str| since(&cards, seen).iter().map(|c| c.id).collect::<Vec<_>>();
        assert_eq!(ids(""), Vec::<&str>::new(), "a first start gets the wizard");
        assert_eq!(ids("0.0.9"), ["a", "b", "c"]);
        assert_eq!(ids("0.1.0"), ["b", "c"]);
        assert_eq!(ids("0.9.0"), ["c"], "0.10 is newer than 0.9");
        assert_eq!(ids("0.10.0"), Vec::<&str>::new());
        let mut st = crate::state::AppState::default();
        seen(&mut st);
        assert!(cards_since(&st.seen_version).is_empty());
        assert_eq!(newest(&cards), "0.10.0");
        assert_eq!(newest(&[]), crate::update::VERSION);
        let all: usize = by_version().iter().map(|(_, l)| l.len()).sum();
        assert_eq!(all, CARDS.len());
    }

    #[test]
    fn tour_every_card_has_its_texts_and_a_feature() {
        let en = crate::i18n::catalogue(crate::i18n::REFERENCE);
        for c in CARDS {
            assert!(crate::update::parse(c.version).is_some(), "{}", c.id);
            assert!(
                crate::features::find(c.id).is_some(),
                "{}: no F1 entry",
                c.id
            );
            for k in ["title", "text"] {
                assert!(
                    en.contains_key(&format!("tour.{}.{k}", c.id)),
                    "{}: no {k}",
                    c.id
                );
            }
        }
        // No card from beyond the next release: at most the next minor version.
        let (major, minor, _) = crate::update::parse(crate::update::VERSION).unwrap();
        let next = format!("{major}.{}.0", minor + 1);
        assert!(
            CARDS
                .iter()
                .all(|c| !crate::update::is_newer(&next, c.version))
        );
    }
}
