//! F1 → Features: every feature, with a line on what it does, two or three questions answered,
//! and its docs page ("Show me"). The texts are `feature.<id>.title`, `.what`, `.q1`/`.a1` …
//! and `.tip` (its Tip, `tips`) in the locales.

use serde::Serialize;

/// A feature: its id (its texts' key), its docs page under `docs/`, where its Tip's *Show me*
/// goes, and the Game families it concerns.
pub struct Feature {
    pub id: &'static str,
    pub docs: &'static str,
    /// `settings:<area>`, `item:<sidebar item>` (of the first Family shown that has it), or
    /// `page:<page>` for a page of its own in the sidebar (`page:clubs`).
    pub open: &'static str,
    /// The Families it concerns; none: every user. Its tour card and Tip show only to someone
    /// with one of them on.
    pub families: &'static [&'static str],
}

const fn f(id: &'static str, docs: &'static str, open: &'static str) -> Feature {
    Feature { id, docs, open, families: &[] }
}

const fn only(id: &'static str, docs: &'static str, open: &'static str, families: &'static [&'static str]) -> Feature {
    Feature { id, docs, open, families }
}

/// Where "Show me" opens a feature's page.
pub const DOCS: &str = "https://github.com/mwo-dk/bragd/blob/master/docs/";

/// Every feature, in the order of `docs/README.md`, with its texts in every language.
pub const FEATURES: &[Feature] = &[
    f("wizard", "getting-started/first-start.md", "settings:overview"),
    f("families", "getting-started/game-families.md", "settings:families"),
    f("tips", "getting-started/tips.md", "settings:tips"),
    f("play", "play/against-yourself.md", "item:play"),
    f("databases", "knowledge/foreign-databases.md", "item:databases"),
    f("parquet", "knowledge/parquet-export.md", "item:databases"),
    f("disk", "knowledge/disk-watch.md", "settings:privacy"),
    f("helper", "knowledge/helper.md", "settings:knowledge"),
    f("engines", "engines/engines.md", "item:engines"),
    f("analyse", "engines/analyse.md", "item:analyse"),
    only("matches", "engines/matches.md", "item:matches", &["chess", "xiangqi", "shogi", "go", "draughts", "backgammon"]),
    only("tablebases", "engines/tablebases.md", "item:tablebases", &["chess"]),
    f("clubs", "clubs/clubs.md", "page:clubs"),
    f("themes", "look/themes.md", "settings:look"),
    f("update", "reference/privacy.md", "settings:privacy"),
];

/// The feature `id`.
pub fn find(id: &str) -> Option<&'static Feature> {
    FEATURES.iter().find(|f| f.id == id)
}

/// Whether what concerns `families` shows to the user of `cfg`: it names none, or one of them
/// is on. For tour cards, Tips and changelog rows alike.
pub fn concerns(cfg: &crate::config::Config, families: &[&str]) -> bool {
    families.is_empty() || families.iter().any(|f| cfg.family_shown(f))
}

/// The Families a changelog row names in its `<!-- families: chess, go -->` marker; none when
/// it has no marker (it concerns everyone).
pub fn row_families(row: &str) -> Vec<&str> {
    row.split("<!--")
        .filter_map(|m| m.split("-->").next()?.trim().strip_prefix("families:"))
        .flat_map(|list| list.split(',').map(str::trim).filter(|f| !f.is_empty()))
        .collect()
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

/// `s` in lower case without accents, so "grunfeld" finds "Grünfeld" and "senet" "sénet".
/// shortcut: folds Latin letters only (Latin-1 and Latin Extended-A, plus ß); take Unicode
/// decomposition when a language with other marks needs it.
pub fn fold(s: &str) -> String {
    const FROM: &str =
        "àáâãäåāăąçćĉċčďđèéêëēĕėęěĝğġģĥħìíîïĩīĭįıĵķĺļľŀłñńņňòóôõöøōŏőŕŗřśŝşšţťŧùúûüũūŭůűųŵýÿŷźżž";
    const TO: &str =
        "aaaaaaaaacccccddeeeeeeeeegggghhiiiiiiiiijklllllnnnnooooooooorrrsssstttuuuuuuuuuuwyyyzzz";
    s.to_lowercase()
        .chars()
        .flat_map(|c| match c {
            'ß' => vec!['s', 's'],
            'æ' => vec!['a', 'e'],
            'œ' => vec!['o', 'e'],
            c => vec![
                FROM.chars()
                    .position(|f| f == c)
                    .and_then(|i| TO.chars().nth(i))
                    .unwrap_or(c),
            ],
        })
        .collect()
}

/// The features as F1 lists them, filtered by `query`: each whose title, line, questions or
/// answers hold every word of it, case and accents aside.
pub fn list(query: &str) -> Vec<Entry> {
    let words: Vec<String> = fold(query).split_whitespace().map(String::from).collect();
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
            );
            let hay = fold(&hay);
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
        assert_eq!(list("DUPLICATE").len(), list("duplicate").len());
        assert!(
            !list("dúplicaté").is_empty(),
            "accents in the query are ignored"
        );
    }

    /// Every Family a Feature, a tour card (through its Feature) or a changelog row names is
    /// one Bragd knows; a misspelt one would hide it from everybody.
    #[test]
    fn features_name_only_known_families() {
        let known = |f: &str| bragd_games::Family::from_key(f).is_some();
        for f in FEATURES {
            assert!(f.families.iter().all(|x| known(x)), "{}: {:?}", f.id, f.families);
            assert!(["settings:", "item:", "page:clubs"].iter().any(|p| f.open.starts_with(p)), "{}", f.id);
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../changelog.d");
        // After a release changelog.d/ may be gone: no rows, nothing to check.
        for e in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let row = std::fs::read_to_string(e.path()).unwrap();
            for f in row_families(&row) {
                assert!(known(f), "{}: unknown Family {f}", e.path().display());
            }
        }
        assert_eq!(row_families("| x | <!-- feature --> <!-- families: chess, go -->"), ["chess", "go"]);
        assert!(row_families("| x | <!-- feature --> <!-- tour: play -->").is_empty());
        assert!(!known("chesss"));

        let mut cfg = crate::config::Config::default();
        assert!(concerns(&cfg, &[]) && concerns(&cfg, &["chess"]));
        cfg.families.family.insert("chess".into(), false);
        assert!(concerns(&cfg, &[]) && !concerns(&cfg, &["chess"]) && concerns(&cfg, &["chess", "go"]));
    }

    #[test]
    fn features_search_ignores_case_and_accents() {
        assert_eq!(fold("Grünfeld"), "grunfeld");
        assert_eq!(fold("ÉCHECS Straße Æsir"), "echecs strasse aesir");
        assert_eq!(fold("Łódź"), "lodz");
        assert_eq!(fold("象棋"), "象棋", "other scripts stay as they are");
    }
}
