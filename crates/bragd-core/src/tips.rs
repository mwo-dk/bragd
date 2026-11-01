//! Tips: now and then, one line suggesting a Feature the user has not used yet. Each Feature
//! counts its uses here (its usage signal, kept in state.json, never sent anywhere), and has its
//! Tip text, `feature.<id>.tip`. At most one Tip a day (*Often*) or a week (*Now and then*, the
//! default), none while the app is busy (a live Game, the wizard), none for a Feature already
//! used, silenced or of Families that are off. After an upgrade the tour cards come first: a new
//! Feature's Tip waits a week, then goes before the others.

use serde::Serialize;

use crate::config::Config;
use crate::features::{self, Feature, FEATURES};
use crate::state::AppState;

pub const DAY: u64 = 24 * 60 * 60;
/// How long a new Feature has to be tried after an upgrade before its Tip shows.
pub const WEEK: u64 = 7 * DAY;

/// The `[tips] frequency` values, as Settings → Tips offers them.
pub const FREQUENCIES: [&str; 3] = ["often", "sometimes", "off"];

/// The settings whose change is a use of a Feature: (name or name prefix, Feature).
pub const SETTINGS: [(&str, &str); 6] = [
    ("family_", "families"),
    ("variant_", "families"),
    ("theme", "themes"),
    ("board_theme", "themes"),
    ("look", "themes"),
    ("tips", "tips"),
];

/// The Feature whose use saving the setting `name` is.
pub fn of_setting(name: &str) -> Option<&'static str> {
    SETTINGS
        .iter()
        .find(|(n, _)| if n.ends_with('_') { name.starts_with(n) } else { name == *n })
        .map(|(_, f)| *f)
}

/// The least time between two Tips; none when they are off.
fn gap(cfg: &Config) -> Option<u64> {
    match cfg.tips.frequency.as_str() {
        "off" => None,
        "often" => Some(DAY),
        _ => Some(WEEK),
    }
}

/// The Feature `id` was used once more.
pub fn count(st: &mut AppState, id: &str) {
    let n = st.feature_uses.entry(id.to_string()).or_default();
    *n = n.saturating_add(1);
}

/// Counts a use of the Feature `id` in state.json. A file that cannot be written loses the
/// count, nothing else.
pub fn used(id: &str) {
    let mut st = AppState::load();
    count(&mut st, id);
    let _ = st.save();
}

/// The Feature whose Tip is due at `now`, if any. `busy`: a live Game, a Call or the wizard is
/// on screen, so nothing shows.
pub fn next(cfg: &Config, st: &AppState, now: u64, busy: bool) -> Option<&'static Feature> {
    let gap = gap(cfg)?;
    if busy || now < st.tip_shown.saturating_add(gap) {
        return None;
    }
    let open = |f: &&Feature| {
        features::concerns(cfg, f.families)
            && st.feature_uses.get(f.id).is_none_or(|n| *n == 0)
            && !st.tips_silenced.iter().any(|s| s == f.id)
    };
    // The one after the last shown first, so *Not now* moves on to another.
    let start = FEATURES.iter().position(|f| f.id == st.tip_last).map_or(0, |i| i + 1);
    let order = FEATURES[start..].iter().chain(&FEATURES[..start]);
    let fresh = |f: &&Feature| crate::tour::newer_than(f.id, &st.upgraded_from);
    if now < st.upgraded.saturating_add(WEEK) {
        order.filter(open).find(|f| !fresh(f))
    } else {
        let candidates: Vec<&Feature> = order.filter(open).collect();
        candidates.iter().find(|f| fresh(f)).or(candidates.first()).copied()
    }
}

/// The Tip of `f` was shown at `now`.
pub fn shown(st: &mut AppState, f: &Feature, now: u64) {
    st.tip_shown = now;
    st.tip_last = f.id.to_string();
}

/// *Don't suggest this again*.
pub fn silence(st: &mut AppState, id: &str) {
    if !st.tips_silenced.iter().any(|s| s == id) {
        st.tips_silenced.push(id.to_string());
    }
}

/// A Tip as the app shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Tip {
    pub id: &'static str,
    pub text: String,
    /// Where *Show me* goes (`Feature::open`).
    pub open: &'static str,
}

pub fn tip(f: &Feature) -> Tip {
    Tip { id: f.id, text: crate::i18n::tr(&format!("feature.{}.tip", f.id), &[]), open: f.open }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(freq: &str) -> Config {
        let mut c = Config::default();
        c.tips.frequency = freq.into();
        c
    }

    #[test]
    fn tips_show_at_most_once_a_day_or_week_and_never_when_busy_or_off() {
        let mut st = AppState::default();
        let now = 1_000 * DAY;
        let first = next(&cfg("sometimes"), &st, now, false).unwrap();
        assert!(next(&cfg("sometimes"), &st, now, true).is_none(), "busy");
        assert!(next(&cfg("off"), &st, now, false).is_none());
        shown(&mut st, first, now);
        assert!(next(&cfg("often"), &st, now + DAY - 1, false).is_none());
        let second = next(&cfg("often"), &st, now + DAY, false).unwrap();
        assert_ne!(second.id, first.id, "Not now moves on to another");
        assert!(next(&cfg("sometimes"), &st, now + 6 * DAY, false).is_none());
        assert!(next(&cfg("sometimes"), &st, now + WEEK, false).is_some());
    }

    #[test]
    fn tips_skip_features_used_silenced_or_of_families_that_are_off() {
        let c = cfg("often");
        let mut st = AppState::default();
        for f in FEATURES {
            if f.id != "tablebases" && f.id != "play" {
                count(&mut st, f.id);
            }
        }
        let ids = |c: &Config, st: &AppState| -> Vec<&str> {
            let mut st = st.clone();
            let mut out = vec![];
            for i in 0..4 {
                if let Some(f) = next(c, &st, (10 + i) * WEEK, false) {
                    out.push(f.id);
                    shown(&mut st, f, (10 + i) * WEEK);
                }
            }
            out
        };
        assert_eq!(ids(&c, &st), ["play", "tablebases", "play", "tablebases"]);
        silence(&mut st, "play");
        assert_eq!(ids(&c, &st), ["tablebases"; 4]);
        let mut no_chess = c.clone();
        no_chess.families.family.insert("chess".into(), false);
        assert!(ids(&no_chess, &st).is_empty(), "tablebases concern chess only");
        assert_eq!(of_setting("family_go"), Some("families"));
        assert_eq!(of_setting("theme"), Some("themes"));
        assert_eq!(of_setting("themes"), None);
        assert_eq!(of_setting("language"), None);
    }

    #[test]
    fn tips_after_an_upgrade_a_new_feature_waits_a_week_then_comes_first() {
        let c = cfg("often");
        let now = 2_000 * DAY;
        let mut st = AppState { upgraded: now, upgraded_from: "0.1.0".into(), ..AppState::default() };
        // Every Feature is newer than 0.1.0 but those without a card; use all but one old one.
        let old = FEATURES.iter().find(|f| !crate::tour::newer_than(f.id, "0.1.0")).unwrap();
        let new = FEATURES.iter().find(|f| crate::tour::newer_than(f.id, "0.1.0")).unwrap();
        assert_eq!(next(&c, &st, now + DAY, false).unwrap().id, old.id, "the tour cards came first");
        st.tip_last = new.id.into();
        assert!(crate::tour::newer_than(next(&c, &st, now + WEEK, false).unwrap().id, "0.1.0"));
        count(&mut st, old.id);
        for f in FEATURES.iter().filter(|f| !crate::tour::newer_than(f.id, "0.1.0")) {
            count(&mut st, f.id);
        }
        assert!(next(&c, &st, now + DAY, false).is_none(), "only new ones are left, and they wait");
    }

    /// Every Feature has its Tip text and a usage signal: a call `used("<id>")` (or
    /// `count(st, "<id>")`) somewhere in the code, or a setting that counts as its use.
    #[test]
    fn tips_every_feature_has_a_tip_and_a_signal() {
        use std::path::{Path, PathBuf};
        let en = crate::i18n::catalogue(crate::i18n::REFERENCE);
        fn walk(d: &Path, out: &mut Vec<PathBuf>) {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut files = vec![];
        walk(&root.join("gui/src-tauri/src"), &mut files);
        for c in std::fs::read_dir(root.join("crates")).into_iter().flatten().flatten() {
            walk(&c.path().join("src"), &mut files);
        }
        let code: String = files
            .iter()
            .filter(|f| !f.ends_with("tips.rs"))
            .map(|f| std::fs::read_to_string(f).unwrap_or_default())
            .collect();
        let mut bad = vec![];
        for f in FEATURES {
            if !en.contains_key(&format!("feature.{}.tip", f.id)) {
                bad.push(format!("{}: no Tip", f.id));
            }
            let signal = code.contains(&format!("tips::used(\"{}\")", f.id))
                || code.contains(&format!("tips::count(st, \"{}\")", f.id))
                || SETTINGS.iter().any(|(_, s)| *s == f.id);
            if !signal {
                bad.push(format!("{}: no usage signal", f.id));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
        assert_eq!(tip(&FEATURES[0]).open, FEATURES[0].open);
    }
}
