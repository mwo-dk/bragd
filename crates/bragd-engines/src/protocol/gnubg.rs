//! GNU Backgammon's own command line (`gnubg -t -q`), over its standard input and output.
//! Both players are made human, so it never moves by itself; a position is set by its
//! `PositionID:MatchID`, the dice by `set dice`, and `hint` lists the best moves with their
//! equity. A position before the roll has no dice: `eval` gives its cube-less equity for the
//! player on roll, the deepest of its plies, and no move. Its answers have no end marker, so its prompt is set to `bragd>` and each request is
//! followed by `show prompt`: its answer, which quotes the prompt (`` `bragd>' ``) in any
//! language, ends the answer. A word it does not know would not do: it complains on standard
//! error, which Bragd does not read. Every line it writes after a command starts with the
//! prompt, which is taken off before the line is read.
//! The sealed image has no network, so GNU Backgammon's socket mode (`external`) is not used.

use super::{About, Best, Dialect, Event, Info, Limit, Position, Protocol, Score, Start, Step};

/// Its prompt, and how `show prompt` and `set prompt` quote it: that line ends an answer.
const PROMPT: &str = "bragd>";
const END: &str = "`bragd>'";

/// `line` without the prompts in front of it.
fn unprompted(line: &str) -> &str {
    let mut l = line;
    while let Some(rest) = l.strip_prefix(PROMPT) {
        l = rest;
    }
    l
}

#[derive(Default)]
pub struct Gnubg {
    /// The best move listed so far.
    first: Option<String>,
    /// The position is before the roll: `eval`, not `hint`.
    before_roll: bool,
    /// `eval`'s equity at its deepest ply so far.
    equity: Option<f32>,
}

/// A line of `eval`: `static: 0.521 0.154 0.008 0.130 0.007 +0.068 +0.090` or `2 ply: …`,
/// five chances and then the cube-less equity.
fn evaluated(line: &str) -> Option<f32> {
    let w: Vec<&str> = line.split_whitespace().collect();
    let at = w.iter().position(|x| *x == "static:" || *x == "ply:")?;
    if w[at] == "ply:" && (at != 1 || w[0].parse::<u32>().is_err()) {
        return None;
    }
    w.get(at + 6)?.parse().ok()
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
        format!("set player 0 human\nset player 1 human\nshow version\nset prompt {PROMPT}\n")
    }

    fn greeted(&mut self, line: &str, about: &mut About) -> Step {
        let line = unprompted(line);
        // `show version` answers after the default prompt, `(No game) GNU Backgammon 1.08.003 …`.
        if let Some(at) = line.find("GNU Backgammon").filter(|_| about.name.is_empty()) {
            about.name = line[at..].trim().to_string();
        } else if line.contains(END) {
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
        self.before_roll = p.dice.is_none();
        out
    }

    fn go(&mut self, _about: &About, _limit: Limit, lines: u32) -> String {
        self.first = None;
        self.equity = None;
        if self.before_roll {
            return "eval\nshow prompt\n".into();
        }
        format!("hint {}\nshow prompt\n", lines.max(1))
    }

    fn read(&mut self, line: &str) -> Vec<Event> {
        let line = unprompted(line);
        if self.before_roll {
            if line.contains(END) {
                let said = self.equity.take().map(|e| Event::Info(Info { depth: None, line: 1, score: Some(Score::Equity(e)), nodes: None, pv: vec![] }));
                return said.into_iter().chain([Event::Best(Best { mv: None, ponder: None })]).collect();
            }
            if let Some(e) = evaluated(line) {
                self.equity = Some(e);
            }
            return vec![];
        }
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
        assert!(d.hello().ends_with("show version\nset prompt bragd>\n"));
        // As GNU Backgammon 1.08.003 writes it: the prompt in front of each answer.
        for l in ["(No game) Moves for gnubg must now be entered manually.", "(No game) GNU Backgammon 1.08.003 20261031", "AUTHORS"] {
            assert_eq!(d.greeted(l, &mut about), Step::More);
        }
        assert_eq!(d.greeted("(No game) The prompt has been set to `bragd>'.", &mut about), Step::Ready(String::new()));
        assert_eq!(about.name, "GNU Backgammon 1.08.003 20261031");
        let mut p = Position::new("backgammon", &[]);
        p.start = Start::Setup("4HPwATDgc/ABMA:cAkAAAAAAAAA".into());
        p.dice = Some([3, 1]);
        assert_eq!(d.position(&about, &p), "new game\nset matchid cAkAAAAAAAAA\nset board 4HPwATDgc/ABMA\nset dice 3 1\n");
        assert_eq!(d.go(&about, Limit::Depth(2), 2), "hint 2\nshow prompt\n");
        let said = "bragd>The dice have been set to 3 and 1.\nbragd>    1. Cubeful 2-ply    8/5 6/5                      Eq.:  +0.164\n       0.551 0.176 0.010 - 0.449 0.124 0.005\n        2-ply cubeful prune [world class]\n    2. Cubeful 2-ply    24/23 13/10                  Eq.: -0.010 (-0.174)\n       0.497 0.140 0.006 - 0.503 0.138 0.005\nbragd>The prompt is set to `bragd>'.";
        let got: Vec<Event> = said.lines().flat_map(|l| d.read(l)).collect();
        assert_eq!(got.len(), 3, "{got:?}");
        assert_eq!(got[0], Event::Info(Info { depth: None, line: 1, score: Some(Score::Equity(0.164)), nodes: None, pv: vec!["8/5 6/5".into()] }));
        assert!(matches!(&got[1], Event::Info(i) if i.line == 2 && i.score == Some(Score::Equity(-0.01))));
        assert_eq!(got[2], Event::Best(Best { mv: Some("8/5 6/5".into()), ponder: None }));
    }

    /// Before the roll: no dice, and `eval`'s cube-less equity at its deepest ply, no move.
    #[test]
    fn gnu_backgammon_evaluates_before_the_roll() {
        let mut d = Protocol::Gnubg.dialect();
        let about = About::default();
        let mut p = Position::new("backgammon", &[]);
        p.start = Start::Setup("4HPwATDgc/ABMA".into());
        assert_eq!(d.position(&about, &p), "new game\nset board 4HPwATDgc/ABMA\n");
        assert_eq!(d.go(&about, Limit::Depth(2), 2), "eval\nshow prompt\n");
        // As GNU Backgammon 1.08.003 writes it.
        let said = "bragd>(nobody) Position ID:\t4HPwATDgc/ABMA\nMatch ID:\tcAkLAAAAAAAA\n\nEvaluator: \tContact\n\n\n        Win     W(g)    W(bg)   L(g)    L(bg)   Equity    Cubeful  \nstatic: 0.521   0.154   0.008   0.130   0.007   +0.068    +0.090    \n 1 ply: 0.527   0.148   0.008   0.128   0.005   +0.077    +0.104    \n 2 ply: 0.525   0.150   0.007   0.125   0.005   +0.076    +0.098    \nbragd>The prompt is set to `bragd>'.";
        let got: Vec<Event> = said.lines().flat_map(|l| d.read(l)).collect();
        assert_eq!(got, [Event::Info(Info { depth: None, line: 1, score: Some(Score::Equity(0.076)), nodes: None, pv: vec![] }), Event::Best(Best { mv: None, ponder: None })]);
        // With dice again: `hint`.
        p.dice = Some([6, 2]);
        d.position(&about, &p);
        assert!(d.go(&about, Limit::Depth(2), 1).starts_with("hint 1"));
    }
}
