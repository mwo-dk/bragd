//! Settings, state, texts, themes and the update check.

pub mod config;
pub mod dirs;
pub mod features;
pub mod fs;
pub mod i18n;
pub mod notices;
pub mod settings;
pub mod state;
pub mod themes;
pub mod tour;
pub mod update;
pub mod wizard;

/// Nothing goes to the network while `BRAGD_OFFLINE` is set (tests and screenshots).
pub fn offline() -> bool {
    std::env::var_os("BRAGD_OFFLINE").is_some()
}
