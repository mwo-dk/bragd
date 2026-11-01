//! Notices and hints. A notice says once that something is off, needs something installed, or
//! is new (a mistake in config.toml, a newer version); one that was dismissed is not shown again.
//! A hint tells what a user would not guess, a few times (`hint_due`).

use serde::Serialize;

use crate::config::Config;
use crate::state::AppState;
use crate::t;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Notice {
    /// Remembered once dismissed.
    pub id: String,
    pub text: String,
    /// The Settings area that does it (`settings::Area::id`).
    pub settings: Option<&'static str>,
    /// A line to copy and run (an upgrade command): shown with a Copy button.
    pub copy: Option<String>,
}

impl Notice {
    fn new(id: impl Into<String>, text: String, settings: Option<&'static str>) -> Notice {
        Notice {
            id: id.into(),
            text,
            settings,
            copy: None,
        }
    }
}

/// One notice for each area of config.toml that could not be used, while it stays wrong.
pub fn refused(cfg: &Config) -> Vec<Notice> {
    cfg.refused
        .iter()
        .map(|r| {
            let text = if r.area.is_empty() {
                t!("notice.config_unreadable", "problem" => r.problem)
            } else {
                t!("notice.config_refused", "area" => r.area, "problem" => r.problem)
            };
            Notice::new(format!("config:{}:{}", r.area, r.problem), text, None)
        })
        .collect()
}

/// The newer version the last update check found, with the command that upgrades this copy.
pub fn update(cfg: &Config, st: &AppState) -> Option<Notice> {
    let v = crate::update::available(cfg, st)?;
    let u = crate::update::hint(st);
    let mut text = t!("update.available", "version" => v);
    text.push(' ');
    text.push_str(&match u.cmd {
        Some(cmd) => t!("update.how", "how" => cmd),
        None => t!("update.download"),
    });
    if let Some(channel) = u.lags {
        text.push(' ');
        text.push_str(&t!("update.lags", "channel" => channel));
    }
    Some(Notice {
        copy: u.cmd.map(String::from),
        ..Notice::new(format!("update-{v}"), text, Some("privacy"))
    })
}

/// The banner on a file of a Family that is turned off: its files still open.
pub fn family_off(family: &str) -> Notice {
    let name = t!(&format!("family.{family}"));
    Notice::new(
        format!("family-off:{family}"),
        t!("notice.family_off", "family" => name),
        Some("families"),
    )
}

/// Every notice still to show.
pub fn all(cfg: &Config, st: &AppState) -> Vec<Notice> {
    let mut all = refused(cfg);
    all.extend(update(cfg, st));
    all.retain(|n| !st.notices_dismissed.contains(&n.id));
    all
}

/// Remembers a notice as seen.
pub fn dismiss(st: &mut AppState, id: &str) {
    if !st.notices_dismissed.iter().any(|d| d == id) {
        st.notices_dismissed.push(id.to_string());
    }
}

/// How many times a hint shows.
pub const TIMES: u32 = 3;

/// Whether the hint `id` (`hint.<id>`) is still to be shown; counted as shown once more when it
/// is, so the state changed exactly when this is true.
pub fn hint_due(st: &mut AppState, id: &str) -> bool {
    let n = st.hints_shown.entry(id.to_string()).or_default();
    if *n >= TIMES {
        return false;
    }
    *n += 1;
    true
}

/// The next hint of `hints` (in order, after `after` when given) still to be shown, counted as
/// shown once more: one hint at a time, the next only once the one before is closed.
pub fn next_hint<'a>(st: &mut AppState, hints: &[&'a str], after: Option<&str>) -> Option<&'a str> {
    let from = after.and_then(|a| hints.iter().position(|h| *h == a)).map_or(0, |i| i + 1);
    hints[from..].iter().copied().find(|h| hint_due(st, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_name_refused_areas_until_dismissed() {
        let cfg = Config::lenient("[look]\ntheme = 7\n");
        let mut st = AppState::default();
        let n = all(&cfg, &st);
        assert_eq!(n.len(), 1);
        assert!(n[0].text.contains("[look]"), "{}", n[0].text);
        dismiss(&mut st, &n[0].id);
        assert!(all(&cfg, &st).is_empty());
    }

    #[test]
    fn notices_tell_of_a_newer_version_only_with_the_check_on() {
        let mut st = AppState {
            latest_version: "999.0.0".into(),
            ..AppState::default()
        };
        let n = update(&Config::default(), &st).unwrap();
        assert!(n.text.contains("999.0.0"));
        assert_eq!(n.settings, Some("privacy"));
        assert!(update(&Config::parse("[updates]\ncheck = false\n").unwrap(), &st).is_none());
        st.latest_version = crate::update::VERSION.into();
        assert!(update(&Config::default(), &st).is_none());
        assert!(family_off("go").text.contains("Go"));
    }

    #[test]
    fn notices_hints_show_three_times() {
        let mut st = AppState::default();
        let shown = (0..5).filter(|_| hint_due(&mut st, "sidebar")).count();
        assert_eq!(shown, TIMES as usize);
        assert!(hint_due(&mut st, "other"));
    }

    #[test]
    fn notices_hints_come_one_at_a_time() {
        let mut st = AppState::default();
        let hints = ["a", "b", "c"];
        assert_eq!(next_hint(&mut st, &hints, None), Some("a"));
        assert_eq!(st.hints_shown.get("b"), None, "the hints after it wait, uncounted");
        assert_eq!(next_hint(&mut st, &hints, Some("a")), Some("b"));
        assert_eq!(next_hint(&mut st, &hints, Some("c")), None);
        st.hints_shown.insert("a".into(), TIMES);
        assert_eq!(next_hint(&mut st, &hints, None), Some("b"));
    }
}
