//! Two Helpers in one process, each with a folder of its own, pairing over 127.0.0.1.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bragd_peers::{CODE_TRIES, CODE_TTL, Failed, Listening, Node, Why};

struct Machine {
    node: Arc<Node>,
    listening: Option<Listening>,
    dir: PathBuf,
}

impl Machine {
    fn new(name: &str) -> Machine {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("bp-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let node = Node::open(&dir, name, "0.5.0").unwrap();
        let listening = Some(node.listen(&["127.0.0.1:0".parse::<SocketAddr>().unwrap()]).unwrap());
        Machine { node, listening, dir }
    }

    fn addr(&self) -> String {
        self.listening.as_ref().unwrap().addrs[0].to_string()
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        self.listening = None;
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn refused(r: Result<impl std::fmt::Debug, Failed>) -> Why {
    match r {
        Err(Failed::Refused(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn paired(a: &Machine, b: &Machine) {
    let code = b.node.show_code(CODE_TTL).unwrap();
    let peer = a.node.pair_with(&b.addr(), &code).unwrap();
    assert_eq!(peer.name, b.node.name());
    assert_eq!(peer.key, b.node.id());
}

#[test]
fn a_code_pairs_two_machines_that_then_say_hello() {
    let (a, b) = (Machine::new("a"), Machine::new("b"));
    paired(&a, &b);
    // Both keep the other's key; the code is gone.
    assert_eq!(a.node.peers().iter().map(|p| p.key.clone()).collect::<Vec<_>>(), [b.node.id()]);
    assert_eq!(b.node.peers().iter().map(|p| p.key.clone()).collect::<Vec<_>>(), [a.node.id()]);
    assert_eq!(b.node.code(), None);
    // Either side says hello and hears the other's name and version.
    let seen = a.node.hello(&b.node.id()).unwrap();
    assert_eq!((seen.name.as_str(), seen.version.as_deref()), ("b", Some("0.5.0")));
    assert_eq!(b.node.peers()[0].version.as_deref(), Some("0.5.0"));
    let back = b.node.hello(&a.node.id()).unwrap();
    assert_eq!(back.name, "a");
    // Kept on disk.
    let again = Node::open(&a.dir, "a", "0.5.0").unwrap();
    assert_eq!(again.peers()[0].key, b.node.id());
}

#[test]
fn a_wrong_code_pairs_nobody_and_uses_a_try() {
    let (a, b) = (Machine::new("a"), Machine::new("b"));
    let code = b.node.show_code(CODE_TTL).unwrap();
    let wrong = format!("{:06}", (code.parse::<u32>().unwrap() + 1) % 1_000_000);
    assert_eq!(refused(a.node.pair_with(&b.addr(), &wrong)), Why::WrongCode);
    assert!(a.node.peers().is_empty() && b.node.peers().is_empty());
    assert_eq!(b.node.code().unwrap().2, CODE_TRIES - 1);
    // The right code still works within its tries.
    a.node.pair_with(&b.addr(), &code).unwrap();
}

#[test]
fn a_code_is_gone_after_its_tries() {
    let (a, b) = (Machine::new("a"), Machine::new("b"));
    let code = b.node.show_code(CODE_TTL).unwrap();
    let wrong = format!("{:06}", (code.parse::<u32>().unwrap() + 1) % 1_000_000);
    for _ in 0..CODE_TRIES {
        assert_eq!(refused(a.node.pair_with(&b.addr(), &wrong)), Why::WrongCode);
    }
    assert_eq!(b.node.code(), None);
    // Not even the right code gets in now: with no code open, the handshake refuses the key.
    assert!(a.node.pair_with(&b.addr(), &code).is_err());
    assert!(b.node.peers().is_empty());
}

#[test]
fn an_expired_code_pairs_nobody() {
    let (a, b) = (Machine::new("a"), Machine::new("b"));
    let code = b.node.show_code(Duration::ZERO).unwrap();
    assert!(a.node.pair_with(&b.addr(), &code).is_err());
    assert!(a.node.peers().is_empty() && b.node.peers().is_empty());
}

#[test]
fn a_used_code_cannot_be_replayed() {
    let (a, b, c) = (Machine::new("a"), Machine::new("b"), Machine::new("c"));
    let code = b.node.show_code(CODE_TTL).unwrap();
    a.node.pair_with(&b.addr(), &code).unwrap();
    // A third machine that saw the code: it was single use.
    assert!(c.node.pair_with(&b.addr(), &code).is_err());
    // And a already paired: the code does not pair it again either.
    assert!(a.node.pair_with(&b.addr(), &code).is_err());
    assert_eq!(b.node.peers().len(), 1);
}

#[test]
fn an_unpaired_key_is_refused() {
    let (a, b) = (Machine::new("a"), Machine::new("b"));
    let mut stranger = Machine::new("s");
    // The stranger still pins b's key, but b has forgotten it without the stranger hearing.
    let code = stranger.node.show_code(CODE_TTL).unwrap();
    b.node.pair_with(&stranger.addr(), &code).unwrap();
    stranger.listening = None;
    std::thread::sleep(Duration::from_millis(300));
    b.node.unpair(&stranger.node.id()).unwrap();
    assert_eq!(stranger.node.peers().len(), 1);
    // Refused in the handshake, before a word is said.
    assert!(matches!(stranger.node.hello(&b.node.id()), Err(Failed::Line(_))));
    // And a key nobody has pinned, with no code open, never gets past TLS.
    assert!(matches!(stranger.node.pair_with(&a.addr(), "000000"), Err(Failed::Line(_))));
    assert!(a.node.peers().is_empty());
}

#[test]
fn a_client_refuses_a_server_with_another_key() {
    let (a, b, c) = (Machine::new("a"), Machine::new("b"), Machine::new("c"));
    paired(&a, &b);
    // b's address now answers with c's key: a's pinned check fails before anything is said.
    a.node.moved(&b.node.id(), &c.addr()).unwrap();
    assert!(matches!(a.node.hello(&b.node.id()), Err(Failed::Line(_))));
    assert!(c.node.peers().is_empty());
}

#[test]
fn either_side_can_unpair() {
    let (a, b) = (Machine::new("a"), Machine::new("b"));
    paired(&a, &b);
    a.node.unpair(&b.node.id()).unwrap();
    // a forgot b, and told b, which forgot a.
    assert!(a.node.peers().is_empty());
    assert!(b.node.peers().is_empty());
    assert!(b.node.hello(&a.node.id()).is_err());
    assert!(a.node.hello(&b.node.id()).is_err());

    // Unpairing a machine that cannot be reached still forgets it here, and it is refused.
    let (c, d) = (Machine::new("c"), Machine::new("d"));
    paired(&c, &d);
    let d_addr = d.addr();
    let d_id = d.node.id();
    let mut d = d;
    d.listening = None;
    std::thread::sleep(Duration::from_millis(300));
    c.node.unpair(&d_id).unwrap();
    assert!(c.node.peers().is_empty());
    d.listening = Some(d.node.listen(&[d_addr.parse().unwrap()]).unwrap());
    assert!(d.node.hello(&c.node.id()).is_err());
}

#[test]
fn a_typed_address_gets_the_peers_port() {
    assert_eq!(bragd_peers::node::resolve("127.0.0.1").unwrap()[0].port(), bragd_peers::PORT);
    assert_eq!(bragd_peers::node::resolve("localhost:4000").unwrap()[0].port(), 4000);
    assert_eq!(bragd_peers::node::resolve("[::1]:4001").unwrap()[0].port(), 4001);
}
