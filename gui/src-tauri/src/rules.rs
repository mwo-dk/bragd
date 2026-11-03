//! The Rules item of each Family: its pages under `docs/rules`, compiled in and turned into
//! HTML, and its teaching positions from `bragd_games::positions`.

use bragd_games::positions::{POSITIONS, Position};
use bragd_games::{Family, State, Variant};
use serde::Serialize;

macro_rules! doc {
    ($file:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/rules/", $file))
    };
}

/// Each Family's pages, in reading order.
fn pages(family: Family) -> &'static [&'static str] {
    match family {
        Family::Chess => &[doc!("chess-variants.md"), doc!("seirawan.md"), doc!("capablanca.md")],
        Family::Xiangqi => &[doc!("xiangqi.md"), doc!("janggi.md")],
        Family::Shogi => &[doc!("shogi.md")],
        Family::Go => &[doc!("go.md")],
        Family::Draughts => &[doc!("draughts.md")],
        Family::Tafl => &[doc!("tafl-sizes.md")],
        Family::Backgammon => &[doc!("backgammon.md")],
        Family::Ur | Family::Senet => &[doc!("ur-and-senet-rules.md")],
    }
}

#[derive(Serialize)]
pub struct Page {
    pub html: String,
    pub positions: Vec<Lesson>,
}

/// A teaching position and the board it shows.
#[derive(Serialize)]
pub struct Lesson {
    pub position: Position,
    /// Its title and text in the current language.
    pub title: String,
    pub text: String,
    pub state: State,
    /// The move the text names, written as the Family's move list writes it (`Rd8`, not `d1d8`).
    pub answer: Option<String>,
}

fn html(markdown: &str) -> String {
    let mut out = String::new();
    let parser = pulldown_cmark::Parser::new_ext(markdown, pulldown_cmark::Options::ENABLE_TABLES);
    pulldown_cmark::html::push_html(&mut out, parser);
    out
}

pub fn page(family: Family) -> Page {
    Page {
        html: pages(family).iter().map(|md| html(md)).collect(),
        positions: POSITIONS
            .iter()
            .filter(|p| Variant::from_key(p.variant).is_some_and(|v| v.family() == family))
            .filter_map(|p| {
                let (initial, line) = p.start();
                let state = bragd_games::replay(p.variant, initial, line).ok()?;
                let answer = p.answer.map(|a| {
                    bragd_games::apply(p.variant, initial, line, a)
                        .ok()
                        .and_then(|after| after.written.last().cloned())
                        .unwrap_or_else(|| a.to_string())
                });
                let text = |part: &str| bragd_core::i18n::tr(&format!("position.{}.{part}", p.key), &[]);
                Some(Lesson { position: *p, title: text("title"), text: text("text"), state, answer })
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_family_has_a_page_and_its_positions_play() {
        for family in Family::ALL {
            let page = page(family);
            assert!(page.html.contains("<h1>"), "{family:?}");
            let want = POSITIONS
                .iter()
                .filter(|p| Variant::from_key(p.variant).is_some_and(|v| v.family() == family))
                .count();
            assert_eq!(page.positions.len(), want, "{family:?}: a teaching position does not play");
        }
    }

    /// The answer reads as the move list writes it, not in the engine's wire form.
    #[test]
    fn a_lesson_names_its_move_as_the_move_list_writes_it() {
        let chess = page(Family::Chess);
        let mate = chess.positions.iter().find(|l| l.position.answer == Some("d1d8")).expect("the back-rank mate");
        assert_eq!(mate.answer.as_deref(), Some("Rd8#"));
    }

    /// A lesson's title and text come from the locales, so they read in the language chosen.
    #[test]
    fn a_lesson_takes_its_texts_from_the_locales() {
        let chess = page(Family::Chess);
        let mate = chess.positions.iter().find(|l| l.position.key == "chess.back-rank-mate").expect("the back-rank mate");
        assert_eq!(mate.title, "The back-rank mate");
        assert!(mate.text.starts_with("White plays Rd8"), "{}", mate.text);
    }
}
