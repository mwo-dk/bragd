//! The options an engine offers, and the values the user sets for them. Values are kept per
//! engine, by its name without its version (`Stockfish 17.1` keeps `Stockfish`'s), in a TOML
//! file of their own (`engine-options.toml`, wherever the caller keeps Bragd's settings), so an
//! upgrade keeps them. Each value is checked against what the engine offers before it is sent.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use crate::protocol::About;

/// One of the options an engine offers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EngineOption {
    pub name: String,
    /// `spin` (a whole number from `min` to `max`), `check` (true or false), `combo` (one of
    /// `vars`), `string` or `button`.
    pub kind: String,
    pub default: String,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub vars: Vec<String>,
}

/// Options Bragd sets itself for each position or engine: not the user's to set.
const MANAGED: &[&str] = &["MultiPV", "USI_MultiPV", "UCI_Chess960", "UCI_Variant", "Ponder", "USI_Ponder", "SyzygyPath", "usemillisec", "variant"];

impl EngineOption {
    /// One the user can set: not one Bragd sets, and no button (each analysis starts the
    /// engine afresh, so a button pressed would do nothing).
    pub fn settable(&self) -> bool {
        self.kind != "button" && !MANAGED.iter().any(|m| m.eq_ignore_ascii_case(&self.name))
    }
}

/// A UCI or USI `option name … type … [default …] [min …] [max …] [var …]*` line.
pub fn parse_option(line: &str) -> Option<EngineOption> {
    let mut o = EngineOption::default();
    let mut field = "";
    for w in line.split_whitespace().skip(1) {
        // A name may hold any word but `type`.
        if matches!(w, "name" | "type" | "default" | "min" | "max" | "var") && (field != "name" || w == "type") {
            field = w;
            if w == "var" {
                o.vars.push(String::new());
            }
            continue;
        }
        let join = |s: &mut String| {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(w);
        };
        match field {
            "name" => join(&mut o.name),
            "type" => o.kind = w.to_string(),
            "default" => join(&mut o.default),
            "min" => o.min = w.parse().ok(),
            "max" => o.max = w.parse().ok(),
            "var" => join(o.vars.last_mut().expect("pushed at var")),
            _ => {}
        }
    }
    if o.default == "<empty>" {
        o.default.clear();
    }
    (!o.name.is_empty() && !o.kind.is_empty()).then_some(o)
}

/// A UCCI `option <name> type … [min …] [max …] [var …]* [default …]` line: its name is the
/// one word after `option`.
pub fn parse_ucci_option(line: &str) -> Option<EngineOption> {
    let mut w = line.split_whitespace().skip(1);
    let name = w.next()?;
    let rest: Vec<&str> = w.collect();
    let mut o = parse_option(&format!("option name {name} {}", rest.join(" ")))?;
    if o.default == "<empty>" {
        o.default.clear();
    }
    o.name = name.into();
    Some(o)
}

/// The engine's name without its version (`Stockfish 17.1` → `Stockfish`, `Lc0 v0.31.2` →
/// `Lc0`): its options' key, kept when it is upgraded.
pub fn engine_key(name: &str) -> String {
    let mut w: Vec<&str> = name.split_whitespace().collect();
    let version = |s: &str| {
        let s = s.trim_start_matches(['v', 'V']);
        s.starts_with(|c: char| c.is_ascii_digit()) || s.starts_with("dev")
    };
    while w.len() > 1 && w.last().is_some_and(|l| version(l)) {
        w.pop();
    }
    if w.is_empty() { "engine".into() } else { w.join(" ") }
}

/// The most a spin option may be set to here: the engine's own most, and for `Threads` no more
/// than this machine's logical processors, for `Hash` (in MB) no more than half its memory (and
/// 1 GB in a sealed container, which is bounded to 3 GB).
fn most(o: &EngineOption, container: bool) -> i64 {
    let own = o.max.unwrap_or(i64::MAX);
    let here = match o.name.to_ascii_lowercase().as_str() {
        "threads" => std::thread::available_parallelism().map_or(1, |n| n.get()) as i64,
        "hash" | "usi_hash" => {
            let half = match crate::tools::ram_gb() {
                0 => i64::MAX,
                gb => gb as i64 * 512,
            };
            if container { half.min(1024) } else { half }
        }
        _ => i64::MAX,
    };
    own.min(here)
}

/// The value `v` for the option `o` as it is sent, or why it cannot be: a spin's whole number
/// within its bounds, a check's true or false, one of a combo's values, a string of at most
/// 1,024 characters with no line break or other control character.
pub fn check(o: &EngineOption, v: &toml::Value, container: bool) -> Result<String, String> {
    let bad = || format!("{}: this value does not fit the option.", o.name);
    match o.kind.as_str() {
        "spin" => {
            let n = match v {
                toml::Value::Integer(n) => *n,
                toml::Value::String(s) => s.trim().parse().map_err(|_| bad())?,
                _ => return Err(bad()),
            };
            let (lo, hi) = (o.min.unwrap_or(i64::MIN), most(o, container));
            if n < lo || n > hi {
                return Err(format!("{}: from {lo} to {hi} here.", o.name));
            }
            Ok(n.to_string())
        }
        "check" => match v {
            toml::Value::Boolean(b) => Ok(b.to_string()),
            toml::Value::String(s) if s == "true" || s == "false" => Ok(s.clone()),
            _ => Err(bad()),
        },
        "combo" => {
            let s = v.as_str().ok_or_else(bad)?;
            o.vars.iter().find(|x| x.eq_ignore_ascii_case(s)).cloned().ok_or_else(bad)
        }
        "string" => {
            let s = v.as_str().ok_or_else(bad)?;
            if s.chars().count() > 1024 || s.chars().any(char::is_control) {
                return Err(bad());
            }
            Ok(if s.is_empty() { "<empty>".into() } else { s.to_string() })
        }
        _ => Err(bad()),
    }
}

/// The option values the user set, by engine key, then option name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OptionStore(pub BTreeMap<String, BTreeMap<String, toml::Value>>);

/// Option names with a value or a reason.
pub type Pairs = Vec<(String, String)>;

/// Not offered by this engine.
const UNKNOWN: &str = "this engine has no such option.";

/// engine-options.toml's format: its top-level `format` key, beside the engines' tables.
pub const FORMAT: bragd_core::format::Toml = bragd_core::format::Toml { file: "engine-options.toml", migrations: &[] };

impl OptionStore {
    /// The store at `path`; empty when there is none yet. One from a newer Bragd is refused.
    pub fn load(path: &Path) -> Result<OptionStore, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let text = FORMAT.upgrade(&text).map_err(|e| e.to_string())?;
                let mut table: toml::Table = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
                table.remove("format");
                toml::Value::Table(table).try_into().map_err(|e| format!("{}: {e}", path.display()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(OptionStore::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Written whole to a file beside `path`, then moved over it: a crash never leaves half.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = format!("format = {}\n\n{}", FORMAT.version(), toml::to_string(self).map_err(|e| e.to_string())?);
        bragd_core::fs::replace(path, text.as_bytes(), false).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// What to send to the engine `about` describes: each set option it offers, with a value
    /// that holds (`check`), and the rest with why not. A UCI_Elo set turns UCI_LimitStrength
    /// on, unless that is set too.
    pub fn to_send(&self, about: &About, container: bool) -> (Pairs, Pairs) {
        let (mut send, mut ignored) = (vec![], vec![]);
        let Some(set) = self.0.get(&engine_key(&about.name)) else { return (send, ignored) };
        let offered = |name: &str| about.options.iter().find(|o| o.settable() && o.name.eq_ignore_ascii_case(name));
        for (name, v) in set {
            match offered(name).map(|o| (o, check(o, v, container))) {
                None => ignored.push((name.clone(), UNKNOWN.to_string())),
                Some((o, Ok(text))) => send.push((o.name.clone(), text)),
                Some((_, Err(why))) => ignored.push((name.clone(), why)),
            }
        }
        let has = |n: &str| set.keys().any(|k| k.eq_ignore_ascii_case(n));
        if has("UCI_Elo")
            && !has("UCI_LimitStrength")
            && let Some(o) = offered("UCI_LimitStrength")
        {
            send.push((o.name.clone(), "true".into()));
        }
        (send, ignored)
    }
}

/// An engine as Settings shows it: its options, with the values set and the ones set that it
/// does not take, with why.
#[derive(Clone, Debug, Default, Serialize)]
pub struct EngineInfo {
    pub name: String,
    /// Its key in the store (`engine_key`).
    pub key: String,
    pub container: bool,
    pub options: Vec<Shown>,
    pub ignored: Vec<(String, String)>,
}

/// An option as Settings shows it: its `max` the most allowed here, its value when set.
#[derive(Clone, Debug, Serialize)]
pub struct Shown {
    #[serde(flatten)]
    pub option: EngineOption,
    pub value: Option<toml::Value>,
}

impl EngineInfo {
    /// The settable options of the engine `about` describes, with the values `store` gives.
    pub fn new(about: &About, container: bool, store: &OptionStore) -> EngineInfo {
        let options = about
            .options
            .iter()
            .filter(|o| o.settable())
            .map(|o| {
                let mut option = o.clone();
                if o.kind == "spin" {
                    option.max = Some(most(o, container));
                }
                Shown { option, value: None }
            })
            .collect();
        let mut info = EngineInfo { name: about.name.clone(), key: engine_key(&about.name), container, options, ignored: vec![] };
        info.refresh(store);
        info
    }

    /// The values `store` gives its options now, and the ones set there it does not take:
    /// after a change, without running the engine again.
    pub fn refresh(&mut self, store: &OptionStore) {
        let set = store.0.get(&self.key);
        let value = |name: &str| set.and_then(|s| s.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone()));
        for s in &mut self.options {
            s.value = value(&s.option.name);
        }
        let container = self.container;
        self.ignored = set
            .into_iter()
            .flatten()
            .filter_map(|(n, v)| match self.options.iter().find(|s| s.option.name.eq_ignore_ascii_case(n)) {
                None => Some((n.clone(), UNKNOWN.to_string())),
                Some(s) => check(&s.option, v, container).err().map(|why| (n.clone(), why)),
            })
            .collect();
    }
}

/// Sets the option `name` of the engine `info` describes in the store at `path`, after checking
/// `value` (a number, true or false, or text; a spin or check may come as text too). None takes
/// it out, so the engine's default applies. The store as saved.
pub fn set_option(path: &Path, info: &EngineInfo, name: &str, value: Option<&serde_json::Value>) -> Result<OptionStore, String> {
    use serde_json::Value as J;
    let s = info.options.iter().find(|s| s.option.name == name).ok_or_else(|| format!("{name}: {UNKNOWN}"))?;
    let o = &s.option;
    let bad = || format!("{}: this value does not fit the option.", o.name);
    let mut store = OptionStore::load(path)?;
    match value {
        None => {
            if let Some(set) = store.0.get_mut(&info.key) {
                set.retain(|k, _| !k.eq_ignore_ascii_case(name));
                if set.is_empty() {
                    store.0.remove(&info.key);
                }
            }
        }
        Some(v) => {
            let tv = match (o.kind.as_str(), v) {
                ("spin", J::Number(n)) => toml::Value::Integer(n.as_i64().ok_or_else(bad)?),
                ("spin", J::String(t)) => toml::Value::Integer(t.trim().parse().map_err(|_| bad())?),
                ("check", J::Bool(b)) => toml::Value::Boolean(*b),
                ("check", J::String(t)) => toml::Value::Boolean(t.parse().map_err(|_| bad())?),
                (_, J::String(t)) => toml::Value::String(t.clone()),
                _ => return Err(bad()),
            };
            check(o, &tv, info.container)?;
            store.0.entry(info.key.clone()).or_default().insert(o.name.clone(), tv);
        }
    }
    store.save(path)?;
    Ok(store)
}

/// Takes every option of the engine `key` out of the store at `path`: the engine's own defaults
/// apply again.
pub fn reset_options(path: &Path, key: &str) -> Result<OptionStore, String> {
    let mut store = OptionStore::load(path)?;
    store.0.remove(key);
    store.save(path)?;
    Ok(store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use toml::Value as V;

    #[test]
    fn options_read_and_checked() {
        let o = parse_option("option name Skill Level type spin default 20 min 0 max 20").unwrap();
        assert_eq!((o.name.as_str(), o.kind.as_str(), o.default.as_str(), o.min, o.max), ("Skill Level", "spin", "20", Some(0), Some(20)));
        let c = parse_option("option name Backend type combo default cuda var cuda var opencl blas var eigen").unwrap();
        assert_eq!(c.vars, ["cuda", "opencl blas", "eigen"]);
        let w = parse_option("option name EvalFile type string default <empty>").unwrap();
        assert_eq!(w.default, "");
        assert!(parse_option("option name Clear Hash type button").is_some_and(|b| !b.settable()));
        for managed in ["MultiPV", "UCI_Variant", "SyzygyPath", "UCI_Chess960"] {
            assert!(!parse_option(&format!("option name {managed} type string default x")).unwrap().settable(), "{managed}");
        }
        assert_eq!(check(&o, &V::Integer(5), false), Ok("5".into()));
        assert!(check(&o, &V::Integer(21), false).is_err());
        assert!(check(&o, &V::String("x".into()), false).is_err());
        assert_eq!(check(&c, &V::String("EIGEN".into()), false), Ok("eigen".into()));
        assert!(check(&c, &V::String("metal".into()), false).is_err());
        assert_eq!(check(&w, &V::String(String::new()), false), Ok("<empty>".into()));
        assert!(check(&w, &V::String("a\nquit".into()), false).is_err(), "no second command smuggled in");
        assert!(check(&w, &V::String("x".repeat(1025)), false).is_err());
        let t = parse_option("option name Threads type spin default 1 min 1 max 1024").unwrap();
        let cpus = std::thread::available_parallelism().unwrap().get() as i64;
        assert!(check(&t, &V::Integer(cpus), false).is_ok() && check(&t, &V::Integer(cpus + 1), false).is_err());
        let h = parse_option("option name Hash type spin default 16 min 1 max 33554432").unwrap();
        if crate::tools::ram_gb() >= 2 {
            assert!(check(&h, &V::Integer(1024), true).is_ok() && check(&h, &V::Integer(1025), true).is_err());
        }
        assert_eq!(engine_key("Stockfish 17.1"), "Stockfish");
        assert_eq!(engine_key("Lc0 v0.31.2"), "Lc0");
        assert_eq!(engine_key("Stockfish dev-20250101-abc"), "Stockfish");
        assert_eq!(engine_key("YaneuraOu NNUE 8.30 64AVX2"), "YaneuraOu NNUE");
        assert_eq!(engine_key("Fairy-Stockfish 14.0.1 LB"), "Fairy-Stockfish 14.0.1 LB");
        let u = parse_ucci_option("option hashsize type spin min 0 max 1024 default 0").unwrap();
        assert_eq!((u.name.as_str(), u.kind.as_str(), u.max, u.default.as_str()), ("hashsize", "spin", Some(1024), "0"));
    }

    fn about() -> About {
        let opts = ["option name Threads type spin default 1 min 1 max 512", "option name UCI_LimitStrength type check default false", "option name UCI_Elo type spin default 1320 min 1320 max 3190", "option name Style type combo default Normal var Solid var Normal"];
        About { name: "Fake 3.1".into(), options: opts.iter().filter_map(|l| parse_option(l)).collect() }
    }

    #[test]
    fn set_options_are_sent_checked() {
        let mut store = OptionStore::default();
        store.0.insert("Fake".into(), toml::from_str("Threads = 1\nUCI_Elo = 1500\nStyle = \"solid\"\nContempt = 5\n").unwrap());
        let (send, ignored) = store.to_send(&about(), false);
        let want = [("Style", "Solid"), ("Threads", "1"), ("UCI_Elo", "1500"), ("UCI_LimitStrength", "true")].map(|(a, b)| (a.to_string(), b.to_string()));
        assert_eq!(send, want);
        assert_eq!(ignored.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["Contempt"]);
    }

    #[test]
    fn options_kept_per_engine_set_and_reset() {
        let d = crate::test_dir("options");
        let path = d.join("engine-options.toml");
        let info = EngineInfo::new(&about(), false, &OptionStore::default());
        assert_eq!(info.key, "Fake");
        let store = set_option(&path, &info, "UCI_Elo", Some(&serde_json::json!(1800))).unwrap();
        assert_eq!(store.0["Fake"]["UCI_Elo"], V::Integer(1800));
        assert!(set_option(&path, &info, "UCI_Elo", Some(&serde_json::json!(9000))).is_err(), "out of range");
        assert!(set_option(&path, &info, "Contempt", Some(&serde_json::json!(1))).is_err(), "not offered");
        set_option(&path, &info, "Style", Some(&serde_json::json!("solid"))).unwrap();
        let back = OptionStore::load(&path).unwrap();
        assert_eq!(back.0["Fake"].len(), 2);
        let mut shown = EngineInfo::new(&about(), false, &back);
        assert_eq!(shown.options.iter().find(|s| s.option.name == "UCI_Elo").unwrap().value, Some(V::Integer(1800)));
        let store = set_option(&path, &info, "UCI_Elo", None).unwrap();
        assert_eq!(store.0["Fake"].len(), 1);
        let store = reset_options(&path, "Fake").unwrap();
        assert!(store.0.is_empty());
        shown.refresh(&store);
        assert!(shown.options.iter().all(|s| s.value.is_none()));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
