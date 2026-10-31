//! The Lichess tablebase API, for chess positions of up to 7 pieces that the tablebase folders
//! do not cover. It is asked only while its switch (`Tablebases::online`) is on: the position
//! is the one thing that leaves the machine.

use serde::{Deserialize, Serialize};

use crate::fetch::Fetch;
use crate::syzygy::{Tablebases, material};

pub const API: &str = "https://tablebase.lichess.ovh/standard";

/// What the tablebase says about a position. `category` is from the side to move's view
/// (`win`, `loss`, `draw`, `cursed-win`, `blessed-loss`, `maybe-win`, `maybe-loss`, `unknown`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Lookup {
    pub category: String,
    pub dtz: Option<i32>,
    pub dtm: Option<i32>,
    #[serde(default)]
    pub checkmate: bool,
    #[serde(default)]
    pub stalemate: bool,
    #[serde(default)]
    pub moves: Vec<TableMove>,
}

/// A move from the position, best first; its `category` is from the opponent's view after it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TableMove {
    pub uci: String,
    pub san: String,
    pub category: String,
    pub dtz: Option<i32>,
    pub dtm: Option<i32>,
    #[serde(default)]
    pub zeroing: bool,
}

/// The URL that asks about `fen`.
pub fn url(fen: &str) -> String {
    format!("{API}?fen={}", fen.split_whitespace().collect::<Vec<_>>().join("_"))
}

pub fn parse(json: &[u8]) -> Result<Lookup, String> {
    serde_json::from_slice(json).map_err(|e| format!("The tablebase answered in a way Bragd does not understand: {e}"))
}

/// What the Lichess tablebase says about `fen`. None when its switch is off, or the position
/// has more than 7 pieces, or the folders cover it already.
pub fn lookup(tb: &Tablebases, fetch: &dyn Fetch, fen: &str) -> Result<Option<Lookup>, String> {
    let Some((w, b)) = material(fen) else { return Ok(None) };
    if !tb.online || w.len() + b.len() > 7 || tb.covers(fen) {
        return Ok(None);
    }
    let body = fetch.get(&url(fen), 1 << 20).map_err(|e| format!("The Lichess tablebase could not be reached: {e}"))?;
    parse(&body).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syzygy::tests::Fake;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    const FEN: &str = "4k3/6KP/8/8/8/8/7p/8 w - - 0 1";

    /// Its answer for `FEN`, recorded on 2026-10-30, its moves cut to three.
    const SAMPLE: &str = r#"{"checkmate":false,"stalemate":false,"variant_win":false,"variant_loss":false,"insufficient_material":false,"dtz":1,"precise_dtz":1,"dtm":17,"dtw":null,"dtc":null,"category":"win","moves":[{"uci":"h7h8q","san":"h8=Q+","zeroing":true,"conversion":true,"checkmate":false,"stalemate":false,"variant_win":false,"variant_loss":false,"insufficient_material":false,"dtz":-2,"precise_dtz":-2,"dtm":-16,"dtw":null,"dtc":null,"category":"loss"},{"uci":"g7g8","san":"Kg8","zeroing":false,"conversion":false,"checkmate":false,"stalemate":false,"variant_win":false,"variant_loss":false,"insufficient_material":false,"dtz":0,"precise_dtz":0,"dtm":0,"dtw":null,"dtc":null,"category":"draw"},{"uci":"g7f6","san":"Kf6","zeroing":false,"conversion":false,"checkmate":false,"stalemate":false,"variant_win":false,"variant_loss":false,"insufficient_material":false,"dtz":1,"precise_dtz":1,"dtm":19,"dtw":null,"dtc":null,"category":"win"}]}"#;

    #[test]
    fn a_recorded_answer_read() {
        let l = parse(SAMPLE.as_bytes()).unwrap();
        assert_eq!((l.category.as_str(), l.dtz, l.dtm, l.checkmate), ("win", Some(1), Some(17), false));
        assert_eq!(l.moves.len(), 3);
        assert_eq!(l.moves[0], TableMove { uci: "h7h8q".into(), san: "h8=Q+".into(), category: "loss".into(), dtz: Some(-2), dtm: Some(-16), zeroing: true });
        assert_eq!(l.moves[1].dtm, Some(0));
        assert!(parse(b"<html>").is_err());
        assert_eq!(url(FEN), "https://tablebase.lichess.ovh/standard?fen=4k3/6KP/8/8/8/8/7p/8_w_-_-_0_1");
    }

    #[test]
    fn asked_only_when_switched_on() {
        let mut files = BTreeMap::new();
        files.insert(url(FEN), SAMPLE.as_bytes().to_vec());
        let fake = Fake { files, asked: Mutex::new(vec![]) };
        let mut tb = Tablebases::default();
        assert_eq!(lookup(&tb, &fake, FEN), Ok(None));
        assert!(fake.asked.lock().unwrap().is_empty(), "nothing leaves the machine while it is off");
        tb.online = true;
        assert_eq!(lookup(&tb, &fake, FEN).unwrap().unwrap().category, "win");
        assert_eq!(lookup(&tb, &fake, "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"), Ok(None), "too many pieces");
        assert_eq!(fake.asked.lock().unwrap().len(), 1);
    }
}
