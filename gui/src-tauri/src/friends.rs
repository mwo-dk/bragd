//! Play with Friends (`bragd_online::bragd_server`): the account on the Bragd server, Friends
//! and Clubs. The Games themselves are the Play online page's, as the server `bragd`
//! ([`crate::online`]). Shown only once a Bragd server address is set and one of its Families is on.

use std::sync::OnceLock;

use bragd_online::bragd_server::{self, BragdServer, FriendsView};
use serde_json::Value;

use crate::help;

type Res<T> = Result<T, String>;

/// The Bragd server for this window; the Play online page's `bragd` is the same one.
pub fn server() -> &'static BragdServer {
    static S: OnceLock<BragdServer> = OnceLock::new();
    S.get_or_init(BragdServer::new)
}

/// Whether the page shows: an address is set, and one of the Families played there is on.
pub fn shown(cfg: &bragd_core::config::Config) -> bool {
    let address = std::env::var(bragd_server::SERVER.url_var).ok().filter(|u| !u.trim().is_empty()).unwrap_or_else(|| cfg.online.server.clone());
    !address.trim().is_empty() && bragd_server::FAMILIES.iter().any(|f| cfg.family_shown(f))
}

fn used() {
    bragd_core::tips::used("friends");
}

/// The page as it stands. The first time, an account whose token is kept is opened.
#[tauri::command(async)]
pub fn friends_view() -> FriendsView {
    // Its error is in the view.
    let _ = bragd_online::Play::open(server());
    server().friends_view()
}

#[tauri::command(async)]
pub fn friends_sign_in(email: String, password: String) -> Res<FriendsView> {
    server().sign_in_email(&email, &password)?;
    used();
    Ok(server().friends_view())
}

#[tauri::command(async)]
pub fn friends_sign_up(email: String, password: String) -> Res<FriendsView> {
    server().sign_up(&email, &password)?;
    Ok(server().friends_view())
}

/// Signs in with `provider` in the browser, and waits up to five minutes for it to come back.
#[tauri::command(async)]
pub fn friends_sign_in_with(provider: String, app: tauri::AppHandle) -> Res<FriendsView> {
    server().sign_in_with(&provider, &|url| help::open(&app, url))?;
    used();
    Ok(server().friends_view())
}

#[tauri::command(async)]
pub fn friends_name(name: String) -> Res<FriendsView> {
    server().set_name(&name)?;
    Ok(server().friends_view())
}

#[tauri::command(async)]
pub fn friends_club_friends(on: bool) -> Res<FriendsView> {
    server().set_club_friends(on)?;
    Ok(server().friends_view())
}

/// Says yes to the Game's Chat, or closes it for both (also *Not now*).
#[tauri::command(async)]
pub fn friends_chat(on: bool) -> Res<()> {
    server().chat(on)?;
    if on {
        bragd_core::tips::used("chat");
    }
    Ok(())
}

/// Keeps the Chat with the Game, if the opponent says so too.
#[tauri::command(async)]
pub fn friends_keep(on: bool) -> Res<()> {
    server().keep_chat(on)
}

/// A Friends action ([`BragdServer::friends`]).
#[tauri::command(async)]
pub fn friends_act(what: String, body: Value) -> Res<FriendsView> {
    server().friends(&what, &body)?;
    used();
    Ok(server().friends_view())
}
