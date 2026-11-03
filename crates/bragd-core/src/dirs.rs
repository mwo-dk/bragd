//! The user's folders: home, config, data and cache. Everything that finds them goes through
//! here, never through `dirs` or `std::env::home_dir` itself, so that tests (this crate's, and
//! the apps' with the `test-home` feature their dev-dependency turns on) get a home of their
//! own in the temporary folder and never read or write the developer's own config and data.

#[cfg(not(any(test, feature = "test-home")))]
pub use ::dirs::{cache_dir, config_dir, data_dir};
#[cfg(not(any(test, feature = "test-home")))]
pub use std::env::home_dir;

#[cfg(any(test, feature = "test-home"))]
mod fake {
    use std::path::PathBuf;

    static HOME: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

    /// `<temp>/bragd-test-home/<test binary>-<pid>-<random>`: a new one for each run of a test
    /// binary, so two runs, at once or one after the other, never see each other's files. It is
    /// removed when the run ends.
    pub fn home_dir() -> Option<PathBuf> {
        let home = HOME.get_or_init(|| {
            let exe = std::env::current_exe().ok();
            let name = exe.as_deref().and_then(|e| e.file_stem()).map_or_else(|| "bragd".into(), |s| s.to_string_lossy().into_owned());
            let parent = std::env::temp_dir().join("bragd-test-home");
            let _ = std::fs::create_dir_all(&parent);
            let home = tempfile::Builder::new().prefix(&format!("{name}-{}-", std::process::id())).tempdir_in(&parent).expect("a test home in the temporary folder").keep();
            // SAFETY: `atexit` only keeps the function to call; `remove` takes nothing.
            unsafe { atexit(remove) };
            home
        });
        Some(home.clone())
    }

    unsafe extern "C" {
        fn atexit(f: extern "C" fn()) -> std::ffi::c_int;
    }

    /// Run as the test binary exits, whether its tests passed or not.
    extern "C" fn remove() {
        if let Some(home) = HOME.get() {
            let _ = std::fs::remove_dir_all(home);
        }
    }

    // shortcut: one layout (XDG's) on every system; give each its own when a test needs it.
    fn under(rel: &str) -> Option<PathBuf> {
        Some(home_dir()?.join(rel))
    }
    pub fn config_dir() -> Option<PathBuf> {
        under(".config")
    }
    pub fn data_dir() -> Option<PathBuf> {
        under(".local/share")
    }
    pub fn cache_dir() -> Option<PathBuf> {
        under(".cache")
    }
}
#[cfg(any(test, feature = "test-home"))]
pub use fake::*;

/// Bragd's own folder in the config folder, `<config>/bragd`. None when the system names no
/// config folder: never a relative path, which would land wherever Bragd was started.
pub fn app_config() -> Option<std::path::PathBuf> {
    Some(config_dir()?.join("bragd"))
}

/// Bragd's own folder in the data folder, `<data>/bragd`: Libraries, the Helper, downloads.
pub fn app_data() -> Option<std::path::PathBuf> {
    Some(data_dir()?.join("bragd"))
}

/// Bragd's own folder in the cache folder, `<cache>/bragd`.
pub fn app_cache() -> Option<std::path::PathBuf> {
    Some(cache_dir()?.join("bragd"))
}

#[cfg(test)]
mod tests {
    /// Every place Bragd keeps the user's things lies in the temporary folder in tests.
    #[test]
    fn dirs_tests_never_reach_the_real_home() {
        let tmp = std::env::temp_dir();
        for (what, path) in [
            ("home", super::home_dir()),
            ("config dir", super::config_dir()),
            ("data dir", super::data_dir()),
            ("cache dir", super::cache_dir()),
            ("app config", super::app_config()),
            ("app data", super::app_data()),
            ("app cache", super::app_cache()),
            ("config", crate::config::Config::path()),
            ("state", crate::state::AppState::path()),
        ] {
            let path = path.unwrap_or_else(|| panic!("{what}: none"));
            assert!(path.is_absolute() && path.starts_with(&tmp), "{what}: {}", path.display());
        }
    }

    /// Each run of a test binary has a home of its own, and it is gone when the run ends.
    #[test]
    fn dirs_each_run_has_its_own_home_removed_after() {
        const CHILD: &str = "BRAGD_TEST_HOME_CHILD";
        let home = super::home_dir().unwrap();
        if std::env::var_os(CHILD).is_some() {
            std::fs::write(home.join("left"), "").unwrap();
            println!("home={}", home.display());
            return;
        }
        let run = || {
            let out = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "dirs::tests::dirs_each_run_has_its_own_home_removed_after", "--nocapture", "--test-threads=1"])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            let said = String::from_utf8_lossy(&out.stdout).into_owned();
            std::path::PathBuf::from(said.lines().find_map(|l| l.split_once("home=").map(|(_, h)| h)).unwrap_or_else(|| panic!("no home: {said}")))
        };
        let (a, b) = (run(), run());
        assert!(a != b && a != home && b != home, "{} {} {}", a.display(), b.display(), home.display());
        assert!(!a.exists() && !b.exists(), "removed when the run ended: {} {}", a.display(), b.display());
    }
}
