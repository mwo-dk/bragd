//! What Bragd remembers between runs that is not a setting: the wizard, the version last seen,
//! the update check, container images, notices and hints. One JSON file in the data folder,
//! written atomically.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    /// The install wizard was gone through or skipped.
    pub guide_seen: bool,
    /// The version whose tour cards were last seen; empty before the first start.
    pub seen_version: String,
    /// Unix time of the last update check, the latest version it found, and who said so: the
    /// copy's channel, or GitHub when the channel could not say.
    pub update_checked: u64,
    pub latest_version: String,
    pub latest_from: String,
    /// Container image -> Unix time an Engine last ran in it (images unused for 90 days go).
    pub images_used: BTreeMap<String, u64>,
    /// Unix time of the last round of image housekeeping.
    pub images_checked: u64,
    /// How often each hint was shown (`notices::hint_due`).
    pub hints_shown: BTreeMap<String, u32>,
    /// Notices dismissed (`notices`).
    pub notices_dismissed: Vec<String>,
    /// How often each Feature was used: its usage signal (`tips`). Never leaves the machine.
    pub feature_uses: BTreeMap<String, u32>,
    /// Unix time the last Tip showed, and its Feature.
    pub tip_shown: u64,
    pub tip_last: String,
    /// Features whose Tip was answered *Don't suggest this again*.
    pub tips_silenced: Vec<String>,
    /// Unix time of the last upgrade (its tour cards seen), and the version it came from.
    pub upgraded: u64,
    pub upgraded_from: String,
    /// The Club files opened or made here, for the Clubs page's list.
    pub clubs: Vec<PathBuf>,
    /// The file is there but could not be read (no permission, say), or a newer Bragd wrote
    /// it: it is never saved over.
    #[serde(skip)]
    pub unreadable: bool,
}

/// state.json's format: its `format` key.
pub const FORMAT: crate::format::Json = crate::format::Json { file: "state.json", migrations: &[] };

impl AppState {
    pub fn path() -> Option<PathBuf> {
        Some(crate::dirs::app_data()?.join("state.json"))
    }

    /// Missing file = defaults. A corrupt file is moved aside to `state.json.bad`
    /// (`state.json.<time>.bad` when that is taken) so saving the defaults never destroys it.
    /// One that cannot be read, or one from a newer Bragd, gives defaults that are never saved
    /// over it.
    pub fn load_from(path: &Path) -> AppState {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return AppState::default(),
            Err(_) => {
                return AppState {
                    unreadable: true,
                    ..AppState::default()
                };
            }
        };
        let mut v: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => return AppState::set_aside(path),
        };
        if FORMAT.upgrade(&mut v).is_err() {
            return AppState { unreadable: true, ..AppState::default() };
        }
        serde_json::from_value(v).unwrap_or_else(|_| AppState::set_aside(path))
    }

    /// A corrupt file moved aside: defaults.
    fn set_aside(path: &Path) -> AppState {
        let mut bad = path.with_extension("json.bad");
        if bad.exists() {
            bad = path.with_extension(format!("json.{}.bad", crate::fs::now()));
        }
        let _ = std::fs::rename(path, bad);
        AppState::default()
    }

    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        if self.unreadable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "{}: could not be read, so it is not saved over",
                    path.display()
                ),
            ));
        }
        let mut v = serde_json::to_value(self)?;
        FORMAT.stamp(&mut v);
        crate::fs::replace(path, &serde_json::to_vec_pretty(&v)?, true)
    }

    pub fn load() -> AppState {
        AppState::path()
            .map(|p| AppState::load_from(&p))
            .unwrap_or_default()
    }

    pub fn save(&self) -> io::Result<()> {
        match AppState::path() {
            Some(p) => self.save_to(&p),
            None => Err(io::Error::new(io::ErrorKind::NotFound, "no data folder")),
        }
    }

    /// Whether the install wizard is due: on the very first start, until it is closed.
    pub fn guide_due(&self) -> bool {
        !self.guide_seen && self.seen_version.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(name: &str) -> PathBuf {
        let dir = crate::dirs::data_dir().unwrap().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("state.json")
    }

    #[test]
    fn a_state_from_a_newer_bragd_is_never_saved_over() {
        let p = fresh("state-newer");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let newer = format!(r#"{{"format":{},"seen_version":"9.0.0"}}"#, FORMAT.version() + 1);
        std::fs::write(&p, &newer).unwrap();
        let st = AppState::load_from(&p);
        assert_eq!((st.unreadable, st.seen_version.as_str()), (true, ""));
        assert!(st.save_to(&p).is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), newer);
    }

    #[test]
    fn state_is_saved_with_its_format() {
        let p = fresh("state-format");
        AppState { seen_version: "1.0.0".into(), ..AppState::default() }.save_to(&p).unwrap();
        let back: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        assert_eq!((back["format"].as_i64(), back["seen_version"].as_str()), (Some(FORMAT.version()), Some("1.0.0")));
        assert_eq!(AppState::load_from(&p).seen_version, "1.0.0");
    }

    #[test]
    fn state_corrupt_file_is_moved_aside() {
        let p = fresh("state-corrupt");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "not json").unwrap();
        let st = AppState::load_from(&p);
        assert_eq!(st, AppState::default());
        assert!(st.guide_due());
        assert_eq!(
            std::fs::read(p.with_extension("json.bad")).unwrap(),
            b"not json"
        );
        st.save_to(&p).unwrap();
        assert_eq!(AppState::load_from(&p), st);
    }
}
