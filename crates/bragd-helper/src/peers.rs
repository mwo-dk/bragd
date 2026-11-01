//! Peers in the Helper (`bragd-peers`): it listens for the user's other machines only while
//! `[peers] on` is set, on the interface picked or the home network's addresses, and announces
//! itself there by mDNS. Settings → Peers asks it to show a code, pair, unpair, look around and
//! say hello. Its key and Peers are in `peers/` in the Helper's folder.

use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use bragd_core::t;
use bragd_peers::find::{self, Advert, Found, Interface};
use bragd_peers::{Failed, Listening, Node, Peer};

/// How long Look around listens for other Helpers.
const LOOK: Duration = Duration::from_secs(3);

/// Peers as Settings → Peers shows them.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct View {
    pub on: bool,
    /// This machine's name and key.
    pub name: String,
    pub id: String,
    /// The addresses listened on; empty while off.
    pub listening: Vec<String>,
    /// Why it does not listen although on.
    pub error: Option<String>,
    /// The code shown: its digits, seconds and tries left.
    pub code: Option<(String, u64, u32)>,
    pub peers: Vec<PeerView>,
    pub interfaces: Vec<Interface>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerView {
    #[serde(flatten)]
    pub peer: Peer,
    /// Whether it answered just now; `None` when not asked.
    pub online: Option<bool>,
}

#[derive(Default)]
struct Live {
    node: Option<Arc<Node>>,
    listening: Option<Listening>,
    advert: Option<Advert>,
    error: Option<String>,
}

static LIVE: LazyLock<Mutex<Live>> = LazyLock::new(Mutex::default);

fn live() -> MutexGuard<'static, Live> {
    LIVE.lock().unwrap_or_else(|e| e.into_inner())
}

/// This machine as a Peer, its key made the first time.
fn node(l: &mut Live) -> Result<Arc<Node>, String> {
    if let Some(n) = &l.node {
        return Ok(n.clone());
    }
    let dir = crate::folder().ok_or("no data folder")?.join("peers");
    let n = Node::open(&dir, &bragd_peers::machine_name(), crate::VERSION).map_err(|e| e.to_string())?;
    l.node = Some(n.clone());
    Ok(n)
}

/// Listens, or stops, as config.toml's `[peers]` says now.
pub fn apply() {
    let cfg = bragd_core::config::Config::load().unwrap_or_default().peers;
    let mut l = live();
    (l.advert, l.listening, l.error) = (None, None, None);
    if !cfg.on {
        return;
    }
    let n = match node(&mut l) {
        Ok(n) => n,
        Err(e) => return l.error = Some(e),
    };
    let addrs = find::listen_addrs(&cfg.interface, bragd_peers::PORT);
    if addrs.is_empty() {
        l.error = Some(t!("peers.error.no_network"));
        return;
    }
    match n.listen(&addrs) {
        Ok(listening) => {
            // Without mDNS a Peer is still reached by its address.
            l.advert = find::advertise(n.name(), &n.id(), &listening.addrs).map_err(|e| crate::note(format_args!("mDNS: {e}"))).ok();
            l.listening = Some(listening);
        }
        Err(e) => l.error = Some(t!("peers.error.listen", "why" => e)),
    }
}

/// Peers, with whether each answered when `hello` asks them.
pub fn view(hello: bool) -> Result<View, String> {
    // Not locked while the hellos wait.
    let (n, listening, error) = {
        let mut l = live();
        let at = l.listening.as_ref().map(|x| x.addrs.iter().map(ToString::to_string).collect()).unwrap_or_default();
        (node(&mut l)?, at, l.error.clone())
    };
    let peers = n.peers();
    let online: Vec<Option<bool>> = if hello {
        let asked: Vec<_> = peers
            .iter()
            .map(|p| {
                let (n, id) = (n.clone(), p.key.clone());
                std::thread::spawn(move || n.hello(&id).is_ok())
            })
            .collect();
        asked.into_iter().map(|h| Some(h.join().unwrap_or(false))).collect()
    } else {
        vec![None; peers.len()]
    };
    Ok(View {
        on: bragd_core::config::Config::load().unwrap_or_default().peers.on,
        name: n.name().into(),
        id: n.id(),
        listening,
        error,
        code: n.code(),
        // Fresh names and versions after the hellos.
        peers: n.peers().into_iter().zip(online).map(|(peer, online)| PeerView { peer, online }).collect(),
        interfaces: find::interfaces(),
    })
}

fn listening() -> Result<Arc<Node>, String> {
    let mut l = live();
    if l.listening.is_none() {
        return Err(t!("peers.error.off"));
    }
    node(&mut l)
}

/// A code for the other machine to type; it listens only while Peers are on.
pub fn show_code() -> Result<View, String> {
    listening()?.show_code(bragd_peers::CODE_TTL).map_err(|e| e.to_string())?;
    view(false)
}

pub fn hide_code() -> Result<View, String> {
    listening()?.hide_code();
    view(false)
}

fn why(f: Failed, addr: &str) -> String {
    match f {
        Failed::Refused(w) => t!(&format!("peers.error.{}", w.id())),
        Failed::Line(e) => t!("peers.error.unreachable", "addr" => addr, "why" => e),
    }
}

/// Pairs with the machine at `addr` showing `code`.
pub fn pair(addr: &str, code: &str) -> Result<View, String> {
    let n = listening()?;
    n.pair_with(addr, code).map_err(|f| why(f, addr))?;
    view(false)
}

pub fn unpair(id: &str) -> Result<View, String> {
    let n = node(&mut live())?;
    n.unpair(id).map_err(|e| e.to_string())?;
    view(false)
}

/// Other Helpers announcing themselves on the home network, and Peers found at a new address.
pub fn look() -> Result<Vec<Found>, String> {
    let n = node(&mut live())?;
    let found = find::look(LOOK, &n.id())?;
    for f in &found {
        if n.peers().iter().any(|p| p.key == f.key && p.addr != f.addr) {
            let _ = n.moved(&f.key, &f.addr);
        }
    }
    Ok(found)
}

/// Whether the Helper stays when no window is open: it does while Peers are on.
pub fn keeps_helper() -> bool {
    live().listening.is_some()
}
