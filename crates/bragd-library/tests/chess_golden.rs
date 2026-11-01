//! Chess reading, pinned: every move, position, key and start FEN the readers give for the
//! corpus below, as `chess-golden/expected.txt` holds them. Made when the readers moved onto
//! `rules-chess`, to prove that nothing a Library stores changed.
//! `BRAGD_GOLDEN_WRITE=1 cargo test -p bragd-library --test chess_golden` writes it anew.

include!("chess-golden/digest.rs");

#[test]
fn chess_readers_give_what_they_gave() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut got = String::new();
    for f in ["tests/chess-golden/edge.pgn", "src/chess/scid-test/hand.pgn", "src/chess/scid-test/test.si4", "src/chess/scid-test/test.si5", "src/chess/cbh-test/Games.cbh"] {
        got.push_str(&digest_file(&root.join(f)));
    }
    let expected = root.join("tests/chess-golden/expected.txt");
    if std::env::var_os("BRAGD_GOLDEN_WRITE").is_some() {
        std::fs::write(&expected, &got).unwrap();
    }
    let want = std::fs::read_to_string(&expected).unwrap().replace("\r\n", "\n");
    for (n, (g, w)) in got.lines().zip(want.lines()).enumerate() {
        assert_eq!(g, w, "line {}", n + 1);
    }
    assert_eq!(got.lines().count(), want.lines().count());
}
