//! What Peers share (`docs/peers/sync.md`): what is new in their Position stores, and their
//! queues of background analysis and model jobs. Libraries, Slices and Settings stay on each
//! machine.
//!
//! - **Sync.** Every `SYNC_EVERY`, and when a Peer gives back a job it did, this Helper takes
//!   what changed in each Peer's store since the last time, a bounded batch at a time, and
//!   merges it by the store's merge rule (`bragd_positions::sync`). Each side takes from the
//!   other, so rows go both ways.
//! - **Shared queue.** A Helper with nothing of its own to do in a lane, inside its limits,
//!   asks its Peers for a job (`Jobs::run_borrowed`). The owner lends it for `jobs::LEASE`; the
//!   Peer renews it as it goes and gives it back done, or not. A lease not renewed runs out and
//!   the job goes back to the owner's queue. Results of analysis come back through the sync;
//!   a translation comes back with the job.
//!
//! The requests go over `Node::call`, so only paired machines, over TLS with pinned keys.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bragd_peers::Node;
use bragd_positions::Store;
use bragd_positions::sync::Changes;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::jobs::{Jobs, Lane, Outcome, Owner, Peers, Status, Work};

/// How often the Helper takes what is new from its Peers.
pub const SYNC_EVERY: Duration = Duration::from_secs(60);
/// A batch of rows, at most: so many, and about so many bytes.
const BATCH_ROWS: usize = 2000;
const BATCH_BYTES: usize = 1024 * 1024;
/// Batches taken from one Peer in one sync; the rest comes at the next.
const BATCHES: u32 = 50;

/// How this machine stands with a Peer, for Settings → Peers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Synced {
    /// When the last sync with it ended, in seconds since 1970, and why it failed if it did.
    pub at: Option<u64>,
    pub error: Option<String>,
    /// Rows taken from it at the last sync, and all told.
    pub got: u64,
    pub rows: u64,
    /// What it works on, as it said at the last sync.
    pub working: Vec<Working>,
    /// It has a language model ready, so it can take model jobs.
    pub model: bool,
}

/// A job a Helper runs now.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Working {
    /// The kind of job (`Work`'s `kind`): `analyse`, `translate`, `fetch`, …
    pub kind: String,
    /// What it is about: a Slice's name, a Game's players; empty when it says nothing.
    pub about: String,
    pub done: u64,
    pub total: Option<u64>,
    /// The Peer it runs for, when it was borrowed.
    pub owner: Option<String>,
}

/// The requests between Helpers.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Ask {
    /// Rows changed after `since` in the store `store` (`Store::changes`).
    Changes { store: String, since: u64 },
    /// A job of `lane` to borrow, if there is one.
    Take { lane: Lane },
    Renew { id: u64, done: u64, total: Option<u64> },
    GiveBack { id: u64, outcome: Outcome },
    /// What it works on, and whether it has a model ready.
    Working,
}

#[derive(Debug, Serialize, Deserialize)]
struct Lent {
    id: u64,
    work: Work,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Busy {
    working: Vec<Working>,
    model: bool,
}

pub struct Share {
    node: Arc<Node>,
    jobs: Arc<Jobs>,
    store: PathBuf,
    /// Peers are on: this machine syncs and borrows only then.
    pub on: AtomicBool,
    seen: Mutex<BTreeMap<String, Synced>>,
}

impl Share {
    /// Sharing `jobs` and the store in its data folder with `node`'s Peers; it answers their
    /// requests from now on.
    pub fn new(node: Arc<Node>, jobs: Arc<Jobs>) -> Arc<Share> {
        let store = jobs.data().join("positions.sqlite");
        let share = Arc::new(Share { node: node.clone(), jobs, store, on: AtomicBool::new(false), seen: Mutex::default() });
        let weak = Arc::downgrade(&share);
        node.on_call(Arc::new(move |peer: &str, body: Value| weak.upgrade().map_or(Value::Null, |s| s.answer(peer, body))));
        share
    }

    fn name_of(&self, peer: &str) -> String {
        self.node.peers().into_iter().find(|p| p.key == peer).map(|p| p.name).unwrap_or_default()
    }

    /// A Peer's request, answered: `{"ok": …}` or `{"error": …}`.
    fn answer(self: &Arc<Self>, peer: &str, body: Value) -> Value {
        let ask: Ask = match serde_json::from_value(body) {
            Ok(a) => a,
            Err(e) => return json!({ "error": e.to_string() }),
        };
        let now = bragd_core::fs::now();
        let ok: Result<Value, String> = match ask {
            Ask::Changes { store, since } => Store::open(&self.store).and_then(|mut s| s.changes(&store, since, BATCH_ROWS, BATCH_BYTES)).map(|c| json!(c)).map_err(|e| e.to_string()),
            Ask::Take { lane } => Ok(json!(self.jobs.lend(peer, &self.name_of(peer), lane, now).map(|(id, work)| Lent { id, work }))),
            Ask::Renew { id, done, total } => Ok(json!(self.jobs.renew(peer, id, done, total, now))),
            Ask::GiveBack { id, outcome } => {
                let analysed = matches!(outcome, Outcome::Done { .. });
                self.jobs.returned(peer, id, outcome);
                // Its results are in that Peer's store now: take them.
                if analysed {
                    let (share, peer) = (self.clone(), peer.to_string());
                    std::thread::spawn(move || share.sync_with(&peer));
                }
                Ok(Value::Null)
            }
            Ask::Working => Ok(json!(Busy { working: self.working(), model: bragd_model::ready(&bragd_core::config::Config::load().unwrap_or_default().model) })),
        };
        match ok {
            Ok(v) => json!({ "ok": v }),
            Err(e) => json!({ "error": e }),
        }
    }

    /// The jobs running here.
    fn working(&self) -> Vec<Working> {
        self.jobs
            .list()
            .into_iter()
            .filter(|j| j.status == Status::Running && j.lent.is_none())
            .map(|j| Working {
                kind: serde_json::to_value(&j.work).ok().and_then(|v| v["kind"].as_str().map(String::from)).unwrap_or_default(),
                about: match &j.work {
                    Work::Analyse { job } => job.about.clone(),
                    _ => String::new(),
                },
                done: j.done,
                total: j.total,
                owner: j.from.map(|o| o.name),
            })
            .collect()
    }

    /// Asks the Peer `peer`: what it answered.
    fn ask(&self, peer: &str, a: &Ask) -> Result<Value, String> {
        let addr = self.node.peers().into_iter().find(|p| p.key == peer).map(|p| p.addr).unwrap_or_default();
        let r = self.node.call(peer, &json!(a)).map_err(|f| crate::peers::why(f, &addr))?;
        if let Some(e) = r.get("error").and_then(Value::as_str) {
            return Err(e.into());
        }
        // A Peer whose Bragd does not share yet answers nothing.
        r.get("ok").cloned().ok_or_else(|| bragd_core::t!("peers.error.too_old"))
    }

    /// Takes what is new in `peer`'s store, then asks what it works on: how it stands now.
    pub fn sync_with(&self, peer: &str) -> Synced {
        let mut s = self.seen.lock().unwrap_or_else(|e| e.into_inner()).get(peer).cloned().unwrap_or_default();
        let taken = || -> Result<(u64, u64, Busy), String> {
            let mut store = Store::open(&self.store).map_err(|e| e.to_string())?;
            let mut got = 0;
            for _ in 0..BATCHES {
                let at = store.pulled(peer).map_err(|e| e.to_string())?;
                let (from, since) = at.map_or((String::new(), 0), |p| (p.store, p.seq));
                let c: Changes = serde_json::from_value(self.ask(peer, &Ask::Changes { store: from, since })?).map_err(|e| e.to_string())?;
                got += store.merge_changes(peer, &c).map_err(|e| e.to_string())?.read;
                if !c.more {
                    break;
                }
            }
            let rows = store.pulled(peer).map_err(|e| e.to_string())?.map_or(0, |p| p.rows);
            let busy = serde_json::from_value(self.ask(peer, &Ask::Working)?).map_err(|e| e.to_string())?;
            Ok((got, rows, busy))
        };
        match taken() {
            Ok((got, rows, busy)) => (s.got, s.rows, s.working, s.model, s.error) = (got, rows, busy.working, busy.model, None),
            Err(e) => s.error = Some(e),
        }
        s.at = Some(bragd_core::fs::now());
        self.seen.lock().unwrap_or_else(|e| e.into_inner()).insert(peer.into(), s.clone());
        s
    }

    /// Syncs with every Peer, while Peers are on.
    pub fn sync_all(&self) {
        if self.on.load(Ordering::Relaxed) {
            for p in self.node.peers() {
                self.sync_with(&p.key);
            }
        }
    }

    /// How this machine stands with `peer`; None before the first sync.
    pub fn synced(&self, peer: &str) -> Option<Synced> {
        self.seen.lock().unwrap_or_else(|e| e.into_inner()).get(peer).cloned()
    }

    /// A Peer has a language model ready, as it said at the last sync.
    pub fn peer_has_model(&self) -> bool {
        self.on.load(Ordering::Relaxed) && self.seen.lock().unwrap_or_else(|e| e.into_inner()).values().any(|s| s.model && s.error.is_none())
    }

    /// Forgets what was taken from `peer`: paired again, it starts from the start.
    pub fn forget(&self, peer: &str) {
        self.seen.lock().unwrap_or_else(|e| e.into_inner()).remove(peer);
        if let Err(e) = Store::open(&self.store).and_then(|mut s| s.forget_peer(peer)) {
            crate::note(format_args!("Position store: {e}"));
        }
    }

    /// Syncs every `SYNC_EVERY` until `quit`.
    pub fn run(&self, quit: &dyn Fn() -> bool) {
        while !quit() {
            self.sync_all();
            let until = std::time::Instant::now() + SYNC_EVERY;
            while !quit() && std::time::Instant::now() < until {
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }
}

impl Peers for Share {
    fn borrow(&self, lane: Lane) -> Option<(Owner, Work)> {
        if !self.on.load(Ordering::Relaxed) {
            return None;
        }
        self.node.peers().into_iter().find_map(|p| {
            let lent: Option<Lent> = serde_json::from_value(self.ask(&p.key, &Ask::Take { lane }).ok()?).ok()?;
            lent.map(|l| (Owner { peer: p.key, name: p.name, id: l.id }, l.work))
        })
    }

    fn renew(&self, owner: &Owner, done: u64, total: Option<u64>) -> bool {
        // An owner that cannot be reached now may be back later: the work goes on.
        self.ask(&owner.peer, &Ask::Renew { id: owner.id, done, total }).map_or(true, |v| v.as_bool() != Some(false))
    }

    fn give_back(&self, owner: &Owner, outcome: Outcome) {
        if let Err(e) = self.ask(&owner.peer, &Ask::GiveBack { id: owner.id, outcome }) {
            // Its lease runs out there and it goes back to that queue.
            crate::note(format_args!("job {} of {}: could not give it back: {e}", owner.id, owner.name));
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::jobs::tests::{CONFIG, analyse_job, fake_analyst, jobs_in_data};
    use crate::net::tests::Recorded;
    use bragd_positions::{Edition, Evaluation, Score};

    struct Machine {
        share: Arc<Share>,
        jobs: Arc<Jobs>,
        _listening: bragd_peers::Listening,
        _dir: crate::tests::Dir,
    }

    impl Machine {
        fn new(name: &str) -> Machine {
            let dir = crate::tests::Dir::new(&format!("share-{name}"));
            let node = Node::open(&dir.0.join("peers"), name, "0.5.0").unwrap();
            let listening = node.listen(&["127.0.0.1:0".parse().unwrap()]).unwrap();
            let jobs = jobs_in_data(&dir.0, &dir.0.join("data"), Recorded::default());
            let share = Share::new(node, jobs.clone());
            share.on.store(true, Ordering::Relaxed);
            let _ = jobs.peers.set(share.clone());
            Machine { share, jobs, _listening: listening, _dir: dir }
        }

        fn store(&self) -> Store {
            Store::open(&self.share.store).unwrap()
        }

        fn addr(&self) -> String {
            self._listening.addrs[0].to_string()
        }

        fn id(&self) -> String {
            self.share.node.id()
        }
    }

    fn pair(a: &Machine, b: &Machine) {
        let code = b.share.node.show_code(bragd_peers::CODE_TTL).unwrap();
        a.share.node.pair_with(&b.addr(), &code).unwrap();
    }

    fn row(key: u64, version: &str, depth: u32) -> Evaluation {
        Evaluation { family: "chess".into(), variant: "chess".into(), key, edition: Edition { engine: "stockfish".into(), version: version.into(), network: None, image: None }, score: Score::Cp(depth as i32), depth, line: "e2e4".into(), time: 1, second: None }
    }

    /// Two Helpers sync both ways, then only what is new.
    #[test]
    fn peers_swap_what_is_new_in_their_stores() {
        let (a, b) = (Machine::new("a"), Machine::new("b"));
        pair(&a, &b);
        a.store().put_all(&[row(1, "Stockfish 17", 20), row(2, "Stockfish 17", 20)]).unwrap();
        b.store().put_all(&[row(1, "Stockfish 17", 25), row(3, "Stockfish 17", 9)]).unwrap();
        let s = a.share.sync_with(&b.id());
        assert_eq!((s.error.as_deref(), s.got, s.rows), (None, 2, 2));
        let s = b.share.sync_with(&a.id());
        assert_eq!((s.error, s.got), (None, 3), "two of A's, and A's copy of B's deeper key 1");
        assert_eq!(a.store().all().unwrap(), b.store().all().unwrap());
        assert_eq!(a.store().evaluations("chess", "chess", 1).unwrap()[0].depth, 25);
        // Nothing new: nothing comes.
        assert_eq!(a.share.sync_with(&b.id()).got, 1, "B's merge of A's rows, once");
        assert_eq!(a.share.sync_with(&b.id()).got, 0);
        b.store().put(&row(4, "Stockfish 17", 9)).unwrap();
        assert_eq!(a.share.sync_with(&b.id()).got, 1);
    }

    /// A job queued on A is done by B, idle: lent, run with B's Engine, given back, and its
    /// results reach A's store through the sync. A Peer that cannot do a job does not get it
    /// again, and a lease not renewed goes back to the queue.
    #[test]
    fn an_idle_peer_does_a_queued_job_and_its_results_come_back() {
        let _config = CONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let (a, b) = (Machine::new("a"), Machine::new("b"));
        pair(&a, &b);
        fake_analyst(&b._dir.0, "share-analyst");
        let k = |n: u64| (u64::from(std::process::id()) << 32) | (1000 + n);
        let id = a.jobs.add(Work::Analyse { job: analyse_job("share-analyst", &[(k(1), 3), (k(2), 1)], "Fake 2", false) });
        assert!(b.jobs.run_borrowed(Lane::Analysis));
        let job = a.jobs.list().into_iter().find(|j| j.id == id).unwrap();
        assert_eq!((job.status, job.done, job.lent), (Status::Done, 2, None));
        let mine = b.jobs.list().into_iter().find(|j| j.from.is_some()).unwrap();
        assert_eq!((mine.status, mine.from.unwrap().id), (Status::Done, id));
        // The owner took the results when the job came back; a sync now makes sure.
        a.share.sync_with(&b.id());
        let e = a.store().shown("chess", "chess", k(1), "share-analyst").unwrap().unwrap();
        assert_eq!((e.edition.version.as_str(), e.depth), ("Fake 2.0", 20));
        assert!(!b.jobs.run_borrowed(Lane::Analysis), "nothing left to lend");

        // Too new an Edition asked: B cannot, and is not asked again.
        let old = a.jobs.add(Work::Analyse { job: analyse_job("share-analyst", &[(k(3), 1)], "Fake 3", false) });
        assert!(b.jobs.run_borrowed(Lane::Analysis));
        assert_eq!(a.jobs.list().into_iter().find(|j| j.id == old).unwrap().status, Status::Queued);
        assert!(!b.jobs.run_borrowed(Lane::Analysis));

        // A Peer that takes a job and vanishes: the lease runs out and the job is queued again.
        let lost = a.jobs.add(Work::Analyse { job: analyse_job("share-analyst", &[(k(4), 1)], "", false) });
        let (owner, _) = b.share.borrow(Lane::Analysis).unwrap();
        assert_eq!(owner.id, lost);
        let now = bragd_core::fs::now();
        assert!(a.jobs.renew(&b.id(), lost, 0, Some(1), now));
        assert!(!a.jobs.renew(&a.id(), lost, 0, Some(1), now), "only the Peer it was lent to");
        a.jobs.expire(now + crate::jobs::LEASE - 1);
        assert_eq!(a.jobs.list().into_iter().find(|j| j.id == lost).unwrap().status, Status::Running);
        a.jobs.expire(now + crate::jobs::LEASE + 1);
        let j = a.jobs.list().into_iter().find(|j| j.id == lost).unwrap();
        assert_eq!((j.status, j.lent), (Status::Queued, None));
        assert!(!b.share.renew(&owner, 1, Some(1)), "the owner wants it back");
        // Cancelled on its owner while lent: the Peer hears so, and the job ends cancelled.
        let (owner, _) = b.share.borrow(Lane::Analysis).unwrap();
        assert!(a.jobs.cancel(owner.id));
        assert!(!b.share.renew(&owner, 0, Some(1)));
        b.share.give_back(&owner, Outcome::Stopped);
        assert_eq!(a.jobs.list().into_iter().find(|j| j.id == owner.id).unwrap().status, Status::Cancelled);
    }

    /// A model job: B, with a model ready, translates for A, which keeps the translation.
    #[test]
    fn a_peers_model_translates_for_this_machine() {
        let _config = CONFIG.lock().unwrap_or_else(|e| e.into_inner());
        let (a, b) = (Machine::new("a"), Machine::new("b"));
        pair(&a, &b);
        let url = fake_model("Good move.");
        let text = format!("Guter Zug {}.", std::process::id());
        let id = a.jobs.add(Work::Translate { text: text.clone(), language: "en-GB".into() });
        assert!(!b.jobs.run_borrowed(Lane::Model), "no model ready on B: it takes no model job");
        set_model(&url);
        let ran = b.jobs.run_borrowed(Lane::Model);
        set_model("");
        assert!(ran);
        let j = a.jobs.list().into_iter().find(|j| j.id == id).unwrap();
        assert_eq!(j.status, Status::Done);
        let kept = bragd_model::Translations::open(&a.jobs.data().join("translations.sqlite")).unwrap().get(&text, "en-GB").unwrap().unwrap();
        assert_eq!((kept.text.as_str(), kept.model.as_str()), ("Good move.", "fake"));
        assert!(bragd_model::Translations::open(&b.jobs.data().join("translations.sqlite")).unwrap().get(&text, "en-GB").unwrap().is_none(), "B keeps none");
    }

    /// The caller holds `CONFIG`.
    fn set_model(url: &str) {
        bragd_core::config::Config::save_value(&["model", "url"], url.into()).unwrap();
        bragd_core::config::Config::save_value(&["model", "name"], if url.is_empty() { "" } else { "fake" }.into()).unwrap();
    }

    /// An OpenAI-compatible server on 127.0.0.1 that answers every chat with `reply`.
    fn fake_model(reply: &str) -> String {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", l.local_addr().unwrap());
        let body = json!({ "choices": [{ "message": { "content": reply } }] }).to_string();
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                use std::io::{BufRead, Read, Write};
                let mut r = std::io::BufReader::new(s);
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    if r.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut sink = vec![0; len];
                let _ = r.read_exact(&mut sink);
                let _ = write!(r.get_mut(), "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            }
        });
        url
    }
}
