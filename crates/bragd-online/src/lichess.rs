//! Lichess's HTTP API, as the Board API uses it: plain requests with the token, and NDJSON
//! streams read a line at a time on a thread of their own.

use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bragd_core::t;
use serde::Deserialize;
use serde_json::Value;

/// The real server. Tests and screenshots name a fake one on this machine instead.
pub const LICHESS: &str = "https://lichess.org";

/// Whether `url` names this machine: nothing sent there leaves it.
pub fn on_this_machine(url: &str) -> bool {
    let Ok(uri) = url.trim().parse::<ureq::http::Uri>() else { return false };
    let host = uri.host().unwrap_or("").trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// An agent that hands back every answer, so Lichess's own `error` text can be shown. Streams
/// have no overall time limit: they last as long as a seek or a Game.
pub fn agent() -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .tls_config(tls)
        .build()
        .into()
}

/// The JSON of an answer, or why there is none, in the user's language.
pub fn read(r: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Value, String> {
    let mut r = r.map_err(|e| t!("online.error.unreachable", "why" => e))?;
    let status = r.status().as_u16();
    let body = r.body_mut().read_to_string().unwrap_or_default();
    let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    match status {
        200..=299 => Ok(v),
        401 => Err(t!("online.error.signed_out")),
        // Lichess asks for a full minute's pause after this.
        429 => Err(t!("online.error.slow_down")),
        _ => Err(match v["error"].as_str().or(v["error_description"].as_str()) {
            Some(e) => t!("online.error.lichess", "error" => e),
            None => t!("online.error.status", "code" => status),
        }),
    }
}

/// The signed-in account.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Account {
    pub id: String,
    pub username: String,
}

/// A challenge: one the user made, or one made to them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Challenge {
    pub id: String,
    pub challenger: Option<crate::game::Player>,
    #[serde(rename = "destUser")]
    pub dest_user: Option<crate::game::Player>,
    pub variant: crate::game::VariantKey,
    pub rated: bool,
    pub speed: String,
    #[serde(rename = "timeControl")]
    pub time_control: TimeControl,
    /// `white`, `black` or `random`: the challenger's colour.
    pub color: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct TimeControl {
    /// `clock`, `correspondence` or `unlimited`.
    #[serde(rename = "type")]
    pub kind: String,
    /// `10+5`, as Lichess shows it.
    pub show: String,
}

/// Lichess with the user's token.
pub struct Client {
    base: String,
    token: String,
    agent: ureq::Agent,
}

impl Client {
    /// Lichess at `base` with `token`. Refused when Bragd is offline and `base` is not this
    /// machine.
    pub fn new(base: &str, token: &str) -> Result<Client, String> {
        if bragd_core::offline() && !on_this_machine(base) {
            return Err(t!("online.error.offline"));
        }
        Ok(Client { base: base.trim_end_matches('/').to_string(), token: token.to_string(), agent: agent() })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn bearer(&self) -> String {
        format!("Bearer {}", self.token)
    }

    pub fn get(&self, path: &str) -> Result<Value, String> {
        read(self.agent.get(self.url(path)).header("Authorization", self.bearer()).header("Accept", "application/json").call())
    }

    pub fn post(&self, path: &str, form: &[(&str, &str)]) -> Result<Value, String> {
        read(self.agent.post(self.url(path)).header("Authorization", self.bearer()).send_form(form.iter().copied()))
    }

    /// Takes the token back from Lichess, on signing out.
    pub fn revoke(&self) -> Result<Value, String> {
        read(self.agent.delete(self.url("/api/token")).header("Authorization", self.bearer()).call())
    }

    pub fn account(&self) -> Result<Account, String> {
        serde_json::from_value(self.get("/api/account")?).map_err(|_| t!("online.error.reply"))
    }

    /// The challenges made to the user, and those they made.
    pub fn challenges(&self) -> Result<(Vec<Challenge>, Vec<Challenge>), String> {
        let v = self.get("/api/challenge")?;
        let list = |k: &str| serde_json::from_value(v[k].clone()).unwrap_or_default();
        Ok((list("in"), list("out")))
    }

    /// Reads the NDJSON stream at `path` (GET, or POST with `form`) and hands each object to
    /// `line`, until it ends or `stop` is set. Lichess's empty keep-alive lines let `stop` be
    /// seen within seconds. Ok when the server ended it or `stop` was set.
    pub fn stream(&self, path: &str, form: Option<&[(&str, &str)]>, stop: &AtomicBool, mut line: impl FnMut(Value)) -> Result<(), String> {
        let r = match form {
            None => self.agent.get(self.url(path)).header("Authorization", self.bearer()).call(),
            Some(f) => self.agent.post(self.url(path)).header("Authorization", self.bearer()).send_form(f.iter().copied()),
        };
        let r = r.map_err(|e| t!("online.error.unreachable", "why" => e))?;
        if !r.status().is_success() {
            return read(Ok(r)).map(|_| ());
        }
        let mut reader = BufReader::new(r.into_body().into_reader());
        let mut buf = String::new();
        loop {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            buf.clear();
            match reader.read_line(&mut buf) {
                Ok(0) => return Ok(()),
                Ok(_) => {
                    if let Ok(v) = serde_json::from_str::<Value>(buf.trim()) {
                        if stop.load(Ordering::Relaxed) {
                            return Ok(());
                        }
                        line(v);
                    }
                }
                Err(e) => return Err(t!("online.error.lost", "why" => e)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{Fake, Reply};

    #[test]
    fn lichess_errors_say_what_lichess_said() {
        let fake = Fake::start(|r, _| match r.path.as_str() {
            "/api/account" => Reply::Json(401, r#"{"error":"No such token"}"#.into()),
            "/api/challenge/x" => Reply::Json(400, r#"{"error":"Bot accounts cannot be challenged"}"#.into()),
            "/api/slow" => Reply::Json(429, "{}".into()),
            _ => Reply::Json(404, "".into()),
        });
        let c = Client::new(&fake.url, "lip_t").unwrap();
        assert_eq!(c.account().unwrap_err(), t!("online.error.signed_out"));
        assert_eq!(c.post("/api/challenge/x", &[]).unwrap_err(), t!("online.error.lichess", "error" => "Bot accounts cannot be challenged"));
        assert_eq!(c.get("/api/slow").unwrap_err(), t!("online.error.slow_down"));
        assert_eq!(c.get("/nowhere").unwrap_err(), t!("online.error.status", "code" => 404));
        assert!(fake.seen().iter().all(|r| r.auth.as_deref() == Some("Bearer lip_t")));
    }

    #[test]
    fn lichess_stream_hands_over_each_line_and_skips_keep_alives() {
        let fake = Fake::start(|_, _| Reply::Stream);
        fake.push("/s", r#"{"type":"a"}"#);
        fake.push("/s", "");
        fake.push("/s", r#"{"type":"b"}"#);
        let f = fake.clone();
        std::thread::spawn(move || {
            while !f.streaming("/s") {
                std::thread::sleep(Duration::from_millis(10));
            }
            f.close("/s");
        });
        let mut got = vec![];
        Client::new(&fake.url, "t").unwrap().stream("/s", None, &AtomicBool::new(false), |v| got.push(v["type"].as_str().unwrap().to_string())).unwrap();
        assert_eq!(got, ["a", "b"]);
    }

    #[test]
    fn lichess_stream_stops_when_asked() {
        let fake = Fake::start(|_, _| Reply::Stream);
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let f = fake.clone();
        std::thread::spawn(move || {
            while !f.streaming("/s") {
                std::thread::sleep(Duration::from_millis(10));
            }
            s.store(true, Ordering::Relaxed);
        });
        // The fake never closes it: only `stop` ends this.
        Client::new(&fake.url, "t").unwrap().stream("/s", Some(&[("rated", "false")]), &stop, |_| {}).unwrap();
        assert_eq!(fake.seen()[0].body, "rated=false");
    }

    #[test]
    fn lichess_is_not_asked_while_offline_but_this_machine_is() {
        assert!(on_this_machine("http://127.0.0.1:9/") && on_this_machine("http://localhost") && !on_this_machine(LICHESS));
    }
}
