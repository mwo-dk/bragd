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
    /// Libraries, Sources, Tablebases, the Helper and Disk watch.
    Knowledge,
    /// Servers to play on, and what is asked of them.
    Online,
    /// How often a Tip may show.
    Tips,
    /// The update check, and everything that can leave the machine.
    Privacy,
    /// The language model: its address, its model, its key and consent.
    Model,
    /// Peers: the user's own machines, paired with a code.
    Peers,
}

impl Area {
    pub const ALL: [Area; 10] = [
        Area::Overview,
        Area::Families,
        Area::Look,
        Area::Engines,
        Area::Knowledge,
        Area::Online,
        Area::Tips,
        Area::Privacy,
        Area::Model,
        Area::Peers,
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
            Area::Tips => "tips",
            Area::Privacy => "privacy",
            Area::Model => "model",
            Area::Peers => "peers",
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

    /// Listed in its area of Settings. Families, Disk watch and the language model draw their
    /// own options; the Source priority is set on Databases → Libraries, where Sources are
    /// picked by name.
    pub fn listed(&self) -> bool {
        !matches!(self.area, Families | Model | Peers) && !self.name.starts_with("disk_") && self.name != "source_priority"
    }

    /// The Game family it concerns, if only one: it leaves its area while that Family is off.
    pub fn family(&self) -> Option<&'static str> {
        // Online play is on Lichess, for chess, for now.
        (self.name.starts_with("tablebases_") || self.name.starts_with("online_")).then_some("chess")
    }

    /// Listed in its area for the user of `cfg`: [`Opt::listed`], and its Family shows.
    pub fn listed_for(&self, cfg: &Config) -> bool {
        self.listed() && self.family().is_none_or(|f| cfg.family_shown(f))
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

const OTHER_OPTS: [Opt; 29] = [
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
    opt("analysis_open_games", &["analysis", "open_games"], Engines, "setting.analysis_open_games"),
    opt("analysis_depth", &["analysis", "depth"], Engines, "setting.analysis_depth"),
    opt("analysis_threads", &["analysis", "threads"], Engines, "setting.analysis_threads"),
    opt("analysis_from_hour", &["analysis", "from_hour"], Engines, "setting.analysis_from_hour"),
    opt("analysis_until_hour", &["analysis", "until_hour"], Engines, "setting.analysis_until_hour"),
    opt("analysis_on_battery", &["analysis", "on_battery"], Engines, "setting.analysis_on_battery"),
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
        "max_unpack_ratio",
        &["knowledge", "max_unpack_ratio"],
        Knowledge,
        "setting.max_unpack_ratio",
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
    opt("online_chat", &["online", "chat"], Online, "setting.online_chat"),
    opt("tips", &["tips", "frequency"], Tips, "setting.tips"),
    opt(
        "check_updates",
        &["updates", "check"],
        Privacy,
        "setting.check_updates",
    ),
    // Drawn by Settings → Language model, which checks the address and asks for consent.
    opt("model_url", &["model", "url"], Model, "setting.model_url"),
    opt("model_name", &["model", "name"], Model, "setting.model_name"),
    opt("model_consent", &["model", "consent"], Model, "setting.model_consent"),
    // Drawn by Settings → Peers, which tells the Helper to listen or stop.
    opt("peers_on", &["peers", "on"], Peers, "setting.peers_on"),
    opt("peers_interface", &["peers", "interface"], Peers, "setting.peers_interface"),
    // Disk watch's limits in bytes, drawn with each item's own by Disk watch in Knowledge.
    opt("disk_total", &["disk", "total"], Knowledge, "setting.disk_total"),
    opt("disk_free_below", &["disk", "free_below"], Knowledge, "setting.disk_free_below"),
];

/// The areas Settings lists for the user of `cfg`: Overview and Game families always, every other
/// one only when it has something to show for the Families that are on.
pub fn areas(cfg: &Config) -> Vec<Area> {
    Area::ALL
        .into_iter()
        .filter(|a| matches!(a, Overview | Families | Model | Peers) || options().any(|o| o.area == *a && o.listed_for(cfg)))
        .collect()
}

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

/// The switches to save, in order, so the Family `id` shows again: the Family itself, and its
/// Variants too when every one of them is off. Empty when it already shows.
pub fn turn_on(cfg: &Config, id: &str) -> Vec<(String, Value)> {
    let Some(family) = Family::from_key(id) else { return Vec::new() };
    let mut out = Vec::new();
    if !cfg.family_on(id) {
        out.push((format!("family_{id}"), Value::Bool(true)));
    }
    // Read each Variant's own switch: while the Family is off, variant_on says no for them all.
    if family.variants().all(|v| cfg.families.variant.get(v.key) == Some(&false)) {
        out.extend(family.variants().map(|v| (format!("variant_{}", v.key), Value::Bool(true))));
    }
    out
}

/// The switches to save, in order, so only the Family `id` shows: it (with its Variants when
/// every one was off) on, and every other Family off. *Only chess* in the wizard.
pub fn only(cfg: &Config, id: &str) -> Vec<(String, Value)> {
    if Family::from_key(id).is_none() {
        return Vec::new();
    }
    let mut out = turn_on(cfg, id);
    out.extend(
        Family::ALL
            .iter()
            .filter(|f| f.key() != id && cfg.family_on(f.key()))
            .map(|f| (format!("family_{}", f.key()), Value::Bool(false))),
    );
    out
}

/// Writes one option's new value to config.toml.
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
    Matches,
    Tablebases,
    OpeningBooks,
    Rules,
}

/// The items of the Family `id`: chess has Tablebases and Opening books too; a Family with no
/// outside Engine has only Play, Databases and Rules (no Engines, no Engine matches).
pub fn items(id: &str) -> Vec<Item> {
    use Item::*;
    match Family::from_key(id) {
        Some(f) if !f.has_engine() => vec![Play, Databases, Rules],
        _ if id == "chess" => vec![
            Play,
            Analyse,
            Databases,
            Engines,
            Matches,
            Tablebases,
            OpeningBooks,
            Rules,
        ],
        _ => vec![Play, Analyse, Databases, Engines, Matches, Rules],
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
    use Item::{Analyse, Databases, Matches, OpeningBooks, Play, Rules, Tablebases};
    const ENGINES: Item = Item::Engines;

    #[test]
    fn settings_lists_what_has_no_page_of_its_own() {
        // Families and Disk watch draw their own options; the rest are listed in their area.
        assert!(!find("family_chess").unwrap().listed());
        assert!(!find("disk_total").unwrap().listed());
        assert!(!find("source_priority").unwrap().listed(), "no text field of Source ids");
        assert!(!find("model_url").unwrap().listed(), "Settings → Language model draws it");
        assert!(find("theme").unwrap().listed());
        let listed = options().filter(|o| o.listed()).count();
        assert_eq!(listed, OTHER_OPTS.len() - 8);
    }

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
    fn only_one_family_turns_the_others_off() {
        let cfg = Config::parse("[families.family]\ngo = false\nchess = false\n").unwrap();
        let switches = only(&cfg, "chess");
        assert_eq!(switches[0], ("family_chess".to_string(), Value::Bool(true)));
        let off: Vec<&str> = switches[1..].iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(off.len(), Family::ALL.len() - 2, "chess on, go already off: {off:?}");
        assert!(!off.contains(&"family_go") && switches[1..].iter().all(|(_, v)| *v == Value::Bool(false)));
        assert!(only(&cfg, "nope").is_empty());
        // Tablebase settings leave Settings with chess.
        let tb = find("tablebases_path").unwrap();
        assert!(tb.listed_for(&Config::default()) && !tb.listed_for(&cfg));
        assert!(find("theme").unwrap().listed_for(&cfg));
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
        assert_eq!(v["tips"], "sometimes");
        assert_eq!(v["source_priority"], serde_json::json!(["twic", "lichess-db"]));
        assert_eq!(v.len(), options().count());
        assert!(v.values().all(|x| !x.is_null()));
        assert_eq!(v["disk_total"], 0);
    }

    #[test]
    fn settings_areas_with_nothing_to_show_are_not_listed() {
        assert_eq!(areas(&Config::default()), Area::ALL);
        let mut go = Config::default();
        for f in Family::ALL.iter().filter(|f| f.key() != "go") {
            go.families.family.insert(f.key().into(), false);
        }
        let listed = areas(&go);
        assert!(!listed.contains(&Online), "Online holds only chess settings: {listed:?}");
        assert!(listed.contains(&Look) && listed.contains(&Families));
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
                    Matches,
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
        assert_eq!(items("go"), [Play, Analyse, Databases, ENGINES, Matches, Rules]);

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
