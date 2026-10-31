//! The built-in version, and a check for a newer release: when the app starts, then once a day
//! while it runs, unless `[updates] check` is off. It asks the channel this copy came from
//! (WinGet, Flathub, nixpkgs), or GitHub for the rest, and names the command that upgrades the
//! copy. Bragd never installs an update itself.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::config::Config;
use crate::state::AppState;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const API_URL: &str = "https://api.github.com/repos/mwo-dk/bragd/releases/latest";
const DAY: u64 = 24 * 3600;

/// The Flatpak app id, the WinGet package and the Homebrew cask.
pub const FLATPAK_ID: &str = "io.github.mwo_dk.Bragd";
pub const WINGET_ID: &str = "mwo-dk.Bragd";
pub const CASK: &str = "bragd";

/// The host part of `url`, as Settings → Privacy and updates names it.
pub fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let rest = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    rest.rsplit_once('@').map_or(rest, |(_, h)| h)
}

/// Where this copy's update check goes, as Settings → Privacy and updates and the wizard name
/// it: its channel's repository, then GitHub's releases when the channel gives no answer.
pub fn host() -> String {
    let gh = host_of(API_URL);
    match channel() {
        Channel::GitHub => gh.to_string(),
        c => format!("{} ({}), {gh}", host_of(&c.url()), c.repo()),
    }
}

/// `major.minor.patch` from "1.2.3" or "v1.2.3"; pre-release suffixes are ignored.
pub fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let mut it = v
        .trim()
        .trim_start_matches('v')
        .split(['.', '-', '+'])
        .map(|p| p.parse().ok());
    Some((it.next()??, it.next()??, it.next()??))
}

/// Is `latest` newer than `current`? Unparseable versions never are.
pub fn is_newer(current: &str, latest: &str) -> bool {
    matches!((parse(current), parse(latest)), (Some(c), Some(l)) if l > c)
}

/// `"v2.3.4"`, `"2.3.4.tar.gz"` as `"2.3.4"`.
fn version_in(s: &str) -> Option<String> {
    let s = s.trim().trim_matches(['"', '\'', ';', ',']);
    parse(s).map(|(a, b, c)| format!("{a}.{b}.{c}"))
}

/// The channel a copy came from, which is where it learns of a new version: announced only once
/// the command it shows can install it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// Our releases page. It holds for the downloads, the install scripts and our Homebrew tap
    /// too: the release run writes the tap right after the GitHub release. The Nix flake builds
    /// from our tags.
    GitHub,
    /// The package's highest version folder in microsoft/winget-pkgs: what `winget upgrade` finds.
    WinGet,
    /// The manifest in Flathub's repository of the app.
    Flathub,
    /// The package file on nixpkgs' master.
    Nixpkgs,
}

impl Channel {
    /// The name the "it may not have it yet" note uses.
    pub const fn name(self) -> &'static str {
        match self {
            Channel::GitHub => "GitHub",
            Channel::WinGet => "WinGet",
            Channel::Flathub => "Flathub",
            Channel::Nixpkgs => "nixpkgs",
        }
    }

    fn repo(self) -> &'static str {
        match self {
            Channel::GitHub => "mwo-dk/bragd",
            Channel::WinGet => "microsoft/winget-pkgs",
            Channel::Flathub => "flathub/io.github.mwo_dk.Bragd",
            Channel::Nixpkgs => "NixOS/nixpkgs",
        }
    }

    /// The one file (or folder listing) asked, all on GitHub.
    fn url(self) -> String {
        const RAW: &str = "https://raw.githubusercontent.com";
        match self {
            Channel::GitHub => API_URL.to_string(),
            // mwo-dk.Bragd → manifests/m/mwo-dk/Bragd
            Channel::WinGet => format!(
                "https://api.github.com/repos/microsoft/winget-pkgs/contents/manifests/{}/{}",
                WINGET_ID[..1].to_lowercase(),
                WINGET_ID.replace('.', "/")
            ),
            Channel::Flathub => format!("{RAW}/flathub/{FLATPAK_ID}/master/{FLATPAK_ID}.yml"),
            Channel::Nixpkgs => {
                format!("{RAW}/NixOS/nixpkgs/master/pkgs/by-name/br/bragd/package.nix")
            }
        }
    }

    /// The version in the channel's answer.
    fn read(self, body: &str) -> Option<String> {
        match self {
            Channel::GitHub => {
                let v: serde_json::Value = serde_json::from_str(body).ok()?;
                version_in(v["tag_name"].as_str()?)
            }
            Channel::WinGet => {
                let v: Vec<serde_json::Value> = serde_json::from_str(body).ok()?;
                v.iter()
                    .filter(|e| e["type"] == "dir")
                    .filter_map(|e| version_in(e["name"].as_str()?))
                    .max_by_key(|v| parse(v))
            }
            Channel::Nixpkgs => body
                .lines()
                .find_map(|l| {
                    l.trim()
                        .strip_prefix("version")?
                        .trim_start()
                        .strip_prefix('=')
                })
                .and_then(version_in),
            // Our source: an archive whose URL names the tag, or a git URL with a `tag:` below.
            Channel::Flathub => {
                let lines: Vec<&str> = body.lines().collect();
                lines
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| l.contains("mwo-dk/bragd"))
                    .find_map(|(i, l)| {
                        l.split(['/', ' '])
                            .filter(|p| p.starts_with('v'))
                            .find_map(version_in)
                            .or_else(|| {
                                lines[i + 1..].iter().take(4).find_map(|l| {
                                    let l = l.replace('"', "");
                                    version_in(
                                        l.trim()
                                            .trim_start_matches('-')
                                            .trim()
                                            .strip_prefix("tag:")?,
                                    )
                                })
                            })
                    })
            }
        }
    }
}

fn get(url: &str) -> Option<String> {
    // The system's certificate store, not a bundled one: a work machine behind a TLS-inspecting
    // proxy trusts its own CA, and the check would otherwise fail silently there.
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .tls_config(tls)
        .build()
        .into();
    agent
        .get(url)
        .header("User-Agent", concat!("bragd/", env!("CARGO_PKG_VERSION")))
        .call()
        .ok()?
        .body_mut()
        .read_to_string()
        .ok()
}

/// The newest version this copy's channel offers, and the channel that said so. When the
/// channel cannot be asked, GitHub's answer, named as GitHub's, so the notice adds that the
/// channel may not have it yet. One request a day, two at most.
fn fetch_latest() -> Option<(String, &'static str)> {
    let c = channel();
    if c != Channel::GitHub
        && let Some(v) = get(&c.url()).and_then(|b| c.read(&b))
    {
        return Some((v, c.name()));
    }
    let gh = Channel::GitHub;
    Some((gh.read(&get(API_URL)?)?, gh.name()))
}

/// Has this run of the app asked yet? Each start asks once, then once a day while it runs.
static ASKED: AtomicBool = AtomicBool::new(false);

/// Is a check due: it is on, and the app has just started or a day has passed since the last.
pub fn due(cfg: &Config, st: &AppState) -> bool {
    cfg.updates.check
        && (!ASKED.load(Ordering::Relaxed)
            || crate::fs::now().saturating_sub(st.update_checked) >= DAY)
}

pub fn record(st: &mut AppState, (latest, from): (String, &'static str)) {
    ASKED.store(true, Ordering::Relaxed);
    st.latest_version = latest;
    st.latest_from = from.to_string();
    st.update_checked = crate::fs::now();
}

/// The newer version the last check found, if any; none while the check is off.
pub fn available(cfg: &Config, st: &AppState) -> Option<String> {
    (cfg.updates.check && is_newer(VERSION, &st.latest_version)).then(|| st.latest_version.clone())
}

/// How to upgrade to what the last check found: the note that the channel may not have it yet
/// only when the answer was GitHub's, not the channel's own.
pub fn hint(st: &AppState) -> Upgrade {
    answered(upgrade_hint(), st)
}

fn answered(mut u: Upgrade, st: &AppState) -> Upgrade {
    if st.latest_from == u.channel.name() {
        u.lags = None;
    }
    u
}

/// Loads the state, asks if due, saves: the newer version and how to get it. Blocks for up to
/// 5 s a request, so run it on a thread. Offline just means "no news". Off: asks nothing.
pub fn check(cfg: &Config) -> Option<(String, Upgrade)> {
    let mut st = AppState::load();
    // BRAGD_OFFLINE: tests and screenshots ask nobody, whatever the setting says.
    if due(cfg, &st)
        && std::env::var_os("BRAGD_OFFLINE").is_none()
        && let Some(latest) = fetch_latest()
    {
        // Read again: a window may have saved the state during the fetch.
        st = AppState::load();
        record(&mut st, latest);
        let _ = st.save();
    }
    available(cfg, &st).map(|v| (v, hint(&st)))
}

/// How this copy upgrades: the command (none: download it from the releases page), and the
/// channel it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Upgrade {
    pub cmd: Option<&'static str>,
    /// The channel (WinGet, Flathub, nixpkgs) that takes a release days after GitHub, while the
    /// last answer was GitHub's (`hint`), so its command may find nothing new yet.
    pub lags: Option<&'static str>,
    #[serde(skip)]
    pub channel: Channel,
}

impl Upgrade {
    const fn now(cmd: &'static str) -> Self {
        Self {
            cmd: Some(cmd),
            lags: None,
            channel: Channel::GitHub,
        }
    }
    const fn via(cmd: &'static str, channel: Channel) -> Self {
        Self {
            cmd: Some(cmd),
            lags: Some(channel.name()),
            channel,
        }
    }
    const DOWNLOAD: Upgrade = Upgrade {
        cmd: None,
        lags: None,
        channel: Channel::GitHub,
    };
}

/// Where this copy's check asks.
pub fn channel() -> Channel {
    upgrade_hint().channel
}

/// What besides the path shows how a copy was installed.
#[derive(Clone, Copy, Default)]
struct Seen {
    /// It runs in a Flatpak.
    flatpak: bool,
    /// Homebrew's cask folder exists, on macOS (a cask copies Bragd.app to /Applications).
    mac_cask: bool,
    /// WinGet's own record of what it installed lists Bragd.
    winget: bool,
    /// Built by our flake (it sets `BRAGD_FROM_FLAKE`), not by nixpkgs.
    flake: bool,
}

/// The command that upgrades this copy, from where it is installed. Worked out once a run.
pub fn upgrade_hint() -> Upgrade {
    static HINT: std::sync::OnceLock<Upgrade> = std::sync::OnceLock::new();
    *HINT.get_or_init(|| {
        // Inside an AppImage the executable is in a temporary mount; $APPIMAGE is the file itself.
        let exe = std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .or_else(|| std::env::current_exe().ok());
        let exe = exe
            .map(|e| std::fs::canonicalize(&e).unwrap_or(e))
            .unwrap_or_default();
        let cask = ["/opt/homebrew", "/usr/local"]
            .iter()
            .any(|p| Path::new(p).join("Caskroom").join(CASK).is_dir());
        let seen = Seen {
            flatpak: std::env::var_os("FLATPAK_ID").is_some()
                && Path::new("/.flatpak-info").is_file(),
            mac_cask: cfg!(target_os = "macos") && cask,
            winget: cfg!(windows) && winget_installed(WINGET_ID),
            flake: option_env!("BRAGD_FROM_FLAKE").is_some(),
        };
        hint_for(&exe.to_string_lossy(), seen)
    })
}

/// Whether WinGet installed package `id`. The installer lands where a download would, so its
/// folder does not tell; WinGet's tracking catalogue does: `installed.db` in a folder per source
/// under its local state, where it records each package it installs and drops it when it
/// uninstalls it. Nothing is run.
fn winget_installed(id: &str) -> bool {
    let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        return false;
    };
    [
        local.join(r"Packages\Microsoft.DesktopAppInstaller_8wekyb3d8bbwe\LocalState"),
        local.join(r"Microsoft\WinGet\State"),
    ]
    .iter()
    .filter_map(|d| std::fs::read_dir(d).ok())
    .flatten()
    .flatten()
    .any(|e| {
        use rusqlite::{Connection, OpenFlags};
        Connection::open_with_flags(
            e.path().join("installed.db"),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .is_ok_and(|db| catalog_lists(&db, id))
    })
}

/// Does a WinGet catalogue hold package `id`? Read by column, not by layout: every table with
/// an `id` column (`ids` in the older index schema, `packages` in the newer one).
fn catalog_lists(db: &rusqlite::Connection, id: &str) -> bool {
    let tables: Vec<String> = db
        .prepare(
            "SELECT m.name FROM sqlite_master m, pragma_table_info(m.name) c \
             WHERE m.type = 'table' AND c.name = 'id'",
        )
        .and_then(|mut s| s.query_map([], |r| r.get(0))?.collect())
        .unwrap_or_default();
    tables.iter().any(|t| {
        db.query_row(
            &format!(
                "SELECT 1 FROM \"{}\" WHERE id = ?1 COLLATE NOCASE LIMIT 1",
                t.replace('"', "\"\"")
            ),
            [id],
            |_| Ok(()),
        )
        .is_ok()
    })
}

fn hint_for(exe: &str, seen: Seen) -> Upgrade {
    let p = exe.replace('\\', "/").to_lowercase();
    if seen.flatpak {
        Upgrade::via("flatpak update io.github.mwo_dk.Bragd", Channel::Flathub)
    } else if p.contains("/caskroom/bragd/")
        || (seen.mac_cask && p.contains("/applications/bragd.app/"))
    {
        Upgrade::now("brew upgrade --cask bragd")
    } else if p.starts_with("/nix/store/") {
        // From the flake it has our release at once; nixpkgs' copy asks nixpkgs.
        if seen.flake {
            Upgrade::now("nix profile upgrade bragd")
        } else {
            Upgrade::via("nix profile upgrade bragd", Channel::Nixpkgs)
        }
    } else if seen.winget {
        Upgrade::via("winget upgrade mwo-dk.Bragd", Channel::WinGet)
    } else {
        Upgrade::DOWNLOAD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_version_compare() {
        assert!(is_newer("1.0.0", "v1.0.1"));
        assert!(is_newer("1.9.9", "2.0.0"));
        assert!(is_newer("1.2.3", "1.10.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("1.1.0", "v1.0.9"));
        assert!(!is_newer("1.0.0", ""));
        assert!(!is_newer("1.0.0", "garbage"));
        assert!(parse(VERSION).is_some());
    }

    // ASKED is process-wide, so only what holds after a record() is checked.
    #[test]
    fn update_due_once_a_day_and_never_when_off() {
        let cfg = Config::default();
        let mut st = AppState::default();
        record(&mut st, ("1.0.0".into(), "GitHub"));
        assert!(!due(&cfg, &st), "asked just now");
        st.update_checked = crate::fs::now() - DAY + 60;
        assert!(!due(&cfg, &st), "less than a day ago");
        st.update_checked = crate::fs::now() - DAY;
        assert!(due(&cfg, &st), "a day ago");
        let off = Config::parse("[updates]\ncheck = false\n").unwrap();
        assert!(!due(&off, &st));
        st.latest_version = "999.0.0".into();
        assert_eq!(available(&cfg, &st).as_deref(), Some("999.0.0"));
        assert_eq!(available(&off, &st), None);
        // Off, `check` asks nothing and finds nothing.
        assert_eq!(check(&off), None);
    }

    fn h(p: &str, seen: Seen) -> Option<&'static str> {
        hint_for(p, seen).cmd
    }

    #[test]
    fn update_hint_follows_the_install_source() {
        let s = Seen::default();
        assert_eq!(
            h(
                "/opt/homebrew/Caskroom/bragd/0.1.0/Bragd.app/Contents/MacOS/bragd",
                s
            ),
            Some("brew upgrade --cask bragd")
        );
        let mac_cask = Seen {
            mac_cask: true,
            ..s
        };
        assert_eq!(
            h("/Applications/Bragd.app/Contents/MacOS/bragd", mac_cask),
            Some("brew upgrade --cask bragd")
        );
        assert_eq!(h("/Applications/Bragd.app/Contents/MacOS/bragd", s), None);
        let flatpak = Seen { flatpak: true, ..s };
        assert_eq!(
            hint_for("/app/bin/bragd", flatpak),
            Upgrade::via("flatpak update io.github.mwo_dk.Bragd", Channel::Flathub)
        );
        let winget = Seen { winget: true, ..s };
        assert_eq!(
            hint_for(r"C:\Program Files\Bragd\bragd.exe", winget),
            Upgrade::via("winget upgrade mwo-dk.Bragd", Channel::WinGet)
        );
        assert_eq!(h(r"C:\Program Files\Bragd\bragd.exe", s), None);
        assert_eq!(h("/home/me/Downloads/Bragd_0.1.0_amd64.AppImage", s), None);
        assert_eq!(h("/usr/bin/bragd", s), None);
    }

    #[test]
    fn update_hint_knows_nix_and_which_channels_lag() {
        let s = Seen::default();
        assert_eq!(
            hint_for("/nix/store/abc-bragd-0.1.0/bin/bragd", s),
            Upgrade::via("nix profile upgrade bragd", Channel::Nixpkgs)
        );
        let flake = Seen { flake: true, ..s };
        assert_eq!(
            hint_for("/nix/store/abc-bragd-0.1.0/bin/bragd", flake),
            Upgrade::now("nix profile upgrade bragd")
        );
        assert_eq!(
            h("/home/me/nix/store/bin/bragd", s),
            None,
            "only the store itself"
        );
        let mut st = AppState::default();
        record(&mut st, ("9.0.0".into(), "GitHub"));
        let u = Upgrade::via("winget upgrade mwo-dk.Bragd", Channel::WinGet);
        assert_eq!(answered(u, &st).lags, Some("WinGet"));
        record(&mut st, ("9.0.0".into(), "WinGet"));
        assert_eq!(answered(u, &st).lags, None);
    }

    #[test]
    fn update_each_channel_asks_its_own_source_on_github() {
        assert_eq!(
            Channel::WinGet.url(),
            "https://api.github.com/repos/microsoft/winget-pkgs/contents/manifests/m/mwo-dk/Bragd"
        );
        assert!(
            Channel::Nixpkgs
                .url()
                .ends_with("/NixOS/nixpkgs/master/pkgs/by-name/br/bragd/package.nix")
        );
        assert!(
            Channel::Flathub
                .url()
                .starts_with("https://raw.githubusercontent.com/flathub/io.github.mwo_dk.Bragd/")
        );
        for c in [
            Channel::GitHub,
            Channel::WinGet,
            Channel::Flathub,
            Channel::Nixpkgs,
        ] {
            let host = host_of(&c.url()).to_string();
            assert!(
                host == "api.github.com" || host == "raw.githubusercontent.com",
                "{c:?}"
            );
        }
    }

    #[test]
    fn update_winget_catalog_lists_by_id() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE ids(rowid INTEGER PRIMARY KEY, id TEXT NOT NULL);
             CREATE TABLE manifest(rowid INTEGER PRIMARY KEY, id INT64, version INT64);
             INSERT INTO ids(id) VALUES ('Other.App');
             INSERT INTO manifest(id, version) VALUES (1, 1);",
        )
        .unwrap();
        assert!(!catalog_lists(&db, WINGET_ID));
        db.execute("INSERT INTO ids(id) VALUES ('MWO-DK.Bragd')", [])
            .unwrap();
        assert!(catalog_lists(&db, WINGET_ID), "WinGet ids ignore case");
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE packages(rowid INTEGER PRIMARY KEY, id TEXT, name TEXT); INSERT INTO packages(id, name) VALUES ('mwo-dk.Bragd', 'Bragd');").unwrap();
        assert!(catalog_lists(&db, WINGET_ID));
    }

    // Answers as each channel gives them, cut down; no network.
    #[test]
    fn update_reads_each_channels_answer() {
        assert_eq!(
            Channel::GitHub.read(r#"{"tag_name":"v0.2.0","draft":false}"#),
            Some("0.2.0".into())
        );
        assert_eq!(Channel::GitHub.read(r#"{"message":"Not Found"}"#), None);
        let listing = |names: &[&str]| {
            serde_json::to_string(
                &names
                    .iter()
                    .map(|n| serde_json::json!({"name": n, "type": "dir"}))
                    .collect::<Vec<_>>(),
            )
            .unwrap()
        };
        // "0.9.0" sorts after "0.10.1" as text.
        assert_eq!(
            Channel::WinGet.read(&listing(&["0.10.1", "0.9.0"])),
            Some("0.10.1".into())
        );
        assert_eq!(Channel::WinGet.read(r#"{"message":"Not Found"}"#), None);
        let nix = "  pname = \"bragd\";\n  version = \"0.3.1\";\n";
        assert_eq!(Channel::Nixpkgs.read(nix), Some("0.3.1".into()));
        let flathub_git = "  - name: bragd\n    sources:\n      - type: git\n        url: https://github.com/mwo-dk/bragd.git\n        tag: v0.4.0\n";
        assert_eq!(Channel::Flathub.read(flathub_git), Some("0.4.0".into()));
        let flathub_archive =
            "        url: https://github.com/mwo-dk/bragd/archive/refs/tags/v0.4.1.tar.gz\n";
        assert_eq!(Channel::Flathub.read(flathub_archive), Some("0.4.1".into()));
        assert_eq!(Channel::Flathub.read("id: io.github.mwo_dk.Bragd\n"), None);
    }
}
