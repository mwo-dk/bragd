//! TURN credentials for a Call (coturn's TURN REST API, `use-auth-secret`): a username
//! `<expiry>:<user id>` and, as the password, the base64 of its HMAC-SHA1 under the secret coturn
//! shares with the game service (`packaging/server/turnserver.conf`). They expire on their own.

/// How long a credential lasts: a long Call outlives it only once it is set up.
pub const TTL: u64 = 12 * 60 * 60;

use base64::Engine;

/// The username and password for `user` at `now` (seconds since 1970), under `secret`.
pub fn credentials(secret: &str, user: &str, now: u64) -> (String, String) {
    let username = format!("{}:{}", now + TTL, user);
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret.as_bytes());
    let signature = ring::hmac::sign(&key, username.as_bytes());
    let password = base64::engine::general_purpose::STANDARD.encode(signature.as_ref());
    (username, password)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_credentials_follow_coturns_rest_api() {
        let (user, pass) = credentials("s3cret", "6f1c0b52-6d3a-4c58-9a57-5f4b1b9f3a10", 1_800_000_000);
        assert_eq!(user, "1800043200:6f1c0b52-6d3a-4c58-9a57-5f4b1b9f3a10");
        // HMAC-SHA1("s3cret", user), in standard base64 with padding, as coturn computes it.
        assert_eq!(pass, "+ROYvlMTR4YQSPM7TG8Jq9lA88I=");
        let (_, other) = credentials("other", "6f1c0b52-6d3a-4c58-9a57-5f4b1b9f3a10", 1_800_000_000);
        assert_ne!(pass, other);
    }
}
