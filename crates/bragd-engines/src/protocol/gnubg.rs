//! GNU Backgammon's own command line (`gnubg -t -q`), over its standard input and output.
//! Both players are made human, so it never moves by itself; a position is set by its
//! `PositionID:MatchID`, the dice by `set dice`, and `hint` lists the best moves with their
//! equity. Its answers have no end marker, so each request is followed by a word it does not
//! know (`bragd-end`): its complaint, which names the word in any language, ends the answer.
//! The sealed image has no network, so GNU Backgammon's socket mode (`external`) is not used.

use super::{About, Best, Dialect, Event, Info, Limit, Position, Protocol, Score, Start, Step};

const READY: &str = "bragd-ready";
const END: &str = "bragd-end";

#[derive(Default)]
pub struct Gnubg {
    /// The best move listed so far.
    first: Option<String>,
}

/// A line of `hint`: `1. Cubeful 2-ply    24/18 13/10    Eq.:  +0.123`. Its place, its move
/// and its equity.
fn candidate(line: &str) -> Option<(usize, String, f32)> {
    let w: Vec<&str> = line.split_whitespace().collect();
    let place = w.first()?.strip_suffix('.')?.parse().ok()?;
    let ply = w.iter().position(|x| x.ends_with("-ply"))?;
    let eq = w.iter().position(|x| *x == "Eq.:")?;
    let equity = w.get(eq + 1)?.parse().ok()?;
    (ply < eq).then(|| (place, w[ply + 1..eq].join(" "), equity))
}

impl Dialect for Gnubg {
    fn protocol(&self) -> Protocol {
        Protocol::Gnubg
    }

    fn hello(&self) -> String {
        format!("set player 0 human\nset player 1 human\nshow version\n{READY}\n")
    }

    fn greeted(&mut self, line: &str, about: &mut About) -> Step {
        if about.name.is_empty() && line.trim_start().starts_with("GNU Backgammon") {
            about.name = line.trim().to_string();
        } else if line.contains(READY) {
            return Step::Ready(String::new());
        }
        Step::More
    }

    fn set_option(&self, name: &str, value: &str) -> String {
        format!("set {name} {value}\n")
    }

    fn position(&mut self, _about: &About, p: &Position) -> String {
        let mut out = String::from("new game\n");
        if let Start::Setup(s) = &p.start {
            let (board, game) = s.split_once(':').unwrap_or((s, ""));
            if !game.is_empty() {
                out.push_str(&format!("set matchid {game}\n"));
            }
            out.push_str(&format!("set board {board}\n"));
        }
        if let Some([a, b]) = p.dice {
            out.push_str(&format!("set dice {a} {b}\n"));
        }
        out
    }

    fn go(&mut self, _about: &About, _limit: Limit, lines: u32) -> String {
        self.first = None;
        format!("hint {}\n{END}\n", lines.max(1))
    }

    fn read(&mut self, line: &str) -> Vec<Event> {
        if line.contains(END) {
            return vec![Event::Best(Best { mv: self.first.take(), ponder: None })];
        }
        let Some((place, mv, equity)) = candidate(line) else { return vec![] };
        if place == 1 {
            self.first = Some(mv.clone());
        }
        vec![Event::Info(Info { depth: None, line: place, score: Some(Score::Equity(equity)), nodes: None, pv: vec![mv] })]
    }

    fn stop(&self) -> String {
        String::new()
    }

    fn quit(&self) -> String {
        // It asks before it leaves a game.
        "quit\ny\n".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gnu_backgammon_hints() {
        let mut d = Protocol::Gnubg.dialect();
        let mut about = About::default();
        assert!(d.hello().ends_with("show version\nbragd-ready\n"));
        for l in ["Player 0 is now human.", "GNU Backgammon 1.07.001", "Copyright (C) 1999--2022 ..."] {
            assert_eq!(d.greeted(l, &mut about), Step::More);
        }
        assert_eq!(d.greeted("Unknown keyword `bragd-ready'.", &mut about), Step::Ready(String::new()));
        assert_eq!(about.name, "GNU Backgammon 1.07.001");
        let mut p = Position::new("backgammon", &[]);
        p.start = Start::Setup("4HPwATDgc/ABMA:cAkAAAAAAAAA".into());
        p.dice = Some([3, 1]);
        assert_eq!(d.position(&about, &p), "new game\nset matchid cAkAAAAAAAAA\nset board 4HPwATDgc/ABMA\nset dice 3 1\n");
        assert_eq!(d.go(&about, Limit::Depth(2), 2), "hint 2\nbragd-end\n");
        let said = "\n    1. Cubeful 2-ply    8/5 6/5                      Eq.:  +0.164\n       0.551 0.176 0.010 - 0.449 0.124 0.005\n        2-ply cubeful prune [world class]\n    2. Cubeful 2-ply    24/23 13/10                  Eq.:  -0.010 ( -0.174)\n       0.497 0.140 0.006 - 0.503 0.138 0.005\nUnknown keyword `bragd-end'.";
        let got: Vec<Event> = said.lines().flat_map(|l| d.read(l)).collect();
        assert_eq!(got.len(), 3, "{got:?}");
        assert_eq!(got[0], Event::Info(Info { depth: None, line: 1, score: Some(Score::Equity(0.164)), nodes: None, pv: vec!["8/5 6/5".into()] }));
        assert!(matches!(&got[1], Event::Info(i) if i.line == 2 && i.score == Some(Score::Equity(-0.01))));
        assert_eq!(got[2], Event::Best(Best { mv: Some("8/5 6/5".into()), ponder: None }));
    }
}
