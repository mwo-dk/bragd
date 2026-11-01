//! The Board API's HTTP side, as Lichess and Lishogi (a fork of Lichess) serve it: plain
//! requests with the token, and NDJSON streams read a line at a time on a thread of their own.

use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bragd_core::t;
use serde::Deserialize;
use serde_json::Value;

use crate::game::{self, Variants};
use crate::oauth::Oauth;
use crate::server::{self, Server};

/// How a Board API server differs from Lichess, whose API it forked.
pub struct Dialect {
    pub server: &'static Server,
    pub oauth: Oauth,
    pub variants: Variants,
    /// The seats' colours in its API, the first to move first: `white`, `black`.
    pub colours: [&'static str; 2],
    /// The times offered for a real-time Game, as [`Time`] writes them.
    pub times: &'static [&'static str],
    /// Takebacks can be asked for.
    pub takeback: bool,
    /// The challenges can be listed (`GET /api/challenge`) and one's own cancelled. Lishogi sends
    /// them on the event stream only, and cancels only on its website.
    pub challenge_list: bool,
    /// Turns a line of its streams into Lichess's words.
    pub normalise: fn(&mut Value),
}

/// The Board API asks for these: playing Games, and reading and making challenges.
const SCOPES: &str = "board:play challenge:read challenge:write";

pub const LICHESS: Dialect = Dialect {
    server: &server::LICHESS,
    // Lichess needs no registration: any name is Bragd's.
    oauth: Oauth { authorize: "/oauth", token: "/api/token", client_id: "bragd", scopes: SCOPES },
    variants: game::LICHESS,
    colours: ["white", "black"],
    // Rapid and classical: what the Board API plays in real time.
    times: &["10+0", "10+5", "15+10", "30+0", "30+20"],
    takeback: true,
    challenge_list: true,
    normalise: |_| {},
};

pub const LISHOGI: Dialect = Dialect {
    server: &server::LISHOGI,
    oauth: Oauth { authorize: "/oauth", token: "/api/token", client_id: "bragd", scopes: SCOPES },
    variants: game::LISHOGI,
    colours: ["sente", "gote"],
    // Lishogi's Board API takes a Game of at least 15 minutes' estimated length when rated
    // (main time + 25 × byoyomi), and a rated Game has one byoyomi period and no increment.
    times: &["10|30", "15|30", "30|30", "30|60"],
    takeback: false,
    challenge_list: false,
    normalise: game::from_lishogi,
};

/// A time control: `10+5` is 10 minutes and 5 seconds more a move, `10|30` 10 minutes and then
/// 30 seconds' byoyomi a move, `10|5x30` the same with five periods of byoyomi.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Time {
    pub minutes: u32,
    pub increment: u32,
    pub byoyomi: u32,
    pub periods: u32,
}

impl Time {
    pub fn parse(s: &str) -> Option<Time> {
        let num = |x: &str| x.trim().parse::<u32>().ok().filter(|n| *n > 0);
        if let Some((m, b)) = s.split_once('|') {
            let (periods, b) = match b.split_once('x') {
                Some((p, b)) => (num(p)?, b),
                None => (1, b),
            };
            return Some(Time { minutes: num(m)?, byoyomi: num(b)?, periods, increment: 0 });
        }
        let (m, i) = s.split_once('+')?;
        Some(Time { minutes: num(m)?, increment: i.trim().parse().ok()?, byoyomi: 0, periods: 0 })
    }

    /// `10 min + 5 s`, `10 min, then 30 s byoyomi`, `10 min, then 5 × 30 s byoyomi`.
    pub fn label(&self) -> String {
        if self.periods > 1 {
            t!("online.minutes_periods", "m" => self.minutes, "n" => self.periods, "s" => self.byoyomi)
        } else if self.byoyomi > 0 {
            t!("online.minutes_byoyomi", "m" => self.minutes, "s" => self.byoyomi)
        } else {
            t!("online.minutes_plus", "m" => self.minutes, "s" => self.increment)
        }
    }

    /// The fields of a seek: minutes, and the increment or the byoyomi with one period.
    pub fn seek_form(&self) -> Vec<(&'static str, String)> {
        let mut f = vec![("time", self.minutes.to_string()), ("increment", self.increment.to_string())];
        if self.byoyomi > 0 {
            f.extend([("byoyomi", self.byoyomi.to_string()), ("periods", self.periods.to_string())]);
        }
        f
    }

    /// The same for a challenge, in seconds.
    pub fn challenge_form(&self) -> Vec<(&'static str, String)> {
        let mut f = vec![("clock.limit", (self.minutes * 60).to_string()), ("clock.increment", self.increment.to_string())];
        if self.byoyomi > 0 {
            f.extend([("clock.byoyomi", self.byoyomi.to_string()), ("clock.periods", self.periods.to_string())]);
        }
        f
    }
}

/// Whether `url` names this machine: nothing sent there leaves it.
pub fn on_this_machine(url: &str) -> bool {
    let Ok(uri) = url.trim().parse::<ureq::http::Uri>() else { return false };
    let host = uri.host().unwrap_or("").trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// An agent that hands back every answer, so the server's own `error` text can be shown. Streams
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

/// The JSON of `server`'s answer, or why there is none, in the user's language.
pub fn read(server: &Server, r: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Value, String> {
    let name = server.name;
    let mut r = r.map_err(|e| t!("online.error.unreachable", "server" => name, "why" => e))?;
    let status = r.status().as_u16();
    let body = r.body_mut().read_to_string().unwrap_or_default();
    let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    match status {
        200..=299 => Ok(v),
        401 => Err(t!("online.error.signed_out", "server" => name)),
        // Lichess asks for a full minute's pause after this.
        429 => Err(t!("online.error.slow_down", "server" => name)),
        _ => Err(match v["error"].as_str().or(v["error_description"].as_str()).or(v["detail"].as_str()) {
            Some(e) => t!("online.error.server", "server" => name, "error" => e),
            None => t!("online.error.status", "server" => name, "code" => status),
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

/// A Board API server with the user's token.
pub struct Client {
    pub server: &'static Server,
    base: String,
    token: String,
    agent: ureq::Agent,
}

impl Client {
    /// `server` at `base` with `token`. Refused when Bragd is offline and `base` is not this
    /// machine.
    pub fn new(server: &'static Server, base: &str, token: &str) -> Result<Client, String> {
        if bragd_core::offline() && !on_this_machine(base) {
            return Err(t!("online.error.offline", "server" => server.name));
        }
        Ok(Client { server, base: base.trim_end_matches('/').to_string(), token: token.to_string(), agent: agent() })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn bearer(&self) -> String {
        format!("Bearer {}", self.token)
    }

    pub fn get(&self, path: &str) -> Result<Value, String> {
        read(self.server, self.agent.get(self.url(path)).header("Authorization", self.bearer()).header("Accept", "application/json").call())
    }

    pub fn post(&self, path: &str, form: &[(&str, &str)]) -> Result<Value, String> {
        read(self.server, self.agent.post(self.url(path)).header("Authorization", self.bearer()).send_form(form.iter().copied()))
    }

    /// Takes the token back from the server, on signing out.
    pub fn revoke(&self) -> Result<Value, String> {
        read(self.server, self.agent.delete(self.url("/api/token")).header("Authorization", self.bearer()).call())
    }

    pub fn account(&self) -> Result<Account, String> {
        serde_json::from_value(self.get("/api/account")?).map_err(|_| t!("online.error.reply", "server" => self.server.name))
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
        let r = r.map_err(|e| t!("online.error.unreachable", "server" => self.server.name, "why" => e))?;
        if !r.status().is_success() {
            return read(self.server, Ok(r)).map(|_| ());
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
                Err(e) => return Err(t!("online.error.lost", "server" => self.server.name, "why" => e)),
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
        let c = Client::new(&server::LICHESS, &fake.url, "lip_t").unwrap();
        assert_eq!(c.account().unwrap_err(), t!("online.error.signed_out", "server" => "Lichess"));
        assert_eq!(c.post("/api/challenge/x", &[]).unwrap_err(), t!("online.error.server", "server" => "Lichess", "error" => "Bot accounts cannot be challenged"));
        assert_eq!(c.get("/api/slow").unwrap_err(), t!("online.error.slow_down", "server" => "Lichess"));
        assert_eq!(c.get("/nowhere").unwrap_err(), t!("online.error.status", "server" => "Lichess", "code" => 404));
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
        Client::new(&server::LICHESS, &fake.url, "t").unwrap().stream("/s", None, &AtomicBool::new(false), |v| got.push(v["type"].as_str().unwrap().to_string())).unwrap();
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
        Client::new(&server::LICHESS, &fake.url, "t").unwrap().stream("/s", Some(&[("rated", "false")]), &stop, |_| {}).unwrap();
        assert_eq!(fake.seen()[0].body, "rated=false");
    }

    #[test]
    fn board_times_parse_both_ways_of_writing() {
        assert_eq!(Time::parse("10+5"), Some(Time { minutes: 10, increment: 5, byoyomi: 0, periods: 0 }));
        assert_eq!(Time::parse("10+0").map(|t| t.increment), Some(0));
        assert_eq!(Time::parse("10|30"), Some(Time { minutes: 10, increment: 0, byoyomi: 30, periods: 1 }));
        assert_eq!(Time::parse("10|5x30"), Some(Time { minutes: 10, increment: 0, byoyomi: 30, periods: 5 }));
        assert_eq!(Time::parse("10|0x30"), None);
        assert_eq!(Time::parse("10"), None);
        assert_eq!(Time::parse("x+5"), None);
        for d in [&LICHESS, &LISHOGI] {
            assert!(d.times.iter().all(|t| Time::parse(t).is_some()), "{}", d.server.id);
        }
        // Lishogi: a rated Game needs 900 s by its estimate.
        assert!(LISHOGI.times.iter().map(|t| Time::parse(t).unwrap()).all(|t| t.minutes * 60 + 25 * t.byoyomi >= 900 && t.increment == 0));
        let f = Time::parse("10|30").unwrap().challenge_form();
        assert_eq!(f, [("clock.limit", "600".to_string()), ("clock.increment", "0".into()), ("clock.byoyomi", "30".into()), ("clock.periods", "1".into())]);
    }

    #[test]
    fn lichess_is_not_asked_while_offline_but_this_machine_is() {
        assert!(on_this_machine("http://127.0.0.1:9/") && on_this_machine("http://localhost") && !on_this_machine(server::LICHESS.url));
    }
}
