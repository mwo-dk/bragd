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

/// Lets the webview's WebRTC carry a Call: on Linux WebKitGTK has it off, and asks before the
/// camera or the microphone opens. Each is granted only while that channel is open for both
/// players ([`BragdServer::call_permits`]). WebView2 and WKWebView ask the user themselves.
pub fn media(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "linux")]
    let _ = window.with_webview(|wv| {
        use webkit2gtk::glib::object::Cast;
        use webkit2gtk::{PermissionRequestExt, SettingsExt, UserMediaPermissionRequest, UserMediaPermissionRequestExt, WebViewExt};
        let view = wv.inner();
        if let Some(s) = WebViewExt::settings(&view) {
            s.set_enable_media_stream(true);
            s.set_enable_webrtc(true);
        }
        view.connect_permission_request(|_, request| {
            let Some(media) = request.downcast_ref::<UserMediaPermissionRequest>() else { return false };
            if server().call_permits(media.is_for_audio_device(), media.is_for_video_device()) {
                request.allow();
            } else {
                request.deny();
            }
            true
        });
    });
    #[cfg(not(target_os = "linux"))]
    let _ = window;
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

/// Deletes the account on the Bragd server, once the user typed their name, and signs out.
#[tauri::command(async)]
pub fn friends_delete_account(confirm: String, keep_games: bool) -> Res<FriendsView> {
    server().delete_account(&confirm, keep_games)?;
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

/// Says yes to a Call's `channel` (`voice` or `camera`), or closes it for both (also *Not now*).
#[tauri::command(async)]
pub fn friends_call_want(channel: String, on: bool) -> Res<()> {
    server().call_want(&channel, on)?;
    if on {
        bragd_core::tips::used("calls");
    }
    Ok(())
}

/// An offer, answer or ICE candidate from the page's WebRTC, for the other machine.
#[tauri::command(async)]
pub fn friends_call_signal(signal: Value) -> Res<()> {
    server().call_signal(signal)
}

/// The other machine's offers, answers and ICE candidates, for the page's WebRTC.
#[tauri::command]
pub fn friends_call_take() -> Vec<Value> {
    server().call_take()
}

/// A Friends action ([`BragdServer::friends`]).
#[tauri::command(async)]
pub fn friends_act(what: String, body: Value) -> Res<FriendsView> {
    server().friends(&what, &body)?;
    used();
    Ok(server().friends_view())
}
