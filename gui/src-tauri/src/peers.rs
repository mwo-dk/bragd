//! Settings → Peers: the Helper's Peers (`bragd-peers`, through `bragd_helper::peers`). The
//! switch and the interface are options saved like any other; then the Helper is told to
//! listen or stop. Showing a code, pairing, unpairing and looking around are the Helper's, and
//! so are the Position store's sync and the shared queue, which this page shows in words.

use bragd_core::t;
use bragd_helper::helper::{Reply, Request};
use bragd_helper::jobs::{Job, Status};
use bragd_helper::peers::View;
use bragd_helper::share::Working;
use bragd_peers::find::Found;
use serde::Serialize;
use tauri::State;

use crate::helper_link::what;
use crate::knowledge::Knowledge;

type Res<T> = Result<T, String>;

/// Settings → Peers: the Helper's view, with the shared queue and each Peer's work in words.
#[derive(Serialize)]
pub struct Page {
    #[serde(flatten)]
    pub view: View,
    pub lines: Vec<Line>,
    /// What each Peer works on, by its key, as it said at the last sync.
    pub working: std::collections::BTreeMap<String, Vec<String>>,
}

/// A job of the shared queue.
#[derive(Serialize)]
pub struct Line {
    pub id: u64,
    pub what: String,
    /// Waiting, on which Peer, or here (for which Peer).
    pub state: String,
    pub count: String,
    /// It can be cancelled here: this machine's own.
    pub ours: bool,
}

fn count(done: u64, total: Option<u64>) -> String {
    match total {
        Some(t) => t!("job.count_of", "done" => done, "total" => t),
        None => t!("job.count", "done" => done),
    }
}

fn line(j: &Job) -> Line {
    let state = match (&j.lent, &j.from, j.status) {
        (Some(l), _, _) => t!("peers.queue.lent", "name" => l.name),
        (_, Some(o), _) => t!("peers.queue.for", "name" => o.name),
        (_, _, Status::Running) => t!("peers.queue.here"),
        _ => t!("peers.queue.waiting"),
    };
    Line { id: j.id, what: what(&j.work), state, count: count(j.done, j.total), ours: j.from.is_none() }
}

fn working(w: &Working) -> String {
    let what = match w.kind.as_str() {
        "analyse" => t!("peers.working.analyse", "about" => w.about),
        "translate" => t!("peers.working.translate"),
        _ => t!("peers.working.other"),
    };
    let what = format!("{what} ({})", count(w.done, w.total));
    match &w.owner {
        Some(name) => t!("peers.working.for", "what" => what, "name" => name),
        None => what,
    }
}

fn page(view: View) -> Page {
    let lines = view.queue.iter().map(line).collect();
    let working = view.peers.iter().map(|p| (p.peer.key.clone(), p.sync.as_ref().map(|s| s.working.iter().map(working).collect()).unwrap_or_default())).collect();
    Page { view, lines, working }
}

fn view(k: &Knowledge, request: &Request) -> Res<Page> {
    match k.link.ask(request)? {
        Reply::Peers { peers } => Ok(page(peers)),
        _ => Err(t!("helper.odd_reply")),
    }
}

/// Takes what is new from every Peer now, rather than at the next minute.
#[tauri::command(async)]
pub fn peers_sync(k: State<'_, Knowledge>) -> Res<Page> {
    view(&k, &Request::PeersSync)
}

/// Peers as they stand; with `hello`, each is asked whether it is there.
#[tauri::command(async)]
pub fn peers_view(hello: bool, k: State<'_, Knowledge>) -> Res<Page> {
    view(&k, &Request::Peers { hello })
}

/// The Helper listens, or stops, as Settings say now.
#[tauri::command(async)]
pub fn peers_apply(k: State<'_, Knowledge>) -> Res<Page> {
    view(&k, &Request::PeersApply)
}

#[tauri::command(async)]
pub fn pair_show(k: State<'_, Knowledge>) -> Res<Page> {
    view(&k, &Request::PairShow)
}

#[tauri::command(async)]
pub fn pair_hide(k: State<'_, Knowledge>) -> Res<Page> {
    view(&k, &Request::PairHide)
}

/// Pairs with the machine at `addr` that shows `code`.
#[tauri::command(async)]
pub fn pair_with(addr: String, code: String, k: State<'_, Knowledge>) -> Res<Page> {
    let v = view(&k, &Request::PairWith { addr, code })?;
    bragd_core::tips::used("peers");
    Ok(v)
}

#[tauri::command(async)]
pub fn peer_unpair(id: String, k: State<'_, Knowledge>) -> Res<Page> {
    view(&k, &Request::Unpair { id })
}

/// Other Helpers announcing themselves on the home network.
#[tauri::command(async)]
pub fn peers_look(k: State<'_, Knowledge>) -> Res<Vec<Found>> {
    match k.link.ask(&Request::PeersLook)? {
        Reply::Found { found } => Ok(found),
        _ => Err(t!("helper.odd_reply")),
    }
}
