//! Every protocol against a real engine: the images `tools/dev/images.sh` builds
//! (`localhost/bragd-<engine>:dev`), run sealed as Bragd runs them. Each test runs only with
//! `BRAGD_DEV_IMAGES=1` and its image built with podman; otherwise it says so and passes.
//!
//!     tools/dev/images.sh scan gnubg katago pikafish fairy-stockfish
//!     BRAGD_DEV_IMAGES=1 cargo test -p bragd-engines --test real_engines -- --nocapture
//!
//! Each asks the engine about a Game played in `bragd-games` and checks that the move it names
//! is a legal move there, which proves the position went over and the answer came back.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use bragd_engines::known::known;
use bragd_engines::options::OptionStore;
use bragd_engines::seam;
use bragd_engines::tools::{self, Runtime};
use bragd_engines::{Engine, Limit, Program, Session, images, think};

/// The runtime and the image built here for `key`, when asked for and there.
fn dev(key: &str) -> Option<(Runtime, String)> {
    if std::env::var("BRAGD_DEV_IMAGES").as_deref() != Ok("1") {
        eprintln!("skipped: BRAGD_DEV_IMAGES=1 is not set");
        return None;
    }
    let rt = tools::container_runtime("podman")?;
    let image = format!("localhost/bragd-{key}:dev");
    if images::pulled(&rt.path, std::slice::from_ref(&image)).is_empty() {
        eprintln!("skipped: {image} is not built (tools/dev/images.sh {key})");
        return None;
    }
    Some((rt, image))
}

/// The engine `key`'s move in the `variant` Game `entries`, checked legal: the entry it is.
fn best(key: &str, variant: &str, entries: &[&str], limit: Limit, network: Option<PathBuf>) -> Option<String> {
    let (rt, image) = dev(key)?;
    let k = known(key).unwrap();
    let program = Program::Container { rt, image, args: vec![], syzygy: None, network };
    let mut e = Engine::start(&program, k.protocol(variant), Duration::from_secs(120), &|| false).unwrap_or_else(|err| panic!("{key} did not start: {err}"));
    assert!(seam::plays(e.about(), e.protocol(), variant), "{key} ({}) does not play {variant}", e.about().name);
    e.configure(&OptionStore::default(), None).unwrap();
    let notation = seam::notation(variant, e.protocol(), &e.about().name);
    let entries: Vec<String> = entries.iter().map(|s| s.to_string()).collect();
    let (p, state) = seam::position(variant, None, &entries, notation).unwrap();
    let a = think(&mut e, &p, limit, 1, &|| false, &mut |_| {}).unwrap();
    let mv = a.best.unwrap_or_else(|| panic!("{key} named no move"));
    let legal = seam::entry(&state, notation, &mv).unwrap_or_else(|| panic!("{key} said {mv}, which is not a legal move of {variant} after {entries:?}"));
    eprintln!("{} ({variant}): {mv} = {}", a.engine, legal.wire);
    Some(legal.wire.clone())
}

/// The wire of the first legal move whose notation is `n`.
fn wire(variant: &str, entries: &[&str], n: &str) -> String {
    let s = bragd_games::replay(variant, None, entries).unwrap();
    s.moves.iter().find(|m| m.notation == n || m.wire == n).unwrap_or_else(|| panic!("no {n} in {:?}", s.moves.iter().map(|m| &m.notation).collect::<Vec<_>>())).wire.clone()
}

#[test]
fn scan_speaks_hub_and_its_captures_come_back() {
    // After 32-28 19-23 White must take: Scan writes it `28x19x23`, the squares taken last.
    let Some(mv) = best("scan", "draughts", &["32-28", "19-23"], Limit::Time(500), None) else { return };
    assert!(mv.starts_with("28"), "the capture is forced: {mv}");
    best("scan", "draughts", &[], Limit::Depth(6), None);
}

#[test]
fn katago_speaks_gtp() {
    best("katago", "go-9", &["e5", "c3"], Limit::Nodes(50), None);
}

#[test]
fn pikafish_speaks_uci_with_ranks_from_zero() {
    let Some((_, _)) = dev("pikafish") else { return };
    // Its network comes with neither the image nor Bragd: downloaded once, here, as the app
    // does it after the licence is agreed to.
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("networks");
    let net = &bragd_engines::network::PIKAFISH;
    let file = if net.path(&dir).is_file() { net.path(&dir) } else { net.download(&dir, &bragd_engines::fetch::Http, &mut |_, _| true).unwrap() };
    let red = wire("xiangqi", &[], "h3e3");
    best("pikafish", "xiangqi", &[red.as_str()], Limit::Depth(8), Some(file));
}

#[test]
fn fairy_stockfish_takes_a_janggi_pass() {
    // Cho passes (its general stays on e2): Han is to move, and Fairy-Stockfish must say so.
    let pass = bragd_games::replay("janggi", None, &[] as &[&str]).unwrap().moves.iter().find(|m| m.notation == "pass").expect("a pass is legal").wire.clone();
    let Some(_) = best("fairy-stockfish", "janggi", &[pass.as_str()], Limit::Depth(6), None) else { return };
    // Then Han passes after Cho's first move: Cho is to move again.
    let first = bragd_games::replay("janggi", None, &[] as &[&str]).unwrap().moves.iter().find(|m| m.notation != "pass").unwrap().wire.clone();
    let han = bragd_games::replay("janggi", None, &[first.as_str()]).unwrap().moves.iter().find(|m| m.notation == "pass").expect("Han may pass").wire.clone();
    best("fairy-stockfish", "janggi", &[first.as_str(), han.as_str()], Limit::Depth(6), None);
}

#[test]
fn gnubg_hints_for_either_side() {
    let first = wire("backgammon", &["3-1"], "8/5 6/5");
    best("gnubg", "backgammon", &["3-1"], Limit::Depth(0), None);
    // Black on roll after White's turn: the Position ID must put the right player first.
    best("gnubg", "backgammon", &["3-1", first.as_str(), "6-4"], Limit::Depth(0), None);
}

/// GNU Backgammon's own Position ID after it plays a turn itself is the one Bragd writes for
/// the same Game: so the order of the two players in the ID is the one GNU Backgammon reads.
#[test]
fn gnubg_position_id_puts_the_players_in_its_order() {
    let Some((rt, image)) = dev("gnubg") else { return };
    let mut c = images::sealed(&rt, &format!("bragd-test-{}", std::process::id()), &image, &[]);
    let mut child = c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let said = "set player 0 human\nset player 1 human\nnew game\nset dice 3 1\nmove 8/5 6/5\nset dice 6 4\nmove 24/18 18/14\nshow board\nquit\ny\n";
    child.stdin.take().unwrap().write_all(said.as_bytes()).unwrap();
    let ids: Vec<String> = BufReader::new(child.stdout.take().unwrap()).lines().map_while(Result::ok).filter_map(|l| l.split("Position ID:").nth(1).map(|s| s.trim().to_string())).collect();
    child.wait().unwrap();
    let theirs = ids.last().expect("GNU Backgammon showed a board");
    let a = wire("backgammon", &["3-1"], "8/5 6/5");
    let b = wire("backgammon", &["3-1", a.as_str(), "6-4"], "24/18 18/14");
    let state = bragd_games::replay("backgammon", None, &["3-1", a.as_str(), "6-4", b.as_str()]).unwrap();
    // Before the next throw: White on roll, nothing thrown yet.
    assert_eq!(&seam::gnubg_position_id(&state).unwrap(), theirs);
}
