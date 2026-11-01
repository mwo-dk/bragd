//! Tournaments DSU's tournament system paired as Dansk Schweizer (the Nordic
//! Swiss), replayed round by round;
//! source URLs at the top of each fixture.
//!
//! The rule text is withdrawn (Skakhåndbogen 4.9.3, last in version 15.10,
//! 2019) and DSU's system departs from it in places, so not every round is the
//! same. Each file's pattern records which rounds are (`=`) and which are not
//! (`x`), so that any change in behaviour shows up here. The rule text's own
//! worked examples are tested exactly in `src/nordic.rs`.
//!
//! Left out: events whose round 1 follows the 2007 colour rule (Larsens
//! Nordjyske 2014, and Nørre Åby Open 2023, the one event that says "Nordisk
//! system"), departs from a1-b1 (Midtjysk EMT 2021), or carries
//! placeholder entries and "modereret" rules (Odysseus).

mod common;

fn pattern(file: &str) -> String {
    let text = std::fs::read_to_string(format!(
        "{}/tests/fixtures/{file}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let t = common::load(&text);
    (1..=t.rounds.unwrap() as usize)
        .map(|round| {
            if common::compare_with(pairing::nordic::pair, &t, round, false).0 {
                '='
            } else {
                'x'
            }
        })
        .collect()
}

#[test]
fn midtjysk_emt() {
    assert_eq!(pattern("dansk-schweizer-midtjysk-emt-2018.trf"), "==xxxx=");
    assert_eq!(pattern("dansk-schweizer-midtjysk-emt-2019.trf"), "=xxxx==");
    assert_eq!(pattern("dansk-schweizer-midtjysk-emt-2020.trf"), "====xxx");
    assert_eq!(pattern("dansk-schweizer-midtjysk-emt-2022.trf"), "===xxxx");
}

#[test]
fn midtjysk_hurtigskak_2019() {
    assert_eq!(
        pattern("dansk-schweizer-midtjysk-hurtigskak-2019.trf"),
        "x=x=xxxx"
    );
}

#[test]
fn haderslev_2015() {
    assert_eq!(
        pattern("dansk-schweizer-haderslev-efteraar-2015.trf"),
        "=xxxxxxxx"
    );
}
