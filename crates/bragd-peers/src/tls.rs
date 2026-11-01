//! TLS between Peers: 1.3 only, each side presenting its Ed25519 key as a raw public key
//! (RFC 7250), no certificate and no CA. A key is accepted only when it is pinned, or, while a
//! pairing code is open, so that the connection can pair and nothing else (`node`).

use std::sync::{Arc, LazyLock};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls13_signature_with_raw_key};
use rustls::pki_types::{CertificateDer, ServerName, SubjectPublicKeyInfoDer, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{CertificateError, DigitallySignedStruct, DistinguishedName, Error, SignatureScheme};

use crate::keys::{Key, from_spki};

/// ring, as the rest of Bragd's TLS.
pub(crate) fn provider() -> Arc<CryptoProvider> {
    static P: LazyLock<Arc<CryptoProvider>> = LazyLock::new(|| Arc::new(rustls::crypto::ring::default_provider()));
    P.clone()
}

/// The only handshake signature a Peer makes.
const SCHEMES: [SignatureScheme; 1] = [SignatureScheme::ED25519];

fn refused() -> Error {
    Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure)
}

fn check_signature(message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
    if dss.scheme != SignatureScheme::ED25519 || from_spki(cert).is_none() {
        return Err(refused());
    }
    verify_tls13_signature_with_raw_key(message, &SubjectPublicKeyInfoDer::from(cert.as_ref()), dss, &provider().signature_verification_algorithms)
}

/// Who may connect to this machine: the server's side.
pub(crate) trait Admit: Send + Sync {
    fn admits(&self, key: &Key) -> bool;
}

struct Clients(Arc<dyn Admit>);

impl std::fmt::Debug for Clients {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Clients")
    }
}

impl ClientCertVerifier for Clients {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(&self, end: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: UnixTime) -> Result<ClientCertVerified, Error> {
        match from_spki(end) {
            Some(k) if self.0.admits(&k) => Ok(ClientCertVerified::assertion()),
            _ => Err(refused()),
        }
    }

    fn verify_tls12_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        Err(refused())
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        check_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        SCHEMES.to_vec()
    }

    fn requires_raw_public_keys(&self) -> bool {
        true
    }
}

/// The server a client expects: one pinned key, or, to pair, any (SPAKE2 then binds it).
#[derive(Debug)]
struct Server(Option<Key>);

impl ServerCertVerifier for Server {
    fn verify_server_cert(&self, end: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, Error> {
        match (from_spki(end), self.0) {
            (Some(k), Some(want)) if k == want => Ok(ServerCertVerified::assertion()),
            (Some(_), None) => Ok(ServerCertVerified::assertion()),
            _ => Err(refused()),
        }
    }

    fn verify_tls12_signature(&self, _: &[u8], _: &CertificateDer<'_>, _: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        Err(refused())
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        check_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        SCHEMES.to_vec()
    }

    fn requires_raw_public_keys(&self) -> bool {
        true
    }
}

pub(crate) fn server(me: Arc<rustls::sign::CertifiedKey>, admit: Arc<dyn Admit>) -> Result<Arc<rustls::ServerConfig>, Error> {
    let c = rustls::ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_client_cert_verifier(Arc::new(Clients(admit)))
        .with_cert_resolver(Arc::new(rustls::server::AlwaysResolvesServerRawPublicKeys::new(me)));
    Ok(Arc::new(c))
}

/// A client config that expects `server`'s key, or any key when `None` (pairing).
pub(crate) fn client(me: Arc<rustls::sign::CertifiedKey>, server: Option<Key>) -> Result<Arc<rustls::ClientConfig>, Error> {
    let c = rustls::ClientConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Server(server)))
        .with_client_cert_resolver(Arc::new(rustls::client::AlwaysResolvesClientRawPublicKeys::new(me)));
    Ok(Arc::new(c))
}
