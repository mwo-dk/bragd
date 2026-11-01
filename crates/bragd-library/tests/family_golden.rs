//! The xiangqi, shogi and draughts readers, pinned: every move, position (whose FEN or SFEN is
//! the key a Library stores) and highlight they give for the corpus in `family-golden/`, as
//! `family-golden/expected.txt` holds them. Made when the readers moved onto the rules crates'
//! positions, to prove that nothing a Library stores changed.
//! `BRAGD_GOLDEN_WRITE=1 cargo test -p bragd-library --test family_golden` writes it anew.

use bragd_library::chess::Table;
use std::fmt::Write;
use std::path::Path;

fn digest_file(path: &Path) -> String {
    let t = Table::open(path).unwrap();
    while !t.progress().done {
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let mut out = format!("== {}\n", path.file_name().unwrap().to_string_lossy());
    for i in 0..t.progress().games as u32 {
        let g = t.game(i).unwrap();
        let _ = writeln!(out, "game {i} start {} error {:?}", g.start, g.error);
        for (j, n) in g.nodes.iter().enumerate() {
            let _ = writeln!(out, "  {j} {} {:?} {:?} {:?} {:?} {}{}", n.san, n.fen, n.from, n.to, n.parent, if n.white { "w" } else { "b" }, n.number);
        }
    }
    out
}

#[test]
fn family_readers_give_what_they_gave() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/family-golden");
    let mut names: Vec<_> = std::fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e != "txt")).collect();
    names.sort();
    let got: String = names.iter().map(|p| digest_file(p)).collect();
    let expected = root.join("expected.txt");
    if std::env::var_os("BRAGD_GOLDEN_WRITE").is_some() {
        std::fs::write(&expected, &got).unwrap();
    }
    let want = std::fs::read_to_string(&expected).unwrap().replace("\r\n", "\n");
    for (n, (g, w)) in got.lines().zip(want.lines()).enumerate() {
        assert_eq!(g, w, "line {}", n + 1);
    }
    assert_eq!(got.lines().count(), want.lines().count());
}
