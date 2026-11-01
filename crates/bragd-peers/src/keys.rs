//! This machine's key: Ed25519, made once from the system's random source and kept in
//! `key.pk8` in the Peers folder, which only the user can read. Its public half is the
//! machine's id, and is what the other machine pins.

use std::io;
use std::path::Path;
use std::sync::Arc;

use ring::signature::{Ed25519KeyPair, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::sign::CertifiedKey;

/// An Ed25519 public key.
pub type Key = [u8; 32];

/// The DER of an Ed25519 SubjectPublicKeyInfo, up to the key itself (RFC 8410).
const SPKI_PREFIX: [u8; 12] = [0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];

/// `key` as the SubjectPublicKeyInfo that TLS sends as a raw public key (RFC 7250).
pub fn spki(key: &Key) -> Vec<u8> {
    [&SPKI_PREFIX[..], key].concat()
}

/// The Ed25519 key in a raw public key; anything else is no key of ours.
pub fn from_spki(der: &[u8]) -> Option<Key> {
    der.strip_prefix(&SPKI_PREFIX[..])?.try_into().ok()
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok()).collect()
}

/// A key written as hex, as `peers.json` keeps it.
pub fn key_of(text: &str) -> Option<Key> {
    unhex(text)?.try_into().ok()
}

pub struct Identity {
    pkcs8: Vec<u8>,
    pub public: Key,
}

impl Identity {
    /// The key in `dir`, made there the first time.
    pub fn load_or_make(dir: &Path) -> io::Result<Identity> {
        let path = dir.join("key.pk8");
        let pkcs8 = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let doc = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new()).map_err(|_| io::Error::other("no random numbers"))?;
                std::fs::create_dir_all(dir)?;
                bragd_core::fs::replace(&path, doc.as_ref(), true)?;
                doc.as_ref().to_vec()
            }
            Err(e) => return Err(e),
        };
        let pair = Ed25519KeyPair::from_pkcs8(&pkcs8).map_err(|e| io::Error::other(format!("{}: {e}", path.display())))?;
        let public = pair.public_key().as_ref().try_into().map_err(|_| io::Error::other("not an Ed25519 key"))?;
        Ok(Identity { pkcs8, public })
    }

    /// The id other machines know this one by: its public key, as hex.
    pub fn id(&self) -> String {
        hex(&self.public)
    }

    /// The key as TLS presents it: the raw public key and its signer.
    pub(crate) fn certified(&self) -> io::Result<Arc<CertifiedKey>> {
        let der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.pkcs8.clone()));
        let signer = crate::tls::provider().key_provider.load_private_key(der).map_err(io::Error::other)?;
        Ok(Arc::new(CertifiedKey::new(vec![CertificateDer::from(spki(&self.public))], signer)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_survives_its_spki_and_hex() {
        let k: Key = std::array::from_fn(|i| i as u8 * 7);
        assert_eq!(from_spki(&spki(&k)), Some(k));
        assert_eq!(key_of(&hex(&k)), Some(k));
        assert_eq!(from_spki(&spki(&k)[1..]), None);
        assert_eq!(key_of("abc"), None);
    }

    #[test]
    fn the_key_is_made_once_and_kept() {
        let dir = std::env::temp_dir().join(format!("bp-key-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let a = Identity::load_or_make(&dir).unwrap();
        let b = Identity::load_or_make(&dir).unwrap();
        assert_eq!(a.public, b.public);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(dir.join("key.pk8")).unwrap().permissions().mode() & 0o077, 0);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
