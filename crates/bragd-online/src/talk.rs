//! A Call's consent, per channel: voice and camera each open only while both Players say yes,
//! and only between Friends. Either Player closes a channel for both. A request from the other
//! Player shows as "Anna would like voice: Allow / Not now", unless the user's default for that
//! channel is *Off*, when it is refused without asking. Pure: the signals go out and come in
//! through the Call's signalling (`realtime`).

use serde::{Deserialize, Serialize};

/// A Call's channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Voice,
    Camera,
}

impl Channel {
    pub const ALL: [Channel; 2] = [Channel::Voice, Channel::Camera];

    fn index(self) -> usize {
        self as usize
    }
}

/// What the user's Settings say to a request: *Ask me* or *Off*.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Default {
    #[default]
    Ask,
    Off,
}

/// What one Player tells the other about a channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "channel", rename_all = "snake_case")]
pub enum Signal {
    /// I say yes: a request, or the answer to one.
    Want(Channel),
    /// I close it, or say *Not now*: it is closed for both.
    Close(Channel),
}

/// One channel as the page shows it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ChannelView {
    /// Both said yes: the media flows, and the live indicator shows.
    pub open: bool,
    /// The other Player asks; the user has not answered.
    pub asked: bool,
    /// The user asked; the other has not answered.
    pub waiting: bool,
}

/// The consent of one Call, between the user and one other Player.
#[derive(Clone, Debug, Default)]
pub struct Consent {
    friend: bool,
    defaults: [Default; 2],
    mine: [bool; 2],
    theirs: [bool; 2],
}

impl Consent {
    /// A Call with a Friend (`friend`), or with a stranger, which never opens.
    pub fn new(friend: bool, voice: Default, camera: Default) -> Consent {
        Consent { friend, defaults: [voice, camera], ..Consent::default() }
    }

    /// The user says yes to `ch`: asks, or allows what the other asked. Refused with a
    /// stranger.
    pub fn want(&mut self, ch: Channel) -> Result<Signal, &'static str> {
        if !self.friend {
            return Err("stranger");
        }
        let idx = ch.index();
        self.mine[idx] = true;
        Ok(Signal::Want(ch))
    }

    /// The user closes `ch`, or answers the other's request with *Not now*.
    pub fn close(&mut self, ch: Channel) -> Signal {
        let idx = ch.index();
        self.mine[idx] = false;
        self.theirs[idx] = false;
        Signal::Close(ch)
    }

    /// The other Player's signal; the answer to send back, if any (a request refused at once,
    /// because they are no Friend or the user's default is *Off*).
    pub fn receive(&mut self, s: Signal) -> Option<Signal> {
        match s {
            Signal::Want(ch) => {
                let idx = ch.index();
                if !self.friend || (self.defaults[idx] == Default::Off && !self.mine[idx]) {
                    return Some(Signal::Close(ch));
                }
                self.theirs[idx] = true;
                None
            }
            Signal::Close(ch) => {
                let idx = ch.index();
                self.mine[idx] = false;
                self.theirs[idx] = false;
                None
            }
        }
    }

    /// The other Player is no longer a Friend (unfriended or blocked): everything closes.
    pub fn unfriend(&mut self) -> Vec<Signal> {
        self.friend = false;
        let mut signals = Vec::new();
        for ch in Channel::ALL {
            let idx = ch.index();
            if self.mine[idx] || self.theirs[idx] {
                signals.push(Signal::Close(ch));
                self.mine[idx] = false;
                self.theirs[idx] = false;
            }
        }
        signals
    }

    pub fn view(&self, ch: Channel) -> ChannelView {
        let idx = ch.index();
        ChannelView {
            open: self.friend && self.mine[idx] && self.theirs[idx],
            asked: self.theirs[idx] && !self.mine[idx],
            waiting: self.mine[idx] && !self.theirs[idx],
        }
    }

    /// Whether `ch` is open: media may flow.
    pub fn open(&self, ch: Channel) -> bool {
        let idx = ch.index();
        self.friend && self.mine[idx] && self.theirs[idx]
    }

    /// Whether any channel is open: the Call's connection may be made.
    pub fn any_open(&self) -> bool {
        Channel::ALL.iter().any(|c| self.open(*c))
    }
}

#[cfg(test)]
mod tests {
    use super::Channel::{Camera, Voice};
    use super::*;

    fn friends() -> (Consent, Consent) {
        (Consent::new(true, Default::Ask, Default::Ask), Consent::new(true, Default::Ask, Default::Ask))
    }

    #[test]
    fn talk_a_channel_opens_only_when_both_say_yes() {
        let (mut anna, mut bo) = friends();
        assert!(!anna.open(Voice), "off by default");
        let ask = anna.want(Voice).unwrap();
        assert_eq!(ask, Signal::Want(Voice));
        assert!(!anna.open(Voice));
        assert!(anna.view(Voice).waiting);
        assert_eq!(bo.receive(ask), None);
        assert!(bo.view(Voice).asked && !bo.open(Voice));
        let yes = bo.want(Voice).unwrap();
        assert!(bo.open(Voice) && !bo.view(Voice).asked);
        assert_eq!(anna.receive(yes), None);
        assert!(anna.open(Voice) && !anna.view(Voice).waiting);
        assert!(!anna.open(Camera) && !bo.open(Camera), "each channel separately");
        assert!(anna.any_open());
    }

    #[test]
    fn talk_either_closes_for_both() {
        let (mut anna, mut bo) = friends();
        bo.receive(anna.want(Camera).unwrap());
        anna.receive(bo.want(Camera).unwrap());
        assert!(anna.open(Camera) && bo.open(Camera));
        let close = bo.close(Camera);
        assert_eq!(close, Signal::Close(Camera));
        assert!(!bo.open(Camera));
        anna.receive(close);
        assert!(!anna.open(Camera) && !anna.view(Camera).waiting, "closed for both, not left asking");
        // Opening again takes both again.
        bo.receive(anna.want(Camera).unwrap());
        assert!(!anna.open(Camera) && bo.view(Camera).asked);
    }

    #[test]
    fn talk_not_now_withdraws_the_request() {
        let (mut anna, mut bo) = friends();
        bo.receive(anna.want(Voice).unwrap());
        anna.receive(bo.close(Voice));
        assert_eq!(anna.view(Voice), ChannelView::default());
        assert_eq!(bo.view(Voice), ChannelView::default());
    }

    #[test]
    fn talk_never_with_a_stranger() {
        let mut anna = Consent::new(false, Default::Ask, Default::Ask);
        assert!(anna.want(Voice).is_err());
        assert_eq!(anna.receive(Signal::Want(Camera)), Some(Signal::Close(Camera)));
        assert!(!anna.view(Camera).asked && !anna.any_open());
    }

    #[test]
    fn talk_a_default_of_off_refuses_without_asking() {
        let mut bo = Consent::new(true, Default::Off, Default::Ask);
        assert_eq!(bo.receive(Signal::Want(Voice)), Some(Signal::Close(Voice)));
        assert!(!bo.view(Voice).asked);
        assert_eq!(bo.receive(Signal::Want(Camera)), None);
        assert!(bo.view(Camera).asked);
        // Off is about requests: the user may still ask themselves.
        let (mut anna, _) = friends();
        anna.receive(bo.want(Voice).unwrap());
        assert!(anna.view(Voice).asked);
    }

    #[test]
    fn talk_unfriending_closes_everything() {
        let (mut anna, mut bo) = friends();
        bo.receive(anna.want(Voice).unwrap());
        anna.receive(bo.want(Voice).unwrap());
        anna.want(Camera).unwrap();
        let out = anna.unfriend();
        assert!(out.contains(&Signal::Close(Voice)) && out.contains(&Signal::Close(Camera)));
        assert!(!anna.any_open() && !anna.view(Camera).waiting);
        assert!(anna.want(Voice).is_err());
        assert_eq!(anna.receive(Signal::Want(Voice)), Some(Signal::Close(Voice)));
    }

    #[test]
    fn talk_signals_travel_as_json() {
        assert_eq!(serde_json::to_string(&Signal::Want(Voice)).unwrap(), r#"{"kind":"want","channel":"voice"}"#);
        assert_eq!(serde_json::from_str::<Signal>(r#"{"kind":"close","channel":"camera"}"#).unwrap(), Signal::Close(Camera));
    }
}
