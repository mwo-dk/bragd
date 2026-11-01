//! FIDE's Berger tables as printed in Handbook C.05 Annex 1; source URL at the
//! top of the fixture. Every round of every table, White first, board order kept.

use pairing::round_robin;

const TABLES: &str = include_str!("fixtures/berger-c05-annex1.txt");

/// One printed round: (size, round, boards as (White, Black)).
type Printed = (u32, u32, Vec<(u32, u32)>);

fn printed() -> Vec<Printed> {
    let mut out = Vec::new();
    let mut size = 0;
    for line in TABLES.lines() {
        if let Some(n) = line.strip_prefix("## ") {
            size = n.parse().unwrap();
        } else if let Some(rest) = line.strip_prefix('R') {
            let (r, boards) = rest.split_once(": ").unwrap();
            let boards = boards
                .split_whitespace()
                .map(|b| {
                    let (w, k) = b.split_once('-').unwrap();
                    (w.parse().unwrap(), k.parse().unwrap())
                })
                .collect();
            out.push((size, r.parse().unwrap(), boards));
        }
    }
    out
}

#[test]
fn every_printed_table_is_reproduced() {
    let tables = printed();
    assert_eq!(tables.len(), 3 + 5 + 7 + 9 + 11 + 13 + 15);
    for (size, round, boards) in tables {
        assert_eq!(
            round_robin::table(size, round),
            boards,
            "{size} players, round {round}"
        );
    }
}

#[test]
fn an_odd_field_gives_the_highest_number_a_bye() {
    // 5 players play the 5/6 table; whoever meets 6 sits out.
    for (size, round, boards) in printed().into_iter().filter(|t| t.0 == 6) {
        let p = round_robin::pair(5, 1, false, round).unwrap();
        let (w, b) = boards[0];
        assert_eq!(p.bye, Some(w.min(b)), "{size} round {round}");
        let real: Vec<_> = p.boards.iter().map(|b| (b.white, b.black)).collect();
        assert_eq!(real, boards[1..].to_vec());
    }
}

#[test]
fn a_double_round_robin_can_swap_the_last_two_rounds_as_recommended() {
    let tables: Vec<_> = printed().into_iter().filter(|t| t.0 == 6).collect();
    let rounds = |r: u32| -> Vec<(u32, u32)> {
        let p = round_robin::pair(6, 2, true, r).unwrap();
        p.boards.iter().map(|b| (b.white, b.black)).collect()
    };
    assert_eq!(rounds(4), tables[4].2);
    assert_eq!(rounds(5), tables[3].2);
    for (k, table) in tables.iter().enumerate() {
        let reversed: Vec<_> = table.2.iter().map(|&(w, b)| (b, w)).collect();
        assert_eq!(rounds(6 + k as u32), reversed);
    }
}

/// FIDE Candidates 2026 (8 players, double round-robin, by draw numbers):
/// https://chess-results.com/tnr1384162.aspx?lan=1&art=2&rd=1&turdet=YES
/// The second cycle repeats the first with colours reversed, with no swap.
const CANDIDATES_2026: &str = "\
1: 1-8 2-7 3-6 4-5
2: 8-5 6-4 7-3 1-2
3: 2-8 3-1 4-7 5-6
4: 8-6 7-5 1-4 2-3
5: 3-8 4-2 5-1 6-7
6: 8-7 1-6 2-5 3-4
7: 4-8 5-3 6-2 7-1
8: 8-1 7-2 6-3 5-4
9: 5-8 4-6 3-7 2-1
10: 8-2 1-3 7-4 6-5
11: 6-8 5-7 4-1 3-2
12: 8-3 2-4 1-5 7-6
13: 7-8 6-1 5-2 4-3
14: 8-4 3-5 2-6 1-7";

#[test]
fn the_candidates_2026_double_round_robin_is_reproduced() {
    for line in CANDIDATES_2026.lines() {
        let (r, boards) = line.split_once(": ").unwrap();
        let p = round_robin::pair(8, 2, false, r.parse().unwrap()).unwrap();
        let ours: Vec<String> = p
            .boards
            .iter()
            .map(|b| format!("{}-{}", b.white, b.black))
            .collect();
        assert_eq!(ours.join(" "), boards, "round {r}");
    }
}
