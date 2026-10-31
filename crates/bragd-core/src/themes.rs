//! Themes, Looks and Board themes. A Theme is the app's colours and the Look laid over them; a
//! Board theme is the four colours of a board. Rust works out every colour; the desktop app
//! only applies the CSS variables `appearance` gives it, and sets `data-look` on the page.

use serde::{Deserialize, Serialize};

use crate::config::Config;

/// A Theme: the Look it brings, whether it has a tube's glow (`crt`), and its colours. In a
/// `[themes.<name>]` table every colour not given is Racing green's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub look: String,
    pub crt: bool,
    /// The page.
    pub bg: String,
    /// The sidebar and bars.
    pub alt: String,
    /// Dialogs, cards and menus.
    pub raised: String,
    pub fg: String,
    /// Quiet text.
    pub dim: String,
    pub border: String,
    pub accent: String,
    /// Text on `accent`.
    pub accent_fg: String,
    /// The selected row.
    pub sel_bg: String,
    pub sel_fg: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub orange: String,
}

impl Default for Theme {
    fn default() -> Self {
        racing_green()
    }
}

struct P {
    bg: &'static str,
    alt: &'static str,
    raised: &'static str,
    fg: &'static str,
    dim: &'static str,
    border: &'static str,
    accent: &'static str,
    accent_fg: &'static str,
    sel_bg: &'static str,
    sel_fg: &'static str,
    red: &'static str,
    green: &'static str,
    yellow: &'static str,
    blue: &'static str,
    magenta: &'static str,
    orange: &'static str,
}

fn pal(look: &str, crt: bool, p: P) -> Theme {
    Theme {
        look: look.into(),
        crt,
        bg: p.bg.into(),
        alt: p.alt.into(),
        raised: p.raised.into(),
        fg: p.fg.into(),
        dim: p.dim.into(),
        border: p.border.into(),
        accent: p.accent.into(),
        accent_fg: p.accent_fg.into(),
        sel_bg: p.sel_bg.into(),
        sel_fg: p.sel_fg.into(),
        red: p.red.into(),
        green: p.green.into(),
        yellow: p.yellow.into(),
        blue: p.blue.into(),
        magenta: p.magenta.into(),
        orange: p.orange.into(),
    }
}

/// The brand ramp: the same in every Theme (the icon, the splash, the wordmark).
pub const BRAND: [(&str, &str); 5] = [
    ("--brand-deep", "#00301b"),
    ("--brand", "#004225"),
    ("--brand-mid", "#0b5c39"),
    ("--brand-light", "#4f9d76"),
    ("--brand-pale", "#e9f0ea"),
];

/// Racing green, the default: the brand on a warm paper page.
fn racing_green() -> Theme {
    pal(
        "modern",
        false,
        P {
            bg: "#faf7f1",
            alt: "#f2ede3",
            raised: "#fffdf9",
            fg: "#1b1d1b",
            dim: "#5c625d",
            border: "#ddd7cb",
            accent: "#004225",
            accent_fg: "#faf7f1",
            sel_bg: "#e9f0ea",
            sel_fg: "#1b1d1b",
            red: "#a3312a",
            green: "#0b5c39",
            yellow: "#8a6d1d",
            blue: "#1f5f8b",
            magenta: "#7a3b69",
            orange: "#b0581c",
        },
    )
}

/// Racing green at night: the brand's light green on a dark slate.
fn racing_green_dark() -> Theme {
    pal(
        "modern",
        false,
        P {
            bg: "#12151a",
            alt: "#14181d",
            raised: "#191d23",
            fg: "#e7e4dd",
            dim: "#99a09d",
            border: "#2b3238",
            accent: "#4f9d76",
            accent_fg: "#12151a",
            sel_bg: "#17251d",
            sel_fg: "#e7e4dd",
            red: "#f28b82",
            green: "#6cb894",
            yellow: "#d8b74a",
            blue: "#78b6d9",
            magenta: "#c99ac0",
            orange: "#e0a066",
        },
    )
}

fn dark() -> Theme {
    pal(
        "modern",
        false,
        P {
            bg: "#1e1f22",
            alt: "#18191b",
            raised: "#2b2d31",
            fg: "#dcdde1",
            dim: "#80848e",
            border: "#313338",
            accent: "#4c8dff",
            accent_fg: "#1e1f22",
            sel_bg: "#2e436e",
            sel_fg: "#dcdde1",
            red: "#f28b82",
            green: "#81c995",
            yellow: "#fdd663",
            blue: "#78d9ec",
            magenta: "#c58af9",
            orange: "#fcad70",
        },
    )
}

fn light() -> Theme {
    pal(
        "modern",
        false,
        P {
            bg: "#ffffff",
            alt: "#f3f4f6",
            raised: "#ffffff",
            fg: "#1f2328",
            dim: "#6e7781",
            border: "#d8dee4",
            accent: "#0969da",
            accent_fg: "#ffffff",
            sel_bg: "#cfe3ff",
            sel_fg: "#1f2328",
            red: "#cf222e",
            green: "#1a7f37",
            yellow: "#9a6700",
            blue: "#0598bc",
            magenta: "#8250df",
            orange: "#bc4c00",
        },
    )
}

fn midnight() -> Theme {
    pal(
        "modern",
        false,
        P {
            bg: "#1a1b26",
            alt: "#16161e",
            raised: "#24283b",
            fg: "#c0caf5",
            dim: "#565f89",
            border: "#292e42",
            accent: "#7aa2f7",
            accent_fg: "#1a1b26",
            sel_bg: "#283457",
            sel_fg: "#c0caf5",
            red: "#f7768e",
            green: "#9ece6a",
            yellow: "#e0af68",
            blue: "#2ac3de",
            magenta: "#bb9af7",
            orange: "#ff9e64",
        },
    )
}

fn nord() -> Theme {
    pal(
        "modern",
        false,
        P {
            bg: "#2e3440",
            alt: "#272c36",
            raised: "#3b4252",
            fg: "#e5e9f0",
            dim: "#7b88a1",
            border: "#3b4252",
            accent: "#88c0d0",
            accent_fg: "#2e3440",
            sel_bg: "#434c5e",
            sel_fg: "#e5e9f0",
            red: "#bf616a",
            green: "#a3be8c",
            yellow: "#ebcb8b",
            blue: "#81a1c1",
            magenta: "#b48ead",
            orange: "#d08770",
        },
    )
}

fn amber() -> Theme {
    pal(
        "crt",
        true,
        P {
            bg: "#0a0600",
            alt: "#070400",
            raised: "#1a1000",
            fg: "#ffb000",
            dim: "#b37a00",
            border: "#4d3300",
            accent: "#ffb000",
            accent_fg: "#0a0600",
            sel_bg: "#4d3300",
            sel_fg: "#ffd27a",
            red: "#ff8c00",
            green: "#e0a020",
            yellow: "#ffe39c",
            blue: "#d9a84e",
            magenta: "#f0c070",
            orange: "#ffd27a",
        },
    )
}

fn paper() -> Theme {
    pal(
        "crt",
        true,
        P {
            bg: "#0b0b0e",
            alt: "#08080a",
            raised: "#141418",
            fg: "#e3e6ea",
            dim: "#888b90",
            border: "#44464c",
            accent: "#e3e6ea",
            accent_fg: "#0b0b0e",
            sel_bg: "#3a3a40",
            sel_fg: "#ffffff",
            red: "#ffffff",
            green: "#e3e6ea",
            yellow: "#b4b7bc",
            blue: "#b4b7bc",
            magenta: "#b4b7bc",
            orange: "#ffffff",
        },
    )
}

fn win31() -> Theme {
    pal(
        "win31",
        false,
        P {
            bg: "#ffffff",
            alt: "#ffffff",
            raised: "#c0c0c0",
            fg: "#000000",
            dim: "#808080",
            border: "#000000",
            accent: "#000080",
            accent_fg: "#ffffff",
            sel_bg: "#000080",
            sel_fg: "#ffffff",
            red: "#800000",
            green: "#008000",
            yellow: "#808000",
            blue: "#0000ff",
            magenta: "#800080",
            orange: "#808000",
        },
    )
}

fn win95() -> Theme {
    pal(
        "win95",
        false,
        P {
            bg: "#ffffff",
            alt: "#c0c0c0",
            raised: "#c0c0c0",
            fg: "#000000",
            dim: "#808080",
            border: "#808080",
            accent: "#000080",
            accent_fg: "#ffffff",
            sel_bg: "#000080",
            sel_fg: "#ffffff",
            red: "#800000",
            green: "#008000",
            yellow: "#808000",
            blue: "#0000ff",
            magenta: "#800080",
            orange: "#808000",
        },
    )
}

fn winxp() -> Theme {
    pal(
        "winxp",
        false,
        P {
            bg: "#ffffff",
            alt: "#d6dff7",
            raised: "#ece9d8",
            fg: "#000000",
            dim: "#7f7f7f",
            border: "#7f9db9",
            accent: "#316ac5",
            accent_fg: "#ffffff",
            sel_bg: "#316ac5",
            sel_fg: "#ffffff",
            red: "#c00000",
            green: "#008000",
            yellow: "#a07000",
            blue: "#0000ff",
            magenta: "#800080",
            orange: "#c05000",
        },
    )
}

fn win7() -> Theme {
    pal(
        "win7",
        false,
        P {
            bg: "#ffffff",
            alt: "#f1f5fb",
            raised: "#f0f0f0",
            fg: "#1e1e1e",
            dim: "#6d6d6d",
            border: "#d5dfe5",
            accent: "#3399ff",
            accent_fg: "#ffffff",
            sel_bg: "#cce8ff",
            sel_fg: "#000000",
            red: "#c42b1c",
            green: "#107c10",
            yellow: "#9d5d00",
            blue: "#0066cc",
            magenta: "#881798",
            orange: "#ca5010",
        },
    )
}

fn win10() -> Theme {
    pal(
        "win10",
        false,
        P {
            bg: "#ffffff",
            alt: "#ffffff",
            raised: "#f0f0f0",
            fg: "#000000",
            dim: "#6d6d6d",
            border: "#e5e5e5",
            accent: "#0078d7",
            accent_fg: "#ffffff",
            sel_bg: "#cce8ff",
            sel_fg: "#000000",
            red: "#c42b1c",
            green: "#107c10",
            yellow: "#9d5d00",
            blue: "#0063b1",
            magenta: "#881798",
            orange: "#ca5010",
        },
    )
}

fn win11() -> Theme {
    pal(
        "win11",
        false,
        P {
            bg: "#ffffff",
            alt: "#f3f3f3",
            raised: "#fbfbfb",
            fg: "#1a1a1a",
            dim: "#5f5f5f",
            border: "#e5e5e5",
            accent: "#005fb8",
            accent_fg: "#ffffff",
            sel_bg: "#d3e5f7",
            sel_fg: "#1a1a1a",
            red: "#c42b1c",
            green: "#0f7b0f",
            yellow: "#9d5d00",
            blue: "#005fb8",
            magenta: "#881798",
            orange: "#ca5010",
        },
    )
}

fn win11_dark() -> Theme {
    pal(
        "win11",
        false,
        P {
            bg: "#1c1c1c",
            alt: "#202020",
            raised: "#2b2b2b",
            fg: "#ffffff",
            dim: "#9d9d9d",
            border: "#333333",
            accent: "#4cc2ff",
            accent_fg: "#1c1c1c",
            sel_bg: "#2e4a66",
            sel_fg: "#ffffff",
            red: "#ff99a4",
            green: "#6ccb5f",
            yellow: "#fce100",
            blue: "#60cdff",
            magenta: "#d59dff",
            orange: "#fcb57c",
        },
    )
}

fn system7() -> Theme {
    pal(
        "system7",
        false,
        P {
            bg: "#ffffff",
            alt: "#ffffff",
            raised: "#ffffff",
            fg: "#000000",
            dim: "#555555",
            border: "#000000",
            accent: "#000000",
            accent_fg: "#ffffff",
            sel_bg: "#000000",
            sel_fg: "#ffffff",
            red: "#dd0806",
            green: "#006411",
            yellow: "#90713a",
            blue: "#0000d4",
            magenta: "#f20884",
            orange: "#ff6403",
        },
    )
}

fn platinum() -> Theme {
    pal(
        "platinum",
        false,
        P {
            bg: "#ffffff",
            alt: "#dddddd",
            raised: "#dddddd",
            fg: "#000000",
            dim: "#777777",
            border: "#999999",
            accent: "#6666cc",
            accent_fg: "#ffffff",
            sel_bg: "#ccccff",
            sel_fg: "#000000",
            red: "#dd0806",
            green: "#006411",
            yellow: "#90713a",
            blue: "#0000d4",
            magenta: "#b000a0",
            orange: "#d05000",
        },
    )
}

fn aqua() -> Theme {
    pal(
        "aqua",
        false,
        P {
            bg: "#ffffff",
            alt: "#e8edf5",
            raised: "#ececec",
            fg: "#000000",
            dim: "#808080",
            border: "#a5a5a5",
            accent: "#3875d7",
            accent_fg: "#ffffff",
            sel_bg: "#3875d7",
            sel_fg: "#ffffff",
            red: "#c4161c",
            green: "#1d8b2c",
            yellow: "#a07000",
            blue: "#1a5fd0",
            magenta: "#9b30b0",
            orange: "#d06000",
        },
    )
}

fn macos() -> Theme {
    pal(
        "macos",
        false,
        P {
            bg: "#ffffff",
            alt: "#ececec",
            raised: "#f6f6f6",
            fg: "#1d1d1f",
            dim: "#86868b",
            border: "#e0e0e0",
            accent: "#007aff",
            accent_fg: "#ffffff",
            sel_bg: "#0064e1",
            sel_fg: "#ffffff",
            red: "#d70015",
            green: "#248a3d",
            yellow: "#a05a00",
            blue: "#0071e3",
            magenta: "#8944ab",
            orange: "#c93400",
        },
    )
}

fn macos_dark() -> Theme {
    pal(
        "macos",
        false,
        P {
            bg: "#1e1e1e",
            alt: "#2a2a2a",
            raised: "#323232",
            fg: "#e5e5e5",
            dim: "#98989d",
            border: "#3a3a3a",
            accent: "#0a84ff",
            accent_fg: "#1e1e1e",
            sel_bg: "#0a5ad6",
            sel_fg: "#ffffff",
            red: "#ff6961",
            green: "#30d158",
            yellow: "#ffd60a",
            blue: "#64d2ff",
            magenta: "#bf5af2",
            orange: "#ff9f0a",
        },
    )
}

fn next() -> Theme {
    pal(
        "next",
        false,
        P {
            bg: "#cccccc",
            alt: "#aaaaaa",
            raised: "#aaaaaa",
            fg: "#000000",
            dim: "#555555",
            border: "#555555",
            accent: "#000000",
            accent_fg: "#ffffff",
            sel_bg: "#ffffff",
            sel_fg: "#000000",
            red: "#550000",
            green: "#005500",
            yellow: "#804000",
            blue: "#0000aa",
            magenta: "#800080",
            orange: "#804000",
        },
    )
}

fn beos() -> Theme {
    pal(
        "beos",
        false,
        P {
            bg: "#ffffff",
            alt: "#d8d8d8",
            raised: "#d8d8d8",
            fg: "#000000",
            dim: "#777777",
            border: "#acacac",
            accent: "#336698",
            accent_fg: "#ffffff",
            sel_bg: "#7378b8",
            sel_fg: "#000000",
            red: "#cc0000",
            green: "#008000",
            yellow: "#8f6b00",
            blue: "#0000e5",
            magenta: "#800080",
            orange: "#a03800",
        },
    )
}

fn os2() -> Theme {
    pal(
        "win95",
        false,
        P {
            bg: "#cccccc",
            alt: "#cccccc",
            raised: "#cccccc",
            fg: "#000000",
            dim: "#666666",
            border: "#808080",
            accent: "#000080",
            accent_fg: "#cccccc",
            sel_bg: "#000080",
            sel_fg: "#ffffff",
            red: "#800000",
            green: "#005a00",
            yellow: "#5c4d00",
            blue: "#0000ff",
            magenta: "#800080",
            orange: "#803300",
        },
    )
}

fn workbench() -> Theme {
    pal(
        "workbench",
        false,
        P {
            bg: "#0055aa",
            alt: "#0055aa",
            raised: "#0055aa",
            fg: "#ffffff",
            dim: "#99bbee",
            border: "#ffffff",
            accent: "#ff8800",
            accent_fg: "#000000",
            sel_bg: "#ff8800",
            sel_fg: "#000000",
            red: "#ffbbbb",
            green: "#66ff66",
            yellow: "#ffcc66",
            blue: "#88ddff",
            magenta: "#ffbbff",
            orange: "#ffcc66",
        },
    )
}

fn atarist() -> Theme {
    pal(
        "gem",
        false,
        P {
            bg: "#ffffff",
            alt: "#ffffff",
            raised: "#00ff00",
            fg: "#000000",
            dim: "#6d6d6d",
            border: "#000000",
            accent: "#000000",
            accent_fg: "#ffffff",
            sel_bg: "#000000",
            sel_fg: "#ffffff",
            red: "#db0000",
            green: "#006d00",
            yellow: "#6d6d00",
            blue: "#0049ff",
            magenta: "#b649b6",
            orange: "#b64900",
        },
    )
}

fn c64() -> Theme {
    pal(
        "crt",
        true,
        P {
            bg: "#2e2c9b",
            alt: "#2e2c9b",
            raised: "#2e2c9b",
            fg: "#b3b0f3",
            dim: "#b2b2b2",
            border: "#706deb",
            accent: "#b3b0f3",
            accent_fg: "#2e2c9b",
            sel_bg: "#b3b0f3",
            sel_fg: "#2e2c9b",
            red: "#d2a09c",
            green: "#a9ff9f",
            yellow: "#edf171",
            blue: "#75cec8",
            magenta: "#c6a1cc",
            orange: "#edf171",
        },
    )
}

fn spectrum() -> Theme {
    pal(
        "crt",
        true,
        P {
            bg: "#000000",
            alt: "#000000",
            raised: "#000000",
            fg: "#ffffff",
            dim: "#d7d7d7",
            border: "#d7d7d7",
            accent: "#00ffff",
            accent_fg: "#000000",
            sel_bg: "#0000d7",
            sel_fg: "#ffffff",
            red: "#ff0000",
            green: "#00ff00",
            yellow: "#ffff00",
            blue: "#00d7d7",
            magenta: "#ff00ff",
            orange: "#00ffff",
        },
    )
}

fn cpc() -> Theme {
    pal(
        "crt",
        true,
        P {
            bg: "#000080",
            alt: "#000080",
            raised: "#0000ff",
            fg: "#ffff00",
            dim: "#8080ff",
            border: "#8080ff",
            accent: "#ffff00",
            accent_fg: "#000080",
            sel_bg: "#ffff00",
            sel_fg: "#000080",
            red: "#ff8080",
            green: "#00ff00",
            yellow: "#ff8000",
            blue: "#80ffff",
            magenta: "#ff00ff",
            orange: "#ff8000",
        },
    )
}

fn msx() -> Theme {
    pal(
        "crt",
        true,
        P {
            bg: "#2424ff",
            alt: "#2424ff",
            raised: "#000000",
            fg: "#ffffff",
            dim: "#b6b6b6",
            border: "#496dff",
            accent: "#49dbff",
            accent_fg: "#2424ff",
            sel_bg: "#000000",
            sel_fg: "#ffffff",
            red: "#ffb6b6",
            green: "#6dff6d",
            yellow: "#dbdb24",
            blue: "#92dbff",
            magenta: "#ffb6ff",
            orange: "#ffdb49",
        },
    )
}

fn bbc() -> Theme {
    pal(
        "crt",
        true,
        P {
            bg: "#000000",
            alt: "#000000",
            raised: "#000000",
            fg: "#ffffff",
            dim: "#ff00ff",
            border: "#0000ff",
            accent: "#ffff00",
            accent_fg: "#000000",
            sel_bg: "#0000ff",
            sel_fg: "#ffffff",
            red: "#ff0000",
            green: "#00ff00",
            yellow: "#ffff00",
            blue: "#00ffff",
            magenta: "#00ffff",
            orange: "#ffff00",
        },
    )
}

/// A built-in Theme: its name (its `theme.<name>` text), its group (a `theme.group.*` key) and
/// how it is made.
pub type Builtin = (&'static str, &'static str, fn() -> Theme);

/// Every built-in Theme, in the order Settings lists them: a group's Themes stand together.
pub const BUILTIN: &[Builtin] = &[
    ("racing-green", "theme.group.bragd", racing_green),
    ("racing-green-dark", "theme.group.bragd", racing_green_dark),
    ("dark", "theme.group.bragd", dark),
    ("light", "theme.group.bragd", light),
    ("midnight", "theme.group.bragd", midnight),
    ("nord", "theme.group.bragd", nord),
    ("amber", "theme.group.bragd", amber),
    ("paper", "theme.group.bragd", paper),
    ("win31", "theme.group.windows", win31),
    ("win95", "theme.group.windows", win95),
    ("winxp", "theme.group.windows", winxp),
    ("win7", "theme.group.windows", win7),
    ("win10", "theme.group.windows", win10),
    ("win11", "theme.group.windows", win11),
    ("win11-dark", "theme.group.windows", win11_dark),
    ("system7", "theme.group.mac", system7),
    ("platinum", "theme.group.mac", platinum),
    ("aqua", "theme.group.mac", aqua),
    ("macos", "theme.group.mac", macos),
    ("macos-dark", "theme.group.mac", macos_dark),
    ("next", "theme.group.desktops", next),
    ("beos", "theme.group.desktops", beos),
    ("os2", "theme.group.desktops", os2),
    ("workbench", "theme.group.desktops", workbench),
    ("atarist", "theme.group.desktops", atarist),
    ("c64", "theme.group.home", c64),
    ("spectrum", "theme.group.home", spectrum),
    ("cpc", "theme.group.home", cpc),
    ("msx", "theme.group.home", msx),
    ("bbc", "theme.group.home", bbc),
];

/// The groups, in order: `theme.group.own` holds the user's `[themes.<name>]`.
pub const GROUPS: [&str; 6] = [
    "theme.group.bragd",
    "theme.group.windows",
    "theme.group.mac",
    "theme.group.desktops",
    "theme.group.home",
    "theme.group.own",
];

/// Every Look (`look.<name>` names it): the chrome and shapes laid over a Theme's colours.
pub const LOOKS: &[&str] = &[
    "modern",
    "crt",
    "workbench",
    "gem",
    "win31",
    "win95",
    "winxp",
    "win7",
    "win10",
    "win11",
    "system7",
    "platinum",
    "aqua",
    "macos",
    "next",
    "beos",
];

/// The built-in Theme `name`.
pub fn builtin(name: &str) -> Option<Theme> {
    BUILTIN
        .iter()
        .find(|(n, ..)| *n == name)
        .map(|(_, _, f)| f())
}

/// The group a Theme is listed under: a `theme.group.*` key; *Your own* for one not built in.
pub fn group(name: &str) -> &'static str {
    BUILTIN
        .iter()
        .find(|(n, ..)| *n == name)
        .map_or("theme.group.own", |(_, g, _)| g)
}

/// Every Theme Settings offers, (name, group): the built-in ones, then the user's own.
pub fn names(cfg: &Config) -> Vec<(String, &'static str)> {
    let mut out: Vec<(String, &'static str)> = BUILTIN
        .iter()
        .map(|(n, g, _)| (n.to_string(), *g))
        .collect();
    out.extend(
        cfg.themes
            .keys()
            .filter(|n| builtin(n).is_none())
            .map(|n| (n.clone(), "theme.group.own")),
    );
    out
}

/// A value safe to hand to CSS as a colour: no way out of the declaration it goes in.
fn safe(c: &str) -> bool {
    !c.is_empty()
        && c.len() < 64
        && c.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || " #(),./%-".contains(ch))
}

/// The Theme `cfg` names (a user's own table first, then a built-in one), its unsafe colours
/// replaced with Racing green's and its Look with `[look] look` when that is set.
pub fn theme(cfg: &Config) -> Theme {
    let name = &cfg.look.theme;
    let mut t = cfg
        .themes
        .get(name)
        .cloned()
        .or_else(|| builtin(name))
        .unwrap_or_default();
    let base = racing_green();
    for (mine, theirs) in t.colours_mut().into_iter().zip(base.colours()) {
        if !safe(mine) {
            *mine = theirs.to_string();
        }
    }
    if LOOKS.contains(&cfg.look.look.as_str()) {
        t.look = cfg.look.look.clone();
    }
    if !LOOKS.contains(&t.look.as_str()) {
        t.look = "modern".into();
    }
    t
}

/// sRGB `#rrggbb` (or `#rgb`) as its channels.
fn rgb(c: &str) -> Option<(u8, u8, u8)> {
    let hex = c.strip_prefix('#')?;
    let hex = match hex.len() {
        3 => hex.chars().flat_map(|ch| [ch, ch]).collect(),
        6 => hex.to_string(),
        _ => return None,
    };
    let v = u32::from_str_radix(&hex, 16).ok()?;
    Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// Relative luminance (WCAG), 0 to 1; a colour that is not hex counts as mid grey.
fn luminance(c: &str) -> f64 {
    let Some((r, g, b)) = rgb(c) else { return 0.5 };
    let lin = |x: u8| {
        let x = f64::from(x) / 255.0;
        if x <= 0.039_28 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

impl Theme {
    fn colours(&self) -> [&str; 16] {
        [
            &self.bg,
            &self.alt,
            &self.raised,
            &self.fg,
            &self.dim,
            &self.border,
            &self.accent,
            &self.accent_fg,
            &self.sel_bg,
            &self.sel_fg,
            &self.red,
            &self.green,
            &self.yellow,
            &self.blue,
            &self.magenta,
            &self.orange,
        ]
    }

    fn colours_mut(&mut self) -> [&mut String; 16] {
        [
            &mut self.bg,
            &mut self.alt,
            &mut self.raised,
            &mut self.fg,
            &mut self.dim,
            &mut self.border,
            &mut self.accent,
            &mut self.accent_fg,
            &mut self.sel_bg,
            &mut self.sel_fg,
            &mut self.red,
            &mut self.green,
            &mut self.yellow,
            &mut self.blue,
            &mut self.magenta,
            &mut self.orange,
        ]
    }

    /// A dark Theme: light text on a dark page.
    pub fn dark(&self) -> bool {
        luminance(&self.bg) < luminance(&self.fg)
    }

    /// Its colours as CSS variables (`--bg`, `--accent-fg`, …), and the brand ramp.
    pub fn css_vars(&self) -> Vec<(String, String)> {
        const NAMES: [&str; 16] = [
            "bg",
            "alt",
            "raised",
            "fg",
            "dim",
            "border",
            "accent",
            "accent-fg",
            "sel-bg",
            "sel-fg",
            "red",
            "green",
            "yellow",
            "blue",
            "magenta",
            "orange",
        ];
        NAMES
            .iter()
            .zip(self.colours())
            .map(|(n, c)| (format!("--{n}"), c.to_string()))
            .chain(BRAND.iter().map(|(n, c)| (n.to_string(), c.to_string())))
            .collect()
    }
}

/// A Board theme: exactly four colours.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BoardTheme {
    pub light: String,
    pub dark: String,
    pub highlight: String,
    pub marker: String,
}

/// Every Board theme (`board_theme.<name>` names it), the default first. `match` takes its
/// colours from the Theme.
pub const BOARD_THEMES: [&str; 4] = ["racing", "slate", "walnut", "match"];

/// The marker: the dark square's colour, at 40 %.
fn marker(dark: &str) -> String {
    let (r, g, b) = rgb(dark).unwrap_or((23, 85, 58));
    format!("rgb({r} {g} {b} / .4)")
}

fn board(light: &str, dark: &str, highlight: &str) -> BoardTheme {
    BoardTheme {
        light: light.into(),
        dark: dark.into(),
        highlight: highlight.into(),
        marker: marker(dark),
    }
}

/// The Board theme `name` (Racing green for one not known), for the Theme `t`. A Board theme
/// never inverts with a dark Theme: `match` takes the lighter of the Theme's page and text as
/// its light square, so the light square stays the lighter one.
pub fn board_theme(name: &str, t: &Theme) -> BoardTheme {
    match name {
        "slate" => board("#dfe3e8", "#68757f", "#e0b64a"),
        "walnut" => board("#e9d3b0", "#9a6c46", "#d8b74a"),
        "match" => {
            let light = if t.dark() { &t.fg } else { &t.bg };
            let l = luminance(light);
            let dark = [&t.accent, &t.sel_bg, &t.border, &t.dim, &t.fg, &t.bg]
                .into_iter()
                .find(|c| rgb(c).is_some() && luminance(c) + 0.2 <= l)
                .unwrap_or(&t.accent);
            board(light, dark, &t.yellow)
        }
        _ => board("#efe7d6", "#17553a", "#d8b74a"),
    }
}

impl BoardTheme {
    /// Its colours as CSS variables: `--board-light`, `--board-dark`, `--board-highlight`,
    /// `--board-marker`.
    pub fn css_vars(&self) -> Vec<(String, String)> {
        [
            ("light", &self.light),
            ("dark", &self.dark),
            ("highlight", &self.highlight),
            ("marker", &self.marker),
        ]
        .into_iter()
        .map(|(n, c)| (format!("--board-{n}"), c.clone()))
        .collect()
    }
}

/// Everything the desktop app applies to the page: `data-look`, the tube's glow, whether the
/// Theme is dark, and every CSS variable of the Theme and the Board theme.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Appearance {
    pub look: String,
    pub crt: bool,
    pub dark: bool,
    pub vars: Vec<(String, String)>,
}

pub fn appearance(cfg: &Config) -> Appearance {
    let t = theme(cfg);
    let mut vars = t.css_vars();
    vars.extend(board_theme(&cfg.look.board_theme, &t).css_vars());
    Appearance {
        look: t.look.clone(),
        crt: t.crt,
        dark: t.dark(),
        vars,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themes_catalogue_has_racing_green_first() {
        assert_eq!(BUILTIN.len(), 30);
        assert_eq!(BUILTIN[0].0, "racing-green");
        assert_eq!(theme(&Config::default()), racing_green());
        let mut names: Vec<&str> = BUILTIN.iter().map(|(n, ..)| *n).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), BUILTIN.len());
        for (n, g, f) in BUILTIN {
            assert!(GROUPS.contains(g), "{n}");
            assert!(LOOKS.contains(&f().look.as_str()), "{n}: {}", f().look);
            assert!(f().colours().iter().all(|c| rgb(c).is_some()), "{n}");
        }
        assert!(!racing_green().dark() && racing_green_dark().dark());
    }

    #[test]
    fn themes_user_theme_fills_from_racing_green_and_is_made_safe() {
        let cfg = Config::parse(
            "[look]\ntheme = \"mine\"\nlook = \"aqua\"\n[themes.mine]\naccent = \"#ff0000\"\nfg = \"red; } body { x: y\"\n",
        )
        .unwrap();
        let t = theme(&cfg);
        assert_eq!(t.accent, "#ff0000");
        assert_eq!(t.fg, racing_green().fg);
        assert_eq!(t.bg, racing_green().bg);
        assert_eq!(t.look, "aqua");
        assert_eq!(group("mine"), "theme.group.own");
        assert!(
            names(&cfg)
                .iter()
                .any(|(n, g)| n == "mine" && *g == "theme.group.own")
        );
        // An unknown Theme falls back to Racing green.
        let cfg = Config::parse("[look]\ntheme = \"nope\"\n").unwrap();
        assert_eq!(theme(&cfg), racing_green());
    }

    #[test]
    fn themes_board_themes_have_four_colours_and_never_invert() {
        let rg = racing_green();
        let b = board_theme("racing", &rg);
        assert_eq!(
            (b.light.as_str(), b.dark.as_str(), b.highlight.as_str()),
            ("#efe7d6", "#17553a", "#d8b74a")
        );
        assert_eq!(b.marker, "rgb(23 85 58 / .4)");
        assert_eq!(board_theme("nope", &rg), b);
        assert_eq!(board_theme("slate", &rg).dark, "#68757f");
        assert_eq!(board_theme("walnut", &rg).light, "#e9d3b0");
        for (n, _, f) in BUILTIN {
            let t = f();
            let m = board_theme("match", &t);
            assert!(
                luminance(&m.light) >= luminance(&m.dark) + 0.2,
                "{n}: {m:?}"
            );
        }
    }

    #[test]
    fn themes_appearance_gives_every_variable() {
        let a = appearance(&Config::default());
        assert_eq!(a.look, "modern");
        assert!(!a.dark);
        let get = |k: &str| a.vars.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("--bg"), Some("#faf7f1"));
        assert_eq!(get("--brand"), Some("#004225"));
        assert_eq!(get("--board-dark"), Some("#17553a"));
        assert_eq!(get("--accent-fg"), Some("#faf7f1"));
        assert_eq!(a.vars.len(), 16 + 5 + 4);
    }
}
