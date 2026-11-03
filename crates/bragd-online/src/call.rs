//! A Call during a Game on the Bragd server, with an opponent who is a Friend. Rust holds who may
//! call, each channel's consent ([`crate::talk`]) and the signalling ([`crate::realtime`]); the
//! webview's own WebRTC carries the media, directly between the two machines, and only while a
//! channel is open. Nothing of a Call is recorded.
//!
//! The page ticks the Call a few times a second ([`BragdServer::call_tick`]): it is set up when
//! the Game shown is against a Friend and under way, and ended otherwise.

use std::sync::Arc;
use std::time::{Duration, Instant};

use bragd_core::t;
use serde::Serialize;
use serde_json::{Value, json};

use crate::bragd_server::BragdServer;
use crate::realtime::{self, Channel};
use crate::talk::{self, Consent, Signal};

/// How long after a failed join the next is tried.
const RETRY: Duration = Duration::from_secs(15);

pub(crate) struct CallState {
    game: String,
    me: String,
    them: String,
    opponent: String,
    consent: Consent,
    channel: Option<Arc<Channel>>,
    tried: Option<Instant>,
    /// Offers, answers and ICE candidates from the other machine, for the webview.
    media: Vec<Value>,
    /// The ICE servers (`GET /v1/turn`), in WebRTC's shape.
    ice: Value,
    error: Option<String>,
}

/// The Call as the page draws it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallView {
    pub opponent: String,
    pub voice: talk::ChannelView,
    pub camera: talk::ChannelView,
    /// This side makes the WebRTC offer: the smaller id, so the two never both offer.
    pub offerer: bool,
    pub ice_servers: Value,
    /// The signalling is up.
    pub connected: bool,
    pub error: Option<String>,
}

fn channel_of(name: &str) -> Option<talk::Channel> {
    match name {
        "voice" => Some(talk::Channel::Voice),
        "camera" => Some(talk::Channel::Camera),
        _ => None,
    }
}

impl BragdServer {
    /// Sets the Call up for the Game shown when it is under way against a Friend, ends it
    /// otherwise, and takes what came from the other machine. `voice` and `camera` are the
    /// user's defaults for a request: *Ask me* or *Off*.
    pub fn call_tick(&self, voice: talk::Default, camera: talk::Default) {
        let game = self.shown();
        let me = self.lock().state["id"].as_str().map(str::to_string);
        let want = match (&game, me) {
            (Some(g), Some(me)) if g["status"] == "playing" && g["friend"] == true => {
                Some((g["id"].as_str().unwrap_or_default().to_string(), me, g["opponent_id"].as_str().unwrap_or_default().to_string(), g["opponent"].as_str().unwrap_or_default().to_string()))
            }
            _ => None,
        };
        let ended = {
            let mut i = self.lock();
            let same = matches!((&i.call, &want), (Some(c), Some((id, ..))) if &c.game == id);
            if same { None } else { i.call.take() }
        };
        if let Some(mut old) = ended {
            // The Game ended, or the opponent is no Friend any more: every channel closes.
            if let Some(ch) = &old.channel {
                for s in old.consent.unfriend() {
                    let _ = ch.send(serde_json::to_value(s).unwrap_or_default());
                }
            }
        }
        let Some((game, me, them, opponent)) = want else { return };
        {
            let mut i = self.lock();
            if i.call.is_none() {
                i.call = Some(CallState { game, me, them, opponent, consent: Consent::new(true, voice, camera), channel: None, tried: None, media: vec![], ice: Value::Null, error: None });
            }
        }
        self.join();
        self.pump();
    }

    /// Joins the pair's signalling topic, and fetches the ICE servers, when not done yet.
    fn join(&self) {
        let (me, them) = {
            let mut i = self.lock();
            let Some(c) = i.call.as_mut() else { return };
            if c.channel.is_some() || c.tried.is_some_and(|t| t.elapsed() < RETRY) {
                return;
            }
            c.tried = Some(Instant::now());
            (c.me.clone(), c.them.clone())
        };
        let joined = (|| {
            let p = self.public()?;
            let token = self.token()?;
            let ch = Channel::join(&p.supabase_url, &p.supabase_key, &token, &realtime::topic(&me, &them), &me)?;
            let ice = self.call("GET", "/v1/turn", &json!({}))?;
            Ok::<_, String>((ch, ice["ice_servers"].clone()))
        })();
        let mut i = self.lock();
        let Some(c) = i.call.as_mut() else { return };
        match joined {
            Ok((ch, ice)) => {
                c.channel = Some(Arc::new(ch));
                c.ice = ice;
                c.error = None;
            }
            Err(e) => c.error = Some(e),
        }
    }

    /// Takes the other machine's signals: consent goes to the consent, media to the webview
    /// while a channel is open.
    fn pump(&self) {
        let mut i = self.lock();
        let Some(c) = i.call.as_mut() else { return };
        let Some(ch) = c.channel.clone() else { return };
        if let Some(e) = ch.lost() {
            c.channel = None;
            c.error = Some(e);
            return;
        }
        for v in ch.take() {
            match serde_json::from_value::<Signal>(v.clone()) {
                Ok(s) => {
                    if let Some(answer) = c.consent.receive(s) {
                        let _ = ch.send(serde_json::to_value(answer).unwrap_or_default());
                    }
                }
                Err(_) if matches!(v["kind"].as_str(), Some("sdp" | "ice")) && c.consent.any_open() => c.media.push(v),
                Err(_) => {}
            }
        }
        if !c.consent.any_open() {
            c.media.clear();
        }
    }

    pub fn call_view(&self) -> Option<CallView> {
        let i = self.lock();
        let c = i.call.as_ref()?;
        Some(CallView {
            opponent: c.opponent.clone(),
            voice: c.consent.view(talk::Channel::Voice),
            camera: c.consent.view(talk::Channel::Camera),
            offerer: c.me < c.them,
            ice_servers: c.ice.clone(),
            connected: c.channel.is_some(),
            error: c.error.clone(),
        })
    }

    /// The user says yes to `channel` (`voice` or `camera`): asks, or allows. `false` closes it
    /// for both, and is also *Not now*.
    pub fn call_want(&self, channel: &str, on: bool) -> Result<(), String> {
        let ch = channel_of(channel).ok_or_else(|| t!("talk.error.signalling"))?;
        let mut i = self.lock();
        let c = i.call.as_mut().ok_or_else(|| t!("talk.error.signalling"))?;
        let link = c.channel.clone().ok_or_else(|| c.error.clone().unwrap_or_else(|| t!("talk.error.signalling")))?;
        let signal = if on { c.consent.want(ch).map_err(|_| t!("talk.error.signalling"))? } else { c.consent.close(ch) };
        link.send(serde_json::to_value(signal).unwrap_or_default())
    }

    /// An offer, answer or ICE candidate from the webview, for the other machine: only while a
    /// channel is open.
    pub fn call_signal(&self, signal: Value) -> Result<(), String> {
        if !matches!(signal["kind"].as_str(), Some("sdp" | "ice")) {
            return Err(t!("talk.error.signalling"));
        }
        let i = self.lock();
        let c = i.call.as_ref().filter(|c| c.consent.any_open()).ok_or_else(|| t!("talk.error.signalling"))?;
        let link = c.channel.clone().ok_or_else(|| t!("talk.error.signalling"))?;
        drop(i);
        link.send(signal)
    }

    /// The other machine's offers, answers and ICE candidates since the last call.
    pub fn call_take(&self) -> Vec<Value> {
        self.lock().call.as_mut().map(|c| std::mem::take(&mut c.media)).unwrap_or_default()
    }

    /// Whether the webview may open the microphone (`audio`) or the camera (`video`) now: only
    /// while that channel is open.
    pub fn call_permits(&self, audio: bool, video: bool) -> bool {
        let i = self.lock();
        i.call.as_ref().is_some_and(|c| (!audio || c.consent.open(talk::Channel::Voice)) && (!video || c.consent.open(talk::Channel::Camera)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Play;
    use crate::fake_bragd;

    fn wait(mut f: impl FnMut() -> bool) -> bool {
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until {
            if f() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    #[test]
    fn call_opens_a_channel_only_when_both_say_yes_and_carries_media_only_then() {
        bragd_model::key::mock();
        let (fake, _) = fake_bragd::bragd();
        let b = BragdServer::at(&fake.url);
        b.sign_in_email("t@example.org", fake_bragd::PASSWORD).unwrap();
        b.answer("g-anna", "accept").unwrap();
        b.call_tick(talk::Default::Ask, talk::Default::Ask);
        let v = b.call_view().unwrap();
        assert!(v.connected, "{:?}", v.error);
        assert!(!v.offerer, "Anna's id is the smaller");
        let anna = Channel::join(&fake.url, "k", fake_bragd::TOKEN, &realtime::topic("id-bragdtester", "id-anna"), "id-anna").unwrap();
        assert!(b.call_signal(json!({ "kind": "sdp", "sdp": "x" })).is_err(), "no media before a channel opens");
        assert!(!b.call_permits(true, false), "nor the microphone");
        anna.send(json!({ "kind": "want", "channel": "voice" })).unwrap();
        assert!(wait(|| {
            b.call_tick(talk::Default::Ask, talk::Default::Ask);
            b.call_view().unwrap().voice.asked
        }));
        b.call_want("voice", true).unwrap();
        assert!(wait(|| anna.take().contains(&json!({ "kind": "want", "channel": "voice" }))));
        assert!(b.call_view().unwrap().voice.open && !b.call_view().unwrap().camera.open);
        assert!(b.call_permits(true, false) && !b.call_permits(false, true), "the camera stays closed");
        b.call_signal(json!({ "kind": "sdp", "type": "offer", "sdp": "v=0" })).unwrap();
        assert!(wait(|| anna.take().iter().any(|s| s["kind"] == "sdp")));
        anna.send(json!({ "kind": "ice", "candidate": "c" })).unwrap();
        assert!(wait(|| {
            b.call_tick(talk::Default::Ask, talk::Default::Ask);
            !b.call_take().is_empty()
        }));
        anna.send(json!({ "kind": "close", "channel": "voice" })).unwrap();
        assert!(wait(|| {
            b.call_tick(talk::Default::Ask, talk::Default::Ask);
            !b.call_view().unwrap().voice.open
        }), "either closes it for both");
        assert!(b.call_signal(json!({ "kind": "ice", "candidate": "c" })).is_err());
    }

    #[test]
    fn call_is_only_with_a_friend_in_a_game_under_way() {
        bragd_model::key::mock();
        let (fake, world) = fake_bragd::bragd();
        let b = BragdServer::at(&fake.url);
        b.sign_in_email("t@example.org", fake_bragd::PASSWORD).unwrap();
        b.call_tick(talk::Default::Ask, talk::Default::Ask);
        assert!(b.call_view().is_none(), "no Game under way");
        world.lock().unwrap().games[0].friend = false;
        b.answer("g-anna", "accept").unwrap();
        b.call_tick(talk::Default::Ask, talk::Default::Ask);
        assert!(b.call_view().is_none(), "never with a stranger");
        world.lock().unwrap().games[0].friend = true;
        b.poll().unwrap();
        b.call_tick(talk::Default::Ask, talk::Default::Ask);
        assert!(b.call_view().is_some());
        b.resign().unwrap();
        b.call_tick(talk::Default::Ask, talk::Default::Ask);
        assert!(b.call_view().is_none(), "the Call ends with the Game");
    }
}
