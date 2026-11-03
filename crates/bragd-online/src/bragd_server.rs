//! Playing on the Bragd server, between Bragd players: the account (Supabase Auth, by email or
//! a sign-in provider), Friends, Clubs, and Games refereed by the game service with Bragd's own
//! rules (`crates/bragd-server`). The server's address is Settings → Online → *Bragd server
//! address*; until one is set, nothing here talks to anyone. The refresh token is kept in the OS
//! keychain only. While the account is open, a thread listens on the user's own Realtime topic
//! and asks the game service how things stand as soon as the referee says a Game changed; it
//! also asks every few seconds, and every two while the topic cannot be joined. The page reads
//! the [`View`] and the [`FriendsView`] from that.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bragd_core::t;
use serde::Serialize;
use serde_json::{Value, json};

use crate::server::Server;
use crate::session::SERVICE;
use crate::{Can, ChallengeView, GameView, Play, View, board, oauth};

/// The Families a Game on the Bragd server may be played in: every one. In the dice Families
/// the referee throws for both players.
pub const FAMILIES: &[&str] = &["chess", "xiangqi", "shogi", "go", "draughts", "tafl", "backgammon", "ur", "senet"];

pub const SERVER: Server = Server {
    id: "bragd",
    name: "Bragd",
    site: "",
    url: "",
    url_var: bragd_core::config::BRAGD_SERVER_VAR,
    token_var: "BRAGD_FRIENDS_TOKEN",
    terms: "",
    families: FAMILIES,
};

/// How often the game service is asked how things stand without the live topic, and with it.
const POLL: Duration = Duration::from_secs(2);
const POLL_LIVE: Duration = Duration::from_secs(10);
/// How often the poll thread looks for news.
const TICK: Duration = Duration::from_millis(50);
/// How long after a failed join of the live topic it is tried again.
const RETRY: Duration = Duration::from_secs(30);
/// How long the browser has for a sign-in.
const SIGN_IN: Duration = Duration::from_secs(300);

/// Whether to ask the game service now: at once on news from the live topic, otherwise when
/// the last ask is `since` ago, longer apart while the live topic is joined.
fn due(news: bool, live: bool, since: Duration) -> bool {
    news || since >= if live { POLL_LIVE } else { POLL }
}

/// The address set in Settings, or the one `BRAGD_FRIENDS_URL` names; empty when none is set.
pub fn address() -> String {
    bragd_core::config::Config::load().unwrap_or_default().bragd_server()
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
    /// When `state` came, for the clock that runs meanwhile.
    polled: Option<Instant>,
    /// The seeks the user declined: not shown again.
    hidden: Vec<String>,
    /// The user's live topic, and the access token it was joined with.
    live: Option<(String, Arc<crate::realtime::Channel>)>,
    /// Backgammon's turn being moved checker by checker: the Game, the ply it is, and the
    /// checkers moved so far, all on this machine until the dice are used.
    part: Option<(String, usize, Vec<String>)>,
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

/// A rating on the Bragd server, as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Rated {
    /// The Family's key, for the Settings filter, and its name.
    pub family: String,
    pub name: String,
    /// `1612`, or `1500?` while it is provisional.
    pub rating: String,
    pub games: u64,
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
        let flagged: Vec<String> = {
            let mut i = self.lock();
            i.state = v;
            i.polled = Some(Instant::now());
            i.error = None;
            // A clock that ran out: the referee is asked to see it, and decides.
            i.state["games"].as_array().into_iter().flatten().filter(|g| run_out(g)).map(|g| s(&g["id"])).collect()
        };
        for id in flagged {
            // The other player may have asked first; the next state tells.
            let _ = self.call("POST", &format!("/v1/games/{id}/flag"), &json!({}));
        }
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
            let (mut asked, mut tried) = (Instant::now(), None);
            while !stop.load(Ordering::Relaxed) {
                std::thread::sleep(TICK);
                let news = this.news(&mut tried);
                if !stop.load(Ordering::Relaxed) && due(news, this.lock().live.is_some(), asked.elapsed()) {
                    asked = Instant::now();
                    let _ = this.poll();
                }
            }
        });
        Ok(())
    }

    /// Whether the referee said a Game changed since the last call. Joins the user's live topic
    /// when it is not joined, on a thread of its own since a join may hang, at most every
    /// [`RETRY`]; it is joined again once lost, or once the access token was refreshed.
    fn news(&self, tried: &mut Option<Instant>) -> bool {
        let (token, live) = {
            let i = self.lock();
            (i.session.as_ref().map(|s| s.access.clone()), i.live.clone())
        };
        match live {
            Some((t, ch)) if Some(&t) == token.as_ref() && ch.lost().is_none() => return !ch.take().is_empty(),
            Some(_) => self.lock().live = None,
            None => {}
        }
        if tried.is_some_and(|t| t.elapsed() < RETRY) {
            return false;
        }
        *tried = Some(Instant::now());
        let this = self.clone();
        std::thread::spawn(move || {
            let joined = (|| {
                let p = this.public()?;
                let token = this.token()?;
                let me = s(&this.lock().state["id"]);
                if me.is_empty() {
                    return Err(String::new());
                }
                let ch = crate::realtime::Channel::join(&p.supabase_url, &p.supabase_key, &token, &format!("player:{me}"), &me)?;
                Ok::<_, String>((token, Arc::new(ch)))
            })();
            let mut i = this.lock();
            // Signed out or in again meanwhile: this join is for nobody.
            if let Ok((token, ch)) = joined
                && i.session.as_ref().is_some_and(|s| s.access == token)
            {
                i.live = Some((token, ch));
            }
        });
        false
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

    /// The user's rating in each Family they played a rated Game in.
    pub fn ratings(&self) -> Vec<Rated> {
        self.lock().state["ratings"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|r| {
                        let family = s(&r["family"]);
                        // Glicko-2's deviation: above 110 the rating is still provisional (docs/adr/0007).
                        let sure = r["deviation"].as_f64().unwrap_or(350.0) <= 110.0;
                        let rating = format!("{}{}", r["rating"].as_f64().unwrap_or(1500.0).round(), if sure { "" } else { "?" });
                        Rated { name: t!(&format!("family.{family}")), family, rating, games: r["games"].as_u64().unwrap_or(0) }
                    })
                    .collect()
            })
            .unwrap_or_default()
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

    /// The checkers moved so far in the turn of the Game `g`, as it now stands.
    fn part_of(&self, g: &Value) -> Vec<String> {
        let ply = g["moves"].as_array().map_or(0, Vec::len);
        match &self.lock().part {
            Some((id, at, part)) if g["id"] == id.as_str() && *at == ply => part.clone(),
            _ => vec![],
        }
    }

    /// The Game `g`, its running clock `since` ms lower than the state said, with the checkers
    /// of `part` moved (backgammon).
    fn game_view(g: &Value, since: u64, part: &[String]) -> GameView {
        let variant = bragd_games::variant(&s(&g["variant"])).ok().map(|v| v.key());
        let entries = strings(&g["moves"]);
        let initial = g["initial"].as_str().map(str::to_string);
        let mut state = variant.and_then(|v| match part {
            [] => bragd_games::replay(v, initial.as_deref(), &entries).ok(),
            _ => bragd_games::step(v, initial.as_deref(), &entries, part).ok().map(|r| r.state),
        });
        let first = g["first"].as_bool().unwrap_or(false);
        // The referee throws; where the player may double first (backgammon), it waits for them
        // to ask, so the page offers *Throw* beside *Double*.
        if let Some(st) = state.as_mut().filter(|st| st.awaiting == bragd_games::Awaiting::Throw && st.to_move.map(|s| s.index) == Some(if first { 0 } else { 1 })) {
            st.moves.push(bragd_games::LegalMove { wire: "throw".into(), notation: "throw".into(), from: None, to: None, drop: None });
        }
        let me = t!("friends.you");
        let them = opponent(g);
        let names = if first { [me, them] } else { [them, me] };
        let over = g["status"] == "over";
        // The referee's own reasons have words; the rules' show in the result itself.
        let key = format!("friends.reason.{}", s(&g["reason"]));
        let result = match t!(&key) {
            _ if !over => String::new(),
            why if why != key => format!("{} · {why}", s(&g["result"])),
            _ => s(&g["result"]),
        };
        let c = &g["clock"];
        let running = c["running"].as_u64().map(|r| r as u8);
        let left = |i: usize| {
            let l = c["left"][i].as_u64().unwrap_or(0);
            if running == Some(i as u8) { l.saturating_sub(since) } else { l }
        };
        GameView {
            id: s(&g["id"]),
            url: String::new(),
            variant,
            variant_name: variant.map(|v| t!(&format!("variant.{v}"))).unwrap_or_default(),
            rated: g["rated"].as_bool().unwrap_or(false),
            against_person: true,
            names,
            seat: Some(if first { 0 } else { 1 }),
            clocks: [left(0), left(1)],
            byoyomi: 0,
            periods: [0, 0],
            running,
            over,
            result,
            offered: (g["draw_offered"].as_bool().unwrap_or(false), false),
            offering: (g["draw_asked"].as_bool().unwrap_or(false), false),
            opponent_gone: false,
            state,
            initial,
            entries,
            dead: None,
            part: part.len(),
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

    /// No clock, or a few time controls, down to bullet: moves reach the other player over the
    /// live topic at once. 10+5 first, as the page's default.
    fn times(&self) -> Vec<(String, String)> {
        std::iter::once(("none".to_string(), t!("friends.no_clock")))
            .chain(["10+5", "1+0", "3+2", "5+3", "15+10", "30+20"].iter().filter_map(|t| Some((t.to_string(), board::Time::parse(t)?.label()))))
            .collect()
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
        let challenge = |id: String, incoming: bool, who: String, variant: &Value, rated: bool, minutes: Option<u64>, increment: u64| {
            let variant = bragd_games::variant(&s(variant)).ok().map(|v| v.key());
            ChallengeView {
                id,
                incoming,
                who,
                variant,
                variant_name: variant.map(|v| t!(&format!("variant.{v}"))).unwrap_or_default(),
                rated,
                time: minutes.map_or_else(|| t!("friends.no_clock"), |m| t!("online.minutes_plus", "m" => m, "s" => increment)),
            }
        };
        let mut challenges: Vec<ChallengeView> = games
            .iter()
            .filter(|g| g["status"] == "offered")
            .map(|g| {
                let c = &g["clock"];
                let minutes = c["left"][0].as_u64().map(|ms| ms / 60_000);
                challenge(s(&g["id"]), !g["challenger"].as_bool().unwrap_or(false), s(&g["opponent"]), &g["variant"], g["rated"].as_bool().unwrap_or(false), minutes, c["increment"].as_u64().unwrap_or(0) / 1000)
            })
            .collect();
        let (seeks, seeking, since, hidden) = {
            let i = self.lock();
            let seeks = i.state["seeks"].as_array().cloned().unwrap_or_default();
            let seeking = seeks.iter().any(|k| k["mine"] == true);
            (seeks, seeking, i.polled.map_or(0, |p| p.elapsed().as_millis() as u64), i.hidden.clone())
        };
        // Other players' seeks, as challenges anyone may accept.
        challenges.extend(seeks.iter().filter(|k| k["mine"] != true).map(|k| (format!("{SEEK}{}", s(&k["id"])), k)).filter(|(id, _)| !hidden.contains(id)).map(|(id, k)| {
            challenge(id, true, s(&k["name"]), &k["variant"], k["rated"].as_bool().unwrap_or(false), k["minutes"].as_u64(), k["increment"].as_u64().unwrap_or(0))
        }));
        let game = self.shown().map(|g| Self::game_view(&g, since, &self.part_of(&g)));
        let i = self.lock();
        View {
            signed_in: i.session.as_ref().and(i.state["me"].as_str().map(str::to_string)),
            signing_in: i.signing_in.is_some(),
            connected: i.poll.is_some(),
            seeking,
            challenges,
            game,
            chat: vec![],
            error: i.error.clone(),
        }
    }

    fn live(&self) -> Option<(bool, bool, bool)> {
        self.shown().map(|g| (g["rated"] == true, true, g["status"] == "over"))
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

    fn seek(&self, variant: &str, time: &str, rated: bool) -> Result<(), String> {
        let mut body = time_body(time);
        body["variant"] = json!(variant);
        body["rated"] = json!(rated);
        self.call("POST", "/v1/seeks", &body)?;
        self.poll()
    }

    fn seek_cancel(&self) {
        // Its error shows on the page; the seek lapses after an hour anyway.
        if self.call("DELETE", "/v1/seeks", &json!({})).is_ok() {
            let _ = self.poll();
        }
    }

    fn challenge(&self, user: &str, variant: &str, time: &str, rated: bool, color: &str) -> Result<(), String> {
        let side = match color {
            "white" => "first",
            "black" => "second",
            _ => "random",
        };
        let mut body = time_body(time);
        body["opponent"] = json!(user.trim());
        body["variant"] = json!(variant);
        body["side"] = json!(side);
        body["rated"] = json!(rated);
        self.call("POST", "/v1/games", &body)?;
        self.poll()
    }

    fn answer(&self, id: &str, what: &str) -> Result<(), String> {
        if !matches!(what, "accept" | "decline" | "cancel") {
            return Err(error_text("not_found"));
        }
        if let Some(seek) = id.strip_prefix(SEEK) {
            if what != "accept" {
                self.lock().hidden.push(id.to_string());
                return Ok(());
            }
            let v = self.call("POST", &format!("/v1/seeks/{seek}/accept"), &json!({}))?;
            self.lock().game = Some(s(&v["id"]));
            return self.poll();
        }
        self.call("POST", &format!("/v1/games/{id}/{what}"), &json!({}))?;
        if what == "accept" {
            self.lock().game = Some(id.to_string());
        }
        self.poll()
    }

    fn play(&self, entry: &str) -> Result<(), String> {
        let g = self.shown().ok_or_else(|| t!("online.error.no_game"))?;
        let part = self.part_of(&g);
        let view = Self::game_view(&g, 0, &part);
        let st = view.state.as_ref().ok_or_else(|| t!("online.error.no_game"))?;
        if st.to_move.map(|s| s.index) != view.seat {
            return Err(t!("online.error.not_your_turn"));
        }
        if entry == "throw" {
            return self.act("throw", json!({}));
        }
        let variant = view.variant.unwrap_or_default();
        // Backgammon, as local Play moves it: a checker at a time, kept here; the rules make the
        // whole turn once the dice are used, and only that goes to the referee.
        if st.steps.iter().any(|m| m.wire == entry) {
            let mut next = part;
            next.push(entry.to_string());
            let turn = bragd_games::step(variant, view.initial.as_deref(), &view.entries, &next).map_err(|e| e.to_string())?.entry;
            let Some(turn) = turn else {
                self.lock().part = Some((view.id, view.entries.len(), next));
                return Ok(());
            };
            self.act("move", json!({ "ply": view.entries.len(), "entry": turn }))?;
            self.lock().part = None;
            return Ok(());
        }
        // Bragd's rules first, so a wrong move never leaves the machine; the referee checks again.
        bragd_games::apply(variant, view.initial.as_deref(), &view.entries, entry).map_err(|e| e.to_string())?;
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

    fn step_back(&self) {
        if let Some((_, _, part)) = self.lock().part.as_mut() {
            part.pop();
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

/// A seek's id on the page, apart from a challenge's.
const SEEK: &str = "seek:";

/// The time control `10+5` as the game service takes it; nothing for `none`.
fn time_body(time: &str) -> Value {
    match board::Time::parse(time) {
        Some(t) => json!({ "minutes": t.minutes, "increment": t.increment }),
        None => json!({}),
    }
}

/// The Game is under way and the clock that runs reached nought.
fn run_out(g: &Value) -> bool {
    g["status"] == "playing" && g["clock"]["running"].as_u64().is_some_and(|r| g["clock"]["left"][r as usize].as_u64() == Some(0))
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
        assert_eq!(BragdServer::game_view(&g, 0, &[]).names[1], t!("friends.deleted_player"));
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
        let anna = v.challenges.iter().find(|c| !c.id.starts_with(SEEK)).unwrap();
        assert!(anna.incoming);
        assert_eq!(anna.who, "Anna");
        b.answer(&anna.id, "accept").unwrap();
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
        b.challenge("Bo", "xiangqi", "10+5", true, "black").unwrap();
        let sent = fake.seen().into_iter().find(|r| r.path == "/v1/games").unwrap();
        let body: Value = serde_json::from_str(&sent.body).unwrap();
        assert_eq!(body, json!({ "opponent": "Bo", "variant": "xiangqi", "side": "second", "minutes": 10, "increment": 5, "rated": true }));
        assert!(!world.lock().unwrap().games.last().unwrap().first);
        let v = b.view();
        let c = v.challenges.iter().find(|c| c.who == "Bo" && !c.incoming).unwrap();
        assert_eq!((c.rated, c.time.as_str()), (true, t!("online.minutes_plus", "m" => 10, "s" => 5).as_str()));
        b.challenge("Bo", "xiangqi", "none", false, "random").unwrap();
        let sent = fake.seen().into_iter().rfind(|r| r.path == "/v1/games").unwrap();
        assert_eq!(serde_json::from_str::<Value>(&sent.body).unwrap()["minutes"], Value::Null, "no clock");
        assert!(b.variants().contains(&"backgammon"));
        assert!(b.variants().contains(&"brandubh"));
    }

    /// The user seeks and takes the seek back; Bo's seek shows as a challenge anyone may take,
    /// and taking it starts a rated Game with its clock.
    #[test]
    fn bragd_server_seeks_and_takes_a_seek() {
        let (b, fake, _) = signed_in();
        b.seek("chess", "5+3", false).unwrap();
        assert!(b.view().seeking);
        let sent = fake.seen().into_iter().find(|r| r.path == "/v1/seeks").unwrap();
        assert_eq!(serde_json::from_str::<Value>(&sent.body).unwrap(), json!({ "variant": "chess", "minutes": 5, "increment": 3, "rated": false }));
        b.seek_cancel();
        assert!(!b.view().seeking);
        let v = b.view();
        let bo = v.challenges.iter().find(|c| c.who == "Bo").unwrap();
        assert!(bo.incoming && bo.rated);
        assert_eq!(bo.variant, Some("shogi"));
        b.answer(&bo.id, "accept").unwrap();
        assert!(fake.seen().iter().any(|r| r.path == "/v1/seeks/s-bo/accept"));
        let g = b.view().game.unwrap();
        assert!(g.rated);
        assert_eq!(g.running, Some(0));
        assert!(g.clocks[0] <= 600_000 && g.clocks[1] == 600_000, "{:?}", g.clocks);
        assert_eq!(b.live(), Some((true, true, false)), "rated: the fair-play lock holds");
    }

    #[test]
    fn bragd_server_a_declined_seek_is_not_shown_again() {
        let (b, fake, _) = signed_in();
        let id = b.view().challenges.iter().find(|c| c.who == "Bo").unwrap().id.clone();
        let sent = fake.seen().len();
        b.answer(&id, "decline").unwrap();
        assert!(!b.view().challenges.iter().any(|c| c.who == "Bo"));
        assert_eq!(fake.seen().len(), sent, "nothing is sent");
    }

    #[test]
    fn bragd_server_shows_ratings_provisional_with_a_question_mark() {
        let (b, _, _) = signed_in();
        let r: Vec<(String, String)> = b.ratings().into_iter().map(|r| (r.family, r.rating)).collect();
        assert_eq!(r, [("chess".to_string(), "1612".to_string()), ("shogi".to_string(), "1480?".to_string())]);
    }

    /// The running clock reached nought: the referee is asked to see the flag.
    #[test]
    fn bragd_server_asks_the_referee_when_a_clock_runs_out() {
        let (b, fake, world) = signed_in();
        {
            let mut w = world.lock().unwrap();
            let g = &mut w.games[0];
            g.status = "playing".into();
            g.clock = json!({ "left": [0, 30_000], "running": 0, "increment": 0 });
        }
        b.poll().unwrap();
        assert!(fake.seen().iter().any(|r| r.path == "/v1/games/g-anna/flag"));
        world.lock().unwrap().games[0].clock = json!({ "left": [5_000, 30_000], "running": 0, "increment": 0 });
        let n = fake.seen().iter().filter(|r| r.path.ends_with("/flag")).count();
        b.poll().unwrap();
        assert_eq!(fake.seen().iter().filter(|r| r.path.ends_with("/flag")).count(), n, "time is left");
    }

    /// Backgammon, Black to throw with the cube in the middle: the page offers Double and Throw,
    /// and Throw asks the referee, never the dice on this machine.
    #[test]
    fn bragd_server_asks_the_referee_to_throw() {
        let (b, fake, world) = signed_in();
        {
            let mut w = world.lock().unwrap();
            let g = &mut w.games[0];
            (g.variant, g.status, g.first, g.initial) = ("backgammon".into(), "playing".into(), false, Some("white".into()));
            g.moves = vec!["3-1".into(), "8/5 6/5".into()];
        }
        b.poll().unwrap();
        let g = b.view().game.unwrap();
        assert_eq!(g.initial.as_deref(), Some("white"));
        let st = g.state.unwrap();
        let plain: Vec<&str> = st.moves.iter().map(|m| m.wire.as_str()).collect();
        assert!(plain.contains(&"double") && plain.contains(&"throw"), "{plain:?}");
        b.play("throw").unwrap();
        assert!(fake.seen().iter().any(|r| r.path == "/v1/games/g-anna/throw"));
        assert_eq!(b.view().game.unwrap().entries.len(), 3, "the referee's throw");
    }

    /// The referee's news on the user's live topic makes the page ask at once; without the
    /// topic it asks every two seconds, with it only now and then.
    #[test]
    fn bragd_server_asks_at_once_when_the_live_topic_says_a_game_changed() {
        let (b, fake, _) = signed_in();
        // The page's own thread stopped: this test takes the news itself.
        b.lock().poll.take().unwrap().store(true, Ordering::Relaxed);
        let mut tried = None;
        // A guard against a hang only.
        let until = Instant::now() + Duration::from_secs(10);
        while b.lock().live.is_none() {
            assert!(!b.news(&mut tried) && Instant::now() < until, "the live topic is joined");
            std::thread::sleep(TICK);
        }
        let join = fake.said();
        assert!(join.iter().any(|t| t.contains("realtime:player:id-bragdtester") && t.contains("phx_join")), "{join:?}");
        assert!(!b.news(&mut tried), "no news yet");
        let news = json!({ "topic": "realtime:player:id-bragdtester", "event": "broadcast", "ref": null,
            "payload": { "type": "broadcast", "event": "signal", "payload": { "from": "bragd", "signal": { "game": "g-anna" } } } });
        fake.tell(&news.to_string());
        let mut heard = false;
        while !heard {
            assert!(Instant::now() < until, "the news arrives");
            std::thread::sleep(TICK);
            heard = b.news(&mut tried);
        }
        assert!(due(true, true, Duration::ZERO), "news: at once");
        assert!(!due(false, true, POLL), "live: no ask every two seconds");
        assert!(due(false, true, POLL_LIVE) && due(false, false, POLL), "a poll stays as the fallback");
        assert!(b.times().iter().any(|(t, _)| t == "1+0") && b.times().iter().any(|(t, _)| t == "3+2"), "bullet and blitz");
    }

    /// Backgammon, the user's turn after a throw: each checker is a step kept on this machine,
    /// and only the whole turn, once the dice are used, goes to the referee.
    #[test]
    fn bragd_server_moves_backgammon_checker_by_checker() {
        let (b, fake, world) = signed_in();
        {
            let mut w = world.lock().unwrap();
            let g = &mut w.games[0];
            (g.variant, g.status, g.first, g.initial) = ("backgammon".into(), "playing".into(), true, Some("white".into()));
            g.moves = vec!["3-1".into()];
        }
        b.poll().unwrap();
        let sent = || fake.seen().iter().filter(|r| r.path == "/v1/games/g-anna/move").count();
        let before = b.view().game.unwrap().state.unwrap();
        let first = before.steps[0].wire.clone();
        b.play(&first).unwrap();
        let g = b.view().game.unwrap();
        assert_eq!((g.part, g.entries.len(), sent()), (1, 1, 0), "one checker moved, nothing sent");
        assert_ne!(g.state.as_ref().unwrap().board, before.board, "the board shows the checker moved");
        b.step_back();
        assert_eq!(b.view().game.unwrap().part, 0);
        b.play(&first).unwrap();
        let second = b.view().game.unwrap().state.unwrap().steps[0].wire.clone();
        b.play(&second).unwrap();
        assert_eq!(sent(), 1, "the whole turn went to the referee");
        let g = b.view().game.unwrap();
        assert_eq!(g.part, 0);
        let turn = bragd_games::step("backgammon", Some("white"), &["3-1"], &[first.as_str(), second.as_str()]).unwrap().entry.unwrap();
        assert_eq!(world.lock().unwrap().games[0].moves[1], turn, "one entry for the turn");
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
