// Shared by `chess_golden.rs`: one line per thing a chess reader decides, so that any change
// in moves, positions, keys or start FENs shows as a changed line.

use bragd_library::chess::{games, pgn, polyglot, Table};
use bragd_library::library::NewGame;
use std::fmt::Write;
use std::path::Path;

fn digest_game(out: &mut String, text: &str) {
    let g = pgn::parse(text);
    let _ = writeln!(out, "start {} error {:?} result {}", g.start, g.error, g.result);
    for (i, n) in g.nodes.iter().enumerate() {
        let key = n.fen.as_deref().and_then(pgn::board).map(|b| polyglot::key(&b));
        let _ = writeln!(out, "  {i} {} {:?} {:?} {:?} {:?} {}{} {} {:?}", n.san, n.fen, n.from, n.to, n.parent, if n.white { "w" } else { "b" }, n.number, n.depth, key);
    }
    let ng = NewGame::from_chess(&g);
    let _ = writeln!(out, "  library {} {:?} {:?} {} {:?} {:?}", ng.variant, ng.moves, ng.start, ng.start_key, ng.keys, ng.chain().last());
}

/// Every game of the file at `path`, as the table reads it.
pub fn digest_file(path: &Path) -> String {
    let t = Table::open(path).unwrap();
    while !t.progress().done {
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let mut out = format!("== {}\n", path.file_name().unwrap().to_string_lossy());
    for i in 0..t.progress().games as u32 {
        let text = t.text(i).unwrap();
        let _ = writeln!(out, "game {i}");
        if !path.to_string_lossy().ends_with(".pgn") {
            out.push_str(&text);
            out.push('\n');
        }
        digest_game(&mut out, &text);
    }
    games::positions(path, &|| false, |i, keys| {
        let _ = writeln!(out, "positions {i} {keys:?}");
    })
    .unwrap();
    out
}
