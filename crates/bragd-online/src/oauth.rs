//! Signing in over OAuth 2 with PKCE (RFC 7636) and a loopback redirect (RFC 8252): the server's
//! own page opens in the browser, the user signs in there, and the browser comes back to a port
//! Bragd listens on at 127.0.0.1 with a code. Bragd swaps the code and its verifier for a token.
//! Bragd never sees the password.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use bragd_core::t;
use sha2::{Digest, Sha256};

/// How a server signs a client in: its pages, Bragd's name there, and what Bragd asks for.
#[derive(Debug, PartialEq, Eq)]
pub struct Oauth {
    /// The sign-in page, after the server's address: `/oauth`.
    pub authorize: &'static str,
    /// Where a code is swapped for a token: `/api/token`.
    pub token: &'static str,
    /// Bragd's name there.
    pub client_id: &'static str,
    /// What Bragd asks to do, separated by spaces.
    pub scopes: &'static str,
}

/// One sign-in's secrets: the verifier, its SHA-256 sent as the challenge, and the state that
/// ties the browser's answer to this sign-in.
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

fn random(n: usize) -> String {
    let mut b = vec![0u8; n];
    getrandom::fill(&mut b).expect("the operating system's random source");
    URL_SAFE_NO_PAD.encode(b)
}

impl Pkce {
    pub fn new() -> Pkce {
        let verifier = random(32);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Pkce { verifier, challenge, state: random(16) }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

pub(crate) fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                match std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(v) => out.push(v),
                    None => out.extend_from_slice(&b[i..i + 3]),
                }
                i += 3;
                continue;
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The server's sign-in page for `pkce`, coming back to `redirect`.
pub fn authorize_url(o: &Oauth, base: &str, redirect: &str, pkce: &Pkce) -> String {
    format!(
        "{}{}?response_type=code&client_id={}&redirect_uri={}&scope={}&code_challenge_method=S256&code_challenge={}&state={}",
        base.trim_end_matches('/'),
        o.authorize,
        encode(o.client_id),
        encode(redirect),
        encode(o.scopes),
        pkce.challenge,
        pkce.state
    )
}

/// The port at 127.0.0.1 the browser comes back to. Only this machine can reach it.
pub struct Loopback {
    listener: TcpListener,
    pub redirect: String,
}

impl Loopback {
    /// Listens on `port`, or on any free one for 0. A server that knows its clients' redirects
    /// beforehand (OGS) needs the one it was told.
    pub fn bind(port: u16) -> Result<Loopback, String> {
        let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| t!("online.error.loopback", "why" => e))?;
        listener.set_nonblocking(true).map_err(|e| t!("online.error.loopback", "why" => e))?;
        let port = listener.local_addr().map_err(|e| t!("online.error.loopback", "why" => e))?.port();
        Ok(Loopback { listener, redirect: format!("http://127.0.0.1:{port}/") })
    }

    /// The code the browser brings back from `server` with `state`, within `limit`. Stops when
    /// `stop` is set. A request without that state (a favicon, another page) is answered and
    /// ignored.
    pub fn wait(&self, server: &str, state: &str, stop: &AtomicBool, limit: Duration) -> Result<String, String> {
        let end = Instant::now() + limit;
        while Instant::now() < end {
            if stop.load(Ordering::Relaxed) {
                return Err(t!("online.error.cancelled"));
            }
            match self.listener.accept() {
                Ok((conn, _)) => {
                    if let Some(answer) = answer(conn, server, state) {
                        return answer;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
                Err(e) => return Err(t!("online.error.loopback", "why" => e)),
            }
        }
        Err(t!("online.error.timeout"))
    }
}

/// Reads one request from the browser and answers it with a short page. The code when it
/// carries `state`, the server's refusal when it says so, none for anything else.
fn answer(conn: TcpStream, server: &str, state: &str) -> Option<Result<String, String>> {
    let _ = conn.set_nonblocking(false);
    let _ = conn.set_read_timeout(Some(Duration::from_secs(5)));
    let mut line = String::new();
    BufReader::new(&conn).read_line(&mut line).ok()?;
    let target = line.split_whitespace().nth(1)?;
    let query = target.split_once('?').map_or("", |(_, q)| q);
    let param = |k: &str| query.split('&').find_map(|p| p.split_once('=').filter(|(n, _)| *n == k).map(|(_, v)| decode(v)));
    let result = if param("state").as_deref() != Some(state) {
        None
    } else if let Some(code) = param("code").filter(|c| !c.is_empty()) {
        Some(Ok(code))
    } else {
        Some(Err(t!("online.error.denied", "server" => server)))
    };
    let text = match &result {
        Some(Ok(_)) => t!("online.browser_done"),
        Some(Err(e)) => e.clone(),
        None => String::new(),
    };
    let body = format!("<!DOCTYPE html><meta charset=\"utf-8\"><title>Bragd</title><p style=\"font:1.2rem sans-serif;margin:2rem\">{}</p>", html(&text));
    let mut out = &conn;
    let _ = write!(out, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    result
}

fn html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The token `server` gives for `code`, proven by the verifier.
pub fn exchange(server: &'static crate::server::Server, o: &Oauth, agent: &ureq::Agent, base: &str, code: &str, pkce: &Pkce, redirect: &str) -> Result<String, String> {
    let url = format!("{}{}", base.trim_end_matches('/'), o.token);
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("code_verifier", pkce.verifier.as_str()),
        ("redirect_uri", redirect),
        ("client_id", o.client_id),
    ];
    let v = crate::board::read(server, agent.post(&url).send_form(form))?;
    v["access_token"].as_str().filter(|t| !t.is_empty()).map(str::to_string).ok_or_else(|| t!("online.error.reply", "server" => server.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_challenge_is_the_verifiers_sha256() {
        let p = Pkce::new();
        assert_eq!(p.verifier.len(), 43, "32 bytes in base64url");
        assert_ne!(p.verifier, Pkce::new().verifier, "each sign-in its own");
        assert_eq!(p.challenge, URL_SAFE_NO_PAD.encode(Sha256::digest(p.verifier.as_bytes())));
        // RFC 7636's example.
        let known = URL_SAFE_NO_PAD.encode(Sha256::digest(b"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"));
        assert_eq!(known, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn oauth_url_names_scopes_and_redirect_encoded() {
        let p = Pkce { verifier: "v".into(), challenge: "ch".into(), state: "st".into() };
        let o = Oauth { authorize: "/oauth", token: "/api/token", client_id: "bragd", scopes: "board:play challenge:read challenge:write" };
        let u = authorize_url(&o, "https://lichess.org/", "http://127.0.0.1:5555/", &p);
        assert!(u.starts_with("https://lichess.org/oauth?response_type=code&client_id=bragd&"), "{u}");
        assert!(u.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A5555%2F"), "{u}");
        assert!(u.contains("scope=board%3Aplay%20challenge%3Aread%20challenge%3Awrite"), "{u}");
        assert!(u.contains("code_challenge_method=S256&code_challenge=ch&state=st"), "{u}");
        assert_eq!(decode("a%3Ab+c%2"), "a:b c%2");
    }

    fn browser(redirect: &str, query: &str) -> String {
        use std::io::Read;
        let addr = redirect.trim_start_matches("http://").trim_end_matches('/');
        let mut c = TcpStream::connect(addr).unwrap();
        write!(c, "GET /?{query} HTTP/1.1\r\nHost: {addr}\r\n\r\n").unwrap();
        let mut s = String::new();
        c.read_to_string(&mut s).unwrap();
        s
    }

    #[test]
    fn oauth_loopback_takes_only_the_code_with_its_state() {
        let lb = Loopback::bind(0).unwrap();
        let r = lb.redirect.clone();
        let t = std::thread::spawn(move || {
            browser(&r, "code=wrong&state=other");
            browser(&r, "code=liu_abc%2B1&state=st")
        });
        let stop = AtomicBool::new(false);
        assert_eq!(lb.wait("Lichess", "st", &stop, Duration::from_secs(20)).unwrap(), "liu_abc+1");
        assert!(t.join().unwrap().contains(&t!("online.browser_done")));
    }

    #[test]
    fn oauth_loopback_reports_a_refusal_and_stops_when_asked() {
        let lb = Loopback::bind(0).unwrap();
        let r = lb.redirect.clone();
        let t = std::thread::spawn(move || browser(&r, "error=access_denied&state=st"));
        assert_eq!(lb.wait("Lichess", "st", &AtomicBool::new(false), Duration::from_secs(20)).unwrap_err(), t!("online.error.denied", "server" => "Lichess"));
        t.join().unwrap();
        assert_eq!(lb.wait("Lichess", "st", &AtomicBool::new(true), Duration::from_secs(20)).unwrap_err(), t!("online.error.cancelled"));
    }
}
