//! The fair-play lock across windows. Each window tells the Helper every few seconds whether a
//! rated online Game against a person is on in it, and hears back whether one is on in another
//! window. A window that stops telling (it crashed, or was killed) holds the lock no longer than
//! [`STALE`] after its last word.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How often a window tells the Helper, at the least. It also tells at once when its own Game
/// starts or ends.
pub const BEAT: Duration = Duration::from_secs(5);
/// How long a window's word holds: three beats, so one late beat does not unlock the others.
pub const STALE: Duration = Duration::from_secs(15);

/// The windows whose locking Game is on, by window id, with when they last said so.
#[derive(Default)]
pub struct Board {
    windows: Mutex<HashMap<String, Instant>>,
}

impl Board {
    /// `window` says whether its locking Game is on, at `now`: whether another window's is.
    pub fn beat(&self, window: &str, locked: bool, now: Instant) -> bool {
        let mut w = self.windows.lock().unwrap_or_else(|e| e.into_inner());
        if locked {
            w.insert(window.to_string(), now);
        } else {
            w.remove(window);
        }
        w.retain(|_, at| now.saturating_duration_since(*at) < STALE);
        w.keys().any(|k| k != window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fairplay_locks_the_other_windows_until_the_game_ends() {
        let b = Board::default();
        let t = Instant::now();
        assert!(!b.beat("1", false, t), "nobody plays");
        assert!(!b.beat("1", true, t), "a window's own Game is its own business");
        assert!(b.beat("2", false, t), "another window sees it");
        assert!(b.beat("2", false, t + BEAT), "still on");
        assert!(!b.beat("1", false, t + BEAT), "over");
        assert!(!b.beat("2", false, t + BEAT), "unlocked at once");
    }

    #[test]
    fn fairplay_window_that_stops_telling_lets_go() {
        let b = Board::default();
        let t = Instant::now();
        b.beat("crashed", true, t);
        assert!(b.beat("2", false, t + STALE - Duration::from_millis(1)));
        assert!(!b.beat("2", false, t + STALE), "its word has run out");
    }
}
