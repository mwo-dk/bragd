//! Supabase Realtime's broadcast, for a Call's signalling: offers, answers, ICE candidates and
//! the consent signals of each channel, between two Friends. One private topic per pair,
//! `call:<a>:<b>` with the two players' ids in order, which the Bragd server's policies open to
//! those two only while they are Friends (`supabase/migrations/…_signalling.sql`). The protocol is
//! Phoenix's channels over a WebSocket: `phx_join`, `broadcast`, `heartbeat`. Media never passes
//! here.
//!
//! The same broadcast tells a player of their Games: `player:<id>`, which only that player may
//! join and on which only the database sends, after each change the referee makes
//! (`supabase/migrations/…_live_games.sql`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bragd_core::t;
use serde_json::{Value, json};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

type Ws = WebSocket<MaybeTlsStream<std::net::TcpStream>>;

/// How often Realtime is told the socket is alive; it closes one silent for a minute.
const HEARTBEAT: Duration = Duration::from_secs(25);
/// How long the join may take before it counts as refused.
const JOIN: Duration = Duration::from_secs(10);

/// The topic of the Call between `a` and `b`: the smaller id first, as the policy wants.
pub fn topic(a: &str, b: &str) -> String {
    if a < b { format!("call:{a}:{b}") } else { format!("call:{b}:{a}") }
}

/// A joined topic. Dropping it leaves.
pub struct Channel {
    out: Mutex<Sender<String>>,
    inbox: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    /// Set when the socket was lost, with why.
    lost: Arc<Mutex<Option<String>>>,
    topic: String,
    me: String,
}

impl Channel {
    /// Joins `topic` at the Supabase project `url` with its publishable `key`, as the user of
    /// `token`, whose id is `me`. Refused unless Realtime lets the user in.
    pub fn join(url: &str, key: &str, token: &str, topic: &str, me: &str) -> Result<Channel, String> {
        let ws_url = match url.strip_prefix("https://") {
            Some(rest) => format!("wss://{rest}/realtime/v1/websocket?apikey={key}&vsn=1.0.0"),
            None => format!("ws://{}/realtime/v1/websocket?apikey={key}&vsn=1.0.0", url.trim_start_matches("http://")),
        };
        let lost = |e: &dyn std::fmt::Display| t!("online.error.unreachable", "server" => "Bragd", "why" => e);
        let (mut ws, _) = tungstenite::connect(ws_url.as_str()).map_err(|e| lost(&e))?;
        let timeout = Some(Duration::from_millis(100));
        match ws.get_mut() {
            MaybeTlsStream::Plain(s) => s.set_read_timeout(timeout),
            MaybeTlsStream::Rustls(s) => s.get_mut().set_read_timeout(timeout),
            _ => Ok(()),
        }
        .map_err(|e| lost(&e))?;
        let full = format!("realtime:{topic}");
        let join = json!({ "topic": full, "event": "phx_join", "ref": "1", "join_ref": "1",
            "payload": { "config": { "broadcast": { "self": false, "ack": false }, "presence": { "key": "" }, "private": true }, "access_token": token } });
        ws.send(Message::text(join.to_string())).map_err(|e| lost(&e))?;
        let until = Instant::now() + JOIN;
        loop {
            if Instant::now() > until {
                return Err(t!("talk.error.signalling"));
            }
            match ws.read() {
                Ok(Message::Text(text)) => {
                    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                    if v["event"] == "phx_reply" && v["ref"] == "1" && v["topic"] == full.as_str() {
                        if v["payload"]["status"] == "ok" {
                            break;
                        }
                        return Err(t!("talk.error.signalling"));
                    }
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(e) => return Err(lost(&e)),
            }
        }
        let (tx, outbox) = channel();
        let ch = Channel {
            out: Mutex::new(tx),
            inbox: Arc::default(),
            stop: Arc::default(),
            lost: Arc::default(),
            topic: full,
            me: me.to_string(),
        };
        let (inbox, stop, gone, me, topic) = (ch.inbox.clone(), ch.stop.clone(), ch.lost.clone(), ch.me.clone(), ch.topic.clone());
        std::thread::spawn(move || listen(ws, &outbox, &inbox, &stop, &gone, &me, &topic));
        Ok(ch)
    }

    /// Sends `payload` to the other player, marked as the user's.
    pub fn send(&self, payload: Value) -> Result<(), String> {
        if let Some(e) = self.lost.lock().unwrap_or_else(|e| e.into_inner()).clone() {
            return Err(e);
        }
        let msg = json!({ "topic": self.topic, "event": "broadcast", "ref": null, "join_ref": "1",
            "payload": { "type": "broadcast", "event": "signal", "payload": { "from": self.me, "signal": payload } } });
        self.out.lock().unwrap_or_else(|e| e.into_inner()).send(msg.to_string()).map_err(|_| t!("talk.error.signalling"))
    }

    /// What the other player sent since the last call.
    pub fn take(&self) -> Vec<Value> {
        std::mem::take(&mut *self.inbox.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// Why the socket was lost, if it was.
    pub fn lost(&self) -> Option<String> {
        self.lost.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn listen(mut ws: Ws, outbox: &Receiver<String>, inbox: &Mutex<Vec<Value>>, stop: &AtomicBool, lost: &Mutex<Option<String>>, me: &str, topic: &str) {
    let mut beat = Instant::now();
    let mut n = 1u64;
    let why = 'run: loop {
        if stop.load(Ordering::Relaxed) {
            let _ = ws.close(None);
            return;
        }
        while let Ok(text) = outbox.try_recv() {
            if let Err(e) = ws.send(Message::text(text)) {
                break 'run e.to_string();
            }
        }
        if beat.elapsed() >= HEARTBEAT {
            beat = Instant::now();
            n += 1;
            let hb = json!({ "topic": "phoenix", "event": "heartbeat", "payload": {}, "ref": n.to_string() });
            if let Err(e) = ws.send(Message::text(hb.to_string())) {
                break e.to_string();
            }
        }
        match ws.read() {
            Ok(Message::Text(text)) => {
                let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                let p = &v["payload"]["payload"];
                // A broadcast from the other player; the user's own never comes back (`self: false`),
                // and one marked as the user's is ignored all the same.
                if v["event"] == "broadcast" && v["topic"] == topic && p["from"].is_string() && p["from"] != me {
                    inbox.lock().unwrap_or_else(|e| e.into_inner()).push(p["signal"].clone());
                }
            }
            Ok(Message::Close(_)) => break "closed".into(),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) => break e.to_string(),
        }
    };
    *lost.lock().unwrap_or_else(|e| e.into_inner()) = Some(t!("online.error.lost", "server" => "Bragd", "why" => why));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake_bragd;

    #[test]
    fn realtime_signals_reach_the_other_friend_only() {
        let (fake, _) = fake_bragd::bragd();
        let t = topic("id-bragdtester", "id-anna");
        assert_eq!(t, "call:id-anna:id-bragdtester");
        let me = Channel::join(&fake.url, "fake-publishable", fake_bragd::TOKEN, &t, "id-bragdtester").unwrap();
        let anna = Channel::join(&fake.url, "fake-publishable", fake_bragd::TOKEN, &t, "id-anna").unwrap();
        me.send(json!({ "kind": "want", "channel": "voice" })).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        let mut got = vec![];
        while got.is_empty() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(50));
            got = anna.take();
        }
        assert_eq!(got, [json!({ "kind": "want", "channel": "voice" })]);
        assert!(me.take().is_empty(), "the user's own signal does not come back");
        let seen = fake.seen().into_iter().find(|r| r.path == "/realtime/v1/websocket").unwrap();
        assert!(seen.query.contains("apikey=fake-publishable"));
    }

    #[test]
    fn realtime_refuses_a_topic_the_policy_closes() {
        let (fake, _) = fake_bragd::bragd();
        let stranger = topic("id-bragdtester", "id-eve");
        assert_eq!(Channel::join(&fake.url, "k", fake_bragd::TOKEN, &stranger, "id-bragdtester").err(), Some(t!("talk.error.signalling")));
        let t = topic("id-bragdtester", "id-anna");
        assert!(Channel::join(&fake.url, "k", "stolen", &t, "id-bragdtester").is_err(), "another's token");
    }
}
