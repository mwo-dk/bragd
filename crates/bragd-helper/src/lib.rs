//! The Helper: one background process per user that every Bragd window and instance shares
//! (ADR 0003). It fetches Sources on their schedule (`sources`), imports them and other files
//! into a Library, merges Duplicates again and exports Libraries to Parquet (`jobs`). Windows
//! talk to it over a local connection, one JSON line per request and per reply (`helper`). It
//! leaves ten minutes after the last window, unless it starts with the session (`service`),
//! and pauses heavy work while the machine runs on its battery (`battery`).

pub mod battery;
pub mod disk;
pub mod helper;
pub mod jobs;
pub mod net;
pub mod service;
pub mod sources;

use std::path::PathBuf;

/// This Helper's version: a newer one takes over from it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Helper's own folder: its lock, address, log, state and downloads. In the data folder,
/// which is the user's own.
pub fn folder() -> Option<PathBuf> {
    Some(bragd_core::dirs::data_dir()?.join("bragd").join("helper"))
}

/// The Library Sources and imports go into when none is named: `main.bragd` in the folder
/// Settings names for Libraries, else in Bragd's data folder.
pub fn default_library(cfg: &bragd_core::config::Config) -> Option<PathBuf> {
    let dir = if cfg.knowledge.library.trim().is_empty() {
        bragd_core::dirs::data_dir()?.join("bragd").join("libraries")
    } else {
        PathBuf::from(cfg.knowledge.library.trim())
    };
    Some(dir.join("main.bragd"))
}

/// `secs` since 1970 as `YYYY-MM-DD HH:MM:SS` UTC, for helper.log.
pub(crate) fn utc(secs: u64) -> String {
    let (days, s) = (secs / 86_400, secs % 86_400);
    // Howard Hinnant's days-to-civil.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// A line in helper.log (the Helper's stderr), with the time.
pub(crate) fn note(what: std::fmt::Arguments) {
    eprintln!("{} bragd-helper {VERSION}: {what}", utc(bragd_core::fs::now()));
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::PathBuf;

    /// A folder in the temporary folder, gone when dropped.
    pub struct Dir(pub PathBuf);

    impl Dir {
        pub fn new(name: &str) -> Dir {
            static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            // Short: a Unix socket's path has room for about 100 bytes.
            let p = std::env::temp_dir().join(format!("bh-{name}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Dir(p)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn utc_dates() {
        assert_eq!(super::utc(0), "1970-01-01 00:00:00");
        assert_eq!(super::utc(1_798_761_600 + 3661), "2027-01-01 01:01:01");
        assert_eq!(super::utc(951_782_400), "2000-02-29 00:00:00");
    }

    #[test]
    fn helper_folders_are_in_the_fake_home() {
        let tmp = std::env::temp_dir();
        assert!(super::folder().unwrap().starts_with(&tmp));
        let cfg = bragd_core::config::Config::default();
        assert!(super::default_library(&cfg).unwrap().starts_with(&tmp));
    }
}
