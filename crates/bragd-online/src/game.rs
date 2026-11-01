//! A Lichess Game as the Board API streams it (`gameFull`, then `gameState` after each move),
//! turned into what Bragd needs: its Variant, its record as entries, the clocks, the offers and
//! whether it is over. Pure: no network here.

use serde::Deserialize;

/// Lichess's Variant keys and Bragd's, for the Variants Bragd has the rules of. Lichess's
/// `fromPosition` is standard chess from a set-up position.
pub const VARIANTS: [(&str, &str); 5] = [
    ("standard", "chess"),
    ("chess960", "chess960"),
    ("kingOfTheHill", "kingofthehill"),
    ("racingKings", "racingkings"),
    ("fromPosition", "chess"),
];

/// Bragd's Variant key for Lichess's `key`; none where Bragd has no rules for it (crazyhouse,
/// atomic, antichess, horde, three-check).
pub fn variant(key: &str) -> Option<&'static str> {
    VARIANTS.iter().find(|(lichess, _)| *lichess == key).map(|&(_, bragd)| bragd)
}

/// Lichess's Variant key for Bragd's `key`, for a seek or a challenge; `fromPosition` is never
/// asked for.
pub fn lichess_variant(key: &str) -> Option<&'static str> {
    VARIANTS.iter().find(|(lichess, bragd)| *bragd == key && *lichess != "fromPosition").map(|&(lichess, _)| lichess)
}

/// A Player as Lichess names them: an account, or Lichess's own engine (`aiLevel`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub rating: Option<u32>,
    /// `GM`, `BOT` …
    pub title: Option<String>,
    #[serde(rename = "aiLevel")]
    pub ai_level: Option<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct VariantKey {
    pub key: String,
    pub name: String,
}

/// The Game's state after a move, an offer or its end.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct GameState {
    /// Every move so far in UCI, separated by spaces.
    pub moves: String,
    /// The clocks in milliseconds, as they stood when this was sent.
    pub wtime: u64,
    pub btime: u64,
    pub winc: u64,
    pub binc: u64,
    /// `created`, `started`, `aborted`, `mate`, `resign`, `stalemate`, `timeout`, `draw`,
    /// `outoftime`, `cheat`, `noStart`, `unknownFinish`, `variantEnd` …
    pub status: String,
    /// `white` or `black`, once someone has won.
    pub winner: Option<String>,
    /// White or Black offers a draw.
    pub wdraw: bool,
    pub bdraw: bool,
    /// White or Black asks to take back a move.
    pub wtakeback: bool,
    pub btakeback: bool,
}

/// The first line of a Game's stream: the whole Game.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct GameFull {
    pub id: String,
    pub variant: VariantKey,
    pub rated: bool,
    /// `rapid`, `classical`, `correspondence` …
    pub speed: String,
    pub white: Player,
    pub black: Player,
    /// `startpos`, or the FEN the Game started from.
    #[serde(rename = "initialFen")]
    pub initial_fen: String,
    pub state: GameState,
}

/// A Game being played, as Bragd keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Live {
    pub id: String,
    /// Lichess's name of its Variant, for one Bragd has no rules for.
    pub variant_name: String,
    /// Bragd's Variant key; none when Bragd has no rules for it.
    pub variant: Option<&'static str>,
    /// The FEN it started from; none for the Variant's own start (`startpos`).
    pub initial: Option<String>,
    pub rated: bool,
    pub speed: String,
    pub white: Player,
    pub black: Player,
    /// 0 when the signed-in user plays White, 1 for Black; none when they play neither.
    pub seat: Option<u8>,
    pub state: GameState,
}

impl Live {
    /// The Game of `full`, for the user whose Lichess id is `me` (any case).
    pub fn from_full(full: GameFull, me: &str) -> Live {
        let is_me = |id: &str| !me.is_empty() && id.eq_ignore_ascii_case(me);
        let seat = if is_me(&full.white.id) { Some(0) } else if is_me(&full.black.id) { Some(1) } else { None };
        let initial = Some(full.initial_fen).filter(|fen| !fen.is_empty() && fen != "startpos");
        Live {
            id: full.id,
            variant_name: full.variant.name,
            variant: variant(&full.variant.key),
            initial,
            rated: full.rated,
            speed: full.speed,
            white: full.white,
            black: full.black,
            seat,
            state: full.state,
        }
    }

    /// The state after a `gameState` line.
    pub fn update(&mut self, state: GameState) {
        self.state = state;
    }

    /// The moves so far, as Bragd's entries (UCI, which Bragd's chess entries are).
    pub fn entries(&self) -> Vec<String> {
        self.state.moves.split_whitespace().map(str::to_owned).collect()
    }

    /// Over: any status but `created` and `started`.
    pub fn over(&self) -> bool {
        !matches!(self.state.status.as_str(), "created" | "started")
    }

    /// The other Player: Black when the user plays White, else White.
    pub fn opponent(&self) -> &Player {
        if self.seat == Some(0) { &self.black } else { &self.white }
    }

    /// The other Player is a person: not Lichess's engine (`aiLevel`) and not a `BOT` account.
    pub fn against_person(&self) -> bool {
        let other = self.opponent();
        other.ai_level.is_none() && other.title.as_deref() != Some("BOT")
    }

    /// The seat whose clock runs: the side to move, once both Sides have made their first move
    /// (Lichess starts the clocks then), while the Game is on. White moves when the number of
    /// moves is even.
    pub fn running(&self) -> Option<u8> {
        let moves = self.entries().len();
        if self.over() || moves < 2 { None } else { Some((moves % 2) as u8) }
    }

    /// White's and Black's time left in milliseconds, `elapsed` ms after the last state came:
    /// the running clock is that much lower, never below 0.
    pub fn clocks(&self, elapsed: u64) -> [u64; 2] {
        let mut clocks = [self.state.wtime, self.state.btime];
        if let Some(seat) = self.running() {
            let clock = &mut clocks[seat as usize];
            *clock = clock.saturating_sub(elapsed);
        }
        clocks
    }

    /// The opponent offers a draw, and asks to take back a move.
    pub fn offered(&self) -> (bool, bool) {
        let s = &self.state;
        if self.seat == Some(0) { (s.bdraw, s.btakeback) } else { (s.wdraw, s.wtakeback) }
    }

    /// The user has offered a draw, and asked to take back a move.
    pub fn offering(&self) -> (bool, bool) {
        let s = &self.state;
        match self.seat {
            Some(0) => (s.wdraw, s.wtakeback),
            Some(_) => (s.bdraw, s.btakeback),
            None => (false, false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full(json: &str) -> GameFull {
        serde_json::from_str(json).unwrap()
    }

    const RATED: &str = r#"{"type":"gameFull","id":"abcd1234","rated":true,"variant":{"key":"standard","name":"Standard","short":"Std"},
        "clock":{"initial":600000,"increment":5000},"speed":"rapid","perf":{"name":"Rapid"},"createdAt":1,
        "white":{"id":"anna","name":"Anna","title":null,"rating":1712,"provisional":false},
        "black":{"id":"bragdtester","name":"BragdTester","rating":1500},
        "initialFen":"startpos",
        "state":{"type":"gameState","moves":"e2e4 c7c5 g1f3","wtime":590000,"btime":600000,"winc":5000,"binc":5000,"status":"started","bdraw":false,"wdraw":true}}"#;

    #[test]
    fn game_variants_map_both_ways() {
        assert_eq!(variant("standard"), Some("chess"));
        assert_eq!(variant("chess960"), Some("chess960"));
        assert_eq!(variant("kingOfTheHill"), Some("kingofthehill"));
        assert_eq!(variant("racingKings"), Some("racingkings"));
        assert_eq!(variant("fromPosition"), Some("chess"));
        assert_eq!(variant("crazyhouse"), None);
        assert_eq!(variant("atomic"), None);
        assert_eq!(lichess_variant("chess"), Some("standard"), "never fromPosition");
        assert_eq!(lichess_variant("racingkings"), Some("racingKings"));
        assert_eq!(lichess_variant("shogi"), None);
        assert_eq!(lichess_variant("seirawan"), None);
    }

    #[test]
    fn game_full_gives_the_record_seat_and_opponent() {
        let g = Live::from_full(full(RATED), "BragdTester");
        assert_eq!(g.id, "abcd1234");
        assert_eq!(g.variant, Some("chess"));
        assert_eq!(g.initial, None, "startpos is the Variant's own start");
        assert_eq!(g.seat, Some(1), "the id is matched in any case");
        assert_eq!(g.entries(), ["e2e4", "c7c5", "g1f3"]);
        assert_eq!(g.opponent().name, "Anna");
        assert!(g.rated && g.against_person() && !g.over());
        assert_eq!(g.offered(), (true, false), "White, the opponent, offers a draw");
        assert_eq!(g.offering(), (false, false));
        let w = Live::from_full(full(RATED), "anna");
        assert_eq!((w.seat, w.opponent().name.as_str()), (Some(0), "BragdTester"));
        assert_eq!(w.offering(), (true, false));
        assert_eq!(Live::from_full(full(RATED), "someoneelse").seat, None);
    }

    #[test]
    fn game_from_a_position_keeps_its_fen_and_an_empty_record() {
        let json = RATED
            .replace(r#""key":"standard""#, r#""key":"chess960""#)
            .replace("startpos", "bqnrkrnb/pppppppp/8/8/8/8/PPPPPPPP/BQNRKRNB w KQkq - 0 1")
            .replace("e2e4 c7c5 g1f3", "");
        let g = Live::from_full(full(&json), "anna");
        assert_eq!(g.variant, Some("chess960"));
        assert_eq!(g.initial.as_deref(), Some("bqnrkrnb/pppppppp/8/8/8/8/PPPPPPPP/BQNRKRNB w KQkq - 0 1"));
        assert!(g.entries().is_empty(), "no moves: no entries, not one empty entry");
        let odd = Live::from_full(full(&RATED.replace(r#""key":"standard","name":"Standard""#, r#""key":"crazyhouse","name":"Crazyhouse""#)), "anna");
        assert_eq!((odd.variant, odd.variant_name.as_str()), (None, "Crazyhouse"));
    }

    #[test]
    fn game_against_an_engine_or_a_bot_is_not_against_a_person() {
        let ai = RATED.replace(r#"{"id":"anna","name":"Anna","title":null,"rating":1712,"provisional":false}"#, r#"{"aiLevel":3}"#);
        let g = Live::from_full(full(&ai), "bragdtester");
        assert!(!g.against_person());
        let bot = RATED.replace(r#""title":null"#, r#""title":"BOT""#);
        assert!(!Live::from_full(full(&bot), "bragdtester").against_person());
        let gm = RATED.replace(r#""title":null"#, r#""title":"GM""#);
        assert!(Live::from_full(full(&gm), "bragdtester").against_person());
    }

    #[test]
    fn game_clock_runs_for_the_side_to_move_after_both_first_moves() {
        let mut g = Live::from_full(full(RATED), "bragdtester");
        // Three moves: Black to move, and Black's clock runs.
        assert_eq!(g.running(), Some(1));
        assert_eq!(g.clocks(1500), [590000, 598500]);
        assert_eq!(g.clocks(700_000), [590000, 0], "never below 0");
        g.update(GameState { moves: "e2e4".into(), wtime: 600000, btime: 600000, status: "started".into(), ..Default::default() });
        assert_eq!(g.running(), None, "Black has not moved yet");
        assert_eq!(g.clocks(5000), [600000, 600000]);
        g.update(GameState { moves: "e2e4 e7e5".into(), wtime: 600000, btime: 599000, status: "started".into(), ..Default::default() });
        assert_eq!(g.running(), Some(0));
        g.update(GameState { moves: "e2e4 e7e5".into(), wtime: 600000, btime: 599000, status: "resign".into(), winner: Some("white".into()), ..Default::default() });
        assert!(g.over());
        assert_eq!(g.running(), None, "no clock runs once it is over");
        assert_eq!(g.offered(), (false, false), "the update replaces the offers");
    }

    #[test]
    fn game_takeback_offers_by_side() {
        let mut g = Live::from_full(full(RATED), "bragdtester");
        g.update(GameState { moves: "e2e4".into(), status: "started".into(), btakeback: true, wtakeback: false, ..Default::default() });
        assert_eq!(g.offering(), (false, true));
        assert_eq!(g.offered(), (false, false));
    }
}
