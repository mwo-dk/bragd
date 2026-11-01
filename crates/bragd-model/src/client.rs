//! An OpenAI-compatible server: its model list (`GET <url>/models`) and one chat answer
//! (`POST <url>/chat/completions`). Ollama, Lemonade, LM Studio and the cloud services all speak
//! it. Local servers found running are offered in Settings (`find`).

use std::time::Duration;

use bragd_core::t;
use serde_json::{Value, json};

/// The local servers Bragd looks for, by name and base address.
pub const LOCAL: [(&str, &str); 4] = [
    ("Ollama", "http://localhost:11434/v1"),
    ("Lemonade", "http://localhost:13305/api/v1"),
    ("Lemonade", "http://localhost:8000/api/v1"),
    ("LM Studio", "http://localhost:1234/v1"),
];

/// A local server found running, with its models.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Found {
    pub name: &'static str,
    pub url: String,
    pub models: Vec<String>,
}

/// Whether `url` names this machine (`localhost`, `127.x.x.x`, `::1`): nothing sent there
/// leaves it.
pub fn on_this_machine(url: &str) -> bool {
    let Ok(uri) = url.trim().parse::<ureq::http::Uri>() else { return false };
    let host = uri.host().unwrap_or("").trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

fn agent(timeout: Duration) -> ureq::Agent {
    // The system's certificate store, as the update check: a proxy's own CA is trusted.
    let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
    ureq::Agent::config_builder().timeout_global(Some(timeout)).tls_config(tls).build().into()
}

fn endpoint(url: &str, path: &str) -> Result<String, String> {
    let url = url.trim().trim_end_matches('/');
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(t!("model.error.address", "url" => url));
    }
    // Tests and screenshots never reach past this machine.
    if bragd_core::offline() && !on_this_machine(url) {
        return Err(t!("model.error.offline"));
    }
    Ok(format!("{url}/{path}"))
}

fn failed(e: ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(401 | 403) => t!("model.error.key"),
        ureq::Error::StatusCode(code) => t!("model.error.status", "code" => code),
        e => t!("model.error.unreachable", "why" => e),
    }
}

fn get_models(url: &str, key: Option<&str>, timeout: Duration) -> Result<Vec<String>, String> {
    let mut req = agent(timeout).get(endpoint(url, "models")?);
    if let Some(k) = key.filter(|k| !k.is_empty()) {
        req = req.header("Authorization", format!("Bearer {k}"));
    }
    let v: Value = req.call().map_err(failed)?.body_mut().read_json().map_err(|_| t!("model.error.reply"))?;
    let data = v["data"].as_array().ok_or_else(|| t!("model.error.reply"))?;
    let mut ids: Vec<String> = data.iter().filter_map(|m| m["id"].as_str().map(str::to_string)).collect();
    ids.sort();
    Ok(ids)
}

/// The models the server at `url` offers, by id.
pub fn models(url: &str, key: Option<&str>) -> Result<Vec<String>, String> {
    get_models(url, key, Duration::from_secs(10))
}

/// The servers of `candidates` that answer at once, with their models.
pub fn find_in(candidates: &[(&'static str, &str)]) -> Vec<Found> {
    let mut out: Vec<Found> = vec![];
    for &(name, url) in candidates {
        // One entry per name: Lemonade's second port only when the first did not answer.
        if out.iter().any(|f| f.name == name) {
            continue;
        }
        if let Ok(models) = get_models(url, None, Duration::from_millis(700)) {
            out.push(Found { name, url: url.to_string(), models });
        }
    }
    out
}

/// The local servers running now (`LOCAL`).
pub fn find() -> Vec<Found> {
    find_in(&LOCAL)
}

/// The model's answer to `user`, told `system` first. A reasoning model's `<think>` block is
/// left out.
pub fn chat(url: &str, key: Option<&str>, model: &str, system: &str, user: &str) -> Result<String, String> {
    let body = json!({
        "model": model,
        "messages": [{ "role": "system", "content": system }, { "role": "user", "content": user }],
        "temperature": 0.2,
        "stream": false,
    });
    // A local model on a laptop can take a while over a long Comment.
    let mut req = agent(Duration::from_secs(180)).post(endpoint(url, "chat/completions")?);
    if let Some(k) = key.filter(|k| !k.is_empty()) {
        req = req.header("Authorization", format!("Bearer {k}"));
    }
    let v: Value = req.send_json(&body).map_err(failed)?.body_mut().read_json().map_err(|_| t!("model.error.reply"))?;
    let text = v["choices"][0]["message"]["content"].as_str().ok_or_else(|| t!("model.error.reply"))?;
    let text = match text.find("</think>") {
        Some(end) if text.trim_start().starts_with("<think>") => &text[end + "</think>".len()..],
        _ => text,
    };
    let text = text.trim();
    if text.is_empty() { Err(t!("model.error.reply")) } else { Ok(text.to_string()) }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// Each request a fake server saw: `METHOD path`, its Authorization and its body.
    pub(crate) type Seen = Arc<Mutex<Vec<(String, String, String)>>>;

    /// A fake server on this machine: it answers each request with the next of `replies`
    /// (status, body) and records each request as `METHOD path`, its Authorization and its body.
    pub(crate) fn fake(replies: Vec<(u16, String)>) -> (String, Seen) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/v1", l.local_addr().unwrap().port());
        let seen: Seen = Arc::new(Mutex::new(vec![]));
        let log = seen.clone();
        std::thread::spawn(move || {
            for (status, body) in replies {
                let Ok((s, _)) = l.accept() else { return };
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                r.read_line(&mut line).unwrap();
                let (mut auth, mut len) = (String::new(), 0usize);
                loop {
                    let mut h = String::new();
                    r.read_line(&mut h).unwrap();
                    if h.trim().is_empty() {
                        break;
                    }
                    let (k, v) = h.split_once(':').unwrap();
                    match k.to_ascii_lowercase().as_str() {
                        "authorization" => auth = v.trim().into(),
                        "content-length" => len = v.trim().parse().unwrap(),
                        _ => {}
                    }
                }
                let mut b = vec![0; len];
                r.read_exact(&mut b).unwrap();
                let req = line.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
                log.lock().unwrap().push((req, auth, String::from_utf8(b).unwrap()));
                let mut s = s;
                let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            }
        });
        (url, seen)
    }

    fn reply(content: &str) -> String {
        json!({ "choices": [{ "message": { "role": "assistant", "content": content } }] }).to_string()
    }

    #[test]
    fn client_lists_models_sorted_with_the_key() {
        let (url, seen) = fake(vec![(200, r#"{"object":"list","data":[{"id":"qwen3"},{"id":"gemma"}]}"#.into())]);
        assert_eq!(models(&url, Some("sk-1")).unwrap(), ["gemma", "qwen3"]);
        let seen = seen.lock().unwrap();
        assert_eq!(seen[0].0, "GET /v1/models");
        assert_eq!(seen[0].1, "Bearer sk-1");
    }

    #[test]
    fn client_chat_sends_the_model_and_both_messages_and_drops_a_think_block() {
        let (url, seen) = fake(vec![(200, reply("<think>hm</think>\n\nGood move.")), (200, reply("  Plain.  "))]);
        assert_eq!(chat(&url, None, "qwen3", "Translate", "Godt træk.").unwrap(), "Good move.");
        assert_eq!(chat(&url, None, "qwen3", "Translate", "x").unwrap(), "Plain.");
        let seen = seen.lock().unwrap();
        assert_eq!(seen[0].0, "POST /v1/chat/completions");
        assert_eq!(seen[0].1, "", "no key, no Authorization");
        let body: Value = serde_json::from_str(&seen[0].2).unwrap();
        assert_eq!(body["model"], "qwen3");
        assert_eq!(body["messages"][0]["content"], "Translate");
        assert_eq!(body["messages"][1]["content"], "Godt træk.");
    }

    #[test]
    fn client_names_a_refused_key_and_a_bad_reply() {
        let (url, _) = fake(vec![(401, "{}".into()), (200, "{}".into()), (500, "{}".into())]);
        assert_eq!(models(&url, Some("bad")).unwrap_err(), t!("model.error.key"));
        assert_eq!(chat(&url, None, "m", "s", "u").unwrap_err(), t!("model.error.reply"));
        assert_eq!(models(&url, None).unwrap_err(), t!("model.error.status", "code" => 500));
        assert_eq!(models("localhost:1/v1", None).unwrap_err(), t!("model.error.address", "url" => "localhost:1/v1"));
    }

    #[test]
    fn client_finds_the_servers_that_answer_once_per_name() {
        let (a, _) = fake(vec![(200, r#"{"data":[{"id":"x"}]}"#.into())]);
        let (b, _) = fake(vec![(200, r#"{"data":[{"id":"y"}]}"#.into())]);
        // A port nothing listens on: bound, then dropped.
        let dead = format!("http://127.0.0.1:{}/v1", TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port());
        let found = find_in(&[("Ollama", &dead), ("Lemonade", &a), ("Lemonade", &b)]);
        assert_eq!(found, [Found { name: "Lemonade", url: a.clone(), models: vec!["x".into()] }]);
    }

    #[test]
    fn client_knows_an_address_on_this_machine() {
        for u in ["http://localhost:11434/v1", "http://127.0.0.1:1234/v1", "http://[::1]:8000/api/v1", "HTTP://LOCALHOST/v1"] {
            assert!(on_this_machine(u), "{u}");
        }
        for u in ["https://api.openai.com/v1", "http://192.168.1.5:11434/v1", "http://evo.local:13305/api/v1", "nonsense", "http://localhost.example.com/v1"] {
            assert!(!on_this_machine(u), "{u}");
        }
    }
}
