//! F1 → Features: every feature, with a line on what it does, two or three questions answered,
//! and its docs page ("Show me"). The texts are `feature.<id>.title`, `.what`, `.q1`/`.a1` …
//! in the locales.

use serde::Serialize;

/// A feature: its id (its texts' key) and its docs page under `docs/`.
pub struct Feature {
    pub id: &'static str,
    pub docs: &'static str,
}

const fn f(id: &'static str, docs: &'static str) -> Feature {
    Feature { id, docs }
}

/// Where "Show me" opens a feature's page.
pub const DOCS: &str = "https://github.com/mwo-dk/bragd/blob/master/docs/";

/// Every feature, in the order of `docs/README.md`, with its texts in every language.
pub const FEATURES: &[Feature] = &[
    f("families", "families/README.md"),
    f("play", "play/play.md"),
    f("analyse", "play/analyse.md"),
    f("databases", "knowledge/databases.md"),
    f("engines", "engines/engines.md"),
    f("tablebases", "knowledge/tablebases.md"),
    f("themes", "look/themes.md"),
    f("helper", "knowledge/helper.md"),
    f("update", "privacy/updates.md"),
];

/// The feature `id`.
pub fn find(id: &str) -> Option<&'static Feature> {
    FEATURES.iter().find(|f| f.id == id)
}

/// A feature as F1 shows it, in the current language.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Entry {
    pub id: &'static str,
    pub title: String,
    pub what: String,
    /// The questions with their answers.
    pub qa: Vec<(String, String)>,
    pub url: String,
}

/// The text `key`, or none when there is no such text.
fn text(key: &str) -> Option<String> {
    let s = crate::i18n::tr(key, &[]);
    (s != key).then_some(s)
}

pub fn entry(f: &Feature) -> Entry {
    let t = |k: &str| text(&format!("feature.{}.{k}", f.id)).unwrap_or_default();
    Entry {
        id: f.id,
        title: t("title"),
        what: t("what"),
        qa: (1..=4)
            .map_while(|i| {
                Some((
                    text(&format!("feature.{}.q{i}", f.id))?,
                    t(&format!("a{i}")),
                ))
            })
            .collect(),
        url: format!("{DOCS}{}", f.docs),
    }
}

/// The features as F1 lists them, filtered by `query`: each whose title, line, questions or
/// answers hold every word of it, case aside.
pub fn list(query: &str) -> Vec<Entry> {
    let words: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(String::from)
        .collect();
    FEATURES
        .iter()
        .map(entry)
        .filter(|e| {
            let hay = format!(
                "{} {} {}",
                e.title,
                e.what,
                e.qa.iter()
                    .map(|(q, a)| format!("{q} {a}"))
                    .collect::<String>()
            )
            .to_lowercase();
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn features_each_has_a_title_a_line_and_questions() {
        let en = crate::i18n::catalogue(crate::i18n::REFERENCE);
        for f in FEATURES {
            for k in ["title", "what", "q1", "a1", "q2", "a2"] {
                assert!(
                    en.contains_key(&format!("feature.{}.{k}", f.id)),
                    "{}: no {k}",
                    f.id
                );
            }
        }
        let all = list("");
        assert_eq!(all.len(), FEATURES.len());
        assert!(all.iter().all(|e| e.qa.len() >= 2 && !e.title.is_empty()));
        assert!(find("update").is_some());
        assert!(list("zzzz-nothing").is_empty());
    }
}
