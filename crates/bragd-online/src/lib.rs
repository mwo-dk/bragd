//! Online play. Phase 1 is Lichess, through its Board API: sign in over OAuth (PKCE, a loopback
//! redirect) with the token in the OS keychain only, seek a Game or play a challenge, and play
//! it on Bragd's own board with Bragd's rules checking each move before it is sent. A Game's
//! chat is kept only while the user has turned chat on.

#[cfg(any(test, feature = "fake"))]
pub mod fake;
pub mod game;
pub mod lichess;
pub mod oauth;
pub mod session;

pub use session::{ChallengeView, ChatLine, GameView, Online, View};
