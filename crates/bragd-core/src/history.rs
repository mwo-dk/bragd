//! Back: the places a window has left, newest last. A place is the page with what is open on it
//! (its tab, a Game, Tournament, League, Library, Slice or player) and how far it was scrolled.
//! The list is bounded and lives only as long as the window: it is never saved.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// The most places a window remembers; the oldest goes first.
pub const BOUND: usize = 50;

/// Where a window was: what it showed, as the window names it, and its scroll in pixels.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub view: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub scroll: f64,
}

/// One window's way back.
#[derive(Debug, Default)]
pub struct History {
    behind: VecDeque<Place>,
}

impl History {
    /// The window goes from `from` to `to`. `from` is kept unless `to` shows the same thing
    /// (a scroll is not a move). Returns whether Back has somewhere to go.
    pub fn go(&mut self, from: Place, to: &Place) -> bool {
        if from.view != to.view {
            self.behind.push_back(from);
            if self.behind.len() > BOUND {
                self.behind.pop_front();
            }
        }
        self.can_back()
    }

    /// Back: the place to return to, or none. Where the window
    /// was is dropped, so going elsewhere after Back starts a new way back from there.
    pub fn back(&mut self) -> Option<Place> {
        self.behind.pop_back()
    }

    pub fn can_back(&self) -> bool {
        !self.behind.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(page: &str) -> Place {
        let mut view = serde_json::Map::new();
        view.insert("kind".into(), page.into());
        Place { view, scroll: 0.0 }
    }

    fn page(p: Option<Place>) -> Option<String> {
        p.map(|p| p.view["kind"].as_str().unwrap().to_owned())
    }

    #[test]
    fn back_returns_where_the_window_was_with_its_scroll() {
        let mut h = History::default();
        assert!(!h.can_back(), "a new window has nowhere to go back to");
        let mut games = at("games");
        games.scroll = 420.0;
        assert!(h.go(games.clone(), &at("help")));
        assert_eq!(h.back(), Some(games));
        assert!(!h.can_back());
        assert_eq!(h.back(), None);
    }

    #[test]
    fn staying_on_the_same_view_is_not_a_move() {
        let mut h = History::default();
        let mut scrolled = at("games");
        scrolled.scroll = 10.0;
        assert!(!h.go(at("games"), &scrolled));
    }

    #[test]
    fn going_elsewhere_after_back_drops_where_back_left() {
        let mut h = History::default();
        h.go(at("a"), &at("b"));
        h.go(at("b"), &at("c"));
        assert_eq!(page(h.back()), Some("b".into()));
        h.go(at("b"), &at("d"));
        assert_eq!(page(h.back()), Some("b".into()), "d goes back to b, not c");
        assert_eq!(page(h.back()), Some("a".into()));
        assert_eq!(h.back(), None);
    }

    #[test]
    fn the_oldest_place_goes_past_the_bound() {
        let mut h = History::default();
        for i in 0..=BOUND {
            h.go(at(&i.to_string()), &at("next"));
        }
        let mut left = 0;
        let mut last = None;
        while let Some(p) = h.back() {
            left += 1;
            last = page(Some(p));
        }
        assert_eq!(left, BOUND);
        assert_eq!(last, Some("1".into()), "place 0 was dropped");
    }
}
