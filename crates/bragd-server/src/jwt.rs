//! Supabase Auth's access tokens, checked: the signature (HS256 with the project's JWT secret,
//! or ES256 and RS256 with the keys Supabase publishes as a JWKS), the expiry, the audience
//! `authenticated` and the role. What is left is the caller's id and their claims, which the
//! database reads as `auth.uid()`.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use ring::signature::{self, UnparsedPublicKey};
use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A signed-in caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caller {
    pub id: uuid::Uuid,
    /// The token's claims, as `request.jwt.claims`.
    pub claims: String,
}

/// A public key from the JWKS.
#[derive(Clone, Debug)]
struct Key {
    kid: String,
    alg: &'static str,
    /// ES256: the uncompressed point; RS256: (n, e).
    bytes: Vec<u8>,
    e: Vec<u8>,
}

/// What checks tokens: the HS256 secret, and the JWKS address with the keys last read there.
pub struct Verifier {
    secret: Option<Vec<u8>>,
    jwks_url: Option<String>,
    keys: Mutex<(Vec<Key>, Option<Instant>)>,
}

/// How often an unknown key id may make the JWKS be read again.
const REFRESH: Duration = Duration::from_secs(60);

impl Verifier {
    pub fn new(secret: Option<&str>, jwks_url: Option<&str>) -> Verifier {
        Verifier {
            secret: secret.filter(|s| !s.is_empty()).map(|s| s.as_bytes().to_vec()),
            jwks_url: jwks_url.filter(|s| !s.is_empty()).map(str::to_string),
            keys: Mutex::new((vec![], None)),
        }
    }

    /// A verifier that knows `jwks` (a JWKS document) and reads no address.
    pub fn with_jwks(secret: Option<&str>, jwks: &str) -> Result<Verifier, String> {
        let v = Verifier::new(secret, None);
        *v.keys.lock().unwrap_or_else(|e| e.into_inner()) = (parse_jwks(jwks)?, Some(Instant::now()));
        Ok(v)
    }

    /// The caller of `token`, or why it is refused.
    pub fn check(&self, token: &str) -> Result<Caller, String> {
        self.check_at(token, now())
    }

    pub fn check_at(&self, token: &str, now: u64) -> Result<Caller, String> {
        let mut parts = token.split('.');
        let (Some(h), Some(p), Some(s), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
            return Err("not a JWT".into());
        };
        let header: Value = serde_json::from_slice(&B64.decode(h).map_err(|_| "bad header")?).map_err(|_| "bad header")?;
        let sig = B64.decode(s).map_err(|_| "bad signature")?;
        let signed = &token.as_bytes()[..h.len() + 1 + p.len()];
        match header["alg"].as_str() {
            Some("HS256") => {
                let secret = self.secret.as_ref().ok_or("HS256 is not accepted here")?;
                let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret);
                ring::hmac::verify(&key, signed, &sig).map_err(|_| "bad signature")?;
            }
            Some(alg @ ("ES256" | "RS256")) => {
                let kid = header["kid"].as_str().unwrap_or_default();
                let key = self.key(kid, alg)?;
                let ok = if alg == "ES256" {
                    UnparsedPublicKey::new(&signature::ECDSA_P256_SHA256_FIXED, &key.bytes).verify(signed, &sig).is_ok()
                } else {
                    signature::RsaPublicKeyComponents { n: &key.bytes, e: &key.e }.verify(&signature::RSA_PKCS1_2048_8192_SHA256, signed, &sig).is_ok()
                };
                if !ok {
                    return Err("bad signature".into());
                }
            }
            _ => return Err("algorithm not accepted".into()),
        }
        let claims: Value = serde_json::from_slice(&B64.decode(p).map_err(|_| "bad claims")?).map_err(|_| "bad claims")?;
        if claims["exp"].as_u64().is_none_or(|exp| exp <= now) {
            return Err("expired".into());
        }
        let aud_ok = match &claims["aud"] {
            Value::String(a) => a == "authenticated",
            Value::Array(a) => a.iter().any(|a| a == "authenticated"),
            _ => false,
        };
        if !aud_ok || claims["role"] != "authenticated" {
            return Err("not a signed-in user".into());
        }
        let id = claims["sub"].as_str().and_then(|s| uuid::Uuid::parse_str(s).ok()).ok_or("no user")?;
        Ok(Caller { id, claims: claims.to_string() })
    }

    /// The key `kid` for `alg`, reading the JWKS again when it is not known (at most once a
    /// minute, so a stream of made-up ids cannot make the server hammer Supabase).
    fn key(&self, kid: &str, alg: &str) -> Result<Key, String> {
        let find = |keys: &[Key]| keys.iter().find(|k| k.kid == kid && k.alg == alg).cloned();
        let mut held = self.keys.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(k) = find(&held.0) {
            return Ok(k);
        }
        let Some(url) = &self.jwks_url else { return Err("unknown key".into()) };
        if held.1.is_some_and(|at| at.elapsed() < REFRESH) {
            return Err("unknown key".into());
        }
        held.1 = Some(Instant::now());
        let body = ureq::get(url).call().map_err(|e| format!("JWKS: {e}"))?.body_mut().read_to_string().map_err(|e| format!("JWKS: {e}"))?;
        held.0 = parse_jwks(&body)?;
        find(&held.0).ok_or_else(|| "unknown key".into())
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The ES256 and RS256 keys of a JWKS document; others are skipped.
fn parse_jwks(text: &str) -> Result<Vec<Key>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("JWKS: {e}"))?;
    let b = |k: &Value, f: &str| k[f].as_str().and_then(|s| B64.decode(s).ok());
    let mut out = vec![];
    for k in v["keys"].as_array().ok_or("JWKS: no keys")? {
        let kid = k["kid"].as_str().unwrap_or_default().to_string();
        match (k["kty"].as_str(), k["crv"].as_str()) {
            (Some("EC"), Some("P-256")) => {
                if let (Some(x), Some(y)) = (b(k, "x"), b(k, "y")) {
                    out.push(Key { kid, alg: "ES256", bytes: [&[4u8][..], &x, &y].concat(), e: vec![] });
                }
            }
            (Some("RSA"), _) => {
                if let (Some(n), Some(e)) = (b(k, "n"), b(k, "e")) {
                    out.push(Key { kid, alg: "RS256", bytes: n, e });
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// A token signed with `secret` (HS256) for `claims`: for tests and the fakes.
pub fn sign_hs256(secret: &str, claims: &Value) -> String {
    let signed = format!("{}.{}", B64.encode(r#"{"alg":"HS256","typ":"JWT"}"#), B64.encode(claims.to_string()));
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_bytes());
    let sig = ring::hmac::sign(&key, signed.as_bytes());
    format!("{signed}.{}", B64.encode(sig.as_ref()))
}

/// The claims Supabase Auth gives a signed-in user `id`, valid until `exp`.
pub fn user_claims(id: uuid::Uuid, exp: u64) -> Value {
    serde_json::json!({ "sub": id.to_string(), "aud": "authenticated", "role": "authenticated", "exp": exp })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair};

    const NOW: u64 = 1_800_000_000;

    fn id() -> uuid::Uuid {
        uuid::Uuid::parse_str("6f1c0b52-6d3a-4c58-9a57-5f4b1b9f3a10").unwrap()
    }

    #[test]
    fn jwt_hs256_accepts_the_right_secret_only() {
        let v = Verifier::new(Some("s3cret"), None);
        let good = sign_hs256("s3cret", &user_claims(id(), NOW + 60));
        assert_eq!(v.check_at(&good, NOW).unwrap().id, id());
        let bad = sign_hs256("other", &user_claims(id(), NOW + 60));
        assert_eq!(v.check_at(&bad, NOW).unwrap_err(), "bad signature");
        assert!(Verifier::new(None, None).check_at(&good, NOW).is_err(), "no secret: HS256 refused");
    }

    #[test]
    fn jwt_refuses_expired_anonymous_and_tampered_tokens() {
        let v = Verifier::new(Some("k"), None);
        assert_eq!(v.check_at(&sign_hs256("k", &user_claims(id(), NOW)), NOW).unwrap_err(), "expired");
        let mut anon = user_claims(id(), NOW + 60);
        anon["role"] = "anon".into();
        assert!(v.check_at(&sign_hs256("k", &anon), NOW).is_err());
        let mut no_aud = user_claims(id(), NOW + 60);
        no_aud["aud"] = "other".into();
        assert!(v.check_at(&sign_hs256("k", &no_aud), NOW).is_err());
        // Another user's claims under the first token's signature.
        let good = sign_hs256("k", &user_claims(id(), NOW + 60));
        let mut other = user_claims(uuid::Uuid::nil(), NOW + 60);
        other["sub"] = "00000000-0000-0000-0000-000000000001".into();
        let parts: Vec<&str> = good.split('.').collect();
        let forged = format!("{}.{}.{}", parts[0], B64.encode(other.to_string()), parts[2]);
        assert_eq!(v.check_at(&forged, NOW).unwrap_err(), "bad signature");
        // `none` is never an algorithm.
        let none = format!("{}.{}.", B64.encode(r#"{"alg":"none"}"#), parts[1]);
        assert!(v.check_at(&none, NOW).is_err());
    }

    #[test]
    fn jwt_es256_checks_against_the_jwks() {
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
        let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &rng).unwrap();
        let point = pair.public_key().as_ref();
        let jwks = serde_json::json!({ "keys": [{ "kty": "EC", "crv": "P-256", "kid": "k1", "alg": "ES256",
            "x": B64.encode(&point[1..33]), "y": B64.encode(&point[33..]) }] });
        let v = Verifier::with_jwks(None, &jwks.to_string()).unwrap();
        let sign = |kid: &str| {
            let signed = format!("{}.{}", B64.encode(format!(r#"{{"alg":"ES256","kid":"{kid}"}}"#)), B64.encode(user_claims(id(), NOW + 60).to_string()));
            let sig = pair.sign(&rng, signed.as_bytes()).unwrap();
            format!("{signed}.{}", B64.encode(sig.as_ref()))
        };
        assert_eq!(v.check_at(&sign("k1"), NOW).unwrap().id, id());
        assert_eq!(v.check_at(&sign("k2"), NOW).unwrap_err(), "unknown key");
        let t = sign("k1");
        let i = t.len() - 10;
        let c = if &t[i..=i] == "A" { "B" } else { "A" };
        let flipped = format!("{}{c}{}", &t[..i], &t[i + 1..]);
        assert!(v.check_at(&flipped, NOW).is_err());
    }
}
