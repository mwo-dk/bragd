//! The install wizard: six steps a new user goes through once (or skips), opened again from
//! Settings → Overview. 1 Welcome: language, Theme, Board theme; 2 Game families and Variants;
//! 3 Engines found here; 4 Knowledge: recommended downloads, with sizes and a free-space
//! warning; 5 Online sign-in (later); 6 the Helper's *Start with my session*. Every step can be
//! skipped and names its Settings area. What each step offers, what is recommended, the totals
//! and what Done saves are decided here; the app only draws them.

use serde::Serialize;

use crate::config::Config;
use crate::settings::Area;
use crate::t;

/// A step: its id (`wizard.step<n>` names it) and the Settings area that holds the same choices.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Step {
    pub id: &'static str,
    pub area: Area,
}

pub const STEPS: [Step; 6] = [
    Step {
        id: "welcome",
        area: Area::Look,
    },
    Step {
        id: "families",
        area: Area::Families,
    },
    Step {
        id: "engines",
        area: Area::Engines,
    },
    Step {
        id: "knowledge",
        area: Area::Knowledge,
    },
    Step {
        id: "online",
        area: Area::Online,
    },
    Step {
        id: "helper",
        area: Area::Knowledge,
    },
];

/// A step as the wizard's frame shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StepView {
    pub id: &'static str,
    /// "Engines (3 of 6)".
    pub title: String,
    pub area: Area,
    /// "You can change this later in Settings → Engines."
    pub later: String,
}

pub fn steps() -> Vec<StepView> {
    STEPS
        .iter()
        .enumerate()
        .map(|(i, s)| StepView {
            id: s.id,
            title: t!("wizard.title", "step" => t!(&format!("wizard.step{}", i + 1)), "n" => i + 1, "of" => STEPS.len()),
            area: s.area,
            later: t!("wizard.later", "area" => s.area.label()),
        })
        .collect()
}

/// The Themes shown as swatches on the Welcome step; every other one is in the list below them.
pub const THEMES: [&str; 6] = [
    "racing-green",
    "racing-green-dark",
    "light",
    "dark",
    "paper",
    "nord",
];

/// A Theme or Board theme as a swatch: its colours, worked out as the app would apply them.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Swatch {
    pub id: &'static str,
    pub label: String,
    /// Page and text, or light and dark square.
    pub bg: String,
    pub fg: String,
    pub accent: String,
}

pub fn theme_swatches() -> Vec<Swatch> {
    THEMES
        .iter()
        .filter_map(|id| {
            let th = crate::themes::builtin(id)?;
            Some(Swatch {
                id,
                label: t!(&format!("theme.{id}")),
                bg: th.bg,
                fg: th.fg,
                accent: th.accent,
            })
        })
        .collect()
}

/// Every Board theme, drawn with the Theme `cfg` names (`match` follows it).
pub fn board_swatches(cfg: &Config) -> Vec<Swatch> {
    let th = crate::themes::theme(cfg);
    crate::themes::BOARD_THEMES
        .iter()
        .map(|id| {
            let b = crate::themes::board_theme(id, &th);
            Swatch {
                id,
                label: t!(&format!("board_theme.{id}")),
                bg: b.light,
                fg: b.dark,
                accent: b.highlight,
            }
        })
        .collect()
}

/// The id of the Syzygy 3–5 piece download in the Knowledge step.
pub const SYZYGY: &str = "syzygy";
/// What the 3–5 piece tables take, WDL and DTZ: about 1 GB. The download reads exact sizes.
pub const SYZYGY_SIZE: u64 = 939_000_000;
/// What the wizard turns on unless the user says otherwise: TWIC every week, and Syzygy 3–5.
pub const RECOMMENDED: [&str; 2] = ["twic", SYZYGY];
/// A download larger than this carries a warning about the disk.
pub const BIG: u64 = 10_000_000_000;
/// Free space the chosen downloads must leave on the disk, or the wizard warns.
pub const SPARE: u64 = 5_000_000_000;

/// A Source as the Helper lists it (`bragd_helper::sources::About`), as much as the wizard needs.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub family: String,
    /// About how many bytes one fetch takes.
    pub size: u64,
    /// "weekly", "monthly"…
    pub schedule: String,
    pub fetchable: bool,
}

/// A download the Knowledge step offers.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Offer {
    pub id: String,
    pub name: String,
    /// "about 1.5 MB every week".
    pub size: String,
    pub recommended: bool,
    /// False: listed so the user knows it is coming, but it cannot be chosen yet.
    pub fetchable: bool,
    /// Why it cannot be chosen, and a warning about its size.
    pub note: Option<String>,
    /// At least `BIG`: the note warns about the disk.
    pub big: bool,
}

/// The downloads the Knowledge step offers: Syzygy 3–5 and every Source, those of Game families
/// that are off left out, the recommended ones first. `bytes` writes a size as shown.
pub fn offers(sources: &[Source], cfg: &Config, bytes: fn(u64) -> String) -> Vec<Offer> {
    let mut out = vec![];
    if cfg.family_shown("chess") {
        out.push(Offer {
            id: SYZYGY.into(),
            name: t!("wizard.syzygy"),
            size: t!("wizard.size_once", "size" => bytes(SYZYGY_SIZE)),
            recommended: true,
            fetchable: true,
            note: None,
            big: false,
        });
    }
    for s in sources.iter().filter(|s| cfg.family_shown(&s.family)) {
        let big = s.size >= BIG;
        let notes: Vec<String> = [
            (!s.fetchable).then(|| t!("notice.source_out_of_scope", "source" => s.name)),
            big.then(|| t!("wizard.big", "size" => bytes(s.size))),
        ]
        .into_iter()
        .flatten()
        .collect();
        out.push(Offer {
            id: s.id.clone(),
            name: s.name.clone(),
            size: t!(&format!("wizard.size_{}", s.schedule), "size" => bytes(s.size)),
            recommended: RECOMMENDED.contains(&s.id.as_str()),
            fetchable: s.fetchable,
            note: (!notes.is_empty()).then(|| notes.join(" ")),
            big,
        });
    }
    out.sort_by_key(|o| (!o.recommended, !o.fetchable));
    out
}

/// What the Knowledge step starts with ticked: the recommended downloads it can fetch.
pub fn ticked(offers: &[Offer]) -> Vec<String> {
    offers
        .iter()
        .filter(|o| o.recommended && o.fetchable)
        .map(|o| o.id.clone())
        .collect()
}

/// The size of what is chosen, against the free space.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Totals {
    pub bytes: u64,
    /// "Chosen: 940 MB to download first."
    pub text: String,
    /// When the downloads would leave less than `SPARE` free.
    pub warning: Option<String>,
}

/// The first download of everything `chosen` (ids of `sources`, and `SYZYGY`), against `free`
/// bytes on the disk that holds Bragd's data (none where the system does not say).
pub fn totals(
    sources: &[Source],
    chosen: &[String],
    free: Option<u64>,
    bytes: fn(u64) -> String,
) -> Totals {
    let size = |id: &str| {
        if id == SYZYGY {
            return SYZYGY_SIZE;
        }
        sources
            .iter()
            .find(|s| s.id == id && s.fetchable)
            .map_or(0, |s| s.size)
    };
    let total: u64 = chosen.iter().map(|id| size(id)).sum();
    let warning = free
        .filter(|&f| total > 0 && total.saturating_add(SPARE) > f)
        .map(|f| t!("wizard.no_room", "size" => bytes(total), "free" => bytes(f), "spare" => bytes(SPARE)));
    Totals {
        bytes: total,
        text: t!("wizard.total", "size" => bytes(total)),
        warning,
    }
}

/// What Done saves: the Sources to turn on (in the Helper's options, each on its own schedule)
/// and whether the Syzygy 3–5 tables download. Ids that are not offered or cannot be fetched
/// are dropped.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Plan {
    pub sources: Vec<String>,
    pub syzygy: bool,
}

pub fn plan(sources: &[Source], chosen: &[String], cfg: &Config) -> Plan {
    Plan {
        sources: sources
            .iter()
            .filter(|s| s.fetchable && cfg.family_shown(&s.family) && chosen.contains(&s.id))
            .map(|s| s.id.clone())
            .collect(),
        syzygy: cfg.family_shown("chess") && chosen.iter().any(|c| c == SYZYGY),
    }
}

/// The last step's lines on what Done does.
pub fn summary(p: &Plan, sources: &[Source], bytes: fn(u64) -> String) -> Vec<String> {
    let mut out: Vec<String> = p
        .sources
        .iter()
        .filter_map(|id| sources.iter().find(|s| &s.id == id))
        .map(|s| t!("wizard.will_fetch", "name" => s.name, "size" => bytes(s.size)))
        .collect();
    if p.syzygy {
        out.push(
            t!("wizard.will_download", "name" => t!("wizard.syzygy"), "size" => bytes(SYZYGY_SIZE)),
        );
    }
    if out.is_empty() {
        out.push(t!("wizard.nothing"));
    }
    out
}

/// The wizard was closed, done or skipped: not shown by itself again, and the tour cards up to
/// this version count as seen (a first start has nothing new to tell).
pub fn seen(state: &mut crate::state::AppState) {
    state.guide_seen = true;
    crate::tour::seen(state);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(n: u64) -> String {
        format!("{n} B")
    }

    fn sources() -> Vec<Source> {
        let s = |id: &str, family: &str, size, fetchable| Source {
            id: id.into(),
            name: id.to_uppercase(),
            family: family.into(),
            size,
            schedule: "weekly".into(),
            fetchable,
        };
        vec![
            s("lichess-db", "chess", 30_000_000_000, false),
            s("twic", "chess", 1_500_000, true),
            s("kgs", "go", 100, false),
            s("custom:x", "go", 7, true),
        ]
    }

    #[test]
    fn wizard_steps_name_their_settings_areas() {
        let v = steps();
        assert_eq!(v.len(), 6);
        assert_eq!(
            v.iter().map(|s| s.id).collect::<Vec<_>>(),
            [
                "welcome",
                "families",
                "engines",
                "knowledge",
                "online",
                "helper"
            ]
        );
        assert!(
            v[2].title.contains("Engines") && v[2].title.contains('3') && v[2].title.contains('6'),
            "{}",
            v[2].title
        );
        assert!(v[1].later.contains("Game families"), "{}", v[1].later);
        assert_eq!(
            theme_swatches().len(),
            THEMES.len(),
            "every swatch is a built-in Theme"
        );
        let boards = board_swatches(&Config::default());
        assert_eq!(boards[0].bg, "#efe7d6");
    }

    #[test]
    fn wizard_recommends_twic_and_syzygy_and_leaves_off_families_out() {
        let cfg = Config::default();
        let o = offers(&sources(), &cfg, b);
        let ids: Vec<&str> = o.iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids, ["syzygy", "twic", "custom:x", "lichess-db", "kgs"]);
        assert_eq!(ticked(&o), ["syzygy", "twic"]);
        assert!(
            o[3].note.as_deref().unwrap().contains("LICHESS-DB")
                && o[3].note.as_deref().unwrap().contains("30000000000")
                && o[3].big,
            "not fetched yet"
        );
        assert!(o[1].note.is_none() && o[1].size.contains("1500000"));

        let mut no_chess = Config::default();
        no_chess.families.family.insert("chess".into(), false);
        let o = offers(&sources(), &no_chess, b);
        assert_eq!(
            o.iter().map(|o| o.id.as_str()).collect::<Vec<_>>(),
            ["custom:x", "kgs"]
        );
        assert!(ticked(&o).is_empty());
        let p = plan(
            &sources(),
            &["syzygy".into(), "twic".into(), "custom:x".into()],
            &no_chess,
        );
        assert_eq!(
            p,
            Plan {
                sources: vec!["custom:x".into()],
                syzygy: false
            }
        );
    }

    #[test]
    fn wizard_big_sources_carry_a_warning() {
        let mut s = sources();
        s[0].fetchable = true;
        let o = offers(&s, &Config::default(), b);
        let lichess = o.iter().find(|o| o.id == "lichess-db").unwrap();
        assert!(
            lichess.note.as_deref().unwrap().contains("30000000000"),
            "{:?}",
            lichess.note
        );
    }

    #[test]
    fn wizard_totals_warn_when_the_disk_has_no_room() {
        let s = sources();
        let chosen: Vec<String> = vec!["syzygy".into(), "twic".into(), "kgs".into()];
        let t = totals(&s, &chosen, Some(100 * SPARE), b);
        assert_eq!(
            t.bytes,
            SYZYGY_SIZE + 1_500_000,
            "kgs cannot be fetched, so it counts nothing"
        );
        assert!(t.warning.is_none());
        let t = totals(&s, &chosen, Some(SPARE), b);
        assert!(
            t.warning.as_deref().unwrap().contains(&SPARE.to_string()),
            "{:?}",
            t.warning
        );
        assert!(
            totals(&s, &chosen, None, b).warning.is_none(),
            "unknown free space: no warning"
        );
        assert!(
            totals(&s, &[], Some(0), b).warning.is_none(),
            "nothing chosen: no warning"
        );
    }

    #[test]
    fn wizard_done_saves_only_what_can_be_fetched() {
        let s = sources();
        let cfg = Config::default();
        let p = plan(
            &s,
            &["twic".into(), "kgs".into(), "nope".into(), SYZYGY.into()],
            &cfg,
        );
        assert_eq!(
            p,
            Plan {
                sources: vec!["twic".into()],
                syzygy: true
            }
        );
        let lines = summary(&p, &s, b);
        assert_eq!(lines.len(), 2);
        assert!(
            lines[0].contains("TWIC") && lines[1].contains(&SYZYGY_SIZE.to_string()),
            "{lines:?}"
        );
        assert_eq!(summary(&Plan::default(), &s, b).len(), 1);

        let mut st = crate::state::AppState::default();
        assert!(st.guide_due());
        seen(&mut st);
        assert!(!st.guide_due() && st.guide_seen);
        assert!(crate::tour::cards_since(&st.seen_version, &Config::default()).is_empty());
    }
}
