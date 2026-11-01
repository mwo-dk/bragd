//! Play online (`bragd-online`): on Lichess, Lishogi and OGS, each for the Families it plays
//! that are on. Signing in in the browser, the lobby (a seek, the challenges), the Game with its
//! clocks and offers, and its chat. The page asks for the view a few times a second; the streams
//! run on threads of the crate's own. The Games being played, in this window and (through the
//! Helper) in every other, are what Analyse's fair-play lock reads (`analyse::locked`).

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use bragd_helper::fairplay;
use bragd_helper::helper::{Reply, Request};
use tauri::Manager;

use bragd_online::{Can, Online, Play, View, board};
use serde::Serialize;

use crate::analyse::LiveGame;
use crate::{Ctx, help};

type Res<T> = Result<T, String>;


/// Every server Bragd plays on, for this window.
fn servers() -> &'static [Box<dyn Play>] {
    static S: OnceLock<Vec<Box<dyn Play>>> = OnceLock::new();
    S.get_or_init(|| vec![Box::new(Online::of(&board::LICHESS)), Box::new(Online::of(&board::LISHOGI)), Box::new(bragd_online::ogs::Ogs::of())])
}

/// The server `id`.
fn server(id: &str) -> Res<&'static dyn Play> {
    servers().iter().find(|s| s.server().id == id).map(|s| &**s).ok_or_else(|| format!("{id}: no such server"))
}

/// The online Games under way in this window, on every server.
fn own() -> Vec<LiveGame> {
    servers().iter().filter_map(|s| s.live()).map(|(rated, against_person, over)| LiveGame { rated, against_person, over }).collect()
}

/// When the Helper last said another window's rated Game against a person is on.
static OTHERS: Mutex<Option<Instant>> = Mutex::new(None);

/// Whether the Helper's word at `said` still holds at `now`: a Helper gone quiet holds the lock
/// no longer than `fairplay::STALE`.
fn others_hold(said: Option<Instant>, now: Instant) -> bool {
    said.is_some_and(|at| now.saturating_duration_since(at) < fairplay::STALE)
}

/// The online Games under way, as the fair-play lock sees it: this window's, and one standing
/// for another window's.
pub fn live() -> Vec<LiveGame> {
    let mut games = own();
    if others_hold(*crate::lock(&OTHERS), Instant::now()) {
        games.push(LiveGame { rated: true, against_person: true, over: false });
    }
    games
}

/// Tells the Helper, shared by every window, whether this window's rated Game against a person
/// is on: at once when that changes, and every `fairplay::BEAT` besides. Its answer locks this
/// window while another window's Game is on.
pub fn start_fairplay(app: &tauri::AppHandle) {
    let app = app.clone();
    let window = std::process::id().to_string();
    std::thread::spawn(move || {
        let mut told: Option<(bool, Instant)> = None;
        loop {
            let mine = crate::analyse::locked(&own());
            if told.is_none_or(|(was, at)| was != mine || at.elapsed() >= fairplay::BEAT) {
                let link = &app.state::<crate::knowledge::Knowledge>().link;
                if let Ok(Reply::FairPlay { others }) = link.ask(&Request::FairPlay { window: window.clone(), locked: mine }) {
                    *crate::lock(&OTHERS) = others.then(Instant::now);
                }
                told = Some((mine, Instant::now()));
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

/// A choice on the page: what is sent back, and its label.
#[derive(Serialize)]
pub struct Choice {
    value: String,
    label: String,
}

/// A server as the Play page and the wizard offer it.
#[derive(Serialize)]
pub struct ServerView {
    pub id: &'static str,
    name: &'static str,
    site: &'static str,
    /// The Family the page draws its board for: the first of its Families that is on.
    family: &'static str,
    /// Bragd can sign in there now ([`Play::ready`]).
    ready: bool,
}

/// The servers that play a Family that is on, each with that Family.
pub fn offered(cfg: &bragd_core::config::Config) -> Vec<ServerView> {
    bragd_online::server::ALL
        .iter()
        .filter_map(|s| s.families.iter().find(|f| cfg.family_shown(f)).map(|f| ServerView { id: s.id, name: s.name, site: s.site, family: f, ready: server(s.id).is_ok_and(|p| p.ready()) }))
        .collect()
}

/// The page: the server, the session's view, the Variants on offer, the times and colours,
/// what the server allows, and whether chat is on.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    #[serde(flatten)]
    view: View,
    server: ServerView,
    terms: &'static str,
    variants: Vec<Choice>,
    times: Vec<Choice>,
    colours: Vec<Choice>,
    can: Can,
    /// Bragd can sign in there: OGS needs Bragd registered with it first.
    ready: bool,
    chat_on: bool,
}

fn page(s: &dyn Play, ctx: &Ctx) -> Page {
    let cfg = ctx.cfg();
    let info = s.server();
    let tr = |k: &str| bragd_core::i18n::tr(k, &[]);
    let variants = s.variants().into_iter().filter(|v| cfg.variant_on(v)).map(|key| Choice { value: key.into(), label: tr(&format!("variant.{key}")) }).collect();
    let times = s.times().into_iter().map(|(value, label)| Choice { value, label }).collect();
    let colours = std::iter::once("random").chain(s.colours()).map(|c| Choice { value: c.into(), label: if c == "random" { tr("online.color.random") } else { tr(&format!("side.{c}")) } }).collect();
    let family = info.families.iter().find(|f| cfg.family_shown(f)).copied().unwrap_or(info.families[0]);
    Page {
        view: s.view(),
        server: ServerView { id: info.id, name: info.name, site: info.site, family, ready: s.ready() },
        terms: info.terms,
        variants,
        times,
        colours,
        can: s.can(),
        ready: s.ready(),
        chat_on: cfg.online.chat,
    }
}

/// Counts a use of `server`'s F1 feature, for its Tip: Lichess's came first, as `online`.
fn used(server: &str) {
    match server {
        "lishogi" => bragd_core::tips::used("lishogi"),
        "ogs" => bragd_core::tips::used("ogs"),
        _ => bragd_core::tips::used("online"),
    }
}

/// The page as it stands. The first time, an account whose token is kept is opened.
#[tauri::command(async)]
pub fn online_view(server: String, ctx: tauri::State<'_, Ctx>) -> Res<Page> {
    let s = self::server(&server)?;
    // Its error is in the view.
    let _ = s.open();
    let _ = s.set_chat(ctx.cfg().online.chat);
    Ok(page(s, &ctx))
}

/// The servers on offer for the Families that are on.
#[tauri::command]
pub fn online_servers(ctx: tauri::State<'_, Ctx>) -> Vec<ServerView> {
    offered(&ctx.cfg())
}

/// Signs in in the browser and waits for it to come back, up to five minutes.
#[tauri::command(async)]
pub fn online_sign_in(server: String, app: tauri::AppHandle, ctx: tauri::State<'_, Ctx>) -> Res<Page> {
    let s = self::server(&server)?;
    s.sign_in(&|url| help::open(&app, url))?;
    used(&server);
    Ok(page(s, &ctx))
}

#[tauri::command]
pub fn online_cancel_sign_in(server: String) -> Res<()> {
    self::server(&server)?.cancel_sign_in();
    Ok(())
}

#[tauri::command(async)]
pub fn online_sign_out(server: String, ctx: tauri::State<'_, Ctx>) -> Res<Page> {
    let s = self::server(&server)?;
    s.sign_out()?;
    Ok(page(s, &ctx))
}

#[tauri::command(async)]
pub fn online_reconnect(server: String, ctx: tauri::State<'_, Ctx>) -> Res<Page> {
    let s = self::server(&server)?;
    s.reconnect()?;
    Ok(page(s, &ctx))
}

#[tauri::command(async)]
pub fn online_seek(server: String, variant: String, time: String, rated: bool, ctx: tauri::State<'_, Ctx>) -> Res<Page> {
    let s = self::server(&server)?;
    s.seek(&variant, &time, rated)?;
    used(&server);
    Ok(page(s, &ctx))
}

#[tauri::command]
pub fn online_seek_cancel(server: String) -> Res<()> {
    self::server(&server)?.seek_cancel();
    Ok(())
}

#[tauri::command(async)]
#[allow(clippy::too_many_arguments, reason = "one argument per field of the form")]
pub fn online_challenge(server: String, user: String, variant: String, time: String, rated: bool, color: String, ctx: tauri::State<'_, Ctx>) -> Res<Page> {
    let s = self::server(&server)?;
    s.challenge(&user, &variant, &time, rated, &color)?;
    used(&server);
    Ok(page(s, &ctx))
}

/// Accepts, declines or cancels a challenge.
#[tauri::command(async)]
pub fn online_answer(server: String, id: String, what: String, ctx: tauri::State<'_, Ctx>) -> Res<Page> {
    let s = self::server(&server)?;
    s.answer(&id, &what)?;
    Ok(page(s, &ctx))
}

#[tauri::command(async)]
pub fn online_move(server: String, entry: String) -> Res<()> {
    self::server(&server)?.play(&entry)
}

/// A Game action: `draw`/`no_draw`, `takeback`/`no_takeback`, `resign`, `abort`, `score` (agree
/// to the dead stones) or `leave`.
#[tauri::command(async)]
pub fn online_action(server: String, what: String) -> Res<()> {
    let o = self::server(&server)?;
    match what.as_str() {
        "draw" => o.draw(true),
        "no_draw" => o.draw(false),
        "takeback" => o.takeback(true),
        "no_takeback" => o.takeback(false),
        "resign" => o.resign(),
        "abort" => o.abort(),
        "score" => o.accept_score(),
        "leave" => {
            o.leave();
            Ok(())
        }
        _ => Err(format!("{what}: no such action")),
    }
}

/// Says a line in the Game's chat.
#[tauri::command(async)]
pub fn online_say(server: String, text: String) -> Res<()> {
    self::server(&server)?.say(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fairplay_another_windows_word_holds_until_it_goes_stale() {
        let now = Instant::now();
        assert!(!others_hold(None, now));
        assert!(others_hold(Some(now), now + fairplay::BEAT));
        assert!(!others_hold(Some(now), now + fairplay::STALE), "a Helper gone quiet does not lock for ever");
    }
}
