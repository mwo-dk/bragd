//! A Tournament written out: FIDE's report file (TRF-2026), PGN, standings and crosstable as
//! one static HTML page, the pairings of a Round for a second screen or the printer, and a
//! `.bragd-tournament` bundle holding all of them, which reads back into a Club.
//!
//! The Danish Chess Union publishes no file format for rating reports: organisers type the
//! results into its web system (turnering.skak.dk), which takes PGN per Round. So there is no
//! DSU export beyond the PGN.

use std::io::{Read, Write};
use std::path::Path;

use bragd_core::t;
use pairing::Colour;
use pairing::trf::{self, Trf, TrfGame, TrfPlayer};

use crate::club::{Error, Result};
use crate::tournament::{Outcome, Slot, Tournament, half_text};

/// A bundle's extension.
pub const EXT: &str = "bragd-tournament";
/// The most a bundle's `tournament.json` may unpack to: a Tournament of thousands of players
/// is a few megabytes.
const MAX_JSON: u64 = 64 << 20;

/// Start numbers: the order players are seeded in, from 1.
fn numbers(t: &Tournament) -> Vec<u32> {
    let mut order: Vec<usize> = (0..t.players.len()).collect();
    if t.seeds.is_empty() {
        order.sort_by(|&a, &b| t.players[b].rating.cmp(&t.players[a].rating).then(t.players[a].name.to_lowercase().cmp(&t.players[b].name.to_lowercase())));
    } else {
        // A round-robin's numbers are its seeds; anyone left out comes after.
        order.sort_by_key(|p| t.seeds.iter().position(|s| s == p).unwrap_or(usize::MAX));
    }
    let mut n = vec![0; t.players.len()];
    for (k, &p) in order.iter().enumerate() {
        n[p] = k as u32 + 1;
    }
    n
}

/// The Tournament as a FIDE Tournament Report File.
pub fn trf(t: &Tournament) -> String {
    let n = numbers(t);
    let standings = t.standings();
    let rank = |p: usize| standings.iter().find(|s| s.player == p).map_or(0, |s| s.rank);
    let mut players: Vec<TrfPlayer> = (0..t.players.len())
        .map(|p| {
            let pl = &t.players[p];
            let rounds = (0..t.played.len())
                .map(|r| {
                    let colour = |white: bool| Some(if white { Colour::White } else { Colour::Black });
                    match t.slot(r, p) {
                        Slot::Played { opponent, white, points } => TrfGame { opponent: Some(n[opponent]), colour: colour(white), result: ['0', '=', '1'][points as usize] },
                        Slot::Forfeit { opponent, white, points } => TrfGame { opponent: Some(n[opponent]), colour: colour(white), result: if points == 2 { '+' } else { '-' } },
                        Slot::Pending { opponent, white } => TrfGame { opponent: Some(n[opponent]), colour: colour(white), result: ' ' },
                        Slot::PairingBye => TrfGame { opponent: None, colour: None, result: 'U' },
                        Slot::HalfBye => TrfGame { opponent: None, colour: None, result: 'H' },
                        Slot::Absent => TrfGame { opponent: None, colour: None, result: 'Z' },
                    }
                })
                .collect();
            TrfPlayer {
                start_rank: n[p],
                sex: ' ',
                title: pl.title.clone(),
                name: pl.name.clone(),
                rating: pl.rating,
                federation: pl.federation.clone(),
                fide_id: pl.fide_id.clone(),
                born: String::new(),
                points: t.points(p),
                rank: rank(p),
                rounds,
            }
        })
        .collect();
    players.sort_by_key(|p| p.start_rank);
    let report = Trf {
        name: t.name.clone(),
        city: t.place.clone(),
        federation: String::new(),
        start: t.date.replace('-', "/"),
        end: String::new(),
        arbiter: t.arbiter.clone(),
        kind: t.system.trf().into(),
        tiebreaks: t.tiebreaks.iter().map(|tb| tb.code().replace('-', "/")).collect::<Vec<_>>().join(","),
        rounds: Some(t.rounds),
        initial_colour: Some(if t.white_first { Colour::White } else { Colour::Black }),
        players,
    };
    trf::write(&report)
}

fn pgn_text(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// The games played over the board, one PGN game each, with no moves: the Round and board,
/// the players, their ratings and the result. Forfeits are not games and are left out.
pub fn pgn(t: &Tournament) -> String {
    let mut out = String::new();
    for (r, round) in t.played.iter().enumerate() {
        for (b, g) in round.games.iter().enumerate() {
            let result = match g.result {
                Some(o) if !o.played() => continue,
                Some(Outcome::White) => "1-0",
                Some(Outcome::Black) => "0-1",
                Some(_) => "1/2-1/2",
                None => "*",
            };
            let (w, k) = (&t.players[g.white], &t.players[g.black]);
            let mut tags = vec![
                ("Event", t.name.clone()),
                ("Site", if t.place.is_empty() { "?".into() } else { t.place.clone() }),
                ("Date", if t.date.is_empty() { "????.??.??".into() } else { t.date.replace('-', ".") }),
                ("Round", format!("{}.{}", r + 1, b + 1)),
                ("White", w.name.clone()),
                ("Black", k.name.clone()),
                ("Result", result.into()),
            ];
            for (tag, p) in [("WhiteElo", w), ("BlackElo", k)] {
                if p.rating > 0 {
                    tags.push((tag, p.rating.to_string()));
                }
            }
            for (tag, p) in [("WhiteFideId", w), ("BlackFideId", k)] {
                if !p.fide_id.is_empty() {
                    tags.push((tag, p.fide_id.clone()));
                }
            }
            // Any Family's Tournament: what was played, and go's handicap.
            if t.variant != "chess" {
                tags.push(("Variant", t.variant.clone()));
            }
            if g.handicap > 0 {
                tags.push(("Handicap", g.handicap.to_string()));
            }
            for (tag, v) in tags {
                out.push_str(&format!("[{tag} \"{}\"]\n", pgn_text(&v)));
            }
            out.push_str(&format!("\n{result}\n\n"));
        }
    }
    out
}

/// Text made safe to put in HTML.
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            c => o.push(c),
        }
    }
    o
}

const STYLE: &str = "body{font-family:system-ui,sans-serif;margin:1.5rem;color:#111;background:#fff}\
table{border-collapse:collapse;margin:0 0 2rem}th,td{border:1px solid #bbb;padding:.25rem .5rem;text-align:start}\
td.n{text-align:end;font-variant-numeric:tabular-nums}caption{text-align:start;font-weight:bold;padding:.5rem 0}\
h1{margin:0 0 .25rem}p.meta{margin:0 0 1.5rem;color:#444}\
.big table{font-size:2rem;width:100%}.big h1{font-size:2.5rem}button.print{font-size:1rem;padding:.4rem 1rem;margin-bottom:1rem}\
@media print{body{margin:0}button.print{display:none}}";

fn page(t: &Tournament, class: &str, title: &str, body: &str) -> String {
    let lang = bragd_core::i18n::language();
    let dir = if bragd_core::i18n::is_rtl(lang) { "rtl" } else { "ltr" };
    let mut meta: Vec<String> = [&t.date, &t.place, &t.arbiter].iter().filter(|s| !s.is_empty()).map(|s| esc(s)).collect();
    meta.push(esc(&t!(t.system.key())));
    format!(
        "<!DOCTYPE html>\n<html lang=\"{lang}\" dir=\"{dir}\"><head><meta charset=\"utf-8\"><title>{}</title><style>{STYLE}</style></head>\n<body class=\"{class}\"><h1>{}</h1><p class=\"meta\">{}</p>\n{body}</body></html>\n",
        esc(title),
        esc(title),
        meta.join(" · ")
    )
}

/// The standings, with every tiebreak, and the crosstable.
pub fn standings_html(t: &Tournament) -> String {
    let rows = t.standings();
    let n = numbers(t);
    let mut h = format!(
        "<table><caption>{}</caption><thead><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th><th>{}</th>",
        esc(&t!("tournaments.standings")),
        esc(&t!("tournaments.rank")),
        esc(&t!("tournaments.number")),
        esc(&t!("tournaments.player")),
        esc(&t!("clubs.rating")),
        esc(&t!("tournaments.points"))
    );
    for tb in &t.tiebreaks {
        h.push_str(&format!("<th title=\"{}\">{}</th>", esc(&t!(&tb.key())), tb.code()));
    }
    h.push_str("</tr></thead><tbody>");
    for s in &rows {
        let p = &t.players[s.player];
        h.push_str(&format!(
            "<tr><td class=\"n\">{}</td><td class=\"n\">{}</td><td>{}{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td>",
            s.rank,
            n[s.player],
            if p.title.is_empty() { String::new() } else { format!("{} ", esc(&p.title)) },
            esc(&p.name),
            if p.rating > 0 { p.rating.to_string() } else { String::new() },
            s.points_text(t.system)
        ));
        for v in &s.tiebreaks {
            h.push_str(&format!("<td class=\"n\">{}</td>", trim(*v)));
        }
        h.push_str("</tr>");
    }
    h.push_str("</tbody></table>\n");
    // The crosstable: one row per player in standings order, one cell per Round, each giving
    // the opponent's number (places can be shared), the colour and the result.
    h.push_str(&format!(
        "<table><caption>{}</caption><thead><tr><th>{}</th><th>{}</th><th>{}</th>",
        esc(&t!("tournaments.crosstable")),
        esc(&t!("tournaments.rank")),
        esc(&t!("tournaments.number")),
        esc(&t!("tournaments.player"))
    ));
    for r in 1..=t.played.len() {
        h.push_str(&format!("<th>{r}</th>"));
    }
    h.push_str(&format!("<th>{}</th></tr></thead><tbody>", esc(&t!("tournaments.points"))));
    for s in &rows {
        h.push_str(&format!("<tr><td class=\"n\">{}</td><td class=\"n\">{}</td><td>{}</td>", s.rank, n[s.player], esc(&t.players[s.player].name)));
        for r in 0..t.played.len() {
            let c = |w: bool| if w { "w" } else { "b" };
            let cell = match t.slot(r, s.player) {
                Slot::Played { opponent, white, points } => format!("{}{}{}", n[opponent], c(white), ["0", "½", "1"][points as usize]),
                Slot::Forfeit { opponent, white, points } => format!("{}{}{}", n[opponent], c(white), if points == 2 { "+" } else { "−" }),
                Slot::Pending { opponent, white } => format!("{}{}", n[opponent], c(white)),
                Slot::PairingBye => format!("{} 1", esc(&t!("tournaments.bye"))),
                Slot::HalfBye => format!("{} ½", esc(&t!("tournaments.bye"))),
                Slot::Absent => "–".into(),
            };
            h.push_str(&format!("<td>{cell}</td>"));
        }
        h.push_str(&format!("<td class=\"n\">{}</td></tr>", s.points_text(t.system)));
    }
    h.push_str("</tbody></table>\n");
    h.push_str(&rounds_html(t, 0..t.played.len()));
    page(t, "", &t.name, &h)
}

/// A tiebreak value without trailing zeros: `12.5`, `9.25`, `2120`.
fn trim(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn rounds_html(t: &Tournament, rounds: std::ops::Range<usize>) -> String {
    let mut h = String::new();
    for r in rounds {
        let round = &t.played[r];
        h.push_str(&format!(
            "<table><caption>{}</caption><thead><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr></thead><tbody>",
            esc(&t!("tournaments.round_n", "n" => r + 1)),
            esc(&t!("tournaments.board")),
            esc(&t!("tournaments.white")),
            esc(&t!("tournaments.points")),
            esc(&t!("tournaments.result")),
            esc(&t!("tournaments.black")),
            esc(&t!("tournaments.points"))
        ));
        for (b, g) in round.games.iter().enumerate() {
            let (w, k) = (&t.players[g.white], &t.players[g.black]);
            let before = |p: usize| half_text((0..r).map(|x| t.slot(x, p).points()).sum());
            h.push_str(&format!(
                "<tr><td class=\"n\">{}</td><td>{}</td><td class=\"n\">{}</td><td>{}</td><td>{}</td><td class=\"n\">{}</td></tr>",
                b + 1,
                esc(&w.name),
                before(g.white),
                g.result.map_or("", Outcome::text),
                esc(&k.name),
                before(g.black)
            ));
        }
        for (p, what) in round.bye.iter().map(|&p| (p, "1")).chain(round.half.iter().map(|&p| (p, "½"))) {
            h.push_str(&format!("<tr><td></td><td>{}</td><td></td><td>{} {what}</td><td></td><td></td></tr>", esc(&t.players[p].name), esc(&t!("tournaments.bye"))));
        }
        h.push_str("</tbody></table>\n");
    }
    h
}

/// The pairings of Round `round` (from 0) in large type, for a second screen or the printer.
pub fn pairings_html(t: &Tournament, round: usize) -> String {
    let round = round.min(t.played.len().saturating_sub(1));
    let mut body = format!("<button class=\"print\" onclick=\"print()\">{}</button>\n", esc(&t!("tournaments.print")));
    if !t.played.is_empty() {
        body.push_str(&rounds_html(t, round..round + 1));
    }
    page(t, "big", &t.name, &body)
}

/// Writes the bundle: `tournament.json` (what reads back), `tournament.trf`, `games.pgn` and
/// `standings.html`.
pub fn bundle(t: &Tournament, path: &Path) -> Result<()> {
    let json = serde_json::to_vec_pretty(t).map_err(|e| Error::Io(e.to_string()))?;
    bragd_core::fs::replace_with(path, false, |f| {
        let mut z = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        for (name, bytes) in [("tournament.json", json.as_slice()), ("tournament.trf", trf(t).as_bytes()), ("games.pgn", pgn(t).as_bytes()), ("standings.html", standings_html(t).as_bytes())] {
            z.start_file(name, opts).map_err(std::io::Error::other)?;
            z.write_all(bytes)?;
        }
        z.finish().map_err(std::io::Error::other)?;
        Ok(())
    })?;
    Ok(())
}

/// The Tournament in a bundle.
pub fn read_bundle(path: &Path) -> Result<Tournament> {
    let f = std::fs::File::open(path)?;
    let bad = || Error::Invalid("tournaments.error.bundle");
    let mut z = zip::ZipArchive::new(f).map_err(|_| bad())?;
    let entry = z.by_name("tournament.json").map_err(|_| bad())?;
    let mut json = Vec::new();
    entry.take(MAX_JSON + 1).read_to_end(&mut json)?;
    if json.len() as u64 > MAX_JSON {
        return Err(bad());
    }
    let t: Tournament = serde_json::from_slice(&json).map_err(|_| bad())?;
    t.sound().then_some(t).ok_or_else(bad)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tournament::{Player, Status, System};

    fn played() -> Tournament {
        let mut t = Tournament::new("Club <Championship>", System::Dutch { baku: false }, 3, "2026-11-05").unwrap();
        t.place = "Aarhus".into();
        for (n, r, id) in [("Anna \"A\"", 2100, "1401234"), ("Bo", 1900, ""), ("Carl", 2000, "")] {
            let p = t.register(Player { name: n.into(), rating: r, fide_id: id.into(), ..Player::default() }).unwrap();
            t.set_status("Mette", p, Status::Here).unwrap();
        }
        t.pair_next().unwrap();
        t.set_result("Mette", 0, 0, Some(Outcome::Draw), "").unwrap();
        t.pair_next().unwrap();
        t.set_result("Mette", 1, 0, Some(Outcome::WhiteForfeit), "").unwrap();
        t
    }

    #[test]
    fn the_trf_reads_back_with_scores_ranks_and_byes() {
        let t = played();
        let back = trf::parse(&trf(&t)).unwrap();
        assert_eq!((back.name.as_str(), back.start.as_str(), back.city.as_str(), back.rounds), ("Club <Championship>", "2026/11/05", "Aarhus", Some(3)));
        assert_eq!(back.tiebreaks, "BH/C2,BH/C1,BH");
        let anna = &back.players[0];
        assert_eq!((anna.start_rank, anna.rating, anna.fide_id.as_str()), (1, 2100, "1401234"));
        assert_eq!(anna.rounds[0].result, '=');
        // Bo, number 3, had the bye in Round 1.
        assert_eq!((back.players[2].name.as_str(), back.players[2].rounds[0].result), ("Bo", 'U'));
        let total: u32 = back.players.iter().map(|p| p.points).sum();
        assert_eq!(total, (0..3).map(|p| t.points(p)).sum::<u32>());
    }

    #[test]
    fn pgn_holds_the_games_played_and_leaves_out_forfeits() {
        let p = pgn(&played());
        assert_eq!(p.matches("[Event ").count(), 1, "{p}");
        assert!(p.contains("[White \"Anna \\\"A\\\"\"]"), "{p}");
        assert!(p.contains("[Result \"1/2-1/2\"]\n") && p.contains("\n\n1/2-1/2\n"));
        assert!(p.contains("[Date \"2026.11.05\"]") && p.contains("[WhiteFideId \"1401234\"]"));
        assert!(!p.contains("[Variant ") && !p.contains("[Handicap "), "chess, even: {p}");
    }

    #[test]
    fn a_go_tournament_names_its_variant_and_each_handicap() {
        let mut t = Tournament::new("Go evening", System::Dutch { baku: false }, 3, "2026-11-05").unwrap();
        t.set_game("go-19", Some(crate::tournament::Handicap::default()), None).unwrap();
        for (n, r) in [("Anna", 2100), ("Bo", 1800)] {
            let p = t.register(Player { name: n.into(), rating: r, ..Player::default() }).unwrap();
            t.set_status("Mette", p, Status::Here).unwrap();
        }
        t.pair_next().unwrap();
        t.set_result("Mette", 0, 0, Some(Outcome::White), "").unwrap();
        let p = pgn(&t);
        assert!(p.contains("[Variant \"go-19\"]") && p.contains("[Handicap \"3\"]") && p.contains("[Black \"Bo\"]"), "{p}");
    }

    #[test]
    fn html_escapes_every_name() {
        let h = standings_html(&played());
        assert!(h.contains("Club &lt;Championship&gt;"));
        assert!(h.contains("Anna &quot;A&quot;"));
        assert!(!h.contains("<Championship>") && !h.contains("\"A\""));
        let p = pairings_html(&played(), 1);
        assert!(p.contains("class=\"big\"") && p.contains("+--"));
    }

    #[test]
    fn a_bundle_reads_back_and_a_broken_one_is_refused() {
        let dir = std::env::temp_dir().join(format!("bragd-bundle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.bragd-tournament");
        let t = played();
        bundle(&t, &path).unwrap();
        assert_eq!(read_bundle(&path).unwrap(), t);
        // A game naming a player who is not there.
        let mut bad = t.clone();
        bad.played[0].games[0].black = 99;
        let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        z.start_file("tournament.json", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(&serde_json::to_vec(&bad).unwrap()).unwrap();
        z.finish().unwrap();
        assert_eq!(read_bundle(&path), Err(Error::Invalid("tournaments.error.bundle")));
        std::fs::write(&path, b"not a zip").unwrap();
        assert_eq!(read_bundle(&path), Err(Error::Invalid("tournaments.error.bundle")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
