//! The engines Bragd knows, per Game family and Variant: where to find each one installed,
//! the protocol it speaks, and its sealed image. An installed engine is used first; otherwise
//! its image from `ghcr.io/mwo-dk/bragd-<engine>`, once pulled.

use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::protocol::Protocol;
use crate::session::Program;
use crate::tools::{self, Runtime};

/// The Game families that have outside engines; tafl, ur and senet have Bragd's own Bots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    Chess,
    Xiangqi,
    Shogi,
    Go,
    Draughts,
    Backgammon,
}

/// The Variant keys of each Family, as engines are set up for them.
pub const VARIANTS: &[(Family, &[&str])] = &[
    (Family::Chess, &["chess", "chess960", "shatranj", "chaturanga", "kingofthehill", "racingkings", "seirawan", "capablanca"]),
    (Family::Xiangqi, &["xiangqi", "janggi"]),
    (Family::Shogi, &["shogi"]),
    (Family::Go, &["go9", "go13", "go19"]),
    (Family::Draughts, &["english", "international", "brazilian", "russian"]),
    (Family::Backgammon, &["backgammon"]),
];

/// The Family of a Variant key.
pub fn family(variant: &str) -> Option<Family> {
    VARIANTS.iter().find(|(_, v)| v.contains(&variant)).map(|(f, _)| *f)
}

/// Where Bragd's engine images are published.
pub const REGISTRY: &str = "ghcr.io/mwo-dk";

// The digests the images are pinned to. None: the image is not published yet, so it is never
// pulled. TODO(unpublished): set each when `packaging/images/<engine>` is first pushed.
pub const PIN_STOCKFISH: Option<&str> = None;
pub const PIN_FAIRY_STOCKFISH: Option<&str> = None;
pub const PIN_PIKAFISH: Option<&str> = None;
pub const PIN_KATAGO: Option<&str> = None;
pub const PIN_GNUGO: Option<&str> = None;

/// An engine Bragd knows.
#[derive(Debug, PartialEq)]
pub struct Known {
    /// Its key: the image is `bragd-<key>`, and it is what settings name.
    pub key: &'static str,
    pub name: &'static str,
    /// Its program's names, the one its packages install first.
    pub programs: &'static [&'static str],
    /// What it needs on its command line to speak its protocol, installed; an image's own
    /// entrypoint already speaks it.
    pub args: &'static [&'static str],
    pub variants: &'static [&'static str],
    /// The image's tag and digest; no tag: Bragd builds no image of it.
    pub tag: Option<&'static str>,
    pub pin: Option<&'static str>,
    pub site: &'static str,
    /// It reads Syzygy tablebases (`SyzygyPath`).
    pub syzygy: bool,
}

const CHESS: &[&str] = &["chess", "chess960"];
const FAIRY: &[&str] = &["chess", "chess960", "shatranj", "chaturanga", "kingofthehill", "racingkings", "seirawan", "capablanca", "xiangqi", "janggi", "shogi"];
const GO: &[&str] = &["go9", "go13", "go19"];

/// The engines of the spec. English, Brazilian and Russian draughts have none: Scan plays
/// international draughts.
pub const KNOWN: &[Known] = &[
    Known { key: "stockfish", name: "Stockfish", programs: &["stockfish"], args: &[], variants: CHESS, tag: Some("17.1"), pin: PIN_STOCKFISH, site: "https://stockfishchess.org/download/", syzygy: true },
    Known {
        key: "fairy-stockfish",
        name: "Fairy-Stockfish",
        programs: &["fairy-stockfish", "fairy-stockfish-largeboard", "fairy-stockfish_largeboard"],
        args: &[],
        variants: FAIRY,
        tag: Some("14.0.1"),
        pin: PIN_FAIRY_STOCKFISH,
        site: "https://github.com/fairy-stockfish/Fairy-Stockfish/releases",
        syzygy: true,
    },
    Known { key: "pikafish", name: "Pikafish", programs: &["pikafish"], args: &[], variants: &["xiangqi"], tag: Some("2026-09-06"), pin: PIN_PIKAFISH, site: "https://github.com/official-pikafish/Pikafish/releases", syzygy: false },
    Known { key: "yaneuraou", name: "YaneuraOu", programs: &["YaneuraOu", "yaneuraou", "YaneuraOu-by-gcc"], args: &[], variants: &["shogi"], tag: None, pin: None, site: "https://github.com/yaneurao/YaneuraOu/releases", syzygy: false },
    Known { key: "katago", name: "KataGo", programs: &["katago"], args: &["gtp"], variants: GO, tag: Some("1.18.2-eigen"), pin: PIN_KATAGO, site: "https://github.com/lightvector/KataGo/releases", syzygy: false },
    Known { key: "gnugo", name: "GNU Go", programs: &["gnugo"], args: &["--mode", "gtp"], variants: GO, tag: Some("3.8"), pin: PIN_GNUGO, site: "https://www.gnu.org/software/gnugo/", syzygy: false },
    Known { key: "scan", name: "Scan", programs: &["scan"], args: &["hub"], variants: &["international"], tag: None, pin: None, site: "https://hjetten.home.xs4all.nl/scan/scan.html", syzygy: false },
    Known { key: "gnubg", name: "GNU Backgammon", programs: &["gnubg"], args: &["-t", "-q"], variants: &["backgammon"], tag: None, pin: None, site: "https://www.gnu.org/software/gnubg/", syzygy: false },
];

impl Known {
    /// The protocol it speaks for `variant`: Fairy-Stockfish speaks USI for shogi, so its moves
    /// are in shogi's own notation, and UCI otherwise.
    pub fn protocol(&self, variant: &str) -> Protocol {
        match self.key {
            "fairy-stockfish" if variant == "shogi" => Protocol::Usi,
            "yaneuraou" => Protocol::Usi,
            "katago" | "gnugo" => Protocol::Gtp,
            "scan" => Protocol::Hub,
            "gnubg" => Protocol::Gnubg,
            _ => Protocol::Uci,
        }
    }

    /// Its image, without the digest: `ghcr.io/mwo-dk/bragd-stockfish:17.1`.
    pub fn image_name(&self) -> Option<String> {
        Some(format!("{REGISTRY}/bragd-{}:{}", self.key, self.tag?))
    }

    /// Its image pinned by digest, the only form ever pulled or run; None while unpublished.
    pub fn image(&self) -> Option<String> {
        Some(format!("{}@{}", self.image_name()?, self.pin?))
    }

    /// Its program installed here, only looked for.
    pub fn installed(&self) -> Option<PathBuf> {
        self.programs.iter().find_map(|p| tools::find(p))
    }
}

/// The engines known for `variant`.
pub fn for_variant(variant: &str) -> impl Iterator<Item = &'static Known> + '_ {
    KNOWN.iter().filter(move |k| k.variants.contains(&variant))
}

/// The known engine `key`.
pub fn known(key: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.key == key)
}

/// The images of every known engine that is published: those Bragd keeps up to date.
pub fn images() -> Vec<String> {
    KNOWN.iter().filter_map(Known::image).collect()
}

/// Why there is no engine to run.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Missing {
    /// Not installed, and its image is pulled once asked (`image`).
    NotPulled { image: String },
    /// Not installed, and Bragd has no image of it (yet).
    NoImage,
    /// Not installed, and neither podman nor docker is.
    NoRuntime,
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Missing::NotPulled { image } => write!(f, "Not installed. Its image {} can be downloaded.", crate::images::short(image)),
            Missing::NoImage => write!(f, "Not installed, and Bragd has no image of it yet."),
            Missing::NoRuntime => write!(f, "Not installed, and neither podman nor docker is there to run its image."),
        }
    }
}

/// The program to run for `k`: installed here (`installed`), else its pinned image when the
/// runtime has it (`has`). `syzygy`: the tablebase folder an image is given read-only.
pub fn choose(k: &Known, installed: Option<PathBuf>, rt: Option<&Runtime>, has: impl Fn(&Runtime, &str) -> bool, syzygy: Option<&Path>) -> Result<Program, Missing> {
    let args: Vec<String> = k.args.iter().map(|a| a.to_string()).collect();
    if let Some(path) = installed {
        return Ok(Program::Installed { path, args });
    }
    let image = k.image().ok_or(Missing::NoImage)?;
    let rt = rt.ok_or(Missing::NoRuntime)?;
    if !has(rt, &image) {
        return Err(Missing::NotPulled { image });
    }
    Ok(Program::Container { rt: rt.clone(), image, args: vec![], syzygy: syzygy.filter(|_| k.syzygy).map(Path::to_path_buf) })
}

/// The program `configured` names, a path or a bare name: a bare name is looked for on PATH
/// only, never in the working folder (where a downloaded file could carry an engine's name);
/// `~` is the home folder `home`, and a relative path starts there.
pub fn configured(configured: &str, home: &Path) -> Option<PathBuf> {
    let c = configured.trim();
    if c.is_empty() {
        return None;
    }
    if !c.starts_with('~') && !c.contains(['/', '\\']) {
        return tools::find(c);
    }
    let rest = c.strip_prefix('~').map(|r| r.trim_start_matches(['/', '\\']));
    let p = match rest {
        Some(r) => home.join(r),
        None if Path::new(c).is_absolute() => PathBuf::from(c),
        None => home.join(c),
    };
    p.is_file().then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_engine_family_has_its_protocol() {
        let fairy = known("fairy-stockfish").unwrap();
        assert_eq!((fairy.protocol("xiangqi"), fairy.protocol("shogi")), (Protocol::Uci, Protocol::Usi));
        assert_eq!(known("pikafish").unwrap().protocol("xiangqi"), Protocol::Uci, "Pikafish speaks UCI");
        assert_eq!(known("katago").unwrap().protocol("go19"), Protocol::Gtp);
        assert_eq!(known("scan").unwrap().protocol("international"), Protocol::Hub);
        assert_eq!(known("gnubg").unwrap().protocol("backgammon"), Protocol::Gnubg);
        for k in KNOWN {
            assert!(k.variants.iter().all(|v| family(v).is_some()), "{}: a Variant no Family has", k.key);
        }
        let xiangqi: Vec<&str> = for_variant("xiangqi").map(|k| k.key).collect();
        assert_eq!(xiangqi, ["fairy-stockfish", "pikafish"]);
        assert_eq!(for_variant("english").count(), 0);
        assert_eq!(known("stockfish").unwrap().image_name().as_deref(), Some("ghcr.io/mwo-dk/bragd-stockfish:17.1"));
    }

    #[test]
    fn pins_are_digests_and_unpublished_images_are_never_offered() {
        for k in KNOWN {
            if let Some(pin) = k.pin {
                assert!(pin.len() == 71 && pin.starts_with("sha256:"), "{}: {pin}", k.key);
                assert!(k.tag.is_some());
            }
            assert_eq!(k.image().is_some(), k.pin.is_some() && k.tag.is_some());
        }
        assert!(images().iter().all(|i| i.contains("@sha256:")));
    }

    #[test]
    fn installed_first_else_the_image() {
        let rt = Runtime { name: "podman", path: PathBuf::from("/nonexistent/podman") };
        let k = Known { key: "stockfish", name: "Stockfish", programs: &["stockfish"], args: &[], variants: CHESS, tag: Some("17.1"), pin: Some("sha256:1111111111111111111111111111111111111111111111111111111111111111"), site: "", syzygy: true };
        let here = PathBuf::from("/usr/games/stockfish");
        assert_eq!(choose(&k, Some(here.clone()), Some(&rt), |_, _| true, None), Ok(Program::Installed { path: here, args: vec![] }));
        let image = k.image().unwrap();
        let tb = Path::new("/data/syzygy");
        assert_eq!(choose(&k, None, Some(&rt), |_, _| true, Some(tb)), Ok(Program::Container { rt: rt.clone(), image: image.clone(), args: vec![], syzygy: Some(tb.into()) }));
        assert_eq!(choose(&k, None, Some(&rt), |_, _| false, None), Err(Missing::NotPulled { image }));
        assert_eq!(choose(&k, None, None, |_, _| true, None), Err(Missing::NoRuntime));
        assert_eq!(choose(known("pikafish").unwrap(), None, Some(&rt), |_, _| true, None), Err(Missing::NoImage), "unpublished");
        // GNU Go installed gets its protocol on its command line; its image's entrypoint has it.
        let g = known("gnugo").unwrap();
        let gnugo = Known { pin: k.pin, programs: g.programs, args: g.args, variants: g.variants, tag: g.tag, site: g.site, ..k };
        assert_eq!(choose(&gnugo, Some("/usr/bin/gnugo".into()), None, |_, _| false, None).unwrap(), Program::Installed { path: "/usr/bin/gnugo".into(), args: vec!["--mode".into(), "gtp".into()] });
        let gnugo = Known { syzygy: false, ..gnugo };
        assert!(matches!(choose(&gnugo, None, Some(&rt), |_, _| true, Some(tb)).unwrap(), Program::Container { args, syzygy: None, .. } if args.is_empty()), "no tablebases for go");
    }

    #[cfg(unix)]
    #[test]
    fn a_configured_engine_by_path_or_name() {
        let d = crate::test_dir("configured");
        let p = crate::fake_program(&d, "pikafish", "exit 0");
        assert_eq!(configured(&p.to_string_lossy(), Path::new("/")), Some(p.clone()));
        assert_eq!(configured("~/pikafish", &d), Some(d.join("pikafish")));
        assert_eq!(configured("missing/engine", &d), None);
        // A bare name is looked for on PATH only, never as a file in the working folder.
        let bare = format!("bragd-test-engine-{}", std::process::id());
        std::fs::write(&bare, "").ok();
        let found = configured(&bare, &d);
        std::fs::remove_file(&bare).ok();
        assert_eq!(found, None);
        std::fs::remove_dir_all(&d).unwrap();
    }
}
