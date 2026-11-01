//! The key of a model server, in the OS keychain only: the Secret Service on Linux and the
//! BSDs, the Keychain on macOS, the Credential Manager on Windows. Never in config.toml or a file
//! of Bragd's. One key per address.

use std::sync::Once;

use bragd_core::t;
use keyring_core::{Entry, Error};

const SERVICE: &str = "bragd-model";

/// The system's keychain, as keyring's default store, set once. Tests set keyring's in-memory
/// sample store first.
fn store() -> Result<(), String> {
    static SET: Once = Once::new();
    let mut failed = None;
    SET.call_once(|| {
        if keyring_core::get_default_store().is_some() {
            return;
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        let s = dbus_secret_service_keyring_store::Store::new();
        #[cfg(target_os = "macos")]
        let s = apple_native_keyring_store::keychain::Store::new();
        #[cfg(windows)]
        let s = windows_native_keyring_store::Store::new();
        match s {
            Ok(s) => keyring_core::set_default_store(s),
            Err(e) => failed = Some(e.to_string()),
        }
    });
    match failed {
        Some(why) => Err(t!("model.error.keychain", "why" => why)),
        None if keyring_core::get_default_store().is_none() => Err(t!("model.error.keychain", "why" => "-")),
        None => Ok(()),
    }
}

fn entry(service: &str, account: &str) -> Result<Entry, String> {
    store()?;
    Entry::new(service, account.trim()).map_err(|e| t!("model.error.keychain", "why" => e))
}

/// The key kept for `url`, if any.
pub fn get(url: &str) -> Option<String> {
    get_in(SERVICE, url)
}

/// Keeps `key` for `url`; an empty one removes it.
pub fn set(url: &str, key: &str) -> Result<(), String> {
    set_in(SERVICE, url, key)
}

/// The secret kept for `account` under `service`, if any. Other crates (an online server's
/// token) keep theirs through here, so the keychain is set up in one place.
pub fn get_in(service: &str, account: &str) -> Option<String> {
    entry(service, account).ok()?.get_password().ok()
}

/// Keeps `key` for `account` under `service`; an empty one removes it.
pub fn set_in(service: &str, account: &str, key: &str) -> Result<(), String> {
    let e = entry(service, account)?;
    let r = if key.trim().is_empty() { e.delete_credential() } else { e.set_password(key.trim()) };
    match r {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(e) => Err(t!("model.error.keychain", "why" => e)),
    }
}

/// keyring's in-memory store in place of the system's keychain, for tests (here and in other
/// crates' tests, which turn on the `mock-keychain` feature).
#[cfg(any(test, feature = "mock-keychain"))]
pub fn mock() {
    static M: Once = Once::new();
    M.call_once(|| keyring_core::set_default_store(keyring_core::sample::Store::new().unwrap()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_kept_per_address_and_removed_by_an_empty_one() {
        mock();
        let (a, b) = ("https://a.example/v1", "https://b.example/v1");
        assert_eq!(get(a), None);
        set(a, " sk-a ").unwrap();
        set(b, "sk-b").unwrap();
        assert_eq!(get(a).as_deref(), Some("sk-a"));
        assert_eq!(get(b).as_deref(), Some("sk-b"));
        set(a, "").unwrap();
        set(a, "").unwrap();
        assert_eq!(get(a), None);
        assert_eq!(get(b).as_deref(), Some("sk-b"));
    }
}
