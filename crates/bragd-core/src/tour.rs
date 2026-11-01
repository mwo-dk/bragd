//! Tour cards: after an upgrade, one card for each feature newer than the version the user last
//! saw, in the same frame as the install wizard. A card's texts are `tour.<id>.title` and
//! `tour.<id>.text`; its feature is the F1 entry of the same id, and names the Game families the
//! card concerns: a card shows only to someone with one of them on (or to all, naming none).

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
    card("0.2.0", "tips"),
    card("0.2.0", "clubs"),
    card("0.2.0", "matches"),
];

/// Whether the card shows to the user of `cfg`: its feature concerns one of their Families.
fn shown(c: &Card, cfg: &crate::config::Config) -> bool {
    crate::features::find(c.id).is_none_or(|f| crate::features::concerns(cfg, f.families))
}

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

/// What's new in the sidebar: every version, newest first, with the cards `cfg`'s Families see.
pub fn by_version(cfg: &crate::config::Config) -> Vec<(&'static str, Vec<View>)> {
    let mut out: Vec<(&'static str, Vec<View>)> = vec![];
    for c in CARDS.iter().rev().filter(|c| shown(c, cfg)) {
        match out.last_mut() {
            Some((v, list)) if *v == c.version => list.push(view(c)),
            _ => out.push((c.version, vec![view(c)])),
        }
    }
    out
}

/// The cards newer than `seen_version` that `cfg`'s Families see. None on a first start (no
/// version seen): the install wizard shows instead.
pub fn cards_since(seen_version: &str, cfg: &crate::config::Config) -> Vec<Card> {
    since(CARDS, seen_version).into_iter().filter(|c| shown(c, cfg)).collect()
}

/// Whether the card `id` is of a version newer than `seen`: a feature new since then.
pub fn newer_than(id: &str, seen: &str) -> bool {
    !seen.is_empty() && CARDS.iter().any(|c| c.id == id && crate::update::is_newer(seen, c.version))
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

/// The cards have been seen, up to this version or the newest card, whichever is later. After
/// an upgrade, when and from which version are kept: the new features' Tips wait a week.
pub fn seen(state: &mut crate::state::AppState) {
    let now = newest(CARDS);
    if !state.seen_version.is_empty() && crate::update::is_newer(&state.seen_version, now) {
        state.upgraded_from = std::mem::take(&mut state.seen_version);
        state.upgraded = crate::fs::now();
    }
    state.seen_version = now.to_string();
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
        let cfg = crate::config::Config::default();
        let mut st = crate::state::AppState::default();
        seen(&mut st);
        assert!(cards_since(&st.seen_version, &cfg).is_empty());
        assert!(st.upgraded_from.is_empty(), "a first start is no upgrade");
        st.seen_version = "0.0.1".into();
        seen(&mut st);
        assert_eq!(st.upgraded_from, "0.0.1");
        assert!(st.upgraded > 0);
        assert!(newer_than("tips", "0.0.1") && !newer_than("tips", "") && !newer_than("nope", "0.0.1"));
        assert_eq!(newest(&cards), "0.10.0");
        assert_eq!(newest(&[]), crate::update::VERSION);
        let all: usize = by_version(&cfg).iter().map(|(_, l)| l.len()).sum();
        assert_eq!(all, CARDS.len());
    }

    /// A card whose feature concerns only Families that are off is not shown: not after an
    /// upgrade, not in What's new.
    #[test]
    fn tour_leaves_out_cards_of_families_that_are_off() {
        let ids = |cfg: &crate::config::Config| -> Vec<&str> { cards_since("0.0.1", cfg).iter().map(|c| c.id).collect() };
        let mut cfg = crate::config::Config::default();
        assert!(ids(&cfg).contains(&"tips"));
        let tb = crate::features::find("tablebases").unwrap();
        assert_eq!(tb.families, ["chess"]);
        let fake = [card("0.2.0", "tablebases"), card("0.2.0", "tips")];
        let shown_ids = |cfg: &crate::config::Config| -> Vec<&str> { fake.iter().filter(|c| shown(c, cfg)).map(|c| c.id).collect() };
        assert_eq!(shown_ids(&cfg), ["tablebases", "tips"]);
        cfg.families.family.insert("chess".into(), false);
        assert_eq!(shown_ids(&cfg), ["tips"]);
        assert!(by_version(&cfg).iter().flat_map(|(_, l)| l).all(|v| v.id != "tablebases"));
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
