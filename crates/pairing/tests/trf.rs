use pairing::Colour;
use pairing::trf::{TrfGame, parse};

const SAMPLE: &str = include_str!("fixtures/sample.trf");

fn game(opponent: Option<u32>, colour: Option<Colour>, result: char) -> TrfGame {
    TrfGame { opponent, colour, result }
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
        vec![game(Some(3), Some(White), '1'), game(Some(2), Some(Black), '='), game(None, None, 'H')]
    );
    assert_eq!(
        t.players[1].rounds,
        vec![game(Some(4), Some(Black), '+'), game(Some(1), Some(White), '='), game(Some(3), Some(Black), '0')]
    );
    assert_eq!(t.players[2].rounds[1], game(None, None, 'U'));
    // A blank cell in the middle of a line is a round with nothing in it.
    assert_eq!(
        t.players[3].rounds,
        vec![game(Some(2), Some(White), '-'), game(None, None, ' '), game(None, None, 'Z')]
    );
}

#[test]
fn black1_gives_black() {
    let t = parse(&SAMPLE.replace("XXC white1", "XXC black1")).unwrap();
    assert_eq!(t.initial_colour, Some(Colour::Black));
}

#[test]
fn missing_optional_headers_are_none() {
    let text: String = SAMPLE.lines().filter(|l| !l.starts_with("XX")).map(|l| format!("{l}\n")).collect();
    let t = parse(&text).unwrap();
    assert_eq!(t.rounds, None);
    assert_eq!(t.initial_colour, None);
}

#[test]
fn unknown_lines_are_ignored_and_crlf_is_accepted() {
    let text = format!("### a comment\r\n132                                                                                        26/03/01\r\n{}", SAMPLE.replace('\n', "\r\n"));
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
