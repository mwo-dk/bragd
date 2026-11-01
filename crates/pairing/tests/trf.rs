use pairing::Colour;
use pairing::trf::{TrfGame, parse};

const SAMPLE: &str = include_str!("fixtures/sample.trf");

fn game(opponent: Option<u32>, colour: Option<Colour>, result: char) -> TrfGame {
    TrfGame {
        opponent,
        colour,
        result,
    }
}

#[test]
fn reads_the_header_lines() {
    let t = parse(SAMPLE).unwrap();
    assert_eq!(t.name, "Test Open");
    assert_eq!(t.rounds, Some(5));
    assert_eq!(t.initial_colour, Some(Colour::White));
    assert_eq!(t.players.len(), 4);
}

#[test]
fn reads_player_columns() {
    let t = parse(SAMPLE).unwrap();
    let p = &t.players[2];
    assert_eq!(p.start_rank, 3);
    assert_eq!(p.name, "Player Three");
    assert_eq!(p.rating, 2200);
}

#[test]
fn reads_round_cells() {
    let t = parse(SAMPLE).unwrap();
    use Colour::*;
    assert_eq!(
        t.players[0].rounds,
        vec![
            game(Some(3), Some(White), '1'),
            game(Some(2), Some(Black), '='),
            game(None, None, 'H')
        ]
    );
    assert_eq!(
        t.players[1].rounds,
        vec![
            game(Some(4), Some(Black), '+'),
            game(Some(1), Some(White), '='),
            game(Some(3), Some(Black), '0')
        ]
    );
    assert_eq!(t.players[2].rounds[1], game(None, None, 'U'));
    // A blank cell in the middle of a line is a round with nothing in it.
    assert_eq!(
        t.players[3].rounds,
        vec![
            game(Some(2), Some(White), '-'),
            game(None, None, ' '),
            game(None, None, 'Z')
        ]
    );
}

#[test]
fn black1_gives_black() {
    let t = parse(&SAMPLE.replace("XXC white1", "XXC black1")).unwrap();
    assert_eq!(t.initial_colour, Some(Colour::Black));
}

#[test]
fn missing_optional_headers_are_none() {
    let text: String = SAMPLE
        .lines()
        .filter(|l| !l.starts_with("XX"))
        .map(|l| format!("{l}\n"))
        .collect();
    let t = parse(&text).unwrap();
    assert_eq!(t.rounds, None);
    assert_eq!(t.initial_colour, None);
}

#[test]
fn unknown_lines_are_ignored_and_crlf_is_accepted() {
    let text = format!(
        "### a comment\r\n132                                                                                        26/03/01\r\n{}",
        SAMPLE.replace('\n', "\r\n")
    );
    let t = parse(&text).unwrap();
    assert_eq!(t.players.len(), 4);
    assert_eq!(t.players[0].rounds.len(), 3);
}

#[test]
fn a_bad_start_rank_names_its_line() {
    let bad = SAMPLE.replacen("001    1", "001    x", 1);
    let e = parse(&bad).unwrap_err();
    assert_eq!(e.line, 5);
}

#[test]
fn a_bad_colour_names_its_line() {
    let bad = SAMPLE.replacen("     3 w 1", "     3 q 1", 1);
    let e = parse(&bad).unwrap_err();
    assert_eq!(e.line, 5);
}

/// TRF-2026's own example lines (C.02 Appendix A), checked column by column.
const WRITTEN: &str = "012 Club Championship
022 Copenhagen
032 DEN
042 2026/11/05
052 2026/12/17
102 Mette Hansen
192 FIDE_DUTCH_2025
202 BH/C2,BH/C1,BH
XXR 7
XXC white1
001    1 m GM Krause, Christian                 2500 GER     4600000 1960/01/01  3.5    1     2 w 1     3 b =     0 - U
001    2 wWIM Doe, Jane                         2300 DEN     1400000 1990/05/17  2.0    2     1 b 0     0 - H     3 w +
001    3 m    Hansen, Lars                      1850 DEN             1975/03/02  1.0    3     0 - Z     1 w =     2 b -
";

#[test]
fn reads_every_column_of_the_trf_2026_example() {
    let t = parse(WRITTEN).unwrap();
    assert_eq!(
        (t.city.as_str(), t.federation.as_str(), t.start.as_str(), t.end.as_str(), t.arbiter.as_str(), t.kind.as_str(), t.tiebreaks.as_str()),
        ("Copenhagen", "DEN", "2026/11/05", "2026/12/17", "Mette Hansen", "FIDE_DUTCH_2025", "BH/C2,BH/C1,BH")
    );
    let p = &t.players[1];
    assert_eq!((p.sex, p.title.as_str(), p.name.as_str(), p.rating, p.federation.as_str(), p.fide_id.as_str(), p.born.as_str(), p.points, p.rank), ('w', "WIM", "Doe, Jane", 2300, "DEN", "1400000", "1990/05/17", 4, 2));
    assert_eq!(t.players[2].fide_id, "");
    assert_eq!(t.players[0].points, 7);
}

#[test]
fn writes_what_it_reads_column_for_column() {
    assert_eq!(pairing::trf::write(&parse(WRITTEN).unwrap()), WRITTEN);
}

#[test]
fn every_published_report_reads_back_the_same_after_writing() {
    for f in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures")).unwrap() {
        let path = f.unwrap().path();
        if path.extension().is_none_or(|e| e != "trf") {
            continue;
        }
        let t = parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(parse(&pairing::trf::write(&t)).unwrap(), t, "{}", path.display());
    }
}
