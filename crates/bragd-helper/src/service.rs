//! "Start with my session": the Helper registered with the system, so it runs from login and
//! stays, instead of starting with the first window and leaving ten minutes after the last.
//! A systemd user unit on Linux, a LaunchAgent on macOS, a Run entry in the registry on
//! Windows, and an XDG autostart entry on the BSDs (the desktop session starts it) and in the
//! Flatpak, which cannot reach systemd. A registered Helper runs with `STAY`.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use bragd_core::update::FLATPAK_ID;

/// The argument of a Helper that stays when the last window has gone.
pub const STAY: &str = "--stay";

/// The unit's, the autostart entry's and the Run entry's name.
const NAME: &str = "bragd-helper";
/// The LaunchAgent's label.
const LABEL: &str = "dk.mwo.bragd.helper";

/// How the Helper is registered on a system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A systemd user unit.
    Systemd,
    /// An XDG autostart entry, which the desktop session starts and nothing restarts.
    Autostart,
    /// An XDG autostart entry in the host's folder that runs the Flatpak.
    Flatpak,
    /// A LaunchAgent.
    Launchd,
    /// A Run entry under `HKCU\…\Run`.
    RunKey,
}

/// Inside a Flatpak.
fn flatpak() -> bool {
    bragd_engines::tools::flatpak().is_some()
}

impl Kind {
    /// This system's.
    pub fn here() -> Kind {
        if cfg!(windows) {
            Kind::RunKey
        } else if cfg!(target_os = "macos") {
            Kind::Launchd
        } else if flatpak() {
            Kind::Flatpak
        } else if cfg!(target_os = "linux") {
            Kind::Systemd
        } else {
            Kind::Autostart
        }
    }

    /// Whether a service manager starts it, so a window asks it to before starting one itself.
    pub fn supervised(self) -> bool {
        matches!(self, Kind::Systemd | Kind::Launchd)
    }

    /// Where the registration is written, under `home` and the config folder `config`; none
    /// for the Run entry, which is in the registry.
    pub fn file(self, home: &Path, config: &Path) -> Option<PathBuf> {
        match self {
            Kind::Systemd => Some(config.join("systemd/user").join(format!("{NAME}.service"))),
            Kind::Autostart => Some(config.join("autostart").join(format!("{NAME}.desktop"))),
            // The host's own autostart folder: the Flatpak's config folder is not the session's.
            Kind::Flatpak => Some(home.join(".config/autostart").join(format!("{NAME}.desktop"))),
            Kind::Launchd => Some(home.join("Library/LaunchAgents").join(format!("{LABEL}.plist"))),
            Kind::RunKey => None,
        }
    }

    /// The registration that starts `exe` as a Helper that stays, its errors going to `log`.
    pub fn unit(self, exe: &Path, log: &Path) -> String {
        let exe = exe.display().to_string();
        const WRITTEN: &str = "# Written by Bragd (Settings → Helper → Start with my session).\n";
        match self {
            Kind::Flatpak => format!(
                "{WRITTEN}[Desktop Entry]\nType=Application\nName=Bragd Helper\nExec=flatpak run --command={NAME} {FLATPAK_ID} {STAY}\nNoDisplay=true\nTerminal=false\n"
            ),
            Kind::Autostart => format!(
                "{WRITTEN}[Desktop Entry]\nType=Application\nName=Bragd Helper\nExec=\"{}\" {STAY}\nNoDisplay=true\nTerminal=false\n",
                desktop(&exe)
            ),
            // A Helper that left on purpose (a newer one took over) is not started again and
            // again; `Nice` and idle I/O let the user go first.
            Kind::Systemd => format!(
                "{WRITTEN}[Unit]\nDescription=Bragd Helper\nStartLimitIntervalSec=0\n\n\
                 [Service]\nExecStart=\"{}\" {STAY}\nRestart=on-failure\nRestartSec=1\nNice=10\nIOSchedulingClass=idle\n\n\
                 [Install]\nWantedBy=default.target\n",
                systemd(&exe)
            ),
            // No `ProcessType` Background: macOS keeps such a job on the efficiency cores with
            // its disk throttled.
            Kind::Launchd => format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>AssociatedBundleIdentifiers</key><string>dk.mwo.bragd</string>
  <key>ProgramArguments</key>
  <array><string>{}</string><string>{STAY}</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
  <key>StandardErrorPath</key><string>{}</string>
  <key>ThrottleInterval</key><integer>2</integer>
  <key>Nice</key><integer>10</integer>
</dict>
</plist>
"#,
                xml(&exe),
                xml(&log.display().to_string()),
            ),
            Kind::RunKey => format!("\"{exe}\" {STAY}"),
        }
    }

    /// The program a registration's text starts: `ExecStart`, `Exec`, the LaunchAgent's first
    /// argument, the Run entry's quoted start. None for the Flatpak's, which names the app.
    pub fn exe_in(self, text: &str) -> Option<PathBuf> {
        let (open, close) = match self {
            Kind::Launchd => ("<array><string>", "</string>"),
            Kind::RunKey => ("\"", "\""),
            Kind::Autostart => ("Exec=\"", "\""),
            Kind::Systemd => ("ExecStart=\"", "\""),
            Kind::Flatpak => return None,
        };
        let rest = text.split_once(open)?.1;
        match self {
            Kind::Launchd => return Some(PathBuf::from(rest.split_once(close)?.0.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&"))),
            Kind::RunKey => return Some(PathBuf::from(rest.split_once(close)?.0)),
            _ => {}
        }
        // The desktop entry's string escape (`\\` for `\`) goes first.
        let line = rest.lines().next()?;
        let line = if self == Kind::Autostart { line.replace("\\\\", "\\") } else { line.to_string() };
        // Up to the closing quote, a backslash taking the next character as it is.
        let (mut out, mut chars) = (String::new(), line.chars());
        loop {
            match chars.next()? {
                '\\' => out.push(chars.next()?),
                '"' => break,
                c => out.push(c),
            }
        }
        let out = out.replace("%%", "%");
        Some(PathBuf::from(if self == Kind::Autostart { out } else { out.replace("$$", "$") }))
    }

    /// Writes the registration for `exe` under `home` and `config`: the file written.
    pub fn write(self, home: &Path, config: &Path, exe: &Path, log: &Path) -> io::Result<Option<PathBuf>> {
        let Some(file) = self.file(home, config) else { return Ok(None) };
        std::fs::create_dir_all(file.parent().unwrap_or(Path::new(".")))?;
        std::fs::write(&file, self.unit(exe, log))?;
        Ok(Some(file))
    }

    /// Removes the registration's file, if there is one.
    pub fn remove(self, home: &Path, config: &Path) -> io::Result<()> {
        match self.file(home, config) {
            Some(f) if f.exists() => std::fs::remove_file(f),
            _ => Ok(()),
        }
    }
}

/// Text for a plist's `<string>`.
fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// A path for a quoted `ExecStart` argument: systemd's escapes, `%` and `$` doubled.
fn systemd(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('%', "%%").replace('$', "$$")
}

/// A path for a quoted `Exec` argument (the desktop entry spec): `"`, `` ` ``, `$` and `\` take
/// a backslash, `%` is doubled, then the string's own escape doubles each backslash.
fn desktop(s: &str) -> String {
    let mut q = String::new();
    for c in s.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            q.push('\\');
        }
        q.push(c);
    }
    q.replace('%', "%%").replace('\\', "\\\\")
}

// ---------------------------------------------------------------- on this system

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

/// Run `c`; an error with its output when it fails.
fn run(c: &mut Command) -> io::Result<()> {
    let out = c.output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{:?}: {}", c.get_program(), String::from_utf8_lossy(&out.stderr).trim())))
    }
}

fn systemctl(args: &[&str]) -> io::Result<()> {
    run(Command::new("systemctl").arg("--user").args(args))
}

fn launchctl(args: &[&str]) -> io::Result<()> {
    run(Command::new("launchctl").args(args))
}

/// launchd's name for this user's login session: `gui/501`.
fn domain() -> String {
    #[cfg(target_os = "macos")]
    // Safety: getuid has no preconditions and cannot fail.
    return format!("gui/{}", unsafe { libc::getuid() });
    #[cfg(not(target_os = "macos"))]
    String::new()
}

fn homes() -> io::Result<(PathBuf, PathBuf)> {
    let none = || io::Error::other("no home folder");
    Ok((bragd_core::dirs::home_dir().ok_or_else(none)?, bragd_core::dirs::config_dir().ok_or_else(none)?))
}

fn log() -> PathBuf {
    crate::folder().unwrap_or_default().join("helper.log")
}

/// Whether the Helper is registered here.
pub fn installed() -> bool {
    match Kind::here() {
        Kind::RunKey => Command::new("reg").args(["query", RUN_KEY, "/v", NAME]).output().is_ok_and(|o| o.status.success()),
        k => homes().ok().and_then(|(h, c)| k.file(&h, &c)).is_some_and(|f| f.exists()),
    }
}

/// The program the registration here starts, when it is in a file.
fn registered() -> Option<PathBuf> {
    let k = Kind::here();
    let (h, c) = homes().ok()?;
    k.exe_in(&std::fs::read_to_string(k.file(&h, &c)?).ok()?)
}

/// After an upgrade that moved the program (a Homebrew path with the version in it, a moved
/// AppImage), a registration may start a program that is gone: written again for `exe`.
pub fn refresh(exe: &Path) {
    if registered().is_some_and(|r| r != exe && !r.exists()) {
        crate::note(format_args!("the registered Helper is gone: registering {}", exe.display()));
        let _ = install(exe);
    }
}

/// Asks systemd or launchd to start the registered Helper now.
pub fn kick() {
    match Kind::here() {
        Kind::Launchd => {
            let _ = launchctl(&["kickstart", &format!("{}/{LABEL}", domain())]);
        }
        Kind::Systemd => {
            let _ = systemctl(&["start", NAME]);
        }
        _ => {}
    }
}

/// Registers `exe` (the `bragd-helper` program) to start with the session, and starts it.
pub fn install(exe: &Path) -> io::Result<()> {
    let k = Kind::here();
    if k == Kind::RunKey {
        run(Command::new("reg").args(["add", RUN_KEY, "/v", NAME, "/t", "REG_SZ", "/d", &k.unit(exe, &log()), "/f"]))?;
        return crate::helper::launch(exe, true);
    }
    let (home, config) = homes()?;
    let file = k.write(&home, &config, exe, &log())?.unwrap_or_default();
    match k {
        Kind::Systemd => {
            systemctl(&["daemon-reload"])?;
            systemctl(&["enable", "--now", NAME])
        }
        Kind::Launchd => {
            let service = format!("{}/{LABEL}", domain());
            let _ = launchctl(&["bootout", &service]);
            launchctl(&["bootstrap", &domain(), &file.to_string_lossy()])
        }
        // The session starts it from the next login; now, this one does.
        _ => crate::helper::launch(exe, true),
    }
}

/// Unregisters the Helper. A supervised one stops too; the next window starts its own.
pub fn uninstall() -> io::Result<()> {
    let k = Kind::here();
    if k == Kind::RunKey {
        return run(Command::new("reg").args(["delete", RUN_KEY, "/v", NAME, "/f"]));
    }
    match k {
        Kind::Launchd => {
            let _ = launchctl(&["bootout", &format!("{}/{LABEL}", domain())]);
        }
        Kind::Systemd => {
            let _ = systemctl(&["disable", "--now", NAME]);
        }
        _ => {}
    }
    let (home, config) = homes()?;
    k.remove(&home, &config)?;
    if k == Kind::Systemd {
        let _ = systemctl(&["daemon-reload"]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Kind; 5] = [Kind::Systemd, Kind::Autostart, Kind::Flatpak, Kind::Launchd, Kind::RunKey];

    #[test]
    fn each_system_gets_its_own_registration_in_the_temp_root() {
        let d = crate::tests::Dir::new("service");
        let (home, config) = (d.0.join("home"), d.0.join("home/.config"));
        let exe = Path::new("/opt/Bragd App/bragd-helper");
        let log = Path::new("/home/u/.local/share/bragd/helper/helper.log");
        let mut written = vec![];
        for k in ALL {
            let file = k.write(&home, &config, exe, log).unwrap();
            if let Some(f) = &file {
                assert!(f.starts_with(&home), "{f:?}");
                assert_eq!(std::fs::read_to_string(f).unwrap(), k.unit(exe, log));
                written.push(f.strip_prefix(&home).unwrap().to_path_buf());
            }
            let text = k.unit(exe, log);
            assert!(text.contains(STAY), "{k:?}");
            if k != Kind::Flatpak {
                assert_eq!(k.exe_in(&text).as_deref(), Some(exe), "{k:?}");
            }
        }
        // One separator on every system: Windows joins with `\`.
        let written: Vec<_> = written.iter().map(|p| p.to_string_lossy().replace('\\', "/")).collect();
        assert_eq!(written, [".config/systemd/user/bragd-helper.service", ".config/autostart/bragd-helper.desktop", ".config/autostart/bragd-helper.desktop", "Library/LaunchAgents/dk.mwo.bragd.helper.plist"]);

        let systemd = Kind::Systemd.unit(exe, log);
        assert!(systemd.contains("ExecStart=\"/opt/Bragd App/bragd-helper\" --stay\n"));
        assert!(systemd.contains("Restart=on-failure") && systemd.contains("WantedBy=default.target"));
        assert!(Kind::Autostart.unit(exe, log).contains("Exec=\"/opt/Bragd App/bragd-helper\" --stay\n"));
        assert!(Kind::Flatpak.unit(exe, log).contains("Exec=flatpak run --command=bragd-helper io.github.mwo_dk.Bragd --stay\n"));
        let plist = Kind::Launchd.unit(exe, log);
        assert!(plist.contains("<array><string>/opt/Bragd App/bragd-helper</string><string>--stay</string></array>"));
        assert!(plist.contains("<key>Label</key><string>dk.mwo.bragd.helper</string>"));
        assert!(plist.contains(&format!("<key>StandardErrorPath</key><string>{}</string>", log.display())));
        assert!(!plist.contains("Background"));
        assert_eq!(Kind::RunKey.unit(exe, log), "\"/opt/Bragd App/bragd-helper\" --stay");

        for k in ALL {
            k.remove(&home, &config).unwrap();
        }
        assert!(!config.join("systemd/user/bragd-helper.service").exists() && !home.join("Library/LaunchAgents/dk.mwo.bragd.helper.plist").exists());
        assert!(Kind::Systemd.supervised() && Kind::Launchd.supervised() && !Kind::Flatpak.supervised() && !Kind::RunKey.supervised());
    }

    /// A path with `&`, `%`, `$`, quotes and spaces is written escaped and read back as it is.
    #[test]
    fn odd_paths_are_escaped() {
        let exe = Path::new("/opt/A & B/100% \"$x\" `y` \\z/bragd-helper");
        let log = Path::new("/l");
        for k in [Kind::Systemd, Kind::Autostart, Kind::Launchd] {
            assert_eq!(k.exe_in(&k.unit(exe, log)).as_deref(), Some(exe), "{k:?}");
        }
        assert!(Kind::Launchd.unit(exe, log).contains("/opt/A &amp; B/100% "));
        assert!(Kind::Autostart.unit(exe, log).contains(r#"Exec="/opt/A & B/100%% \\"\\$x\\" \\`y\\` \\\\z/bragd-helper""#));
        assert!(Kind::Systemd.unit(exe, log).contains(r#"ExecStart="/opt/A & B/100%% \"$$x\" `y` \\z/bragd-helper""#));
    }
}
