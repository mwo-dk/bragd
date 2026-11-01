//! Online play. Lichess and Lishogi through their Board API (`board`, `session`), OGS through its
//! REST API and its realtime socket (`ogs`). Each signs in over OAuth (PKCE, a loopback
//! redirect) with the token in the OS keychain only, seeks a Game or plays a challenge, and plays
//! it on Bragd's own board with Bragd's rules checking each move before it is sent. A Game's
//! chat is kept only while the user has turned chat on.

pub mod board;
#[cfg(any(test, feature = "fake"))]
pub mod fake;
pub mod game;
pub mod oauth;
pub mod ogs;
pub mod server;
pub mod session;

pub use session::{ChallengeView, ChatLine, GameView, Online, View};

/// What a server lets a Game do beyond moving and resigning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Can {
    pub takeback: bool,
    /// Cancel a challenge the user made.
    pub cancel: bool,
    pub abort: bool,
    /// Offer a draw.
    pub draw: bool,
}

/// A server Bragd plays on, for one window: the Play online page drives it and reads its
/// [`View`] a few times a second. Nothing here blocks for long but signing in.
pub trait Play: Send + Sync {
    fn server(&self) -> &'static server::Server;
    /// Bragd's Variants it plays.
    fn variants(&self) -> Vec<&'static str>;
    /// The time controls a seek or a challenge may ask for: the value sent back, and its label.
    fn times(&self) -> Vec<(String, String)>;
    /// The colours a challenge may ask for, as its API names them: the first to move first.
    fn colours(&self) -> [&'static str; 2];
    fn can(&self) -> Can;
    /// Bragd can sign in there now.
    fn ready(&self) -> bool {
        true
    }
    /// Everything the page shows, now.
    fn view(&self) -> View;
    /// The Game being played is rated, against a person, and over: engine help is locked while
    /// the first two hold and it is not over.
    fn live(&self) -> Option<(bool, bool, bool)>;
    /// Opens the account and its streams when a token is kept and they are not open yet.
    fn open(&self) -> Result<(), String>;
    /// Opens them again after the connection was lost.
    fn reconnect(&self) -> Result<(), String>;
    /// Signs in in the browser: `open` shows the server's page, which comes back to Bragd with
    /// a code that is swapped for a token. The token goes into the OS keychain only.
    fn sign_in(&self, open: &dyn Fn(&str) -> Result<(), String>) -> Result<(), String>;
    fn cancel_sign_in(&self);
    /// Signs out: the token is taken back where the server allows it, and removed from the
    /// keychain.
    fn sign_out(&self) -> Result<(), String>;
    /// Seeks a Game of `variant` (Bragd's key) at `time` (one of [`Play::times`]), rated or
    /// casual. It waits until someone takes it or it is cancelled.
    fn seek(&self, variant: &str, time: &str, rated: bool) -> Result<(), String>;
    fn seek_cancel(&self);
    /// Challenges `user`; `color` is `random` or one of [`Play::colours`].
    fn challenge(&self, user: &str, variant: &str, time: &str, rated: bool, color: &str) -> Result<(), String>;
    /// Accepts, declines or cancels the challenge `id`: `what` is `accept`, `decline` or
    /// `cancel`.
    fn answer(&self, id: &str, what: &str) -> Result<(), String>;
    /// Plays `entry`, once Bragd's rules have found it legal and it is the user's turn.
    fn play(&self, entry: &str) -> Result<(), String>;
    /// Offers or accepts a draw (`yes`), or declines one.
    fn draw(&self, yes: bool) -> Result<(), String>;
    /// Asks for or accepts a takeback (`yes`), or declines one.
    fn takeback(&self, yes: bool) -> Result<(), String>;
    fn resign(&self) -> Result<(), String>;
    fn abort(&self) -> Result<(), String>;
    /// Agrees to the dead stones the server marks while a go Game's score is counted.
    fn accept_score(&self) -> Result<(), String> {
        Err(bragd_core::t!("online.error.no_game"))
    }
    /// Back to the lobby from a Game that is over.
    fn leave(&self);
    /// Turns the chat on or off. Off, no line is kept or shown and none can be sent; on, the
    /// Game's chat so far is fetched. Nothing happens when it is already so.
    fn set_chat(&self, on: bool) -> Result<(), String>;
    /// Says `text` in the Game's chat, to the opponent.
    fn say(&self, text: &str) -> Result<(), String>;
}
