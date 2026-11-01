//! Play online (`bragd-online`): signing in to Lichess in the browser, the lobby (a seek, the
//! challenges), the Game with its clocks and offers, and its chat. The page asks for the view a
//! few times a second; the streams run on threads of the crate's own. The Game being played is
//! what Analyse's fair-play lock reads (`analyse::locked`).

use std::sync::OnceLock;

use bragd_online::{Online, View};
use serde::Serialize;

use crate::analyse::LiveGame;
use crate::{Ctx, help};

type Res<T> = Result<T, String>;

/// Lichess for this window.
pub fn online() -> &'static Online {
    static O: OnceLock<Online> = OnceLock::new();
    O.get_or_init(|| Online::new(&bragd_online::session::base()))
}

/// The online Game under way, as the fair-play lock sees it.
pub fn live() -> Vec<LiveGame> {
    online().live().map(|(rated, against_person, over)| LiveGame { rated, against_person, over }).into_iter().collect()
}

/// A Variant Lichess plays that the user has on, by Bragd's key and name.
#[derive(Serialize)]
pub struct VariantChoice {
    key: &'static str,
    name: String,
}

/// The page: the session's view, the Variants on offer, and whether chat is on.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    #[serde(flatten)]
    view: View,
    variants: Vec<VariantChoice>,
    chat_on: bool,
}

fn page(ctx: &Ctx) -> Page {
    let cfg = ctx.cfg();
    let variants = bragd_online::game::VARIANTS
        .iter()
        .filter(|(l, _)| *l != "fromPosition")
        .map(|(_, b)| *b)
        .filter(|b| cfg.variant_on(b))
        .map(|key| VariantChoice { key, name: bragd_core::i18n::tr(&format!("variant.{key}"), &[]) })
        .collect();
    Page { view: online().view(), variants, chat_on: cfg.online.chat }
}

/// The page as it stands. The first time, an account whose token is kept is opened.
#[tauri::command(async)]
pub fn online_view(ctx: tauri::State<'_, Ctx>) -> Page {
    let o = online();
    // Its error is in the view.
    let _ = o.open();
    let _ = o.set_chat(ctx.cfg().online.chat);
    page(&ctx)
}

/// Signs in in the browser and waits for it to come back, up to five minutes.
#[tauri::command(async)]
pub fn online_sign_in(app: tauri::AppHandle, ctx: tauri::State<'_, Ctx>) -> Res<Page> {
    online().sign_in(|url| help::open(&app, url))?;
    bragd_core::tips::used("online");
    Ok(page(&ctx))
}

#[tauri::command]
pub fn online_cancel_sign_in() {
    online().cancel_sign_in();
}

#[tauri::command(async)]
pub fn online_sign_out(ctx: tauri::State<'_, Ctx>) -> Res<Page> {
    online().sign_out()?;
    Ok(page(&ctx))
}

#[tauri::command(async)]
pub fn online_reconnect(ctx: tauri::State<'_, Ctx>) -> Res<Page> {
    online().reconnect()?;
    Ok(page(&ctx))
}

#[tauri::command(async)]
pub fn online_seek(variant: String, minutes: u32, increment: u32, rated: bool, ctx: tauri::State<'_, Ctx>) -> Res<Page> {
    online().seek(&variant, minutes, increment, rated)?;
    bragd_core::tips::used("online");
    Ok(page(&ctx))
}

#[tauri::command]
pub fn online_seek_cancel() {
    online().seek_cancel();
}

#[tauri::command(async)]
#[allow(clippy::too_many_arguments, reason = "one argument per field of the form")]
pub fn online_challenge(user: String, variant: String, minutes: u32, increment: u32, rated: bool, color: String, ctx: tauri::State<'_, Ctx>) -> Res<Page> {
    online().challenge(&user, &variant, minutes, increment, rated, &color)?;
    bragd_core::tips::used("online");
    Ok(page(&ctx))
}

/// Accepts, declines or cancels a challenge.
#[tauri::command(async)]
pub fn online_answer(id: String, what: String, ctx: tauri::State<'_, Ctx>) -> Res<Page> {
    online().answer(&id, &what)?;
    Ok(page(&ctx))
}

#[tauri::command(async)]
pub fn online_move(entry: String) -> Res<()> {
    online().play(&entry)
}

/// A Game action: `draw`/`no_draw`, `takeback`/`no_takeback`, `resign`, `abort` or `leave`.
#[tauri::command(async)]
pub fn online_action(what: String) -> Res<()> {
    let o = online();
    match what.as_str() {
        "draw" => o.draw(true),
        "no_draw" => o.draw(false),
        "takeback" => o.takeback(true),
        "no_takeback" => o.takeback(false),
        "resign" => o.resign(),
        "abort" => o.abort(),
        "leave" => {
            o.leave();
            Ok(())
        }
        _ => Err(format!("{what}: no such action")),
    }
}

/// Says a line in the Game's chat.
#[tauri::command(async)]
pub fn online_say(text: String) -> Res<()> {
    online().say(&text)
}
