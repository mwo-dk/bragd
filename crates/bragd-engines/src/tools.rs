//! Programs on the machine that Bragd can run as engines: found the way `which` finds them
//! (plus `/usr/games` and, on Windows, downloads that kept their long names), the container
//! runtime (podman or docker), and the command that starts a program outside Bragd's own
//! environment (an AppImage's libraries, a Flatpak's sandbox).

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A program on PATH, like `which`. In a Flatpak, the host's `/usr/bin` too.
pub fn which(program: &str) -> Option<PathBuf> {
    let names = if cfg!(windows) { vec![format!("{program}.exe"), format!("{program}.cmd"), format!("{program}.bat")] } else { vec![program.to_string()] };
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        // An empty or relative entry would find the program in the working folder.
        .filter(|dir| dir.is_absolute())
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .chain(flatpak().into_iter().flat_map(|_| ["usr/bin", "usr/local/bin"]).map(|d| Path::new(HOST).join(d).join(program)))
        .find(|p| p.is_file())
}

/// An engine program called `name`: on PATH, on Debian and its kin in `/usr/games` (where
/// their engines go), and on Windows a download that kept its long name
/// (`stockfish-windows-x86-64-avx2.exe`) in a PATH folder or WinGet's package folders. Only
/// looked for, never run.
pub fn find(name: &str) -> Option<PathBuf> {
    which(name)
        .or_else(|| {
            let p = Path::new("/usr/games").join(name);
            (cfg!(unix) && p.is_file()).then_some(p)
        })
        .or_else(|| if cfg!(windows) { long_named(name, ".exe", windows_dirs()) } else { None })
}

/// PATH's folders, then WinGet's package folders and the one level inside each (its zips
/// unpack into a folder of their own). Never the Downloads folder: whatever lands there
/// would run as the engine.
fn windows_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).filter(|d| d.is_absolute()).collect()).unwrap_or_default();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let packages = Path::new(&local).join(r"Microsoft\WinGet\Packages");
        for pkg in std::fs::read_dir(packages).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
            dirs.extend(std::fs::read_dir(&pkg).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()));
            dirs.push(pkg);
        }
    }
    dirs
}

/// The first file (by name) in the first of `dirs` that has one named `<name>-…` or `<name>_…`
/// and ending in `ext`, letter case aside: an engine downloaded under its long name.
pub(crate) fn long_named(name: &str, ext: &str, dirs: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    let name = name.to_ascii_lowercase();
    dirs.into_iter().filter_map(|d| std::fs::read_dir(d).ok()).find_map(|rd| {
        let mut hits: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let f = p.file_name().and_then(|f| f.to_str()).unwrap_or_default().to_ascii_lowercase();
                f.strip_prefix(&name).is_some_and(|r| r.starts_with(['-', '_'])) && f.ends_with(ext) && p.is_file()
            })
            .collect();
        hits.sort();
        hits.into_iter().next()
    })
}

/// The container runtime: its name (`podman` or `docker`) and where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Runtime {
    pub name: &'static str,
    pub path: PathBuf,
}

/// podman or docker, as the setting `choice` allows: `auto` (podman first), `podman`,
/// `docker` or `off`.
pub fn container_runtime(choice: &str) -> Option<Runtime> {
    let order: &[&'static str] = match choice {
        "off" => &[],
        "podman" => &["podman"],
        "docker" => &["docker"],
        _ => &["podman", "docker"],
    };
    order.iter().find_map(|&name| which(name).map(|path| Runtime { name, path }))
}

/// The app id when Bragd runs in a Flatpak. The host's programs are then seen under
/// `/run/host`, and run there with `flatpak-spawn --host`.
pub fn flatpak() -> Option<&'static str> {
    static ID: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    ID.get_or_init(|| std::env::var("FLATPAK_ID").ok().filter(|_| Path::new("/.flatpak-info").is_file())).as_deref()
}

/// Where a Flatpak sees the host's `/usr`.
const HOST: &str = "/run/host";

/// In a Flatpak, a program `which` found on the host: its path there.
fn host_path(program: &Path) -> Option<PathBuf> {
    flatpak()?;
    Some(Path::new("/").join(program.strip_prefix(HOST).ok()?))
}

/// A command for a program other than Bragd. In a Flatpak a host program runs on the host,
/// ending when Bragd's side of it ends. In an AppImage, every setting that names the
/// AppImage's folder loses those entries, so the program never loads Bragd's packed libraries.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    if let Some(host) = host_path(Path::new(program.as_ref())) {
        let mut c = Command::new("flatpak-spawn");
        c.args(["--host", "--watch-bus"]).arg(host);
        return c;
    }
    let mut c = Command::new(program);
    if let Some(appdir) = std::env::var_os("APPDIR") {
        outside(&mut c, Path::new(&appdir), std::env::vars_os());
    }
    #[cfg(windows)]
    // Windows gives a console program its own window when its parent has none.
    if unsafe { windows_sys::Win32::System::Console::GetConsoleWindow() }.is_null() {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::os::windows::process::CommandExt::creation_flags(&mut c, CREATE_NO_WINDOW);
    }
    c
}

/// `c` without what an AppImage unpacked at `appdir` put in the environment `vars`.
fn outside(c: &mut Command, appdir: &Path, vars: impl Iterator<Item = (OsString, OsString)>) {
    for (key, value) in vars {
        if !value.to_string_lossy().contains(&*appdir.to_string_lossy()) {
            continue;
        }
        let kept: Vec<PathBuf> = std::env::split_paths(&value).filter(|p| !p.starts_with(appdir) && !p.as_os_str().is_empty()).collect();
        match std::env::join_paths(&kept) {
            Ok(v) if !kept.is_empty() => c.env(&key, v),
            _ => c.env_remove(&key),
        };
    }
    for key in ["APPDIR", "APPIMAGE", "ARGV0", "GDK_BACKEND", "GTK_THEME"] {
        c.env_remove(key);
    }
}

/// What `c` prints, with no input, given `timeout`: its output up to `max` bytes and up to
/// 64 KB of its complaints, read while it runs so a full pipe never holds it up. Killed when it
/// runs past `timeout`.
pub fn capture(mut c: Command, timeout: Duration, max: u64) -> std::io::Result<std::process::Output> {
    fn drain(pipe: Option<impl Read + Send + 'static>, max: u64) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut v = vec![];
            if let Some(mut p) = pipe {
                let _ = (&mut p).take(max).read_to_end(&mut v);
                let _ = std::io::copy(&mut p, &mut std::io::sink());
            }
            v
        })
    }
    c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn()?;
    let (out, err) = (drain(child.stdout.take(), max), drain(child.stderr.take(), 64 << 10));
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "took too long"));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Ok(std::process::Output { status, stdout: out.join().unwrap_or_default(), stderr: err.join().unwrap_or_default() })
}

/// This machine's memory in whole gigabytes; 0 when it cannot be told.
pub fn ram_gb() -> u64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|m| m.lines().find_map(|l| l.strip_prefix("MemTotal:").and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())))
        .map_or(0, |kb| kb / (1024 * 1024))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_named_downloads_are_found() {
        let root = crate::test_dir("engine-names");
        let (d, e) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::create_dir_all(&e).unwrap();
        for f in ["stockfishbook.exe", "fairy-stockfish-largeboard.exe", "stockfish-notes.txt"] {
            std::fs::write(d.join(f), b"").unwrap();
        }
        std::fs::write(e.join("Stockfish-windows-x86-64-avx2.exe"), b"").unwrap();
        std::fs::write(e.join("stockfish_17_x64.exe"), b"").unwrap();
        let dirs = || vec![d.clone(), e.clone()];
        assert_eq!(long_named("stockfish", ".exe", dirs()), Some(e.join("Stockfish-windows-x86-64-avx2.exe")));
        assert_eq!(long_named("pikafish", ".exe", dirs()), None);
        assert_eq!(long_named("fairy-stockfish", ".exe", dirs()), Some(d.join("fairy-stockfish-largeboard.exe")));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_appimage_environment_is_left_behind() {
        let mut c = Command::new("x");
        let vars = [("LD_LIBRARY_PATH", "/tmp/.mount_x/usr/lib:/usr/lib"), ("PATH", "/usr/bin"), ("GTK_PATH", "/tmp/.mount_x/gtk")].map(|(k, v)| (OsString::from(k), OsString::from(v)));
        outside(&mut c, Path::new("/tmp/.mount_x"), vars.into_iter());
        let envs: Vec<(String, Option<String>)> = c.get_envs().map(|(k, v)| (k.to_string_lossy().into(), v.map(|v| v.to_string_lossy().into()))).collect();
        assert!(envs.contains(&("LD_LIBRARY_PATH".into(), Some("/usr/lib".into()))), "{envs:?}");
        assert!(envs.contains(&("GTK_PATH".into(), None)), "{envs:?}");
        assert!(!envs.iter().any(|(k, _)| k == "PATH"));
    }

    #[test]
    fn no_runtime_when_containers_are_off() {
        assert_eq!(container_runtime("off"), None);
    }
}
