//! Playing on Lichess: the signed-in account, its event stream (Games starting, challenges), the
//! Game being played and its own stream, a seek waiting for an opponent, and the Game's chat.
//! Each stream reads on a thread of its own; the page asks for a [`View`] a few times a second.
//! No request is made while the lock is held.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bragd_core::t;
use bragd_games::State;
use serde::Serialize;
use serde_json::Value;

use crate::game::{self, GameFull, GameState, Live};
use crate::lichess::{self, Account, Challenge, Client};
use crate::oauth;

/// The token's place in the OS keychain: this service, the server's address as the account.
pub const SERVICE: &str = "bragd-online";
/// How long the browser has for the sign-in.
const SIGN_IN: Duration = Duration::from_secs(300);

/// The server Bragd plays on: Lichess, or `BRAGD_LICHESS_URL` for a fake one in tests and
/// screenshots.
pub fn base() -> String {
    std::env::var("BRAGD_LICHESS_URL").ok().filter(|u| !u.trim().is_empty()).unwrap_or_else(|| lichess::LICHESS.to_string())
}

/// The token kept for `base`. `BRAGD_LICHESS_TOKEN` stands in for the keychain only for a server
/// on this machine (the screenshots' fake, whose sandbox has no keychain).
fn token(base: &str) -> Option<String> {
    let fake = std::env::var("BRAGD_LICHESS_TOKEN").ok().filter(|_| lichess::on_this_machine(base));
    fake.or_else(|| bragd_model::key::get_in(SERVICE, base))
}

/// A line of a Game's chat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChatLine {
    pub user: String,
    pub text: String,
}

/// A challenge as the page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeView {
    pub id: String,
    /// Made to the user; otherwise made by them.
    pub incoming: bool,
    /// The other Player, with their rating.
    pub who: String,
    pub variant: Option<&'static str>,
    pub variant_name: String,
    pub rated: bool,
    /// `10+5`, or Lichess's own word for a correspondence Game.
    pub time: String,
}

/// The Game as the page draws it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    pub id: String,
    pub url: String,
    pub variant: Option<&'static str>,
    pub variant_name: String,
    pub rated: bool,
    pub against_person: bool,
    /// White's and Black's names, with ratings.
    pub names: [String; 2],
    /// The user's seat: 0 White, 1 Black.
    pub seat: Option<u8>,
    /// White's and Black's time left, in milliseconds.
    pub clocks: [u64; 2],
    pub running: Option<u8>,
    pub over: bool,
    /// How it ended, worded; empty while it is on.
    pub result: String,
    /// The opponent offers a draw, and asks for a takeback.
    pub offered: (bool, bool),
    /// The user offered a draw, and asked for a takeback.
    pub offering: (bool, bool),
    pub opponent_gone: bool,
    /// The board, the legal moves and the record, by Bragd's rules; none for a Variant Bragd has
    /// no rules for.
    pub state: Option<State>,
    pub initial: Option<String>,
    pub entries: Vec<String>,
}

/// Everything the page shows.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// The account's name, once signed in.
    pub signed_in: Option<String>,
    pub signing_in: bool,
    /// The event stream is open.
    pub connected: bool,
    pub seeking: bool,
    pub challenges: Vec<ChallengeView>,
    pub game: Option<GameView>,
    pub chat: Vec<ChatLine>,
    pub error: Option<String>,
}

#[derive(Default)]
struct Inner {
    client: Option<Arc<Client>>,
    me: Option<Account>,
    signing_in: Option<Arc<AtomicBool>>,
    events: Option<Arc<AtomicBool>>,
    seek: Option<Arc<AtomicBool>>,
    challenges: Vec<Challenge>,
    /// The Game's id and its stream's stop switch.
    game_id: Option<(String, Arc<AtomicBool>)>,
    game: Option<(Live, Instant)>,
    opponent_gone: bool,
    chat_on: bool,
    chat: Vec<ChatLine>,
    error: Option<String>,
    /// The kept token was tried: the page's view, asked a few times a second, tries it once,
    /// and then only Reconnect or a sign-in asks Lichess again.
    tried: bool,
}

impl Inner {
    fn stop_all(&mut self) {
        for s in [self.events.take(), self.seek.take(), self.game_id.take().map(|g| g.1)].into_iter().flatten() {
            s.store(true, Ordering::Relaxed);
        }
    }
}

/// Lichess for this window. Clones share it.
#[derive(Clone)]
pub struct Online {
    base: String,
    inner: Arc<Mutex<Inner>>,
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn named(p: &game::Player) -> String {
    let name = if let Some(level) = p.ai_level { t!("online.lichess_ai", "level" => level) } else if p.name.is_empty() { p.id.clone() } else { p.name.clone() };
    match (&p.title, p.rating) {
        (Some(title), Some(r)) => format!("{title} {name} ({r})"),
        (None, Some(r)) => format!("{name} ({r})"),
        (Some(title), None) => format!("{title} {name}"),
        (None, None) => name,
    }
}

/// How a Game over ended, worded.
fn result(g: &Live) -> String {
    if !g.over() {
        return String::new();
    }
    let why = match g.state.status.as_str() {
        s @ ("aborted" | "mate" | "resign" | "stalemate" | "timeout" | "draw" | "outoftime" | "noStart" | "variantEnd") => t!(&format!("online.status.{s}")),
        _ => t!("online.status.other"),
    };
    match g.state.winner.as_deref() {
        Some("white") => format!("{} · {why}", t!("play.wins", "side" => t!("side.white"))),
        Some("black") => format!("{} · {why}", t!("play.wins", "side" => t!("side.black"))),
        _ => why,
    }
}

impl Online {
    pub fn new(base: &str) -> Online {
        Online { base: base.to_string(), inner: Arc::default() }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn fail(&self, e: String) -> String {
        let mut i = self.lock();
        if e == t!("online.error.signed_out") {
            // The token is gone or was taken back on Lichess: a new sign-in is needed.
            let _ = bragd_model::key::set_in(SERVICE, &self.base, "");
            i.stop_all();
            i.client = None;
            i.me = None;
        }
        i.error = Some(e.clone());
        e
    }

    fn client(&self) -> Result<Arc<Client>, String> {
        self.lock().client.clone().ok_or_else(|| t!("online.error.not_signed_in"))
    }

    /// Everything the page shows, now.
    pub fn view(&self) -> View {
        let i = self.lock();
        let me = i.me.as_ref().map(|m| m.id.to_lowercase());
        let challenges = i
            .challenges
            .iter()
            .map(|c| {
                let incoming = c.challenger.as_ref().is_none_or(|p| Some(p.id.to_lowercase()) != me);
                let other = if incoming { c.challenger.as_ref() } else { c.dest_user.as_ref() };
                ChallengeView {
                    id: c.id.clone(),
                    incoming,
                    who: other.map(named).unwrap_or_else(|| t!("online.open_challenge")),
                    variant: game::variant(&c.variant.key),
                    variant_name: c.variant.name.clone(),
                    rated: c.rated,
                    time: if c.time_control.show.is_empty() { c.speed.clone() } else { c.time_control.show.clone() },
                }
            })
            .collect();
        let game = i.game.as_ref().map(|(g, since)| {
            let entries = g.entries();
            let state = g.variant.and_then(|v| bragd_games::replay(v, g.initial.as_deref(), &entries).ok());
            GameView {
                id: g.id.clone(),
                url: format!("{}/{}", self.base, g.id),
                variant: g.variant,
                variant_name: g.variant_name.clone(),
                rated: g.rated,
                against_person: g.against_person(),
                names: [named(&g.white), named(&g.black)],
                seat: g.seat,
                clocks: g.clocks(since.elapsed().as_millis() as u64),
                running: g.running(),
                over: g.over(),
                result: result(g),
                offered: g.offered(),
                offering: g.offering(),
                opponent_gone: i.opponent_gone,
                state,
                initial: g.initial.clone(),
                entries,
            }
        });
        View {
            signed_in: i.me.as_ref().map(|m| m.username.clone()),
            signing_in: i.signing_in.is_some(),
            connected: i.events.is_some(),
            seeking: i.seek.is_some(),
            challenges,
            game,
            chat: if i.chat_on { i.chat.clone() } else { vec![] },
            error: i.error.clone(),
        }
    }

    /// The Game being played is rated, against a person, and on: engine help is locked.
    pub fn live(&self) -> Option<(bool, bool, bool)> {
        self.lock().game.as_ref().map(|(g, _)| (g.rated, g.against_person(), g.over()))
    }

    /// Opens the account and its event stream when a token is kept and it is not open yet.
    pub fn open(&self) -> Result<(), String> {
        {
            let mut i = self.lock();
            if i.client.is_some() || i.tried {
                return Ok(());
            }
            i.tried = true;
        }
        let Some(token) = token(&self.base) else { return Ok(()) };
        self.connect(&token)
    }

    /// Reconnects after the connection was lost.
    pub fn reconnect(&self) -> Result<(), String> {
        {
            let mut i = self.lock();
            i.stop_all();
            i.client = None;
            i.error = None;
            i.tried = false;
        }
        self.open()
    }

    fn connect(&self, token: &str) -> Result<(), String> {
        let client = Arc::new(Client::new(&self.base, token).map_err(|e| self.fail(e))?);
        let me = client.account().map_err(|e| self.fail(e))?;
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut i = self.lock();
            i.client = Some(client.clone());
            i.me = Some(me);
            i.events = Some(stop.clone());
            i.error = None;
        }
        let this = self.clone();
        std::thread::spawn(move || {
            let r = client.stream("/api/stream/event", None, &stop, |v| this.event(&v));
            let mut i = this.lock();
            if !stop.load(Ordering::Relaxed) {
                i.events = None;
                i.error = Some(r.err().unwrap_or_else(|| t!("online.error.lost", "why" => "-")));
            }
        });
        self.refresh().map_err(|e| self.fail(e))
    }

    fn refresh(&self) -> Result<(), String> {
        let (incoming, outgoing) = self.client()?.challenges()?;
        self.lock().challenges = incoming.into_iter().chain(outgoing).collect();
        Ok(())
    }

    fn event(&self, v: &Value) {
        match v["type"].as_str() {
            Some("gameStart") => {
                let g = &v["game"];
                let id = g["gameId"].as_str().or(g["id"].as_str()).unwrap_or_default().to_string();
                if !id.is_empty() {
                    self.watch(id);
                }
            }
            Some("challenge") => {
                if let Ok(c) = serde_json::from_value::<Challenge>(v["challenge"].clone()) {
                    let mut i = self.lock();
                    i.challenges.retain(|x| x.id != c.id);
                    i.challenges.push(c);
                }
            }
            Some("challengeCanceled" | "challengeDeclined") => {
                let id = s(&v["challenge"]["id"]);
                self.lock().challenges.retain(|x| x.id != id);
            }
            _ => {}
        }
    }

    /// Follows the Game `id`'s stream, in place of any other.
    fn watch(&self, id: String) {
        let stop = Arc::new(AtomicBool::new(false));
        let client = {
            let mut i = self.lock();
            if i.game_id.as_ref().is_some_and(|(g, s)| *g == id && !s.load(Ordering::Relaxed)) {
                return;
            }
            if let Some((_, old)) = i.game_id.replace((id.clone(), stop.clone())) {
                old.store(true, Ordering::Relaxed);
            }
            // A seek is over once a Game starts.
            if let Some(s) = i.seek.take() {
                s.store(true, Ordering::Relaxed);
            }
            i.challenges.retain(|c| c.id != id);
            i.chat.clear();
            i.opponent_gone = false;
            let Some(c) = i.client.clone() else { return };
            c
        };
        let this = self.clone();
        std::thread::spawn(move || {
            let r = client.stream(&format!("/api/board/game/stream/{id}"), None, &stop, |v| this.game_line(&v));
            let mut i = this.lock();
            let over = i.game.as_ref().is_none_or(|(g, _)| g.over());
            if !stop.load(Ordering::Relaxed) {
                i.game_id = None;
                if !over {
                    i.error = Some(r.err().unwrap_or_else(|| t!("online.error.lost", "why" => "-")));
                }
            }
        });
    }

    fn game_line(&self, v: &Value) {
        let mut i = self.lock();
        match v["type"].as_str() {
            Some("gameFull") => {
                if let Ok(full) = serde_json::from_value::<GameFull>(v.clone()) {
                    let me = i.me.as_ref().map(|m| m.id.clone()).unwrap_or_default();
                    i.game = Some((Live::from_full(full, &me), Instant::now()));
                }
            }
            Some("gameState") => {
                if let (Ok(st), Some((g, since))) = (serde_json::from_value::<GameState>(v.clone()), i.game.as_mut()) {
                    g.update(st);
                    *since = Instant::now();
                }
            }
            Some("chatLine") if i.chat_on && v["room"].as_str() == Some("player") => {
                let line = ChatLine { user: s(&v["username"]), text: s(&v["text"]) };
                i.chat.push(line);
            }
            Some("opponentGone") => i.opponent_gone = v["gone"].as_bool().unwrap_or(false),
            _ => {}
        }
    }

    /// Signs in in the browser: `open` shows Lichess's page, which comes back to Bragd with a
    /// code that is swapped for a token. The token goes into the OS keychain only.
    pub fn sign_in(&self, open: impl FnOnce(&str) -> Result<(), String>) -> Result<(), String> {
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut i = self.lock();
            if i.signing_in.is_some() {
                return Ok(());
            }
            i.signing_in = Some(stop.clone());
            i.error = None;
        }
        let r = (|| {
            if bragd_core::offline() && !lichess::on_this_machine(&self.base) {
                return Err(t!("online.error.offline"));
            }
            let pkce = oauth::Pkce::new();
            let lb = oauth::Loopback::bind()?;
            open(&oauth::authorize_url(&self.base, &lb.redirect, &pkce))?;
            let code = lb.wait(&pkce.state, &stop, SIGN_IN)?;
            let token = oauth::exchange(&lichess::agent(), &self.base, &code, &pkce, &lb.redirect)?;
            bragd_model::key::set_in(SERVICE, &self.base, &token)?;
            Ok(token)
        })();
        self.lock().signing_in = None;
        match r {
            Ok(token) => self.connect(&token),
            Err(e) => Err(self.fail(e)),
        }
    }

    pub fn cancel_sign_in(&self) {
        if let Some(s) = self.lock().signing_in.take() {
            s.store(true, Ordering::Relaxed);
        }
    }

    /// Signs out: the token is taken back from Lichess and removed from the keychain.
    pub fn sign_out(&self) -> Result<(), String> {
        let client = {
            let mut i = self.lock();
            i.stop_all();
            let c = i.client.take();
            *i = Inner { chat_on: i.chat_on, ..Inner::default() };
            c
        };
        if let Some(c) = client {
            let _ = c.revoke();
        }
        bragd_model::key::set_in(SERVICE, &self.base, "")
    }

    /// Seeks a Game: `variant` (Bragd's key), `minutes` + `increment` seconds, rated or casual.
    /// The seek waits on its own thread until someone takes it or it is cancelled.
    pub fn seek(&self, variant: &str, minutes: u32, increment: u32, rated: bool) -> Result<(), String> {
        let v = game::lichess_variant(variant).ok_or_else(|| t!("online.error.variant"))?;
        let client = self.client()?;
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut i = self.lock();
            if let Some(old) = i.seek.replace(stop.clone()) {
                old.store(true, Ordering::Relaxed);
            }
            i.error = None;
        }
        let this = self.clone();
        std::thread::spawn(move || {
            let (time, inc) = (minutes.to_string(), increment.to_string());
            let form = [("rated", if rated { "true" } else { "false" }), ("time", &time), ("increment", &inc), ("variant", v)];
            let r = client.stream("/api/board/seek", Some(&form), &stop, |_| {});
            let mut i = this.lock();
            if i.seek.as_ref().is_some_and(|s| Arc::ptr_eq(s, &stop)) {
                i.seek = None;
            }
            if let Err(e) = r {
                i.error = Some(e);
            }
        });
        Ok(())
    }

    pub fn seek_cancel(&self) {
        if let Some(s) = self.lock().seek.take() {
            s.store(true, Ordering::Relaxed);
        }
    }

    /// Challenges `user` to a Game; `color` is `white`, `black` or `random`.
    pub fn challenge(&self, user: &str, variant: &str, minutes: u32, increment: u32, rated: bool, color: &str) -> Result<(), String> {
        let v = game::lichess_variant(variant).ok_or_else(|| t!("online.error.variant"))?;
        let user = user.trim();
        if user.is_empty() || !user.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(t!("online.error.user"));
        }
        let (limit, inc) = ((minutes * 60).to_string(), increment.to_string());
        let form = [("rated", if rated { "true" } else { "false" }), ("clock.limit", &limit), ("clock.increment", &inc), ("color", color), ("variant", v)];
        self.client()?.post(&format!("/api/challenge/{user}"), &form).map_err(|e| self.fail(e))?;
        self.refresh().map_err(|e| self.fail(e))
    }

    /// Accepts, declines or cancels the challenge `id`: `what` is `accept`, `decline` or
    /// `cancel`.
    pub fn answer(&self, id: &str, what: &str) -> Result<(), String> {
        if !matches!(what, "accept" | "decline" | "cancel") || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(t!("online.error.reply"));
        }
        self.client()?.post(&format!("/api/challenge/{id}/{what}"), &[]).map_err(|e| self.fail(e))?;
        self.lock().challenges.retain(|c| c.id != id);
        Ok(())
    }

    fn game_path(&self, rest: &str) -> Result<(Arc<Client>, String), String> {
        let i = self.lock();
        let (g, _) = i.game.as_ref().ok_or_else(|| t!("online.error.no_game"))?;
        let c = i.client.clone().ok_or_else(|| t!("online.error.not_signed_in"))?;
        Ok((c, format!("/api/board/game/{}/{rest}", g.id)))
    }

    /// Plays `entry`, once Bragd's rules have found it legal and it is the user's turn.
    pub fn play(&self, entry: &str) -> Result<(), String> {
        {
            let i = self.lock();
            let (g, _) = i.game.as_ref().ok_or_else(|| t!("online.error.no_game"))?;
            let variant = g.variant.ok_or_else(|| t!("online.error.variant"))?;
            let entries = g.entries();
            let mine = g.seat.is_some_and(|s| entries.len() % 2 == s as usize);
            if g.over() || !mine {
                return Err(t!("online.error.not_your_turn"));
            }
            bragd_games::apply(variant, g.initial.as_deref(), &entries, entry).map_err(|e| e.to_string())?;
        }
        let (c, path) = self.game_path(&format!("move/{entry}"))?;
        c.post(&path, &[]).map(|_| ()).map_err(|e| self.fail(e))
    }

    /// Offers or accepts a draw (`yes`), or declines one.
    pub fn draw(&self, yes: bool) -> Result<(), String> {
        let (c, path) = self.game_path(if yes { "draw/yes" } else { "draw/no" })?;
        c.post(&path, &[]).map(|_| ()).map_err(|e| self.fail(e))
    }

    /// Asks for or accepts a takeback (`yes`), or declines one.
    pub fn takeback(&self, yes: bool) -> Result<(), String> {
        let (c, path) = self.game_path(if yes { "takeback/yes" } else { "takeback/no" })?;
        c.post(&path, &[]).map(|_| ()).map_err(|e| self.fail(e))
    }

    pub fn resign(&self) -> Result<(), String> {
        let (c, path) = self.game_path("resign")?;
        c.post(&path, &[]).map(|_| ()).map_err(|e| self.fail(e))
    }

    pub fn abort(&self) -> Result<(), String> {
        let (c, path) = self.game_path("abort")?;
        c.post(&path, &[]).map(|_| ()).map_err(|e| self.fail(e))
    }

    /// Back to the lobby from a Game that is over.
    pub fn leave(&self) {
        let mut i = self.lock();
        if i.game.as_ref().is_some_and(|(g, _)| g.over()) {
            i.game = None;
            i.chat.clear();
            if let Some((_, s)) = i.game_id.take() {
                s.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Turns the chat on or off. Off, no line is kept or shown and none can be sent; on, the
    /// Game's chat so far is fetched. Nothing happens when it is already so.
    pub fn set_chat(&self, on: bool) -> Result<(), String> {
        {
            let mut i = self.lock();
            if i.chat_on == on {
                return Ok(());
            }
            i.chat_on = on;
            i.chat.clear();
            if !on || i.game.is_none() {
                return Ok(());
            }
        }
        let (c, path) = self.game_path("chat")?;
        let v = c.get(&path).map_err(|e| self.fail(e))?;
        let lines = v.as_array().map(|a| a.iter().map(|l| ChatLine { user: s(&l["user"]), text: s(&l["text"]) }).collect()).unwrap_or_default();
        self.lock().chat = lines;
        Ok(())
    }

    /// Says `text` in the Game's chat, to the opponent.
    pub fn say(&self, text: &str) -> Result<(), String> {
        let text = text.trim();
        if !self.lock().chat_on {
            return Err(t!("online.error.chat_off"));
        }
        if text.is_empty() {
            return Ok(());
        }
        // Lichess takes at most 140 characters a line.
        let text: String = text.chars().take(140).collect();
        let (c, path) = self.game_path("chat")?;
        c.post(&path, &[("room", "player"), ("text", &text)]).map(|_| ()).map_err(|e| self.fail(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake;

    /// Waits for `what` to hold; the limit only guards against a hang.
    fn until(online: &Online, what: impl Fn(&View) -> bool) -> View {
        for _ in 0..1000 {
            let v = online.view();
            if what(&v) {
                return v;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("never came: {:?}", online.view());
    }

    /// The browser: follows the sign-in page's redirect back to Bragd with a code.
    fn browser(url: &str) -> Result<(), String> {
        let q = url.split_once('?').unwrap().1;
        let p = |k: &str| q.split('&').find_map(|x| x.strip_prefix(&format!("{k}="))).map(oauth::decode).unwrap();
        let (redirect, state) = (p("redirect_uri"), p("state"));
        std::thread::spawn(move || {
            let addr = redirect.trim_start_matches("http://").trim_end_matches('/').to_string();
            let mut c = std::net::TcpStream::connect(&addr).unwrap();
            use std::io::{Read, Write};
            write!(c, "GET /?code=liu_code&state={state} HTTP/1.1\r\nHost: {addr}\r\n\r\n").unwrap();
            let _ = c.read_to_string(&mut String::new());
        });
        Ok(())
    }

    fn signed_in() -> (fake::Fake, Online) {
        bragd_model::key::mock();
        let f = fake::lichess();
        let online = Online::new(&f.url);
        online.sign_in(browser).unwrap();
        (f, online)
    }

    #[test]
    fn online_sign_in_keeps_the_token_in_the_keychain_only() {
        let (f, online) = signed_in();
        assert_eq!(bragd_model::key::get_in(SERVICE, &f.url).as_deref(), Some(fake::TOKEN));
        let v = until(&online, |v| v.connected);
        assert_eq!(v.signed_in.as_deref(), Some("BragdTester"));
        let token = f.seen().into_iter().find(|r| r.path == "/api/token").unwrap();
        assert!(token.body.contains("grant_type=authorization_code") && token.body.contains("code=liu_code"), "{}", token.body);
        assert!(token.body.contains("code_verifier=") && token.body.contains("redirect_uri=http"), "{}", token.body);
        assert_eq!(v.challenges.len(), 1);
        let c = &v.challenges[0];
        assert!(c.incoming && c.rated && c.variant == Some("chess"));
        assert_eq!((c.who.as_str(), c.time.as_str()), ("Anna (1712)", "10+5"));
        // A second window's Online finds the token kept.
        let other = Online::new(&f.url);
        other.open().unwrap();
        assert_eq!(other.view().signed_in.as_deref(), Some("BragdTester"));
        online.sign_out().unwrap();
        assert_eq!(bragd_model::key::get_in(SERVICE, &f.url), None);
        assert!(f.seen().iter().any(|r| r.method == "DELETE" && r.path == "/api/token"), "the token is taken back");
        assert_eq!(online.view().signed_in, None);
    }

    #[test]
    fn online_kept_token_that_fails_is_tried_once() {
        bragd_model::key::mock();
        let f = fake::lichess();
        bragd_model::key::set_in(SERVICE, &f.url, "lip_wrong").unwrap();
        let online = Online::new(&f.url);
        online.open().unwrap_err();
        for _ in 0..4 {
            online.open().unwrap();
        }
        assert_eq!(f.seen().len(), 1, "the page's polling does not hammer Lichess");
        assert_eq!(online.view().error, Some(t!("online.error.signed_out")));
        assert_eq!(bragd_model::key::get_in(SERVICE, &f.url), None, "a token Lichess refuses is dropped");
    }

    #[test]
    fn online_without_a_token_asks_nothing() {
        bragd_model::key::mock();
        let f = fake::lichess();
        let online = Online::new(&f.url);
        online.open().unwrap();
        assert_eq!(online.view(), View::default());
        assert!(f.seen().is_empty());
    }

    #[test]
    fn online_rated_game_locks_until_it_ends_and_moves_are_checked_first() {
        let (f, online) = signed_in();
        assert_eq!(online.live(), None);
        online.answer("c1anna", "accept").unwrap();
        let v = until(&online, |v| v.game.is_some());
        let g = v.game.unwrap();
        assert!(g.rated && g.against_person && !g.over);
        assert_eq!((g.seat, g.names[1].as_str()), (Some(0), "Anna (1712)"));
        assert!(v.challenges.is_empty(), "the accepted challenge has left the list");
        assert_eq!(online.live(), Some((true, true, false)), "locked");
        let before = f.seen().len();
        assert!(online.play("e2e5").is_err(), "refused by Bragd's rules");
        assert_eq!(f.seen().len(), before, "nothing sent for an illegal move");
        online.play("e2e4").unwrap();
        let g = until(&online, |v| v.game.as_ref().is_some_and(|g| g.entries.len() == 2)).game.unwrap();
        assert_eq!(g.entries, ["e2e4", "e7e5"]);
        assert_eq!(g.state.unwrap().history, ["e4", "e5"]);
        assert_eq!(g.running, Some(0));
        online.resign().unwrap();
        let g = until(&online, |v| v.game.as_ref().is_some_and(|g| g.over)).game.unwrap();
        assert_eq!(g.result, format!("{} · {}", t!("play.wins", "side" => t!("side.black")), t!("online.status.resign")));
        assert_eq!(online.live(), Some((true, true, true)), "over: unlocked");
        assert_eq!(online.play("g1f3").unwrap_err(), t!("online.error.not_your_turn"));
        online.leave();
        assert!(online.view().game.is_none());
    }

    #[test]
    fn online_seek_starts_a_game_and_only_known_variants_are_sought() {
        let (f, online) = signed_in();
        until(&online, |v| v.connected);
        assert_eq!(online.seek("shogi", 10, 5, false).unwrap_err(), t!("online.error.variant"));
        online.seek("chess", 10, 5, false).unwrap();
        let v = until(&online, |v| v.game.is_some() && !v.seeking);
        assert!(!v.game.unwrap().rated);
        let seek = f.seen().into_iter().find(|r| r.path == "/api/board/seek").unwrap();
        assert_eq!(seek.body, "rated=false&time=10&increment=5&variant=standard");
        assert_eq!(online.live(), Some((false, true, false)), "casual: not locked");
    }

    #[test]
    fn online_chat_is_off_until_turned_on() {
        let (f, online) = signed_in();
        online.answer("c1anna", "accept").unwrap();
        until(&online, |v| v.game.is_some());
        assert_eq!(online.say("hi").unwrap_err(), t!("online.error.chat_off"));
        assert!(!f.seen().iter().any(|r| r.path.ends_with("/chat")), "nothing asked of the chat while it is off");
        online.set_chat(true).unwrap();
        assert_eq!(online.view().chat.len(), 2, "the Game's chat so far");
        online.say("well played").unwrap();
        let v = until(&online, |v| v.chat.len() == 4);
        assert_eq!(v.chat[2], ChatLine { user: "BragdTester".into(), text: "well played".into() });
        online.set_chat(false).unwrap();
        assert!(online.view().chat.is_empty());
        online.say("x").unwrap_err();
    }

    #[test]
    fn online_challenge_names_only_a_lichess_user() {
        let (f, online) = signed_in();
        assert_eq!(online.challenge("anna/../x", "chess", 10, 5, false, "random").unwrap_err(), t!("online.error.user"));
        online.challenge("anna", "chess960", 15, 10, true, "white").unwrap();
        let r = f.seen().into_iter().find(|r| r.path == "/api/challenge/anna").unwrap();
        assert_eq!(r.body, "rated=true&clock.limit=900&clock.increment=10&color=white&variant=chess960");
    }
}
