//! Playing on a Board API server (Lichess, Lishogi): the signed-in account, its event stream
//! (Games starting, challenges), the Game being played and its own stream, a seek waiting for an
//! opponent, and the Game's chat. Each stream reads on a thread of its own; the page asks for a
//! [`View`] a few times a second. No request is made while the lock is held.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bragd_core::t;
use bragd_games::State;
use serde::Serialize;
use serde_json::Value;

use crate::Play;
use crate::board::{self, Account, Challenge, Client, Dialect, Time};
use crate::game::{self, GameFull, GameState, Live};
use crate::oauth;

/// The token's place in the OS keychain: this service, the server's address as the account.
pub const SERVICE: &str = "bragd-online";
/// How long the browser has for the sign-in.
const SIGN_IN: Duration = Duration::from_secs(300);

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
    /// `10+5`, or the server's own word for a correspondence Game.
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
    /// The first and the second seat's names, with ratings.
    pub names: [String; 2],
    /// The user's seat: 0 White, 1 Black.
    pub seat: Option<u8>,
    /// The first and the second seat's time left, in milliseconds.
    pub clocks: [u64; 2],
    /// Seconds of byoyomi a move once the main time is spent; 0 for none.
    pub byoyomi: u64,
    /// The first and the second seat's byoyomi periods left, when more than one is played
    /// (OGS); 0 otherwise.
    pub periods: [u64; 2],
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
    /// While a go Game's score is counted: the stones the server marks dead.
    pub dead: Option<Vec<String>>,
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
    /// and then only Reconnect or a sign-in asks the server again.
    tried: bool,
}

impl Inner {
    fn stop_all(&mut self) {
        for s in [self.events.take(), self.seek.take(), self.game_id.take().map(|g| g.1)].into_iter().flatten() {
            s.store(true, Ordering::Relaxed);
        }
    }
}

/// A Board API server for this window. Clones share it.
#[derive(Clone)]
pub struct Online {
    dialect: &'static Dialect,
    base: String,
    inner: Arc<Mutex<Inner>>,
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn named(server: &str, p: &game::Player) -> String {
    let name = if let Some(level) = p.ai_level { t!("online.server_ai", "server" => server, "level" => level) } else if p.name.is_empty() { p.id.clone() } else { p.name.clone() };
    match (&p.title, p.rating) {
        (Some(title), Some(r)) => format!("{title} {name} ({r})"),
        (None, Some(r)) => format!("{name} ({r})"),
        (Some(title), None) => format!("{title} {name}"),
        (None, None) => name,
    }
}

/// How a Game over ended, worded, its seats named by `colours`.
fn result(g: &Live, colours: [&str; 2]) -> String {
    if !g.over() {
        return String::new();
    }
    let why = match g.state.status.as_str() {
        s @ ("aborted" | "mate" | "resign" | "stalemate" | "timeout" | "draw" | "outoftime" | "noStart" | "variantEnd" | "impasse27" | "perpetualCheck" | "repetition" | "tryRule") => {
            t!(&format!("online.status.{s}"))
        }
        _ => t!("online.status.other"),
    };
    let side = |c: &str| t!("play.wins", "side" => t!(&format!("side.{c}")));
    match g.state.winner.as_deref() {
        Some("white") => format!("{} · {why}", side(colours[0])),
        Some("black") => format!("{} · {why}", side(colours[1])),
        _ => why,
    }
}

impl Online {
    /// The server of `dialect` at `base`: its real address, or a fake one in tests.
    pub fn new(dialect: &'static Dialect, base: &str) -> Online {
        Online { dialect, base: base.trim_end_matches('/').to_string(), inner: Arc::default() }
    }

    /// The server of `dialect` at the address Bragd plays at ([`crate::server::Server::base`]).
    pub fn of(dialect: &'static Dialect) -> Online {
        Online::new(dialect, &dialect.server.base())
    }

    fn name(&self) -> &'static str {
        self.dialect.server.name
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn fail(&self, e: String) -> String {
        let mut i = self.lock();
        if e == t!("online.error.signed_out", "server" => self.name()) {
            // The token is gone or was taken back on the server: a new sign-in is needed.
            let _ = bragd_model::key::set_in(SERVICE, &self.base, "");
            i.stop_all();
            i.client = None;
            i.me = None;
        }
        i.error = Some(e.clone());
        e
    }

    fn client(&self) -> Result<Arc<Client>, String> {
        self.lock().client.clone().ok_or_else(|| t!("online.error.not_signed_in", "server" => self.name()))
    }

    fn time(&self, time: &str) -> Result<Time, String> {
        Time::parse(time).filter(|_| self.dialect.times.contains(&time)).ok_or_else(|| t!("online.error.time", "server" => self.name()))
    }

    fn variant(&self, variant: &str) -> Result<&'static str, String> {
        game::server_variant(self.dialect.variants, variant).ok_or_else(|| t!("online.error.variant", "site" => self.dialect.server.site))
    }


    fn connect(&self, token: &str) -> Result<(), String> {
        let client = Arc::new(Client::new(self.dialect.server, &self.base, token).map_err(|e| self.fail(e))?);
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
            let r = client.stream("/api/stream/event", None, &stop, |mut v| {
                (this.dialect.normalise)(&mut v);
                this.event(&v)
            });
            let mut i = this.lock();
            if !stop.load(Ordering::Relaxed) {
                i.events = None;
                i.error = Some(r.err().unwrap_or_else(|| t!("online.error.lost", "server" => this.name(), "why" => "-")));
            }
        });
        self.refresh().map_err(|e| self.fail(e))
    }

    fn refresh(&self) -> Result<(), String> {
        if !self.dialect.challenge_list {
            // The event stream brings the challenges as it opens.
            return Ok(());
        }
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
            let r = client.stream(&format!("/api/board/game/stream/{id}"), None, &stop, |mut v| {
                (this.dialect.normalise)(&mut v);
                this.game_line(&v)
            });
            let mut i = this.lock();
            let over = i.game.as_ref().is_none_or(|(g, _)| g.over());
            if !stop.load(Ordering::Relaxed) {
                i.game_id = None;
                if !over {
                    i.error = Some(r.err().unwrap_or_else(|| t!("online.error.lost", "server" => this.name(), "why" => "-")));
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
                    i.game = Some((Live::from_full(full, &me, self.dialect.variants), Instant::now()));
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

    fn game_path(&self, rest: &str) -> Result<(Arc<Client>, String), String> {
        let i = self.lock();
        let (g, _) = i.game.as_ref().ok_or_else(|| t!("online.error.no_game"))?;
        let c = i.client.clone().ok_or_else(|| t!("online.error.not_signed_in", "server" => self.name()))?;
        Ok((c, format!("/api/board/game/{}/{rest}", g.id)))
    }
}

impl Play for Online {
    fn server(&self) -> &'static crate::server::Server {
        self.dialect.server
    }

    fn variants(&self) -> Vec<&'static str> {
        game::bragd_variants(self.dialect.variants)
    }

    fn times(&self) -> Vec<(String, String)> {
        self.dialect.times.iter().filter_map(|t| Some((t.to_string(), Time::parse(t)?.label()))).collect()
    }

    fn colours(&self) -> [&'static str; 2] {
        self.dialect.colours
    }

    fn can(&self) -> crate::Can {
        crate::Can { takeback: self.dialect.takeback, cancel: self.dialect.challenge_list, abort: true, draw: true }
    }

    fn view(&self) -> View {
        let server = self.name();
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
                    who: other.map(|p| named(server, p)).unwrap_or_else(|| t!("online.open_challenge")),
                    variant: game::variant(self.dialect.variants, &c.variant.key),
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
                names: [named(server, &g.white), named(server, &g.black)],
                seat: g.seat,
                clocks: g.clocks(since.elapsed().as_millis() as u64),
                byoyomi: g.state.byo / 1000,
                periods: [0, 0],
                running: g.running(),
                over: g.over(),
                result: result(g, self.dialect.colours),
                offered: g.offered(),
                offering: g.offering(),
                opponent_gone: i.opponent_gone,
                state,
                initial: g.initial.clone(),
                entries,
                dead: None,
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

    fn live(&self) -> Option<(bool, bool, bool)> {
        self.lock().game.as_ref().map(|(g, _)| (g.rated, g.against_person(), g.over()))
    }

    fn open(&self) -> Result<(), String> {
        {
            let mut i = self.lock();
            if i.client.is_some() || i.tried {
                return Ok(());
            }
            i.tried = true;
        }
        let Some(token) = self.dialect.server.token(&self.base) else { return Ok(()) };
        self.connect(&token)
    }

    fn reconnect(&self) -> Result<(), String> {
        {
            let mut i = self.lock();
            i.stop_all();
            i.client = None;
            i.error = None;
            i.tried = false;
        }
        self.open()
    }

    fn sign_in(&self, open: &dyn Fn(&str) -> Result<(), String>) -> Result<(), String> {
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
            if bragd_core::offline() && !board::on_this_machine(&self.base) {
                return Err(t!("online.error.offline", "server" => self.name()));
            }
            let o = &self.dialect.oauth;
            let pkce = oauth::Pkce::new();
            let lb = oauth::Loopback::bind(0)?;
            open(&oauth::authorize_url(o, &self.base, &lb.redirect, &pkce))?;
            let code = lb.wait(self.name(), &pkce.state, &stop, SIGN_IN)?;
            let token = oauth::exchange(self.dialect.server, o, &board::agent(), &self.base, &code, &pkce, &lb.redirect)?;
            bragd_model::key::set_in(SERVICE, &self.base, &token)?;
            Ok(token)
        })();
        self.lock().signing_in = None;
        match r {
            Ok(token) => self.connect(&token),
            Err(e) => Err(self.fail(e)),
        }
    }

    fn cancel_sign_in(&self) {
        if let Some(s) = self.lock().signing_in.take() {
            s.store(true, Ordering::Relaxed);
        }
    }

    fn sign_out(&self) -> Result<(), String> {
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

    fn seek(&self, variant: &str, time: &str, rated: bool) -> Result<(), String> {
        let v = self.variant(variant)?;
        let time = self.time(time)?;
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
            let mut form = vec![("rated", if rated { "true" } else { "false" }.to_string())];
            form.extend(time.seek_form());
            form.push(("variant", v.to_string()));
            let form: Vec<(&str, &str)> = form.iter().map(|(k, v)| (*k, v.as_str())).collect();
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

    fn seek_cancel(&self) {
        if let Some(s) = self.lock().seek.take() {
            s.store(true, Ordering::Relaxed);
        }
    }

    fn challenge(&self, user: &str, variant: &str, time: &str, rated: bool, color: &str) -> Result<(), String> {
        let v = self.variant(variant)?;
        let time = self.time(time)?;
        let user = user.trim();
        if user.is_empty() || !user.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(t!("online.error.user", "server" => self.name()));
        }
        if color != "random" && !self.dialect.colours.contains(&color) {
            return Err(t!("online.error.reply", "server" => self.name()));
        }
        let mut form = vec![("rated", if rated { "true" } else { "false" }.to_string())];
        form.extend(time.challenge_form());
        form.extend([("color", color.to_string()), ("variant", v.to_string())]);
        let form: Vec<(&str, &str)> = form.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.client()?.post(&format!("/api/challenge/{user}"), &form).map_err(|e| self.fail(e))?;
        self.refresh().map_err(|e| self.fail(e))
    }

    fn answer(&self, id: &str, what: &str) -> Result<(), String> {
        let cancel = what == "cancel" && self.dialect.challenge_list;
        if !(matches!(what, "accept" | "decline") || cancel) || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(t!("online.error.reply", "server" => self.name()));
        }
        self.client()?.post(&format!("/api/challenge/{id}/{what}"), &[]).map_err(|e| self.fail(e))?;
        self.lock().challenges.retain(|c| c.id != id);
        Ok(())
    }

    fn play(&self, entry: &str) -> Result<(), String> {
        {
            let i = self.lock();
            let (g, _) = i.game.as_ref().ok_or_else(|| t!("online.error.no_game"))?;
            let variant = g.variant.ok_or_else(|| t!("online.error.variant", "site" => self.dialect.server.site))?;
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

    fn draw(&self, yes: bool) -> Result<(), String> {
        let (c, path) = self.game_path(if yes { "draw/yes" } else { "draw/no" })?;
        c.post(&path, &[]).map(|_| ()).map_err(|e| self.fail(e))
    }

    fn takeback(&self, yes: bool) -> Result<(), String> {
        if !self.dialect.takeback {
            return Err(t!("online.error.no_takeback", "server" => self.name()));
        }
        let (c, path) = self.game_path(if yes { "takeback/yes" } else { "takeback/no" })?;
        c.post(&path, &[]).map(|_| ()).map_err(|e| self.fail(e))
    }

    fn resign(&self) -> Result<(), String> {
        let (c, path) = self.game_path("resign")?;
        c.post(&path, &[]).map(|_| ()).map_err(|e| self.fail(e))
    }

    fn abort(&self) -> Result<(), String> {
        let (c, path) = self.game_path("abort")?;
        c.post(&path, &[]).map(|_| ()).map_err(|e| self.fail(e))
    }

    fn leave(&self) {
        let mut i = self.lock();
        if i.game.as_ref().is_some_and(|(g, _)| g.over()) {
            i.game = None;
            i.chat.clear();
            if let Some((_, s)) = i.game_id.take() {
                s.store(true, Ordering::Relaxed);
            }
        }
    }

    fn set_chat(&self, on: bool) -> Result<(), String> {
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

    fn say(&self, text: &str) -> Result<(), String> {
        let text = text.trim();
        if !self.lock().chat_on {
            return Err(t!("online.error.chat_off"));
        }
        if text.is_empty() {
            return Ok(());
        }
        // Lichess and Lishogi take at most 140 characters a line.
        let text: String = text.chars().take(140).collect();
        let (c, path) = self.game_path("chat")?;
        c.post(&path, &[("room", "player"), ("text", &text)]).map(|_| ()).map_err(|e| self.fail(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake;
    use crate::board::LICHESS;

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
        let online = Online::new(&LICHESS, &f.url);
        online.sign_in(&browser).unwrap();
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
        let other = Online::new(&LICHESS, &f.url);
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
        let online = Online::new(&LICHESS, &f.url);
        online.open().unwrap_err();
        for _ in 0..4 {
            online.open().unwrap();
        }
        assert_eq!(f.seen().len(), 1, "the page's polling does not hammer Lichess");
        assert_eq!(online.view().error, Some(t!("online.error.signed_out", "server" => "Lichess")));
        assert_eq!(bragd_model::key::get_in(SERVICE, &f.url), None, "a token Lichess refuses is dropped");
    }

    #[test]
    fn online_without_a_token_asks_nothing() {
        bragd_model::key::mock();
        let f = fake::lichess();
        let online = Online::new(&LICHESS, &f.url);
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
        assert_eq!(online.seek("shogi", "10+5", false).unwrap_err(), t!("online.error.variant", "site" => "lichess.org"));
        assert_eq!(online.seek("chess", "3+0", false).unwrap_err(), t!("online.error.time", "server" => "Lichess"), "only the times offered");
        online.seek("chess", "10+5", false).unwrap();
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
        assert_eq!(online.challenge("anna/../x", "chess", "10+5", false, "random").unwrap_err(), t!("online.error.user", "server" => "Lichess"));
        online.challenge("anna", "chess960", "15+10", true, "white").unwrap();
        let r = f.seen().into_iter().find(|r| r.path == "/api/challenge/anna").unwrap();
        assert_eq!(r.body, "rated=true&clock.limit=900&clock.increment=10&color=white&variant=chess960");
    }

    fn signed_in_lishogi() -> (fake::Fake, Online) {
        bragd_model::key::mock();
        let f = fake::lishogi();
        let online = Online::new(&crate::board::LISHOGI, &f.url);
        online.sign_in(&browser).unwrap();
        (f, online)
    }

    #[test]
    fn lishogi_challenge_comes_on_the_stream_and_the_game_is_shogi() {
        let (f, online) = signed_in_lishogi();
        let v = until(&online, |v| !v.challenges.is_empty());
        assert!(!f.seen().iter().any(|r| r.path == "/api/challenge"), "Lishogi has no list to ask for");
        let c = &v.challenges[0];
        assert_eq!((c.variant, c.who.as_str()), (Some("shogi"), "Anna (1712)"));
        assert_eq!(online.answer("c1anna", "cancel").unwrap_err(), t!("online.error.reply", "server" => "Lishogi"), "Lishogi cancels only on its website");
        online.answer("c1anna", "accept").unwrap();
        let g = until(&online, |v| v.game.is_some()).game.unwrap();
        assert_eq!((g.seat, g.byoyomi, g.names[1].as_str()), (Some(0), 30, "Anna (1712)"), "the user plays sente");
        assert_eq!(online.live(), Some((true, true, false)), "locked");
        assert!(online.play("7g7e").is_err(), "refused by Bragd's shogi rules");
        online.play("7g7f").unwrap();
        let g = until(&online, |v| v.game.as_ref().is_some_and(|g| g.entries.len() == 2)).game.unwrap();
        assert_eq!(g.entries, ["7g7f", "3c3d"]);
        assert_eq!(g.state.unwrap().variant, bragd_games::Variant::Shogi);
        assert_eq!(online.takeback(true).unwrap_err(), t!("online.error.no_takeback", "server" => "Lishogi"));
        online.resign().unwrap();
        let g = until(&online, |v| v.game.as_ref().is_some_and(|g| g.over)).game.unwrap();
        assert_eq!(g.result, format!("{} · {}", t!("play.wins", "side" => t!("side.gote")), t!("online.status.resign")));
    }

    #[test]
    fn lishogi_seek_sends_byoyomi_and_challenge_names_sente() {
        let (f, online) = signed_in_lishogi();
        until(&online, |v| v.connected);
        assert_eq!(online.seek("shogi", "10+5", true).unwrap_err(), t!("online.error.time", "server" => "Lishogi"));
        online.seek("shogi", "10|30", true).unwrap();
        until(&online, |v| v.game.is_some() && !v.seeking);
        let seek = f.seen().into_iter().find(|r| r.path == "/api/board/seek").unwrap();
        assert_eq!(seek.body, "rated=true&time=10&increment=0&byoyomi=30&periods=1&variant=standard");
        assert_eq!(online.challenge("anna", "shogi", "15|30", false, "white").unwrap_err(), t!("online.error.reply", "server" => "Lishogi"), "Lishogi's colours are sente and gote");
        online.challenge("anna", "shogi", "15|30", false, "gote").unwrap();
        let r = f.seen().into_iter().find(|r| r.path == "/api/challenge/anna").unwrap();
        assert_eq!(r.body, "rated=false&clock.limit=900&clock.increment=0&clock.byoyomi=30&clock.periods=1&color=gote&variant=standard");
    }
}
