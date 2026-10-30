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
    pub state: State,
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
                Some(Lesson { position: *p, state })
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
}
