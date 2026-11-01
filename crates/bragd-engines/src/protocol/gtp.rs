//! GTP, the Go Text Protocol (KataGo, GNU Go). Every command gets one answer, `= …` or `? …`,
//! ended by an empty line; the dialect keeps a queue of the answers it waits for. KataGo
//! thinks aloud with `kata-genmove_analyze`; GNU Go only names its move (`genmove`).

use std::collections::VecDeque;

use super::{About, Best, Dialect, Event, Info, Limit, Position, Protocol, Score, Step, after};

/// What an answer is for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Wait {
    /// A command whose answer says nothing (`play`, `komi`).
    Ack,
    /// `genmove`: the move.
    Move,
    /// `kata-genmove_analyze`: `=`, then `info` lines, then `play <move>`.
    Analyse,
}

#[derive(Default)]
pub struct Gtp {
    waits: VecDeque<Wait>,
    katago: bool,
    /// Inside `kata-genmove_analyze`'s answer.
    analysing: bool,
    /// The colour to move in the position set.
    colour_next: &'static str,
}

/// The board size of a go Variant key (`go-9`, `go-13`, `go-19`).
fn size(variant: &str) -> u32 {
    variant.strip_prefix("go-").and_then(|s| s.parse().ok()).unwrap_or(19)
}

impl Gtp {
    fn send(&mut self, out: &mut String, cmd: &str, wait: Wait) {
        out.push_str(cmd);
        out.push('\n');
        self.waits.push_back(wait);
    }

    /// The colour to move: Black first, then in turn.
    fn colour(moves: usize) -> &'static str {
        if moves.is_multiple_of(2) { "B" } else { "W" }
    }
}

/// What KataGo's `info` reports on one line say: one `Info` each.
fn katago_infos(line: &str) -> Vec<Info> {
    line.split("info ")
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            let w: Vec<&str> = s.split_whitespace().collect();
            let winrate = after(&w, "winrate").and_then(|v| v.parse().ok());
            let lead = after(&w, "scoreLead").and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let from = w.iter().position(|x| *x == "pv").map_or(w.len(), |i| i + 1);
            Info {
                depth: None,
                line: after(&w, "order").and_then(|o| o.parse::<usize>().ok()).map_or(1, |o| o + 1),
                score: winrate.map(|winrate| Score::Go { winrate, lead }),
                nodes: after(&w, "visits").and_then(|v| v.parse().ok()),
                pv: w[from..].iter().take_while(|m| !m.contains(char::is_lowercase) || **m == "pass").map(|m| m.to_string()).collect(),
            }
        })
        .collect()
}

impl Dialect for Gtp {
    fn protocol(&self) -> Protocol {
        Protocol::Gtp
    }

    fn hello(&self) -> String {
        "name\nversion\n".into()
    }

    fn greeted(&mut self, line: &str, about: &mut About) -> Step {
        let Some(said) = line.strip_prefix('=') else { return Step::More };
        let said = said.trim_start_matches(|c: char| c.is_ascii_digit()).trim();
        if about.name.is_empty() {
            about.name = said.to_string();
            self.katago = said.eq_ignore_ascii_case("katago");
            return Step::More;
        }
        about.name = format!("{} {said}", about.name);
        Step::Ready(String::new())
    }

    fn set_option(&self, name: &str, value: &str) -> String {
        if self.katago { format!("kata-set-param {name} {value}\n") } else { String::new() }
    }

    fn position(&mut self, _about: &About, p: &Position) -> String {
        let mut out = String::new();
        self.send(&mut out, &format!("boardsize {}", size(&p.variant)), Wait::Ack);
        self.send(&mut out, "clear_board", Wait::Ack);
        self.send(&mut out, "komi 7.5", Wait::Ack);
        for (i, m) in p.moves.iter().enumerate() {
            self.send(&mut out, &format!("play {} {m}", Gtp::colour(i)), Wait::Ack);
        }
        self.colour_next = Gtp::colour(p.moves.len());
        out
    }

    fn go(&mut self, _about: &About, limit: Limit, _lines: u32) -> String {
        let mut out = String::new();
        match limit {
            Limit::Time(ms) => self.send(&mut out, &format!("time_settings 0 {} 1", ms.div_ceil(1000).max(1)), Wait::Ack),
            Limit::Nodes(n) if self.katago => self.send(&mut out, &format!("kata-set-param maxVisits {n}"), Wait::Ack),
            // GNU Go's level, 0 to 10, stands in for a depth.
            Limit::Depth(d) if !self.katago => self.send(&mut out, &format!("level {}", d.min(10)), Wait::Ack),
            _ => {}
        }
        let colour = if self.colour_next.is_empty() { "B" } else { self.colour_next };
        // Moves per period are Canadian byo-yomi: so many stones in the time left.
        if let Limit::Clock { own, to_go, .. } = limit {
            let s = own / 1000;
            let settings = match to_go {
                Some(m) => format!("time_settings 0 {s} {m}"),
                None => format!("time_settings {s} 0 0"),
            };
            self.send(&mut out, &settings, Wait::Ack);
            self.send(&mut out, &format!("time_left {colour} {s} {}", to_go.unwrap_or(0)), Wait::Ack);
        }
        if self.katago {
            self.send(&mut out, &format!("kata-genmove_analyze {colour} 50"), Wait::Analyse);
        } else {
            self.send(&mut out, &format!("genmove {colour}"), Wait::Move);
        }
        out
    }

    fn read(&mut self, line: &str) -> Vec<Event> {
        let line = line.trim();
        if self.analysing {
            if let Some(mv) = line.strip_prefix("play ") {
                self.analysing = false;
                return vec![Event::Best(best(mv))];
            }
            return katago_infos(line).into_iter().map(Event::Info).collect();
        }
        let (ok, said) = match line.chars().next() {
            Some('=') => (true, &line[1..]),
            Some('?') => (false, &line[1..]),
            _ => return vec![],
        };
        // An answer may carry the command's number first.
        let said = said.trim_start_matches(|c: char| c.is_ascii_digit()).trim();
        match (self.waits.pop_front(), ok) {
            (_, false) => vec![Event::Refused(said.to_string())],
            (Some(Wait::Move), true) => vec![Event::Best(best(said))],
            (Some(Wait::Analyse), true) => {
                self.analysing = true;
                vec![]
            }
            _ => vec![],
        }
    }

    fn stop(&self) -> String {
        String::new()
    }

    fn quit(&self) -> String {
        "quit\n".into()
    }
}

fn best(mv: &str) -> Best {
    let mv = mv.trim();
    let mv = match mv {
        "" => None,
        m if m.eq_ignore_ascii_case("resign") => None,
        m if m.eq_ignore_ascii_case("pass") => Some("pass".into()),
        m => Some(m.to_uppercase()),
    };
    Best { mv, ponder: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(d: &mut dyn Dialect, lines: &str) -> Vec<Event> {
        lines.lines().flat_map(|l| d.read(l)).collect()
    }

    #[test]
    fn gnu_go_names_its_move() {
        let mut d = Protocol::Gtp.dialect();
        let mut about = About::default();
        assert_eq!(d.hello(), "name\nversion\n");
        assert_eq!(d.greeted("= GNU Go", &mut about), Step::More);
        assert_eq!(d.greeted("", &mut about), Step::More);
        assert_eq!(d.greeted("= 3.8", &mut about), Step::Ready(String::new()));
        assert_eq!(about.name, "GNU Go 3.8");
        assert_eq!(d.position(&about, &Position::new("go-9", &["E5", "pass"])), "boardsize 9\nclear_board\nkomi 7.5\nplay B E5\nplay W pass\n");
        assert_eq!(d.go(&about, Limit::Depth(14), 1), "level 10\ngenmove B\n");
        let got = run(&mut *d, "= \n\n=\n\n=\n\n= \n\n=\n\n=\n\n= c3\n\n");
        assert_eq!(got, [Event::Best(Best { mv: Some("C3".into()), ponder: None })]);
        // A refusal is said, not taken for the move.
        d.position(&about, &Position::new("go-9", &["Z99"]));
        assert_eq!(run(&mut *d, "=\n\n=\n\n=\n\n? illegal move\n\n"), [Event::Refused("illegal move".into())]);
    }

    #[test]
    fn a_clock_is_told_as_time_settings_and_time_left() {
        let mut d = Protocol::Gtp.dialect();
        let mut about = About::default();
        d.greeted("= GNU Go", &mut about);
        d.greeted("", &mut about);
        d.greeted("= 3.8", &mut about);
        d.position(&about, &Position::new("go-9", &["E5"]));
        let canadian = Limit::Clock { own: 90_400, other: 1, inc: 0, to_go: Some(10), first: false };
        assert_eq!(d.go(&about, canadian, 1), "time_settings 0 90 10\ntime_left W 90 10\ngenmove W\n");
        let sudden = Limit::Clock { own: 30_000, other: 1, inc: 0, to_go: None, first: false };
        assert_eq!(d.go(&about, sudden, 1), "time_settings 30 0 0\ntime_left W 30 0\ngenmove W\n");
    }

    #[test]
    fn katago_thinks_aloud() {
        let mut d = Protocol::Gtp.dialect();
        let mut about = About::default();
        d.greeted("=1 KataGo", &mut about);
        assert_eq!(d.greeted("= 1.15.3", &mut about), Step::Ready(String::new()));
        assert_eq!(d.set_option("maxVisits", "400"), "kata-set-param maxVisits 400\n");
        assert_eq!(d.position(&about, &Position::new("go-19", &["Q16"])), "boardsize 19\nclear_board\nkomi 7.5\nplay B Q16\n");
        assert_eq!(d.go(&about, Limit::Time(2000), 2), "time_settings 0 2 1\nkata-genmove_analyze W 50\n");
        let got = run(
            &mut *d,
            "=\n\n=\n\n=\n\n=\n\n=\n\n=\ninfo move D4 visits 120 edgeVisits 120 utility -0.03 winrate 0.4712 scoreMean -0.4 scoreStdev 30.1 scoreLead -0.41 scoreSelfplay -0.6 prior 0.21 lcb 0.46 utilityLcb -0.05 order 0 pv D4 Q4 D16 info move D16 visits 80 winrate 0.4650 scoreLead -0.62 order 1 pv D16 D4\nplay D4\n\n",
        );
        assert_eq!(got.len(), 3, "{got:?}");
        assert_eq!(got[0], Event::Info(Info { depth: None, line: 1, score: Some(Score::Go { winrate: 0.4712, lead: -0.41 }), nodes: Some(120), pv: vec!["D4".into(), "Q4".into(), "D16".into()] }));
        assert!(matches!(&got[1], Event::Info(i) if i.line == 2 && i.pv == ["D16", "D4"]));
        assert_eq!(got[2], Event::Best(Best { mv: Some("D4".into()), ponder: None }));
    }
}
