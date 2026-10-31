//! Settings: every option the app lets the user change, described once, with its place in
//! config.toml, the area of Settings it sits in and its texts. The desktop app's Settings
//! window is built from this list. Also here: the sidebar, worked out from what is on.

use serde::Serialize;
use serde_json::Value;

use crate::config::Config;
use bragd_games::Family;
use std::sync::LazyLock;

/// An area of Settings, by what users come for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Area {
    /// How everything stands: what is on, what is installed, what is new.
    Overview,
    Families,
    Look,
    Engines,
    /// Libraries, Sources, Tablebases and the Helper.
    Knowledge,
    /// Servers to play on, and what is asked of them.
    Online,
    /// The update check, and everything that can leave the machine.
    Privacy,
}

impl Area {
    pub const ALL: [Area; 7] = [
        Area::Overview,
        Area::Families,
        Area::Look,
        Area::Engines,
        Area::Knowledge,
        Area::Online,
        Area::Privacy,
    ];

    /// Its name in the `settings.area.<id>` text.
    pub fn id(self) -> &'static str {
        match self {
            Area::Overview => "overview",
            Area::Families => "families",
            Area::Look => "look",
            Area::Engines => "engines",
            Area::Knowledge => "knowledge",
            Area::Online => "online",
            Area::Privacy => "privacy",
        }
    }

    pub fn label(self) -> String {
        crate::i18n::tr(&format!("settings.area.{}", self.id()), &[])
    }
}

/// One option.
#[derive(Debug, Serialize)]
pub struct Opt {
    /// What the app calls it.
    pub name: &'static str,
    /// Where it lives in config.toml: `["look", "theme"]`.
    pub path: &'static [&'static str],
    pub area: Area,
    /// The key of its label; `<texts>.hint` is its one-line explanation, when there is one.
    pub texts: &'static str,
}

impl Opt {
    pub fn label(&self) -> String {
        crate::i18n::tr(self.texts, &[])
    }

    /// Its explanation, or empty.
    pub fn hint(&self) -> String {
        let key = format!("{}.hint", self.texts);
        let s = crate::i18n::tr(&key, &[]);
        if s == key { String::new() } else { s }
    }
}

const fn opt(
    name: &'static str,
    path: &'static [&'static str],
    area: Area,
    texts: &'static str,
) -> Opt {
    Opt {
        name,
        path,
        area,
        texts,
    }
}

use Area::*;

/// A Family's or Variant's switch, made once from `bragd_games`' catalogue.
fn switch(kind: &str, id: &'static str) -> Opt {
    let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
    let table: &'static str = if kind == "family" { "family" } else { "variant" };
    Opt {
        name: leak(format!("{kind}_{id}")),
        path: Box::leak(Box::new(["families", table, id])),
        area: Families,
        texts: leak(format!("{kind}.{id}")),
    }
}

static SWITCHES: LazyLock<Vec<Opt>> = LazyLock::new(|| {
    let families = Family::ALL.iter().map(|f| switch("family", f.key()));
    let variants = bragd_games::VARIANTS.iter().map(|v| switch("variant", v.key));
    families.chain(variants).collect()
});

const OTHER_OPTS: [Opt; 15] = [
    opt("language", &["language"], Look, "setting.language"),
    opt("theme", &["look", "theme"], Look, "setting.theme"),
    opt("look", &["look", "look"], Look, "setting.look"),
    opt(
        "board_theme",
        &["look", "board_theme"],
        Look,
        "setting.board_theme",
    ),
    opt("file_picker", &["look", "file_picker"], Look, "setting.file_picker"),
    opt(
        "engines_containers",
        &["engines", "containers"],
        Engines,
        "setting.engines_containers",
    ),
    opt(
        "engines_runtime",
        &["engines", "runtime"],
        Engines,
        "setting.engines_runtime",
    ),
    opt(
        "library",
        &["knowledge", "library"],
        Knowledge,
        "setting.library",
    ),
    opt(
        "source_priority",
        &["knowledge", "source_priority"],
        Knowledge,
        "setting.source_priority",
    ),
    opt(
        "tablebases_path",
        &["tablebases", "path"],
        Knowledge,
        "setting.tablebases_path",
    ),
    opt(
        "start_with_session",
        &["helper", "start_with_session"],
        Knowledge,
        "setting.start_with_session",
    ),
    opt(
        "tablebases_online",
        &["tablebases", "online"],
        Online,
        "setting.tablebases_online",
    ),
    opt(
        "check_updates",
        &["updates", "check"],
        Privacy,
        "setting.check_updates",
    ),
    // Disk watch's limits in bytes; each item's own limit is set on the Disk use page.
    opt("disk_total", &["disk", "total"], Privacy, "setting.disk_total"),
    opt("disk_free_below", &["disk", "free_below"], Privacy, "setting.disk_free_below"),
];

/// Every option: the Families and Variants, then the rest, each in its area's order.
pub fn options() -> impl Iterator<Item = &'static Opt> {
    SWITCHES.iter().chain(OTHER_OPTS.iter())
}

/// The option `name`.
pub fn find(name: &str) -> Option<&'static Opt> {
    options().find(|o| o.name == name)
}

/// The value of every option in `cfg`, by name.
pub fn values(cfg: &Config) -> serde_json::Map<String, Value> {
    let all = serde_json::to_value(cfg).unwrap_or_default();
    options()
        .map(|o| {
            let v = o.path.iter().try_fold(&all, |v, k| v.get(k));
            // A Family or Variant not written down is on.
            let v = v.cloned().unwrap_or(if o.area == Families {
                Value::Bool(true)
            } else {
                Value::Null
            });
            (o.name.to_string(), v)
        })
        .collect()
}

/// Writes one option's new value to config.toml.
/// The switches to save, in order, so the Family `id` shows again: the Family itself, and its
/// Variants too when every one of them is off. Empty when it already shows.
pub fn turn_on(cfg: &Config, id: &str) -> Vec<(String, Value)> {
    todo!()
}

pub fn save(name: &str, value: &Value) -> Result<Config, String> {
    let o = find(name).ok_or_else(|| format!("no setting {name}"))?;
    Config::save_value(o.path, to_toml(&as_number(o, value)?)?)
}

/// A number typed into a text field, for an option that holds a number.
fn as_number(o: &Opt, value: &Value) -> Result<Value, String> {
    let default = values(&Config::default()).remove(o.name);
    match (default, value) {
        (Some(Value::Number(_)), Value::String(s)) => s.trim().parse::<u64>().map(Value::from).map_err(|_| format!("{name}: {s} is not a whole number", name = o.name)),
        _ => Ok(value.clone()),
    }
}

fn to_toml(v: &Value) -> Result<toml_edit::Value, String> {
    Ok(match v {
        Value::Bool(b) => (*b).into(),
        Value::String(s) => s.as_str().into(),
        Value::Number(n) if n.is_i64() => n.as_i64().unwrap_or_default().into(),
        Value::Number(n) => n.as_f64().unwrap_or_default().into(),
        Value::Array(a) => a
            .iter()
            .map(to_toml)
            .collect::<Result<toml_edit::Array, _>>()?
            .into(),
        _ => return Err(format!("{v} cannot be written to config.toml")),
    })
}

/// An item of the sidebar under a Family (`sidebar.<id>` names it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Item {
    Play,
    Analyse,
    Databases,
    Engines,
    Tablebases,
    OpeningBooks,
    Rules,
}

/// The items of the Family `id`: chess has Tablebases and Opening books too; a Family with no
/// outside Engine has only Play, Databases and Rules.
pub fn items(id: &str) -> Vec<Item> {
    use Item::*;
    match Family::from_key(id) {
        Some(f) if !f.has_engine() => vec![Play, Databases, Rules],
        _ if id == "chess" => vec![
            Play,
            Analyse,
            Databases,
            Engines,
            Tablebases,
            OpeningBooks,
            Rules,
        ],
        _ => vec![Play, Analyse, Databases, Engines, Rules],
    }
}

/// A Family in the sidebar.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SidebarFamily {
    pub id: &'static str,
    pub items: Vec<Item>,
}

/// The sidebar above Settings, Help and What's new, which are always there. With two or more
/// Families on, the Families are the outer items; with one, its items come straight to the top.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Sidebar {
    Families { families: Vec<SidebarFamily> },
    One { family: SidebarFamily },
    Empty,
}

pub fn sidebar(cfg: &Config) -> Sidebar {
    let mut on: Vec<SidebarFamily> = Family::ALL
        .iter()
        .filter(|f| cfg.family_shown(f.key()))
        .map(|f| SidebarFamily {
            id: f.key(),
            items: items(f.key()),
        })
        .collect();
    match on.len() {
        0 => Sidebar::Empty,
        1 => Sidebar::One {
            family: on.remove(0),
        },
        _ => Sidebar::Families { families: on },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Item::{Analyse, Databases, OpeningBooks, Play, Rules, Tablebases};
    const ENGINES: Item = Item::Engines;

    #[test]
    fn settings_every_option_has_its_texts_and_a_unique_name() {
        let en = crate::i18n::catalogue(crate::i18n::REFERENCE);
        let mut names = vec![];
        for o in options() {
            assert!(en.contains_key(o.texts), "{}: no text {}", o.name, o.texts);
            if o.texts.starts_with("setting.") {
                assert!(
                    en.contains_key(&format!("{}.hint", o.texts)),
                    "{}: no hint",
                    o.name
                );
            }
            names.push(o.name);
        }
        let n = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), n);
        assert_eq!(options().filter(|o| o.area == Families).count(), 9 + 25);
        for a in Area::ALL {
            assert!(
                en.contains_key(&format!("settings.area.{}", a.id())),
                "{a:?}"
            );
        }
        // Every Family and Variant of the catalogue has its switch.
        for f in Family::ALL {
            assert!(find(&format!("family_{}", f.key())).is_some(), "{f:?}");
            for v in f.variants() {
                assert!(find(&format!("variant_{}", v.key)).is_some(), "{}", v.key);
            }
        }
    }

    #[test]
    fn a_family_that_is_off_is_turned_back_on() {
        let on = |text: &str| turn_on(&Config::parse(text).unwrap(), "go");
        assert!(on("").is_empty());
        assert_eq!(on("[families.family]\ngo = false\n"), [("family_go".to_string(), Value::Bool(true))]);
        let all_off = "[families.variant]\ngo-9 = false\ngo-13 = false\ngo-19 = false\n";
        let want: Vec<_> = Family::Go.variants().map(|v| (format!("variant_{}", v.key), Value::Bool(true))).collect();
        assert_eq!(on(all_off), want);
        let mut both = vec![("family_go".to_string(), Value::Bool(true))];
        both.extend(want);
        assert_eq!(on(&format!("[families.family]\ngo = false\n{all_off}")), both);
        // One Variant on is enough: the others stay as they were.
        assert_eq!(on("[families.family]\ngo = false\n[families.variant]\ngo-9 = false\n"), [("family_go".to_string(), Value::Bool(true))]);
    }

    #[test]
    fn settings_values_read_every_option() {
        let cfg =
            Config::parse("[families.family]\ngo = false\n[look]\ntheme = \"nord\"\n").unwrap();
        let v = values(&cfg);
        assert_eq!(v["family_go"], Value::Bool(false));
        assert_eq!(v["family_chess"], Value::Bool(true));
        assert_eq!(v["variant_chess960"], Value::Bool(true));
        assert_eq!(v["theme"], "nord");
        assert_eq!(v["check_updates"], true);
        assert_eq!(v["source_priority"], serde_json::json!(["twic", "lichess"]));
        assert_eq!(v.len(), options().count());
        assert!(v.values().all(|x| !x.is_null()));
        assert_eq!(v["disk_total"], 0);
    }

    #[test]
    fn settings_a_number_typed_as_text_is_saved_as_a_number() {
        let total = find("disk_total").unwrap();
        assert_eq!(as_number(total, &Value::from(" 5000000000 ")).unwrap(), Value::from(5_000_000_000u64));
        assert!(as_number(total, &Value::from("5 GB")).is_err());
        let theme = find("theme").unwrap();
        assert_eq!(as_number(theme, &Value::from("42")).unwrap(), Value::from("42"));
    }

    #[test]
    fn settings_sidebar_follows_what_is_on() {
        // All on: every Family as an outer item.
        let Sidebar::Families { families } = sidebar(&Config::default()) else {
            panic!()
        };
        assert_eq!(families.len(), 9);
        assert_eq!(
            families[0],
            SidebarFamily {
                id: "chess",
                items: vec![
                    Play,
                    Analyse,
                    Databases,
                    ENGINES,
                    Tablebases,
                    OpeningBooks,
                    Rules
                ]
            }
        );
        let tafl = families.iter().find(|f| f.id == "tafl").unwrap();
        assert_eq!(tafl.items, [Play, Databases, Rules]);
        for id in ["ur", "senet"] {
            assert_eq!(items(id), [Play, Databases, Rules]);
        }
        assert_eq!(items("go"), [Play, Analyse, Databases, ENGINES, Rules]);

        // One Family on: its items at the top.
        let off = |ids: &[&str]| {
            let mut c = Config::default();
            for f in Family::ALL {
                c.families.family.insert(f.key().into(), ids.contains(&f.key()));
            }
            c
        };
        assert_eq!(
            sidebar(&off(&["go"])),
            Sidebar::One {
                family: SidebarFamily {
                    id: "go",
                    items: items("go")
                }
            }
        );
        let Sidebar::Families { families } = sidebar(&off(&["go", "ur"])) else {
            panic!()
        };
        assert_eq!(
            families.iter().map(|f| f.id).collect::<Vec<_>>(),
            ["go", "ur"]
        );
        assert_eq!(sidebar(&off(&[])), Sidebar::Empty);

        // A Family whose Variants are all off is gone too.
        let mut c = off(&["go", "ur"]);
        c.families.variant.insert("ur".into(), false);
        assert!(matches!(sidebar(&c), Sidebar::One { family } if family.id == "go"));
    }
}
