//! config.toml: what the user chose, by area. Every key has a default, so a missing file, area
//! or key means the default. A key or area this version does not know (a newer Bragd's) is
//! ignored and left in the file; a mistake in one area costs only that area (`Config::lenient`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::t;
use crate::themes::Theme;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// "auto" (the system's language, or the nearest one Bragd has) or a code such as "da",
    /// "en-AU" or "es-AR"; see `i18n::LANGUAGES`.
    pub language: String,
    pub look: LookConfig,
    pub families: FamiliesConfig,
    pub engines: EnginesConfig,
    pub tablebases: TablebasesConfig,
    pub knowledge: KnowledgeConfig,
    pub helper: HelperConfig,
    pub updates: UpdatesConfig,
    /// The user's own Themes, `[themes.<name>]`: each colour not given is Racing green's. One
    /// named like a built-in Theme replaces it.
    pub themes: BTreeMap<String, Theme>,
    /// The areas config.toml gives that could not be used, each left at its default
    /// (`Config::lenient`): a notice names each one.
    #[serde(skip)]
    pub refused: Vec<Refused>,
}

/// An area of config.toml that could not be used: its name (empty for the whole file) and what
/// is wrong with it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Refused {
    pub area: String,
    pub problem: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LookConfig {
    /// A Theme's name (`themes::BUILTIN`, or a `[themes.<name>]` table).
    pub theme: String,
    /// The chrome laid over the Theme (`themes::LOOKS`); empty: the Theme's own.
    pub look: String,
    /// `racing`, `slate`, `walnut`, or `match` for colours taken from the Theme.
    pub board_theme: String,
}

impl Default for LookConfig {
    fn default() -> Self {
        LookConfig {
            theme: "racing-green".into(),
            look: String::new(),
            board_theme: "racing".into(),
        }
    }
}

/// Which Game families and Variants show. A missing key is on: only what was turned off is
/// written down.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FamiliesConfig {
    /// Family id -> on (`families::FAMILIES`).
    pub family: BTreeMap<String, bool>,
    /// Variant id -> on.
    pub variant: BTreeMap<String, bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnginesConfig {
    /// Pull a sealed container image for an Engine that is not installed.
    pub containers: bool,
    /// `auto` (podman, else docker), `podman` or `docker`.
    pub runtime: String,
    /// Engine name -> the program to run instead of the one found.
    pub paths: BTreeMap<String, String>,
}

impl Default for EnginesConfig {
    fn default() -> Self {
        EnginesConfig {
            containers: true,
            runtime: "auto".into(),
            paths: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TablebasesConfig {
    /// The Syzygy folder; empty: the one in Bragd's data folder.
    pub path: String,
    /// Ask the Lichess tablebase API for a position the local tables do not cover.
    pub online: bool,
}

impl Default for TablebasesConfig {
    fn default() -> Self {
        TablebasesConfig {
            path: String::new(),
            online: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KnowledgeConfig {
    /// Where Libraries are kept; empty: the one in Bragd's data folder.
    pub library: String,
    /// Sources, the first winning a header conflict between Duplicates.
    pub source_priority: Vec<String>,
}

impl Default for KnowledgeConfig {
    fn default() -> Self {
        KnowledgeConfig {
            library: String::new(),
            source_priority: vec!["twic".into(), "lichess".into()],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HelperConfig {
    /// Start the Helper with the user's session, not only with a window.
    pub start_with_session: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdatesConfig {
    /// Ask this copy's channel for a newer release at start and once a day.
    pub check: bool,
}

impl Default for UpdatesConfig {
    fn default() -> Self {
        UpdatesConfig { check: true }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            language: "auto".into(),
            look: LookConfig::default(),
            families: FamiliesConfig::default(),
            engines: EnginesConfig::default(),
            tablebases: TablebasesConfig::default(),
            knowledge: KnowledgeConfig::default(),
            helper: HelperConfig::default(),
            updates: UpdatesConfig::default(),
            themes: BTreeMap::new(),
            refused: vec![],
        }
    }
}

/// `table[key]` into `slot`; a value that does not fit is named in `refused` and the default kept.
fn take<T: DeserializeOwned>(
    table: &toml::Table,
    key: &str,
    slot: &mut T,
    refused: &mut Vec<Refused>,
) {
    if let Some(v) = table.get(key) {
        match v.clone().try_into() {
            Ok(x) => *slot = x,
            Err(e) => refused.push(Refused {
                area: key.into(),
                problem: e.to_string().trim().to_string(),
            }),
        }
    }
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        Some(crate::dirs::config_dir()?.join("bragd").join("config.toml"))
    }

    /// The user's config, the defaults when there is no file. A mistake costs only its area
    /// (`lenient`); only a file that cannot be read at all is an error.
    pub fn load() -> Result<Config, String> {
        match Config::path().map(std::fs::read_to_string) {
            Some(Ok(text)) => Ok(Config::lenient(&text)),
            Some(Err(e)) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("config: {e}")),
            _ => Ok(Config::default()),
        }
    }

    /// `text` as a whole; unknown keys and areas are ignored.
    pub fn parse(text: &str) -> Result<Config, String> {
        toml::from_str(text).map_err(|e| format!("config: {e}"))
    }

    /// `text` read as far as it can be: each area with a mistake is left at its default and
    /// named in `refused`, the rest kept as the user set it. A file that is not TOML at all
    /// gives the defaults.
    pub fn lenient(text: &str) -> Config {
        if let Ok(c) = Config::parse(text) {
            return c;
        }
        let table: toml::Table = match text.parse() {
            Ok(t) => t,
            Err(e) => {
                let why = e.message().to_string();
                return Config {
                    refused: vec![Refused {
                        area: String::new(),
                        problem: t!("config.bad_toml", "why" => why),
                    }],
                    ..Config::default()
                };
            }
        };
        let mut c = Config::default();
        let mut refused = vec![];
        take(&table, "language", &mut c.language, &mut refused);
        take(&table, "look", &mut c.look, &mut refused);
        take(&table, "families", &mut c.families, &mut refused);
        take(&table, "engines", &mut c.engines, &mut refused);
        take(&table, "tablebases", &mut c.tablebases, &mut refused);
        take(&table, "knowledge", &mut c.knowledge, &mut refused);
        take(&table, "helper", &mut c.helper, &mut refused);
        take(&table, "updates", &mut c.updates, &mut refused);
        take(&table, "themes", &mut c.themes, &mut refused);
        c.refused = refused;
        c
    }

    /// `text` (a config.toml) with the value at `keys` (`["look", "theme"]`) set, its comments
    /// and layout kept.
    pub fn edit(text: &str, keys: &[&str], value: toml_edit::Value) -> Result<String, String> {
        let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e| format!("config: {e}"))?;
        let (last, parents) = keys.split_last().ok_or("config: no key")?;
        // Parents as tables or as inline tables (`family = { go = false }`): both parse.
        let mut table: &mut dyn toml_edit::TableLike = doc.as_table_mut();
        let mut inline = false;
        for k in parents {
            if table.get(k).is_none() {
                // A table cannot sit inside an inline one.
                let new = if inline {
                    toml_edit::Item::Value(toml_edit::InlineTable::new().into())
                } else {
                    let mut t = toml_edit::Table::new();
                    t.set_implicit(true);
                    toml_edit::Item::Table(t)
                };
                table.insert(k, new);
            }
            let item = table.get_mut(k).expect("there now");
            inline |= item.is_inline_table();
            table = item
                .as_table_like_mut()
                .ok_or_else(|| t!("config.not_a_table", "key" => k))?;
        }
        // Update in place where the key exists, so its comments stay.
        match table.get_mut(last).and_then(|i| i.as_value_mut()) {
            Some(old) => {
                let decor = old.decor().clone();
                *old = value;
                *old.decor_mut() = decor;
            }
            None => {
                table.insert(last, toml_edit::value(value));
            }
        }
        Ok(doc.to_string())
    }

    /// The settings `new_text` would save, where `old_text` is the file now: refused only when
    /// it has a mistake `old_text` did not have, so Settings still save while the user has not
    /// mended a mistake of their own yet.
    pub fn parse_saved(old_text: &str, new_text: &str) -> Result<Config, String> {
        Config::parse(new_text).or_else(|e| {
            let (old, new) = (Config::lenient(old_text), Config::lenient(new_text));
            let had = |r: &Refused| old.refused.iter().any(|o| o.area == r.area);
            if !new.refused.is_empty() && new.refused.iter().all(had) {
                Ok(new)
            } else {
                Err(e)
            }
        })
    }

    /// Sets one value in the user's config.toml and writes it; only what still parses is written.
    pub fn save_value(keys: &[&str], value: toml_edit::Value) -> Result<Config, String> {
        let path = Config::path().ok_or_else(|| t!("config.no_folder"))?;
        Config::save_value_at(&path, keys, value)
    }

    pub fn save_value_at(
        path: &std::path::Path,
        keys: &[&str],
        value: toml_edit::Value,
    ) -> Result<Config, String> {
        let old = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(format!("config: {e}")),
        };
        let new = Config::edit(&old, keys, value)?;
        let cfg = Config::parse_saved(&old, &new)?;
        crate::fs::replace(path, new.as_bytes(), false).map_err(|e| format!("config: {e}"))?;
        Ok(cfg)
    }

    /// Whether the Family `id` is turned on.
    pub fn family_on(&self, id: &str) -> bool {
        self.families.family.get(id) != Some(&false)
    }

    /// Whether the Variant `id` shows: it and its Family are on.
    pub fn variant_on(&self, id: &str) -> bool {
        crate::families::of_variant(id).is_some_and(|f| self.family_on(f.id))
            && self.families.variant.get(id) != Some(&false)
    }

    /// Whether the Family `id` shows: it is on, and so is one of its Variants at least.
    pub fn family_shown(&self, id: &str) -> bool {
        crate::families::find(id)
            .is_some_and(|f| self.family_on(id) && f.variants.iter().any(|v| self.variant_on(v)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_have_everything_on() {
        let c = Config::parse("").unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(c.look.theme, "racing-green");
        assert_eq!(c.look.board_theme, "racing");
        assert_eq!(c.knowledge.source_priority, ["twic", "lichess"]);
        assert!(c.updates.check && c.tablebases.online && !c.helper.start_with_session);
        for v in crate::families::variants() {
            assert!(c.variant_on(v), "{v}");
        }
    }

    #[test]
    fn config_tolerates_a_newer_versions_config() {
        let text = r#"
language = "da"
telepathy = true

[look]
theme = "nord"
sparkles = "many"

[families.variant]
chess960 = false
hexchess = false

[holograms]
on = true
"#;
        let c = Config::parse(text).unwrap();
        assert_eq!(c.language, "da");
        assert_eq!(c.look.theme, "nord");
        assert!(!c.variant_on("chess960"));
        assert!(c.variant_on("chess"));
        assert!(c.refused.is_empty());
        // Saving a setting keeps what this version does not know.
        let saved = Config::edit(text, &["look", "theme"], "aqua".into()).unwrap();
        assert!(
            saved.contains("sparkles")
                && saved.contains("[holograms]")
                && saved.contains("hexchess")
        );
        assert_eq!(Config::parse(&saved).unwrap().look.theme, "aqua");
    }

    #[test]
    fn config_a_mistake_costs_only_its_area() {
        let c = Config::lenient("[look]\ntheme = 7\n[updates]\ncheck = false\n");
        assert_eq!(c.look, LookConfig::default());
        assert!(!c.updates.check);
        assert_eq!(c.refused.len(), 1);
        assert_eq!(c.refused[0].area, "look");
        let c = Config::lenient("[look\n");
        assert_eq!(c.refused.len(), 1);
        assert_eq!(c.refused[0].area, "");
    }

    #[test]
    fn config_family_off_hides_its_variants() {
        let c = Config::parse("[families.family]\ngo = false\n[families.variant]\nur = false\n")
            .unwrap();
        assert!(!c.variant_on("go-9"));
        assert!(!c.family_shown("go"));
        assert!(
            !c.family_shown("ur"),
            "a Family with every Variant off does not show"
        );
        assert!(c.family_shown("chess"));
        assert!(!c.variant_on("no-such-variant"));
    }

    #[test]
    fn config_save_value_writes_and_keeps_comments() {
        let path = crate::dirs::config_dir()
            .unwrap()
            .join("save-value")
            .join("config.toml");
        let _ = std::fs::remove_file(&path);
        Config::save_value_at(&path, &["families", "family", "go"], false.into()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, format!("# mine\n{text}")).unwrap();
        let c = Config::save_value_at(&path, &["updates", "check"], false.into()).unwrap();
        assert!(!c.family_on("go") && !c.updates.check);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .starts_with("# mine")
        );
        // A value of the wrong kind is not written.
        assert!(Config::save_value_at(&path, &["updates", "check"], "maybe".into()).is_err());
        assert!(
            !Config::parse(&std::fs::read_to_string(&path).unwrap())
                .unwrap()
                .updates
                .check
        );
    }
}
