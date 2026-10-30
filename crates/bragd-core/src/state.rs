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
    /// What a newer Bragd keeps here that this one does not know: saved as it was, so going
    /// back to an older version and forward again loses none of it.
    #[serde(flatten)]
    pub newer: serde_json::Map<String, serde_json::Value>,
    /// The file is there but could not be read (no permission, say): it is never saved over.
    #[serde(skip)]
    pub unreadable: bool,
}

impl AppState {
    pub fn path() -> Option<PathBuf> {
        Some(crate::dirs::data_dir()?.join("bragd").join("state.json"))
    }

    /// Missing file = defaults. A corrupt file is moved aside to `state.json.bad`
    /// (`state.json.<time>.bad` when that is taken) so saving the defaults never destroys it.
    /// One that cannot be read gives defaults that are never saved over it.
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
        serde_json::from_slice(&bytes).unwrap_or_else(|_| {
            let mut bad = path.with_extension("json.bad");
            if bad.exists() {
                bad = path.with_extension(format!("json.{}.bad", crate::fs::now()));
            }
            let _ = std::fs::rename(path, bad);
            AppState::default()
        })
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
        crate::fs::replace(path, &serde_json::to_vec_pretty(self)?, true)
    }

    /// A copy kept in memory takes what another window wrote since it was read, before it is
    /// changed and saved. A missing or corrupt file leaves the copy as it is.
    pub fn reload_from(&mut self, path: &Path) {
        match std::fs::read(path) {
            Ok(bytes) => {
                if let Ok(st) = serde_json::from_slice(&bytes) {
                    *self = st;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(_) => self.unreadable = true,
        }
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
    fn state_keeps_what_a_newer_version_wrote() {
        let p = fresh("state-newer");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(
            &p,
            r#"{"seen_version":"9.0.0","ratings":{"go":1800},"guide_seen":true}"#,
        )
        .unwrap();
        let mut st = AppState::load_from(&p);
        assert_eq!(st.seen_version, "9.0.0");
        st.hints_shown.insert("sidebar".into(), 1);
        st.save_to(&p).unwrap();
        let back: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        assert_eq!(back["ratings"]["go"], 1800);
        assert_eq!(back["hints_shown"]["sidebar"], 1);
        assert_eq!(AppState::load_from(&p), st);
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
