//! DSU Monrad tournaments from DSU's own tournament system (turnering.skak.dk),
//! replayed round by round; source URLs at the top of each fixture. Every round
//! from the second on is paired from the history before it and must equal what
//! was played, colours included. Round 1 pairs no. 2 against no. 1, no. 4
//! against no. 3, and so on, by the draw numbers the files keep.

mod common;

fn replay(file: &str) {
    let text = std::fs::read_to_string(format!(
        "{}/tests/fixtures/{file}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let t = common::load(&text);
    let mut wrong = Vec::new();
    for round in 1..=t.rounds.unwrap() as usize {
        let (same, why) = common::compare_with(pairing::monrad::pair, &t, round, false);
        if !same {
            wrong.push(why);
        }
    }
    assert!(wrong.is_empty(), "{file}:\n{}", wrong.join("\n"));
}

#[test]
fn maribo_2024() {
    replay("maribo-2024.trf");
}

#[test]
fn frem_lyn_2025() {
    replay("frem-lyn-2025.trf");
}
