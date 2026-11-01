//! Settings → Peers: the Helper's Peers (`bragd-peers`, through `bragd_helper::peers`). The
//! switch and the interface are options saved like any other; then the Helper is told to
//! listen or stop. Showing a code, pairing, unpairing and looking around are the Helper's.

use bragd_helper::helper::{Reply, Request};
use bragd_helper::peers::View;
use bragd_peers::find::Found;
use tauri::State;

use crate::knowledge::Knowledge;

type Res<T> = Result<T, String>;

fn view(k: &Knowledge, request: &Request) -> Res<View> {
    match k.link.ask(request)? {
        Reply::Peers { peers } => Ok(peers),
        _ => Err(bragd_core::t!("helper.odd_reply")),
    }
}

/// Peers as they stand; with `hello`, each is asked whether it is there.
#[tauri::command(async)]
pub fn peers_view(hello: bool, k: State<'_, Knowledge>) -> Res<View> {
    view(&k, &Request::Peers { hello })
}

/// The Helper listens, or stops, as Settings say now.
#[tauri::command(async)]
pub fn peers_apply(k: State<'_, Knowledge>) -> Res<View> {
    view(&k, &Request::PeersApply)
}

#[tauri::command(async)]
pub fn pair_show(k: State<'_, Knowledge>) -> Res<View> {
    view(&k, &Request::PairShow)
}

#[tauri::command(async)]
pub fn pair_hide(k: State<'_, Knowledge>) -> Res<View> {
    view(&k, &Request::PairHide)
}

/// Pairs with the machine at `addr` that shows `code`.
#[tauri::command(async)]
pub fn pair_with(addr: String, code: String, k: State<'_, Knowledge>) -> Res<View> {
    let v = view(&k, &Request::PairWith { addr, code })?;
    bragd_core::tips::used("peers");
    Ok(v)
}

#[tauri::command(async)]
pub fn peer_unpair(id: String, k: State<'_, Knowledge>) -> Res<View> {
    view(&k, &Request::Unpair { id })
}

/// Other Helpers announcing themselves on the home network.
#[tauri::command(async)]
pub fn peers_look(k: State<'_, Knowledge>) -> Res<Vec<Found>> {
    match k.link.ask(&Request::PeersLook)? {
        Reply::Found { found } => Ok(found),
        _ => Err(bragd_core::t!("helper.odd_reply")),
    }
}
