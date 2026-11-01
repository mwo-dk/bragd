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

    /// `<temp>/bragd-test-home/<test binary>`: one per test binary, so runs leave one folder
    /// each.
    pub fn home_dir() -> Option<PathBuf> {
        static HOME: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        let home = HOME.get_or_init(|| {
            let exe = std::env::current_exe().ok();
            let name = exe.as_deref().and_then(|e| e.file_stem()).map_or_else(
                || format!("pid-{}", std::process::id()),
                |s| s.to_string_lossy().into_owned(),
            );
            let home = std::env::temp_dir().join("bragd-test-home").join(name);
            let _ = std::fs::create_dir_all(&home);
            home
        });
        Some(home.clone())
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
}
