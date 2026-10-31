//! The engines Bragd knows, per Game family and Variant: where to find each one installed,
//! the protocol it speaks, and its sealed image. An installed engine is used first; otherwise
//! its image from `ghcr.io/mwo-dk/bragd-<engine>`, once pulled.

use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::protocol::Protocol;
use crate::session::Program;
use crate::tools::{self, Runtime};

/// The Family of a Variant key, from bragd-games' catalogue.
pub fn family(variant: &str) -> Option<bragd_games::Family> {
    bragd_games::Variant::from_key(variant).map(|v| v.family())
}

/// Where Bragd's engine images are published.
pub const REGISTRY: &str = "ghcr.io/mwo-dk";

/// Set to `1`, Bragd also runs the images `tools/dev/images.sh` builds on this machine
/// (`localhost/bragd-<key>:dev`), which are pinned by nothing. Without it, in any build, only
/// images pinned by digest are ever run.
pub const DEV_IMAGES: &str = "BRAGD_DEV_IMAGES";

/// Whether images built here may run: only when `BRAGD_DEV_IMAGES` is `1`.
pub fn dev_images() -> bool {
    dev_wanted(std::env::var(DEV_IMAGES).ok().as_deref())
}

fn dev_wanted(value: Option<&str>) -> bool {
    value == Some("1")
}

// The digests the images are pinned to. None: the image is not published yet, so it is never
// pulled. TODO(unpublished): set each when `packaging/images/<engine>` is first pushed.
pub const PIN_STOCKFISH: Option<&str> = None;
pub const PIN_FAIRY_STOCKFISH: Option<&str> = None;
pub const PIN_PIKAFISH: Option<&str> = None;
pub const PIN_KATAGO: Option<&str> = None;
pub const PIN_GNUGO: Option<&str> = None;
pub const PIN_LC0: Option<&str> = None;
pub const PIN_BERSERK: Option<&str> = None;
pub const PIN_ETHEREAL: Option<&str> = None;
pub const PIN_RUBICHESS: Option<&str> = None;
pub const PIN_KOIVISTO: Option<&str> = None;
pub const PIN_ARASAN: Option<&str> = None;
pub const PIN_FRUIT: Option<&str> = None;

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
    /// About how many bytes its image takes to download, shown before a pull; 0 without one.
    pub image_size: u64,
    /// Its download page; empty when it has none any more (install it from your system's
    /// packages).
    pub site: &'static str,
    /// It reads Syzygy tablebases (`SyzygyPath`).
    pub syzygy: bool,
}

const CHESS: &[&str] = &["chess", "chess960"];
/// Engines without `UCI_Chess960`.
const CHESS_ONLY: &[&str] = &["chess"];
const FAIRY: &[&str] = &["chess", "chess960", "shatranj", "chaturanga", "kingofthehill", "racingkings", "seirawan", "capablanca", "xiangqi", "janggi", "shogi"];
const GO: &[&str] = &["go-9", "go-13", "go-19"];

/// The engines of the spec, and more UCI chess engines, strongest-known first after Stockfish;
/// Fairy-Stockfish comes last of the chess ones, as it plays chess as one Variant of many. Igel
/// has no image: it needs AVX2 and cannot be built for arm64. English, Brazilian and Russian
/// draughts have no engine: Scan plays international draughts.
pub const KNOWN: &[Known] = &[
    Known { key: "stockfish", name: "Stockfish", programs: &["stockfish"], args: &[], variants: CHESS, tag: Some("17.1"), pin: PIN_STOCKFISH, image_size: 65_244_394, site: "https://stockfishchess.org/download/", syzygy: true },
    Known { key: "lc0", name: "Leela Chess Zero", programs: &["lc0"], args: &[], variants: CHESS, tag: Some("0.32.1"), pin: PIN_LC0, image_size: 1_548_746, site: "https://lczero.org/play/download/", syzygy: true },
    Known { key: "berserk", name: "Berserk", programs: &["berserk"], args: &[], variants: CHESS, tag: Some("14"), pin: PIN_BERSERK, image_size: 18_313_378, site: "https://github.com/jhonnold/berserk/releases", syzygy: true },
    Known { key: "ethereal", name: "Ethereal", programs: &["ethereal", "Ethereal", "ethereal-chess"], args: &[], variants: CHESS, tag: Some("14.00"), pin: PIN_ETHEREAL, image_size: 200_582, site: "https://github.com/AndyGrant/Ethereal", syzygy: true },
    Known { key: "rubichess", name: "RubiChess", programs: &["rubichess", "RubiChess"], args: &[], variants: CHESS, tag: Some("20240817"), pin: PIN_RUBICHESS, image_size: 22_029_340, site: "https://github.com/Matthies/RubiChess/releases", syzygy: true },
    Known { key: "koivisto", name: "Koivisto", programs: &["koivisto", "Koivisto"], args: &[], variants: CHESS_ONLY, tag: Some("9.0"), pin: PIN_KOIVISTO, image_size: 9_159_930, site: "https://github.com/Luecx/Koivisto/releases", syzygy: true },
    Known { key: "igel", name: "Igel", programs: &["igel"], args: &[], variants: CHESS_ONLY, tag: None, pin: None, image_size: 0, site: "https://github.com/vshcherbyna/igel/releases", syzygy: true },
    Known { key: "arasan", name: "Arasan", programs: &["arasanx-64", "arasanx"], args: &[], variants: CHESS_ONLY, tag: Some("26.1"), pin: PIN_ARASAN, image_size: 14_429_650, site: "https://www.arasanchess.org/", syzygy: true },
    // Fruit 2.1 (2005) has no site any more: install it from your system's packages.
    Known { key: "fruit", name: "Fruit", programs: &["fruit"], args: &[], variants: CHESS_ONLY, tag: Some("2.1"), pin: PIN_FRUIT, image_size: 112_384, site: "", syzygy: false },
    Known {
        key: "fairy-stockfish",
        name: "Fairy-Stockfish",
        programs: &["fairy-stockfish", "fairy-stockfish-largeboard", "fairy-stockfish_largeboard"],
        args: &[],
        variants: FAIRY,
        tag: Some("14.0.1"),
        pin: PIN_FAIRY_STOCKFISH,
        image_size: 917_359,
        site: "https://github.com/fairy-stockfish/Fairy-Stockfish/releases",
        syzygy: true,
    },
    Known { key: "pikafish", name: "Pikafish", programs: &["pikafish"], args: &[], variants: &["xiangqi"], tag: Some("2026-09-06"), pin: PIN_PIKAFISH, image_size: 957_480, site: "https://github.com/official-pikafish/Pikafish/releases", syzygy: false },
    Known { key: "yaneuraou", name: "YaneuraOu", programs: &["YaneuraOu", "yaneuraou", "YaneuraOu-by-gcc"], args: &[], variants: &["shogi"], tag: None, pin: None, image_size: 0, site: "https://github.com/yaneurao/YaneuraOu/releases", syzygy: false },
    Known { key: "katago", name: "KataGo", programs: &["katago"], args: &["gtp"], variants: GO, tag: Some("1.18.2-eigen"), pin: PIN_KATAGO, image_size: 51_764_399, site: "https://github.com/lightvector/KataGo/releases", syzygy: false },
    Known { key: "gnugo", name: "GNU Go", programs: &["gnugo"], args: &["--mode", "gtp"], variants: GO, tag: Some("3.8"), pin: PIN_GNUGO, image_size: 1_459_667, site: "https://www.gnu.org/software/gnugo/", syzygy: false },
    Known { key: "scan", name: "Scan", programs: &["scan"], args: &["hub"], variants: &["draughts"], tag: None, pin: None, image_size: 0, site: "https://hjetten.home.xs4all.nl/scan/scan.html", syzygy: false },
    Known { key: "gnubg", name: "GNU Backgammon", programs: &["gnubg"], args: &["-t", "-q"], variants: &["backgammon"], tag: None, pin: None, image_size: 0, site: "https://www.gnu.org/software/gnubg/", syzygy: false },
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

    /// Its image pinned by digest, the only form ever pulled, and the only one run unless
    /// `dev`; None while unpublished.
    pub fn image(&self) -> Option<String> {
        Some(format!("{}@{}", self.image_name()?, self.pin?))
    }

    /// Its image as `tools/dev/images.sh` builds it here, when Bragd has one to build and
    /// `dev` (`dev_images()`) allows it: never pulled, only run when the runtime has it.
    pub fn dev_image(&self, dev: bool) -> Option<String> {
        self.tag.filter(|_| dev).map(|_| format!("localhost/bragd-{}:dev", self.key))
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
    /// Its image is there, but the evaluation network it needs (`network`) is not downloaded:
    /// that waits for the user to read its licence and agree.
    NoNetwork,
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Missing::NotPulled { image } => write!(f, "Not installed. Its image {} can be downloaded.", crate::images::short(image)),
            Missing::NoImage => write!(f, "Not installed, and Bragd has no image of it yet."),
            Missing::NoRuntime => write!(f, "Not installed, and neither podman nor docker is there to run its image."),
            Missing::NoNetwork => write!(f, "Not installed, and its image needs its evaluation network, which is not downloaded yet."),
        }
    }
}

/// The program to run for `k`: installed here (`installed`), else its image built here when
/// `dev` allows it and the runtime has it (`has`), else its pinned image when the runtime has it. `syzygy`: the tablebase folder an image is given read-only;
/// `networks`: where the networks images need are kept (`network`), an image's given it
/// read-only. An installed engine brings its own network.
#[allow(clippy::too_many_arguments, reason = "each is one thing the choice depends on, given so tests need no machine")]
pub fn choose(k: &Known, installed: Option<PathBuf>, rt: Option<&Runtime>, has: impl Fn(&Runtime, &str) -> bool, syzygy: Option<&Path>, networks: &Path, dev: bool) -> Result<Program, Missing> {
    let args: Vec<String> = k.args.iter().map(|a| a.to_string()).collect();
    if let Some(path) = installed {
        return Ok(Program::Installed { path, args });
    }
    let (local, pinned) = (k.dev_image(dev), k.image());
    if local.is_none() && pinned.is_none() {
        return Err(Missing::NoImage);
    }
    let rt = rt.ok_or(Missing::NoRuntime)?;
    let image = match local.filter(|i| has(rt, i)) {
        Some(i) => i,
        None => {
            let image = pinned.ok_or(Missing::NoImage)?;
            if !has(rt, &image) {
                return Err(Missing::NotPulled { image });
            }
            image
        }
    };
    let network = match crate::network::for_engine(k.key) {
        Some(n) => Some(Some(n.path(networks)).filter(|p| p.is_file()).ok_or(Missing::NoNetwork)?),
        None => None,
    };
    Ok(Program::Container { rt: rt.clone(), image, args: vec![], syzygy: syzygy.filter(|p| k.syzygy && p.is_dir()).map(Path::to_path_buf), network })
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
        assert_eq!(known("katago").unwrap().protocol("go-19"), Protocol::Gtp);
        assert_eq!(known("scan").unwrap().protocol("draughts"), Protocol::Hub);
        assert_eq!(known("gnubg").unwrap().protocol("backgammon"), Protocol::Gnubg);
        for k in KNOWN {
            assert!(k.variants.iter().all(|v| family(v).is_some()), "{}: a Variant no Family has", k.key);
        }
        let xiangqi: Vec<&str> = for_variant("xiangqi").map(|k| k.key).collect();
        assert_eq!(xiangqi, ["fairy-stockfish", "pikafish"]);
        assert_eq!(for_variant("checkers").count(), 0);
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
        let k = Known { key: "stockfish", name: "Stockfish", programs: &["stockfish"], args: &[], variants: CHESS, tag: Some("17.1"), pin: Some("sha256:1111111111111111111111111111111111111111111111111111111111111111"), image_size: 1, site: "", syzygy: true };
        let here = PathBuf::from("/usr/games/stockfish");
        let nets = Path::new("/nonexistent/networks");
        assert_eq!(choose(&k, Some(here.clone()), Some(&rt), |_, _| true, None, nets, false), Ok(Program::Installed { path: here, args: vec![] }));
        let image = k.image().unwrap();
        let tb = &std::env::temp_dir();
        assert_eq!(choose(&k, None, Some(&rt), |_, _| true, Some(tb), nets, false), Ok(Program::Container { rt: rt.clone(), image: image.clone(), args: vec![], syzygy: Some(tb.into()), network: None }));
        // No tablebases downloaded yet: a mount of a missing folder would stop the container.
        let missing = Path::new("/nonexistent/bragd/tablebases");
        assert!(matches!(choose(&k, None, Some(&rt), |_, _| true, Some(missing), nets, false), Ok(Program::Container { syzygy: None, .. })));
        assert_eq!(choose(&k, None, Some(&rt), |_, _| false, None, nets, false), Err(Missing::NotPulled { image }));
        assert_eq!(choose(&k, None, None, |_, _| true, None, nets, false), Err(Missing::NoRuntime));
        assert_eq!(choose(known("pikafish").unwrap(), None, Some(&rt), |_, _| true, None, nets, false), Err(Missing::NoImage), "unpublished");
        // GNU Go installed gets its protocol on its command line; its image's entrypoint has it.
        let g = known("gnugo").unwrap();
        let gnugo = Known { pin: k.pin, programs: g.programs, args: g.args, variants: g.variants, tag: g.tag, site: g.site, ..k };
        assert_eq!(choose(&gnugo, Some("/usr/bin/gnugo".into()), None, |_, _| false, None, nets, false).unwrap(), Program::Installed { path: "/usr/bin/gnugo".into(), args: vec!["--mode".into(), "gtp".into()] });
        let gnugo = Known { syzygy: false, ..gnugo };
        assert!(matches!(choose(&gnugo, None, Some(&rt), |_, _| true, Some(tb), nets, false).unwrap(), Program::Container { args, syzygy: None, .. } if args.is_empty()), "no tablebases for go");
    }

    /// An image built here (`tools/dev/images.sh`) runs only when `BRAGD_DEV_IMAGES=1` asks for
    /// it: without that, an unpublished engine has no image, whatever the runtime holds.
    #[test]
    fn images_built_here_run_only_when_asked_for() {
        assert!(dev_wanted(Some("1")));
        for v in [None, Some(""), Some("0"), Some("true"), Some("yes")] {
            assert!(!dev_wanted(v), "{v:?}");
        }
        let rt = Runtime { name: "podman", path: PathBuf::from("/nonexistent/podman") };
        let nets = Path::new("/nonexistent/networks");
        let k = known("gnugo").unwrap();
        assert_eq!(k.pin, None, "unpublished");
        let local = "localhost/bragd-gnugo:dev";
        assert_eq!(k.dev_image(true).as_deref(), Some(local));
        assert_eq!(k.dev_image(false), None);
        let built = |_: &Runtime, i: &str| i == local;
        // Not asked for: the image built here is never run, even though the runtime has it.
        assert_eq!(choose(k, None, Some(&rt), built, None, nets, false), Err(Missing::NoImage));
        // Asked for: it runs, sealed like every image.
        assert!(matches!(choose(k, None, Some(&rt), built, None, nets, true), Ok(Program::Container { image, .. }) if image == local));
        // Asked for but not built: still no image, and no runtime means none either way.
        assert_eq!(choose(k, None, Some(&rt), |_, _| false, None, nets, true), Err(Missing::NoImage));
        assert_eq!(choose(k, None, None, built, None, nets, true), Err(Missing::NoRuntime));
        // A published image is still pulled when nothing was built here.
        let p = Known { pin: Some("sha256:3333333333333333333333333333333333333333333333333333333333333333"), ..*k };
        assert_eq!(choose(&p, None, Some(&rt), |_, _| false, None, nets, true), Err(Missing::NotPulled { image: p.image().unwrap() }));
        // Bragd builds no image of Scan: nothing to build here either.
        assert_eq!(known("scan").unwrap().dev_image(true), None);
    }

    /// Pikafish's image runs only once its network is downloaded, and is then given it; an
    /// installed Pikafish needs none.
    #[test]
    fn the_pikafish_image_waits_for_its_network() {
        let rt = Runtime { name: "podman", path: PathBuf::from("/nonexistent/podman") };
        let p = Known { pin: Some("sha256:2222222222222222222222222222222222222222222222222222222222222222"), ..*known("pikafish").unwrap() };
        let d = crate::test_dir("networks");
        assert_eq!(choose(&p, None, Some(&rt), |_, _| true, None, &d, false), Err(Missing::NoNetwork));
        assert!(matches!(choose(&p, Some("/usr/bin/pikafish".into()), Some(&rt), |_, _| true, None, &d, false), Ok(Program::Installed { .. })));
        let file = crate::network::PIKAFISH.path(&d);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "net").unwrap();
        assert!(matches!(choose(&p, None, Some(&rt), |_, _| true, None, &d, false), Ok(Program::Container { network: Some(n), .. }) if n == file));
        // An engine without a network is given none.
        let k = Known { pin: p.pin, ..*known("stockfish").unwrap() };
        assert!(matches!(choose(&k, None, Some(&rt), |_, _| true, None, &d, false), Ok(Program::Container { network: None, .. })));
        std::fs::remove_dir_all(&d).unwrap();
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
