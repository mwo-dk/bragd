//! Playing on the Bragd server, between Bragd players: the account (Supabase Auth, by email or
//! a sign-in provider), Friends, Clubs, and Games refereed by the game service with Bragd's own
//! rules (`crates/bragd-server`). The server's address is Settings → Online → *Bragd server
//! address*; until one is set, nothing here talks to anyone. The refresh token is kept in the OS
//! keychain only. A thread asks the game service how things stand every two seconds while the
//! account is open, and the page reads the [`View`] and the [`FriendsView`] from that.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bragd_core::t;
use serde::Serialize;
use serde_json::{Value, json};

use crate::server::Server;
use crate::session::SERVICE;
use crate::{Can, ChallengeView, GameView, Play, View, board, oauth};

/// The Families a Game on the Bragd server may be played in: those without dice, until the
/// referee throws for both players.
pub const FAMILIES: &[&str] = &["chess", "xiangqi", "shogi", "go", "draughts", "tafl"];

pub const SERVER: Server = Server {
    id: "bragd",
    name: "Bragd",
    site: "",
    url: "",
    url_var: "BRAGD_FRIENDS_URL",
    token_var: "BRAGD_FRIENDS_TOKEN",
    terms: "",
    families: FAMILIES,
};

/// How often the game service is asked how things stand.
const POLL: Duration = Duration::from_secs(2);
/// How long the browser has for a sign-in.
const SIGN_IN: Duration = Duration::from_secs(300);

/// The address set in Settings, or the one `BRAGD_FRIENDS_URL` names; empty when none is set.
pub fn address() -> String {
    let set = std::env::var(SERVER.url_var).ok().filter(|u| !u.trim().is_empty());
    set.or_else(|| bragd_core::config::Config::load().ok().map(|c| c.online.server)).unwrap_or_default().trim().trim_end_matches('/').to_string()
}

/// What the game service says before signing in (`/v1/config`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Public {
    pub(crate) supabase_url: String,
    pub(crate) supabase_key: String,
    oauth: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct Session {
    access: String,
    refresh: String,
    /// When the access token runs out.
    until: Option<Instant>,
}

#[derive(Default)]
pub(crate) struct Inner {
    base: String,
    public: Option<Public>,
    session: Option<Session>,
    /// The last `/v1/state`.
    pub(crate) state: Value,
    /// The Game the page shows.
    game: Option<String>,
    poll: Option<Arc<AtomicBool>>,
    signing_in: Option<Arc<AtomicBool>>,
    error: Option<String>,
    /// A one-line message after an action: an invite's code, a sign-up to confirm.
    notice: Option<String>,
    tried: bool,
    /// The Call with the opponent of the Game shown, when they are a Friend.
    pub(crate) call: Option<crate::call::CallState>,
}

/// The Bragd server for this window. Clones share it.
#[derive(Clone, Default)]
pub struct BragdServer {
    /// An address of its own, for tests; none: Settings' ([`address`]).
    fixed: Option<String>,
    inner: Arc<Mutex<Inner>>,
}

/// A Friend as the page lists them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Friend {
    pub name: String,
    /// A Friend because you share a Club, not by a request.
    pub club: bool,
}

/// A Club on the Bragd server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Club {
    pub id: String,
    pub name: String,
    /// The code that lets others join; only its maker sees it.
    pub code: String,
    pub members: u64,
}

/// The account, Friends and Clubs, as the Play with Friends page shows them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendsView {
    /// An address is set in Settings.
    pub address: String,
    /// The sign-in providers the server offers: its id, such as `github`, and its name.
    pub providers: Vec<(String, String)>,
    pub signed_in: bool,
    pub signing_in: bool,
    /// The public name; none until one is chosen.
    pub name: Option<String>,
    /// What Delete my account asks to be typed: the name, or without one a word.
    pub delete_word: String,
    /// The label asking for it.
    pub delete_prompt: String,
    pub friends: Vec<Friend>,
    /// Who asks to be the user's Friend.
    pub asked: Vec<String>,
    /// Whom the user asked.
    pub asking: Vec<String>,
    pub blocked: Vec<String>,
    pub clubs: Vec<Club>,
    pub club_friends: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
}

/// A Chat line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Line {
    pub mine: bool,
    pub text: String,
}

/// The Chat of the Game shown: it opens only while both players say yes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatView {
    pub opponent: String,
    /// The opponent is a Friend: their lines are never filtered.
    pub friend: bool,
    pub open: bool,
    /// The opponent asks; the user has not answered.
    pub asked: bool,
    /// The user asked; the opponent has not answered.
    pub waiting: bool,
    pub keep_mine: bool,
    pub keep_theirs: bool,
    pub lines: Vec<Line>,
    /// The Game is under way: a Chat can be asked for, and lines said.
    pub live: bool,
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

/// A Game's opponent: their name, or a deleted player who left the Game to the user.
fn opponent(g: &Value) -> String {
    g["opponent"].as_str().map_or_else(|| t!("friends.deleted_player"), str::to_string)
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().map(s).collect()).unwrap_or_default()
}

/// How a sign-in provider writes its name.
fn provider_name(id: &str) -> String {
    match id {
        "github" => "GitHub".into(),
        "gitlab" => "GitLab".into(),
        "linkedin_oidc" => "LinkedIn".into(),
        _ => id.chars().take(1).flat_map(char::to_uppercase).chain(id.chars().skip(1)).collect(),
    }
}

/// The text for the server's error `code`, in the user's language.
fn error_text(code: &str) -> String {
    let key = format!("friends.error.{code}");
    let text = t!(&key);
    if text == key { t!("online.error.server", "server" => SERVER.name, "error" => code) } else { text }
}

/// The JSON of an answer, or why there is none.
fn read(r: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Value, String> {
    let mut r = r.map_err(|e| t!("online.error.unreachable", "server" => SERVER.name, "why" => e))?;
    let status = r.status().as_u16();
    let v: Value = serde_json::from_str(&r.body_mut().read_to_string().unwrap_or_default()).unwrap_or(Value::Null);
    match status {
        200..=299 => Ok(v),
        401 if v["error"] == "signed_out" || v["error"].is_null() && v["error_code"].is_null() => Err(t!("online.error.signed_out", "server" => SERVER.name)),
        _ => {
            // The game service's `error`, or Supabase Auth's `error_code`.
            let code = v["error_code"].as_str().or(v["error"].as_str()).unwrap_or_default();
            Err(if code.is_empty() { t!("online.error.status", "server" => SERVER.name, "code" => status) } else { error_text(code) })
        }
    }
}

impl BragdServer {
    pub fn new() -> BragdServer {
        BragdServer::default()
    }

    /// The Bragd server at `base`, whatever Settings say: for tests.
    pub fn at(base: &str) -> BragdServer {
        BragdServer { fixed: Some(base.trim_end_matches('/').to_string()), ..BragdServer::default() }
    }

    fn address(&self) -> String {
        self.fixed.clone().unwrap_or_else(address)
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The address now, and the account and everything with it forgotten when it changed.
    fn base(&self) -> Result<String, String> {
        let now = self.address();
        let mut i = self.lock();
        if i.base != now {
            if let Some(p) = i.poll.take() {
                p.store(true, Ordering::Relaxed);
            }
            *i = Inner { base: now.clone(), ..Inner::default() };
        }
        if now.is_empty() {
            Err(t!("friends.error.no_address"))
        } else if bragd_core::offline() && !board::on_this_machine(&now) {
            Err(t!("online.error.offline", "server" => SERVER.name))
        } else {
            Ok(now)
        }
    }

    fn fail(&self, e: String) -> String {
        let mut i = self.lock();
        if e == t!("online.error.signed_out", "server" => SERVER.name) {
            let _ = bragd_model::key::set_in(SERVICE, &i.base, "");
            if let Some(p) = i.poll.take() {
                p.store(true, Ordering::Relaxed);
            }
            i.session = None;
            i.state = Value::Null;
        }
        i.error = Some(e.clone());
        e
    }

    pub(crate) fn public(&self) -> Result<Public, String> {
        let base = self.base()?;
        if let Some(p) = self.lock().public.clone() {
            return Ok(p);
        }
        let v = read(board::agent().get(&format!("{base}/v1/config")).call())?;
        let p = Public { supabase_url: s(&v["supabase_url"]).trim_end_matches('/').to_string(), supabase_key: s(&v["supabase_key"]), oauth: strings(&v["oauth"]) };
        self.lock().public = Some(p.clone());
        Ok(p)
    }

    /// Supabase Auth's `path` with `body`.
    fn auth(&self, path: &str, body: &Value) -> Result<Value, String> {
        let p = self.public()?;
        read(board::agent().post(&format!("{}/auth/v1/{path}", p.supabase_url)).header("apikey", &p.supabase_key).send_json(body))
    }

    /// Keeps the tokens of Supabase Auth's answer: the refresh token in the keychain.
    fn keep(&self, v: &Value) -> Result<(), String> {
        let access = s(&v["access_token"]);
        let refresh = s(&v["refresh_token"]);
        if access.is_empty() || refresh.is_empty() {
            return Err(t!("friends.error.confirm"));
        }
        let base = self.base()?;
        // A fake server on this machine with its token in the variable: the screenshots'
        // sandbox has no keychain.
        if std::env::var(SERVER.token_var).is_err() || !board::on_this_machine(&base) {
            bragd_model::key::set_in(SERVICE, &base, &refresh)?;
        }
        let until = v["expires_in"].as_u64().map(|s| Instant::now() + Duration::from_secs(s.saturating_sub(60)));
        let mut i = self.lock();
        i.session = Some(Session { access, refresh, until });
        i.error = None;
        Ok(())
    }

    /// A fresh access token from the refresh token kept.
    fn refresh(&self, refresh: &str) -> Result<(), String> {
        let v = self.auth("token?grant_type=refresh_token", &json!({ "refresh_token": refresh }))?;
        self.keep(&v)
    }

    /// The access token, refreshed when it is about to run out.
    pub(crate) fn token(&self) -> Result<String, String> {
        let session = self.lock().session.clone().ok_or_else(|| t!("online.error.not_signed_in", "server" => SERVER.name))?;
        if session.until.is_some_and(|u| Instant::now() >= u) {
            self.refresh(&session.refresh).map_err(|e| self.fail(e))?;
            return self.lock().session.as_ref().map(|s| s.access.clone()).ok_or_else(|| t!("online.error.not_signed_in", "server" => SERVER.name));
        }
        Ok(session.access)
    }

    /// The game service's `method path` with `body`, as the signed-in user.
    pub(crate) fn call(&self, method: &str, path: &str, body: &Value) -> Result<Value, String> {
        let base = self.base()?;
        let token = self.token()?;
        let url = format!("{base}{path}");
        let auth = format!("Bearer {token}");
        let agent = board::agent();
        let r = match method {
            "GET" => agent.get(&url).header("Authorization", &auth).call(),
            "PUT" => agent.put(&url).header("Authorization", &auth).send_json(body),
            "DELETE" => agent.delete(&url).header("Authorization", &auth).force_send_body().send_json(body),
            _ => agent.post(&url).header("Authorization", &auth).send_json(body),
        };
        read(r).map_err(|e| self.fail(e))
    }

    /// Asks how things stand now.
    pub fn poll(&self) -> Result<(), String> {
        let v = self.call("GET", "/v1/state", &json!({}))?;
        let mut i = self.lock();
        i.state = v;
        i.error = None;
        Ok(())
    }

    /// Opens the account after a sign-in: how things stand, then a thread asking again.
    fn start(&self) -> Result<(), String> {
        self.poll()?;
        let stop = Arc::new(AtomicBool::new(false));
        if let Some(old) = self.lock().poll.replace(stop.clone()) {
            old.store(true, Ordering::Relaxed);
        }
        let this = self.clone();
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                std::thread::sleep(POLL);
                if !stop.load(Ordering::Relaxed) {
                    let _ = this.poll();
                }
            }
        });
        Ok(())
    }

    pub fn sign_in_email(&self, email: &str, password: &str) -> Result<(), String> {
        let v = self.auth("token?grant_type=password", &json!({ "email": email.trim(), "password": password })).map_err(|e| self.fail(e))?;
        self.keep(&v)?;
        self.start()
    }

    /// Makes an account. Supabase Auth may first send an email to confirm, and then there is
    /// no session yet: the page says so.
    pub fn sign_up(&self, email: &str, password: &str) -> Result<(), String> {
        let v = self.auth("signup", &json!({ "email": email.trim(), "password": password })).map_err(|e| self.fail(e))?;
        if v["access_token"].is_string() {
            self.keep(&v)?;
            return self.start();
        }
        self.lock().notice = Some(t!("friends.confirm_email", "email" => email.trim()));
        Ok(())
    }

    /// Signs in with `provider` in the browser: Supabase Auth's page, and back to Bragd with a
    /// code, swapped with the PKCE verifier for the tokens.
    pub fn sign_in_with(&self, provider: &str, open: &dyn Fn(&str) -> Result<(), String>) -> Result<(), String> {
        let p = self.public()?;
        if !p.oauth.iter().any(|o| o == provider) {
            return Err(error_text("provider"));
        }
        let pkce = oauth::Pkce::new();
        let lo = oauth::Loopback::bind(0)?;
        // Supabase Auth adds `code` to this address; the state comes back as it was sent.
        let back = format!("{}?state={}", lo.redirect, pkce.state);
        let url = format!(
            "{}/auth/v1/authorize?provider={}&redirect_to={}&code_challenge={}&code_challenge_method=s256",
            p.supabase_url,
            oauth::encode(provider),
            oauth::encode(&back),
            pkce.challenge
        );
        let stop = Arc::new(AtomicBool::new(false));
        self.lock().signing_in = Some(stop.clone());
        let code = open(&url).and_then(|()| lo.wait(SERVER.name, &pkce.state, &stop, SIGN_IN));
        self.lock().signing_in = None;
        let v = self.auth("token?grant_type=pkce", &json!({ "auth_code": code?, "code_verifier": pkce.verifier })).map_err(|e| self.fail(e))?;
        self.keep(&v)?;
        self.start()
    }

    /// Chooses or changes the public name.
    pub fn set_name(&self, name: &str) -> Result<(), String> {
        self.call("PUT", "/v1/me", &json!({ "name": name.trim() }))?;
        self.poll()
    }

    /// Deletes the account on the Bragd server and everything with it; with `keep_games`, the
    /// Games that ended stay with the other players, against a deleted player. Then signs out
    /// here and removes the sign-in from the keychain. `confirm` is what the user typed: it
    /// must be their name, or the word shown when they have none (`delete_confirm`).
    pub fn delete_account(&self, confirm: &str, keep_games: bool) -> Result<(), String> {
        let (word, prompt) = delete_confirm(&self.lock().state);
        if confirm.trim() != word {
            return Err(prompt);
        }
        self.call("DELETE", "/v1/me", &json!({ "keep_games": keep_games }))?;
        <Self as Play>::sign_out(self)?;
        self.lock().notice = Some(t!("friends.deleted"));
        Ok(())
    }

    /// Lets members of the user's Clubs count as Friends, or not.
    pub fn set_club_friends(&self, on: bool) -> Result<(), String> {
        self.call("PUT", "/v1/me", &json!({ "club_friends": on }))?;
        self.poll()
    }

    /// A Friends action: `friends` (ask or accept `name`), `friends/remove`, `invites` (a new
    /// code), `invites/redeem` (`code`), `blocks`, `blocks/remove`, `reports` (`name`, `reason`),
    /// `clubs` (`name`), `clubs/join` (`code`), `clubs/leave` (`id`).
    pub fn friends(&self, what: &str, body: &Value) -> Result<(), String> {
        const WHAT: [&str; 10] = ["friends", "friends/remove", "invites", "invites/redeem", "blocks", "blocks/remove", "reports", "clubs", "clubs/join", "clubs/leave"];
        if !WHAT.contains(&what) {
            return Err(error_text("not_found"));
        }
        let v = self.call("POST", &format!("/v1/{what}"), body)?;
        let notice = match what {
            "invites" => Some(t!("friends.invite_code", "code" => s(&v["code"]))),
            "invites/redeem" => Some(t!("friends.now_friends", "name" => s(&v["name"]))),
            "clubs" => Some(t!("friends.club_code", "code" => s(&v["code"]))),
            "clubs/join" => Some(t!("friends.joined", "club" => s(&v["name"]))),
            "reports" => Some(t!("friends.reported")),
            _ => None,
        };
        self.lock().notice = notice;
        self.poll()
    }

    pub fn friends_view(&self) -> FriendsView {
        let address = self.address();
        let providers = self.lock().public.as_ref().map(|p| p.oauth.iter().map(|o| (o.clone(), provider_name(o))).collect()).unwrap_or_default();
        let i = self.lock();
        let st = &i.state;
        let (delete_word, delete_prompt) = delete_confirm(st);
        FriendsView {
            address,
            providers,
            signed_in: i.session.is_some(),
            signing_in: i.signing_in.is_some(),
            name: st["me"].as_str().map(str::to_string),
            delete_word,
            delete_prompt,
            friends: st["friends"].as_array().map(|a| a.iter().map(|f| Friend { name: s(&f["name"]), club: f["club"].as_bool().unwrap_or(false) }).collect()).unwrap_or_default(),
            asked: strings(&st["asked"]),
            asking: strings(&st["asking"]),
            blocked: strings(&st["blocked"]),
            clubs: st["clubs"]
                .as_array()
                .map(|a| a.iter().map(|c| Club { id: s(&c["id"]), name: s(&c["name"]), code: s(&c["code"]), members: c["members"].as_u64().unwrap_or(0) }).collect())
                .unwrap_or_default(),
            club_friends: st["club_friends"].as_bool().unwrap_or(false),
            error: i.error.clone(),
            notice: i.notice.clone(),
        }
    }

    /// The Games in the last state.
    fn games(&self) -> Vec<Value> {
        self.lock().state["games"].as_array().cloned().unwrap_or_default()
    }

    /// The Game the page shows: the one chosen, else one under way.
    pub(crate) fn shown(&self) -> Option<Value> {
        let chosen = self.lock().game.clone();
        let games = self.games();
        match chosen {
            Some(id) => games.into_iter().find(|g| g["id"] == id.as_str()),
            None => games.into_iter().find(|g| g["status"] == "playing"),
        }
    }

    fn game_view(g: &Value) -> GameView {
        let variant = bragd_games::variant(&s(&g["variant"])).ok().map(|v| v.key());
        let entries = strings(&g["moves"]);
        let state = variant.and_then(|v| bragd_games::replay(v, None, &entries).ok());
        let first = g["first"].as_bool().unwrap_or(false);
        let me = t!("friends.you");
        let them = opponent(g);
        let names = if first { [me, them] } else { [them, me] };
        let over = g["status"] == "over";
        let result = if over {
            let why = t!(&format!("friends.reason.{}", s(&g["reason"])));
            format!("{} · {why}", s(&g["result"]))
        } else {
            String::new()
        };
        GameView {
            id: s(&g["id"]),
            url: String::new(),
            variant,
            variant_name: variant.map(|v| t!(&format!("variant.{v}"))).unwrap_or_default(),
            rated: false,
            against_person: true,
            names,
            seat: Some(if first { 0 } else { 1 }),
            clocks: [0, 0],
            byoyomi: 0,
            periods: [0, 0],
            running: None,
            over,
            result,
            offered: (g["draw_offered"].as_bool().unwrap_or(false), false),
            offering: (g["draw_asked"].as_bool().unwrap_or(false), false),
            opponent_gone: false,
            state,
            initial: None,
            entries,
            dead: None,
        }
    }

    /// The Chat of the Game shown; with `filter`, a stranger's listed words are masked.
    pub fn chat_view(&self, filter: bool) -> Option<ChatView> {
        let g = self.shown()?;
        let c = &g["chat"];
        let (mine, theirs) = (c["mine"].as_bool().unwrap_or(false), c["theirs"].as_bool().unwrap_or(false));
        let friend = g["friend"].as_bool().unwrap_or(false);
        let lines = c["lines"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|l| {
                        let mine = l["mine"].as_bool().unwrap_or(false);
                        let text = s(&l["text"]);
                        Line { mine, text: if filter && !friend && !mine { crate::words::mask(&text) } else { text } }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(ChatView {
            opponent: opponent(&g),
            friend,
            open: mine && theirs,
            asked: theirs && !mine,
            waiting: mine && !theirs,
            keep_mine: c["keep_mine"].as_bool().unwrap_or(false),
            keep_theirs: c["keep_theirs"].as_bool().unwrap_or(false),
            lines,
            live: g["status"] == "playing",
        })
    }

    /// Says yes to the Chat (asks, or allows), or closes it for both (`on` false, also *Not now*).
    pub fn chat(&self, on: bool) -> Result<(), String> {
        self.act("chat", json!({ "on": on }))
    }

    /// Keeps the Chat with the Game, if the opponent says so too.
    pub fn keep_chat(&self, on: bool) -> Result<(), String> {
        self.act("keep", json!({ "on": on }))
    }

    fn act(&self, what: &str, body: Value) -> Result<(), String> {
        let id = self.shown().map(|g| s(&g["id"])).ok_or_else(|| t!("online.error.no_game"))?;
        self.call("POST", &format!("/v1/games/{id}/{what}"), &body)?;
        self.poll()
    }
}

impl Play for BragdServer {
    fn server(&self) -> &'static Server {
        &SERVER
    }

    fn variants(&self) -> Vec<&'static str> {
        bragd_games::Family::ALL.iter().filter(|f| FAMILIES.contains(&f.key())).flat_map(|f| f.variants().map(|v| v.key)).collect()
    }

    fn times(&self) -> Vec<(String, String)> {
        vec![("none".into(), t!("friends.no_clock"))]
    }

    fn colours(&self) -> [&'static str; 2] {
        ["white", "black"]
    }

    fn can(&self) -> Can {
        Can { takeback: false, cancel: true, abort: false, draw: true }
    }

    fn ready(&self) -> bool {
        !self.address().is_empty()
    }

    fn view(&self) -> View {
        let games = self.games();
        let challenges = games
            .iter()
            .filter(|g| g["status"] == "offered")
            .map(|g| {
                let variant = bragd_games::variant(&s(&g["variant"])).ok().map(|v| v.key());
                ChallengeView {
                    id: s(&g["id"]),
                    incoming: !g["challenger"].as_bool().unwrap_or(false),
                    who: s(&g["opponent"]),
                    variant,
                    variant_name: variant.map(|v| t!(&format!("variant.{v}"))).unwrap_or_default(),
                    rated: false,
                    time: t!("friends.no_clock"),
                }
            })
            .collect();
        let game = self.shown().map(|g| Self::game_view(&g));
        let i = self.lock();
        View {
            signed_in: i.session.as_ref().and(i.state["me"].as_str().map(str::to_string)),
            signing_in: i.signing_in.is_some(),
            connected: i.poll.is_some(),
            seeking: false,
            challenges,
            game,
            chat: vec![],
            error: i.error.clone(),
        }
    }

    fn live(&self) -> Option<(bool, bool, bool)> {
        self.shown().map(|g| (false, true, g["status"] == "over"))
    }

    fn open(&self) -> Result<(), String> {
        let base = self.base()?;
        {
            let mut i = self.lock();
            if i.session.is_some() || i.tried {
                return Ok(());
            }
            i.tried = true;
        }
        // The sign-in providers, for the page; a server that does not answer says so on the page.
        self.public().map_err(|e| self.fail(e))?;
        let kept = std::env::var(SERVER.token_var).ok().filter(|_| board::on_this_machine(&base)).or_else(|| bragd_model::key::get_in(SERVICE, &base)).filter(|t| !t.is_empty());
        let Some(refresh) = kept else { return Ok(()) };
        self.refresh(&refresh).map_err(|e| self.fail(e))?;
        self.start()
    }

    fn reconnect(&self) -> Result<(), String> {
        self.lock().tried = false;
        self.open()
    }

    fn sign_in(&self, open: &dyn Fn(&str) -> Result<(), String>) -> Result<(), String> {
        let first = self.public()?.oauth.first().cloned().ok_or_else(|| error_text("provider"))?;
        self.sign_in_with(&first, open)
    }

    fn cancel_sign_in(&self) {
        if let Some(s) = self.lock().signing_in.take() {
            s.store(true, Ordering::Relaxed);
        }
    }

    fn sign_out(&self) -> Result<(), String> {
        let session = self.lock().session.clone();
        if let (Some(s), Ok(p)) = (session, self.public()) {
            // Ends the session on Supabase too; signed out here whatever it answers.
            let _ = board::agent().post(&format!("{}/auth/v1/logout", p.supabase_url)).header("apikey", &p.supabase_key).header("Authorization", &format!("Bearer {}", s.access)).send_empty();
        }
        let base = {
            let mut i = self.lock();
            if let Some(p) = i.poll.take() {
                p.store(true, Ordering::Relaxed);
            }
            let base = i.base.clone();
            *i = Inner { base: base.clone(), public: i.public.take(), tried: true, ..Inner::default() };
            base
        };
        bragd_model::key::set_in(SERVICE, &base, "")
    }

    fn seek(&self, _variant: &str, _time: &str, _rated: bool) -> Result<(), String> {
        Err(t!("friends.error.no_seek"))
    }

    fn seek_cancel(&self) {}

    fn challenge(&self, user: &str, variant: &str, _time: &str, _rated: bool, color: &str) -> Result<(), String> {
        let side = match color {
            "white" => "first",
            "black" => "second",
            _ => "random",
        };
        self.call("POST", "/v1/games", &json!({ "opponent": user.trim(), "variant": variant, "side": side }))?;
        self.poll()
    }

    fn answer(&self, id: &str, what: &str) -> Result<(), String> {
        if !matches!(what, "accept" | "decline" | "cancel") {
            return Err(error_text("not_found"));
        }
        self.call("POST", &format!("/v1/games/{id}/{what}"), &json!({}))?;
        if what == "accept" {
            self.lock().game = Some(id.to_string());
        }
        self.poll()
    }

    fn play(&self, entry: &str) -> Result<(), String> {
        let g = self.shown().ok_or_else(|| t!("online.error.no_game"))?;
        let view = Self::game_view(&g);
        let st = view.state.as_ref().ok_or_else(|| t!("online.error.no_game"))?;
        if st.to_move.map(|s| s.index) != view.seat {
            return Err(t!("online.error.not_your_turn"));
        }
        // Bragd's rules first, so a wrong move never leaves the machine; the referee checks again.
        bragd_games::apply(view.variant.unwrap_or_default(), None, &view.entries, entry).map_err(|e| e.to_string())?;
        self.act("move", json!({ "ply": view.entries.len(), "entry": entry }))
    }

    fn draw(&self, yes: bool) -> Result<(), String> {
        self.act("draw", json!({ "yes": yes }))
    }

    fn takeback(&self, _yes: bool) -> Result<(), String> {
        Err(t!("online.error.no_takeback", "server" => SERVER.name))
    }

    fn resign(&self) -> Result<(), String> {
        self.act("resign", json!({}))
    }

    fn abort(&self) -> Result<(), String> {
        self.resign()
    }

    fn leave(&self) {
        let mut i = self.lock();
        // Back to the lobby: no Game is chosen, and the one that ended is not picked again.
        i.game = None;
        if let Some(games) = i.state["games"].as_array_mut() {
            games.retain(|g| g["status"] != "over");
        }
    }

    /// The Play online page's chat switch is Lichess's: the Bragd server's Chat asks both
    /// players instead ([`BragdServer::chat`]).
    fn set_chat(&self, _on: bool) -> Result<(), String> {
        Ok(())
    }

    fn say(&self, text: &str) -> Result<(), String> {
        self.act("say", json!({ "text": text.trim() }))
    }
}

/// What Delete my account asks to be typed, and the label asking for it: the user's name, or a
/// word when they chose none. The page shows these, and [`BragdServer::delete_account`] compares
/// with the same word.
fn delete_confirm(state: &Value) -> (String, String) {
    match state["me"].as_str().filter(|n| !n.is_empty()) {
        Some(name) => (name.to_string(), t!("friends.delete_account.confirm", "name" => name)),
        None => {
            let word = t!("friends.delete_account.word");
            let prompt = t!("friends.delete_account.confirm_word", "word" => &word);
            (word, prompt)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake_bragd;

    fn signed_in() -> (BragdServer, crate::fake::Fake, Arc<Mutex<fake_bragd::World>>) {
        bragd_model::key::mock();
        let (fake, world) = fake_bragd::bragd();
        let b = BragdServer::at(&fake.url);
        b.sign_in_email("tester@example.org", fake_bragd::PASSWORD).unwrap();
        (b, fake, world)
    }

    #[test]
    fn bragd_server_nothing_is_asked_without_an_address() {
        let b = BragdServer::at("");
        assert_eq!(b.open(), Err(t!("friends.error.no_address")));
        assert!(!b.ready());
        assert!(b.sign_in_email("a@b.c", "x").is_err());
    }

    #[test]
    fn bragd_server_signs_in_by_email_and_keeps_only_the_refresh_token() {
        let (b, fake, _) = signed_in();
        assert_eq!(bragd_model::key::get_in(SERVICE, &fake.url).as_deref(), Some(fake_bragd::REFRESH));
        let v = b.friends_view();
        assert!(v.signed_in);
        let json = serde_json::to_value(&v).unwrap();
        assert_eq!((&json["signedIn"], &json["clubFriends"]), (&json!(true), &json!(true)), "the page reads camelCase");
        assert_eq!(v.name.as_deref(), Some("BragdTester"));
        assert_eq!(v.friends, [Friend { name: "Anna".into(), club: false }, Friend { name: "Bo".into(), club: true }]);
        assert_eq!(v.asked, ["Carl"]);
        let auth = fake.seen().into_iter().find(|r| r.path == "/auth/v1/token").unwrap();
        assert_eq!(auth.query, "grant_type=password");
        // A second window opens the account from the kept token.
        let other = BragdServer::at(&fake.url);
        other.open().unwrap();
        assert!(other.friends_view().signed_in);
        assert!(fake.seen().iter().any(|r| r.query == "grant_type=refresh_token"));
        b.sign_out().unwrap();
        assert!(bragd_model::key::get_in(SERVICE, &fake.url).filter(|t| !t.is_empty()).is_none());
        assert!(!b.friends_view().signed_in);
    }

    #[test]
    fn bragd_server_deletes_the_account_and_forgets_the_sign_in() {
        let (b, fake, world) = signed_in();
        assert!(b.delete_account("Bragd", true).is_err(), "only the user's own name confirms");
        assert_eq!(world.lock().unwrap().deleted, None);
        b.delete_account(" BragdTester ", true).unwrap();
        assert_eq!(world.lock().unwrap().deleted, Some(true));
        assert!(bragd_model::key::get_in(SERVICE, &fake.url).filter(|t| !t.is_empty()).is_none());
        let v = b.friends_view();
        assert!(!v.signed_in);
        assert_eq!(v.notice, Some(t!("friends.deleted")));
        assert!(b.delete_account("BragdTester", false).is_err(), "signed out: nothing more is sent");
    }

    #[test]
    fn bragd_server_an_account_without_a_name_confirms_with_the_word_shown() {
        let (b, _fake, world) = signed_in();
        assert_eq!(b.friends_view().delete_word, "BragdTester", "a named account types its name");
        world.lock().unwrap().me = None;
        b.poll().unwrap();
        let word = b.friends_view().delete_word;
        assert_eq!(word, t!("friends.delete_account.word"));
        assert!(b.friends_view().delete_prompt.contains(&word), "the label shows the word");
        assert!(!word.is_empty());
        assert!(b.delete_account("", true).is_err(), "an empty name does not confirm");
        assert!(b.delete_account("BragdTester", true).is_err());
        assert_eq!(world.lock().unwrap().deleted, None);
        b.delete_account(&format!(" {word} "), false).unwrap();
        assert_eq!(world.lock().unwrap().deleted, Some(false));
    }

    #[test]
    fn bragd_server_names_a_deleted_opponent() {
        let g = json!({ "opponent": null, "first": true, "variant": "chess", "status": "over", "moves": [] });
        assert_eq!(BragdServer::game_view(&g).names[1], t!("friends.deleted_player"));
    }

    #[test]
    fn bragd_server_a_wrong_password_says_so() {
        bragd_model::key::mock();
        let (fake, _) = fake_bragd::bragd();
        let b = BragdServer::at(&fake.url);
        assert_eq!(b.sign_in_email("a@b.c", "wrong"), Err(t!("friends.error.invalid_credentials")));
        assert!(!b.friends_view().signed_in);
    }

    #[test]
    fn bragd_server_friends_actions_reach_the_server() {
        let (b, _fake, world) = signed_in();
        b.friends("friends", &json!({ "name": "Carl" })).unwrap();
        assert!(b.friends_view().friends.iter().any(|f| f.name == "Carl"));
        b.friends("blocks", &json!({ "name": "Anna" })).unwrap();
        assert_eq!(b.friends_view().blocked, ["Anna"]);
        b.friends("reports", &json!({ "name": "Anna", "reason": "rude" })).unwrap();
        assert_eq!(world.lock().unwrap().reports, [("Anna".to_string(), "rude".to_string())]);
        b.friends("invites", &json!({})).unwrap();
        assert_eq!(b.friends_view().notice, Some(t!("friends.invite_code", "code" => "fake-invite-code")));
        assert!(b.friends("../v1/games", &json!({})).is_err(), "only the actions named");
        b.set_club_friends(false).unwrap();
        assert!(!b.friends_view().club_friends);
    }

    #[test]
    fn bragd_server_plays_a_game_checked_by_the_rules_first() {
        let (b, fake, _) = signed_in();
        let v = b.view();
        assert_eq!(v.challenges.len(), 1);
        assert!(v.challenges[0].incoming);
        assert_eq!(v.challenges[0].who, "Anna");
        b.answer(&v.challenges[0].id, "accept").unwrap();
        let g = b.view().game.unwrap();
        assert_eq!(g.seat, Some(0));
        assert_eq!(g.names[1], "Anna");
        assert!(!g.rated);
        let sent = fake.seen().len();
        assert!(b.play("e2e5").is_err());
        assert_eq!(fake.seen().len(), sent, "an illegal move never leaves the machine");
        b.play("e2e4").unwrap();
        let g = b.view().game.unwrap();
        assert_eq!(g.entries.len(), 2, "Anna answered");
        b.resign().unwrap();
        let g = b.view().game.unwrap();
        assert!(g.over);
        assert!(g.result.starts_with("0-1"));
        assert_eq!(b.live(), Some((false, true, true)), "never rated: no fair-play lock");
        b.leave();
        assert!(b.view().game.is_none());
    }

    #[test]
    fn bragd_server_challenges_with_the_side_asked_for() {
        let (b, fake, world) = signed_in();
        b.challenge("Bo", "xiangqi", "none", false, "black").unwrap();
        let sent = fake.seen().into_iter().find(|r| r.path == "/v1/games").unwrap();
        let body: Value = serde_json::from_str(&sent.body).unwrap();
        assert_eq!(body, json!({ "opponent": "Bo", "variant": "xiangqi", "side": "second" }));
        assert!(!world.lock().unwrap().games.last().unwrap().first);
        assert!(b.seek("chess", "none", false).is_err());
        assert!(!b.variants().contains(&"backgammon"));
        assert!(b.variants().contains(&"brandubh"));
    }
}

#[cfg(test)]
mod chat_tests {
    use super::*;
    use crate::fake_bragd;

    #[test]
    fn bragd_server_chat_asks_both_and_filters_a_stranger() {
        bragd_model::key::mock();
        let (fake, world) = fake_bragd::bragd();
        let b = BragdServer::at(&fake.url);
        b.sign_in_email("t@example.org", fake_bragd::PASSWORD).unwrap();
        b.answer("g-anna", "accept").unwrap();
        assert_eq!(b.chat_view(true).map(|c| (c.open, c.asked, c.waiting)), Some((false, false, false)), "off by default");
        b.chat(true).unwrap();
        assert!(b.chat_view(true).unwrap().waiting);
        world.lock().unwrap().games[0].chat_theirs = true;
        world.lock().unwrap().games[0].lines.push((false, "what the fuck".into()));
        b.poll().unwrap();
        let c = b.chat_view(true).unwrap();
        assert!(c.open);
        assert_eq!(c.lines[0].text, "what the fuck", "Anna is a Friend: never filtered");
        world.lock().unwrap().games[0].friend = false;
        b.poll().unwrap();
        assert_eq!(b.chat_view(true).unwrap().lines[0].text, "what the ****");
        assert_eq!(b.chat_view(false).unwrap().lines[0].text, "what the fuck", "the filter can be turned off");
        b.say("hello").unwrap();
        assert_eq!(world.lock().unwrap().games[0].lines.last().unwrap(), &(true, "hello".to_string()));
        b.chat(false).unwrap();
        let c = b.chat_view(true).unwrap();
        assert!(!c.open && !c.waiting, "closed for both");
    }
}
