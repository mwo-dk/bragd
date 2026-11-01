//! Makes a Library from game files, for the screenshots' demo home (docs/screenshots/demo-home.sh):
//!
//!     cargo run -p bragd-library --example demo_library -- <library.bragd> <source> <file>...
//!
//! Each file is imported as coming from `<source>`, and a Slice "White wins" is kept. A file
//! named `backgammon` is no file: two backgammon Games are played here instead, the dice and
//! moves picked by a fixed rule, since no game file a Library reads holds backgammon. No
//! network, nothing else touched.

use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [out, source, files @ ..] = args.as_slice() else {
        return Err("usage: demo_library <library.bragd> <source> <file>...".into());
    };
    if let Some(dir) = Path::new(out).parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut lib = bragd_library::library::Library::open(Path::new(out))?;
    for f in files {
        if f == "backgammon" {
            for (k, (white, black)) in [("Holm, Siri", "Vik, Jon"), ("Vik, Jon", "Dahl, Eva")].into_iter().enumerate() {
                lib.add(&backgammon(k * 2 + 1, white, black), source, &format!("demo#{}", k + 1))?;
            }
            println!("backgammon: 2 added");
            continue;
        }
        let p = lib.import(Path::new(f), source, &|| false, &mut |_| {})?;
        println!("{f}: {} read, {} added, {} merged", p.read, p.added, p.merged);
    }
    // A Slice to show in the sidebar.
    lib.save_slice("White wins", &bragd_library::slice::Query { result: Some("1-0".into()), ..Default::default() })?;
    // And one for each other Family with an Engine whose Games are here, for Analyse in background.
    for (name, family) in [("Go club", "go"), ("Shogi club", "shogi"), ("Backgammon club", "backgammon"), ("Xiangqi club", "xiangqi")] {
        if lib.families()?.iter().any(|f| f == family) {
            lib.save_slice(name, &bragd_library::slice::Query { family: Some(family.into()), ..Default::default() })?;
        }
    }
    println!("{out}: {} Games", lib.count()?);
    Ok(())
}

/// A backgammon Game of 40 entries, its throws and moves picked by `pick` from those the rules
/// list, between `white` and `black`.
fn backgammon(pick: usize, white: &str, black: &str) -> bragd_library::library::NewGame {
    let mut entries: Vec<String> = vec![];
    for i in 0..40 {
        let st = bragd_games::replay("backgammon", None, &entries).expect("the rules' own entries replay");
        let from = if st.throws.is_empty() { st.moves.iter().map(|m| m.wire.clone()).collect() } else { st.throws.clone() };
        let Some(e) = from.get((i * pick + 3) % from.len().max(1)).cloned() else { break };
        entries.push(e);
    }
    let positions: Vec<String> = (0..=entries.len()).map(|k| bragd_games::replay("backgammon", None, &entries[..k]).unwrap().position).collect();
    let history = bragd_games::replay("backgammon", None, &entries).unwrap().history;
    let headers = [("Event", "Club night"), ("Date", "2026.10.14"), ("White", white), ("Black", black)].map(|(k, v)| (k.to_string(), v.to_string())).to_vec();
    bragd_library::library::NewGame::played("backgammon", "backgammon", None, history, &positions, headers, "*")
}
