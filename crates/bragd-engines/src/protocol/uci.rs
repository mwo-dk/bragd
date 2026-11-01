//! UCI and its two kin: USI (shogi) differs in its words and its time limit, UCCI (xiangqi) in
//! its options, its scores and its time limit. Pikafish speaks UCI, not UCCI, so it is started
//! as UCI; UCCI is there for the engines that still speak it.

use super::{About, Best, Dialect, Event, Info, Limit, Position, Protocol, Score, Start, Step, after};
use crate::options::{parse_option, parse_ucci_option};

/// The Variant's name for Fairy-Stockfish's `UCI_Variant`, for the Variant keys it plays.
pub fn fairy_variant(key: &str) -> Option<&'static str> {
    Some(match key {
        "chess" | "chess960" => "chess",
        "shatranj" => "shatranj",
        "chaturanga" => "chaturanga",
        "kingofthehill" => "kingofthehill",
        "racingkings" => "racingkings",
        "seirawan" => "seirawan",
        "capablanca" => "capablanca",
        "xiangqi" => "xiangqi",
        "janggi" => "janggi",
        "shogi" => "shogi",
        _ => return None,
    })
}

/// UCI, USI or UCCI.
pub struct Uci {
    protocol: Protocol,
    /// UCCI: times in milliseconds (`usemillisec` is on), else in seconds.
    millis: bool,
}

impl Uci {
    pub fn new(protocol: Protocol) -> Uci {
        Uci { protocol, millis: false }
    }

    fn word(&self) -> &'static str {
        match self.protocol {
            Protocol::Usi => "usi",
            Protocol::Ucci => "ucci",
            _ => "uci",
        }
    }
}

impl Dialect for Uci {
    fn protocol(&self) -> Protocol {
        self.protocol
    }

    fn hello(&self) -> String {
        format!("{}\n", self.word())
    }

    fn greeted(&mut self, line: &str, about: &mut About) -> Step {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.first().copied() {
            Some("id") if w.get(1) == Some(&"name") => about.name = w[2..].join(" "),
            // No engine offers thousands: a runaway one is not kept in memory.
            Some("option") if about.options.len() < 1_000 => {
                let o = if self.protocol == Protocol::Ucci { parse_ucci_option(line) } else { parse_option(line) };
                about.options.extend(o);
            }
            Some(ok) if ok == format!("{}ok", self.word()) => {
                if self.protocol == Protocol::Ucci && about.option("usemillisec").is_some() {
                    self.millis = true;
                    return Step::Ready("setoption usemillisec true\n".into());
                }
                return Step::Ready(String::new());
            }
            _ => {}
        }
        Step::More
    }

    fn set_option(&self, name: &str, value: &str) -> String {
        match self.protocol {
            Protocol::Ucci => format!("setoption {name} {value}\n"),
            _ => format!("setoption name {name} value {value}\n"),
        }
    }

    fn position(&mut self, about: &About, p: &Position) -> String {
        let mut out = String::new();
        if let Some(o) = about.option("UCI_Variant")
            && let Some(v) = fairy_variant(&p.variant)
        {
            out.push_str(&self.set_option(&o.name, v));
        }
        if let Some(o) = about.option("UCI_Chess960") {
            out.push_str(&self.set_option(&o.name, &(p.variant == "chess960").to_string()));
        }
        match self.protocol {
            Protocol::Usi => out.push_str("isready\nusinewgame\n"),
            Protocol::Uci => out.push_str("isready\n"),
            _ => {}
        }
        let start = match (&p.start, self.protocol) {
            (Start::Setup(s), Protocol::Usi) => format!("sfen {s}"),
            (Start::Setup(s), _) => format!("fen {s}"),
            // UCCI has no `startpos`.
            (Start::Initial, Protocol::Ucci) => "fen rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1".into(),
            (Start::Initial, _) => "startpos".into(),
        };
        out.push_str(&format!("position {start}"));
        if !p.moves.is_empty() {
            out.push_str(" moves ");
            out.push_str(&p.moves.join(" "));
        }
        out.push('\n');
        out
    }

    fn go(&mut self, about: &About, limit: Limit, lines: u32) -> String {
        let mut out = String::new();
        if let Some(o) = ["MultiPV", "USI_MultiPV"].iter().find_map(|n| about.option(n)) {
            out.push_str(&self.set_option(&o.name, &lines.max(1).to_string()));
        }
        out.push_str(&match (self.protocol, limit) {
            (Protocol::Usi, Limit::Time(ms)) => format!("go btime 0 wtime 0 byoyomi {ms}"),
            (Protocol::Ucci, Limit::Time(ms)) if self.millis => format!("go time {ms}"),
            (Protocol::Ucci, Limit::Time(ms)) => format!("go time {}", ms.div_ceil(1000)),
            (_, Limit::Time(ms)) => format!("go movetime {ms}"),
            (_, Limit::Depth(d)) => format!("go depth {d}"),
            (_, Limit::Nodes(n)) => format!("go nodes {n}"),
            // USI's black (`b`) is sente, who moves first; UCI's white is.
            (Protocol::Usi, Limit::Clock { own, other, inc, first, .. }) => {
                let (b, w) = if first { (own, other) } else { (other, own) };
                format!("go btime {b} wtime {w} binc {inc} winc {inc}")
            }
            (Protocol::Ucci, Limit::Clock { own, other, inc, to_go, .. }) => {
                let t = |ms: u32| if self.millis { ms } else { ms / 1000 };
                match to_go {
                    Some(m) => format!("go time {} movestogo {m} opptime {}", t(own), t(other)),
                    None => format!("go time {} increment {} opptime {}", t(own), t(inc), t(other)),
                }
            }
            (_, Limit::Clock { own, other, inc, to_go, first }) => {
                let (w, b) = if first { (own, other) } else { (other, own) };
                let to_go = to_go.map(|m| format!(" movestogo {m}")).unwrap_or_default();
                format!("go wtime {w} btime {b} winc {inc} binc {inc}{to_go}")
            }
        });
        out.push('\n');
        out
    }

    fn read(&mut self, line: &str) -> Vec<Event> {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.first().copied() {
            Some("info") if !w.contains(&"lowerbound") && !w.contains(&"upperbound") && w.contains(&"pv") => {
                let mut i = Info { line: after(&w, "multipv").and_then(|k| k.parse().ok()).unwrap_or(1), ..Default::default() };
                i.depth = after(&w, "depth").and_then(|d| d.parse().ok());
                i.nodes = after(&w, "nodes").and_then(|d| d.parse().ok());
                if let Some(at) = w.iter().position(|x| *x == "score") {
                    i.score = match (w.get(at + 1), w.get(at + 2)) {
                        (Some(&"mate"), Some(v)) => v.parse().ok().map(Score::Mate),
                        (Some(&"cp"), Some(v)) => v.parse().ok().map(Score::Cp),
                        // UCCI: the score alone.
                        (Some(v), _) => v.parse().ok().map(Score::Cp),
                        _ => None,
                    };
                }
                let from = w.iter().position(|x| *x == "pv").map_or(w.len(), |i| i + 1);
                i.pv = w[from..].iter().map(|s| s.to_string()).collect();
                vec![Event::Info(i)]
            }
            Some("bestmove") => {
                let mv = w.get(1).filter(|m| !matches!(**m, "(none)" | "0000" | "none" | "resign")).map(|m| m.to_string());
                vec![Event::Best(Best { mv, ponder: after(&w, "ponder").map(String::from) })]
            }
            Some("nobestmove") => vec![Event::Best(Best::default())],
            _ => vec![],
        }
    }

    fn stop(&self) -> String {
        "stop\n".into()
    }

    fn quit(&self) -> String {
        "quit\n".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds the engine's side of a transcript to the dialect: the handshake, then the rest.
    fn greet(d: &mut dyn Dialect, lines: &str) -> (About, Step) {
        let mut about = About::default();
        let mut last = Step::More;
        for l in lines.lines() {
            last = d.greeted(l, &mut about);
        }
        (about, last)
    }

    fn events(d: &mut dyn Dialect, lines: &str) -> Vec<Event> {
        lines.lines().flat_map(|l| d.read(l)).collect()
    }

    #[test]
    fn a_uci_session_with_fairy_stockfish() {
        let mut d = Protocol::Uci.dialect();
        assert_eq!(d.hello(), "uci\n");
        let (about, ready) = greet(
            &mut *d,
            "Fairy-Stockfish 14.0.1 LB by Fabian Fichter\nid name Fairy-Stockfish 14.0.1 LB\nid author the Stockfish developers\noption name Threads type spin default 1 min 1 max 512\noption name MultiPV type spin default 1 min 1 max 500\noption name UCI_Chess960 type check default false\noption name UCI_Variant type combo default chess var chess var xiangqi var shogi\nuciok",
        );
        assert_eq!(ready, Step::Ready(String::new()));
        assert_eq!(about.name, "Fairy-Stockfish 14.0.1 LB");
        assert_eq!(about.options.len(), 4);
        let p = Position::new("xiangqi", &["h2e2", "h9g7"]);
        assert_eq!(
            d.position(&about, &p),
            "setoption name UCI_Variant value xiangqi\nsetoption name UCI_Chess960 value false\nisready\nposition startpos moves h2e2 h9g7\n"
        );
        assert_eq!(d.go(&about, Limit::Time(500), 2), "setoption name MultiPV value 2\ngo movetime 500\n");
        let got = events(
            &mut *d,
            "readyok\ninfo depth 5 score cp 40 lowerbound pv a7a6\ninfo depth 9 seldepth 12 multipv 1 score cp 30 nodes 99 pv b0c2 b9c7\ninfo depth 9 multipv 2 score mate -4 pv c3c4\nbestmove b0c2 ponder b9c7",
        );
        assert_eq!(
            got,
            [
                Event::Info(Info { depth: Some(9), line: 1, score: Some(Score::Cp(30)), nodes: Some(99), pv: vec!["b0c2".into(), "b9c7".into()] }),
                Event::Info(Info { depth: Some(9), line: 2, score: Some(Score::Mate(-4)), nodes: None, pv: vec!["c3c4".into()] }),
                Event::Best(Best { mv: Some("b0c2".into()), ponder: Some("b9c7".into()) }),
            ]
        );
        assert_eq!(events(&mut *d, "bestmove (none)"), [Event::Best(Best::default())]);
        let mut c = Position::new("chess960", &[]);
        c.start = Start::Setup("bqnb1rkr/pp3ppp/3ppn2/2p5/5P2/P2P4/NPP1P1PP/BQ1BNRKR w HFhf - 2 9".into());
        assert!(d.position(&about, &c).ends_with("setoption name UCI_Chess960 value true\nisready\nposition fen bqnb1rkr/pp3ppp/3ppn2/2p5/5P2/P2P4/NPP1P1PP/BQ1BNRKR w HFhf - 2 9\n"));
    }

    #[test]
    fn a_plain_uci_engine_gets_no_variant_options() {
        let mut d = Protocol::Uci.dialect();
        let (about, _) = greet(&mut *d, "id name Pikafish 2025-06-23\noption name Hash type spin default 16 min 1 max 33554432\nuciok");
        assert_eq!(d.position(&about, &Position::new("xiangqi", &["h2e2"])), "isready\nposition startpos moves h2e2\n");
        assert_eq!(d.go(&about, Limit::Depth(12), 3), "go depth 12\n");
        let clock = |first| Limit::Clock { own: 50_000, other: 60_000, inc: 1_000, to_go: Some(7), first };
        assert_eq!(d.go(&about, clock(false), 1), "go wtime 60000 btime 50000 winc 1000 binc 1000 movestogo 7\n", "Black to move");
    }

    #[test]
    fn a_usi_session_with_yaneuraou() {
        let mut d = Protocol::Usi.dialect();
        assert_eq!(d.hello(), "usi\n");
        let (about, ready) = greet(&mut *d, "id name YaneuraOu NNUE 8.30 64AVX2\nid author by yaneurao\noption name USI_Hash type spin default 256 min 1 max 33554432\noption name MultiPV type spin default 1 min 1 max 800\nusiok");
        assert_eq!((about.name.as_str(), ready), ("YaneuraOu NNUE 8.30 64AVX2", Step::Ready(String::new())));
        assert_eq!(d.position(&about, &Position::new("shogi", &["7g7f"])), "isready\nusinewgame\nposition startpos moves 7g7f\n");
        assert_eq!(d.go(&about, Limit::Time(1000), 1), "setoption name MultiPV value 1\ngo btime 0 wtime 0 byoyomi 1000\n");
        let clock = Limit::Clock { own: 30_000, other: 40_000, inc: 500, to_go: None, first: true };
        assert_eq!(d.go(&about, clock, 1), "setoption name MultiPV value 1\ngo btime 30000 wtime 40000 binc 500 winc 500\n", "sente to move");
        let mut s = Position::new("shogi", &[]);
        s.start = Start::Setup("lnsgkgsnl/1r5b1/ppppppppp/9/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL w - 2".into());
        assert!(d.position(&about, &s).ends_with("position sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL w - 2\n"));
        let got = events(&mut *d, "info depth 14 seldepth 18 score cp -52 nodes 123456 nps 900000 time 137 pv 3c3d 2g2f 4c4d\nbestmove 3c3d ponder 2g2f");
        assert_eq!(got.len(), 2);
        assert!(matches!(&got[0], Event::Info(i) if i.score == Some(Score::Cp(-52)) && i.pv[0] == "3c3d"));
        assert_eq!(events(&mut *d, "bestmove resign"), [Event::Best(Best::default())]);
    }

    #[test]
    fn a_ucci_session() {
        let mut d = Protocol::Ucci.dialect();
        assert_eq!(d.hello(), "ucci\n");
        let (about, ready) = greet(
            &mut *d,
            "id name ElephantEye 3.31\nid copyright 2004-2016 www.xqbase.com\noption usemillisec type check default false\noption hashsize type spin min 0 max 1024 default 0\noption idle type combo var none var small var medium var large default none\nucciok",
        );
        assert_eq!(ready, Step::Ready("setoption usemillisec true\n".into()));
        assert_eq!(about.option("idle").unwrap().vars, ["none", "small", "medium", "large"]);
        assert_eq!(about.option("hashsize").unwrap().max, Some(1024));
        assert_eq!(d.set_option("hashsize", "64"), "setoption hashsize 64\n");
        assert_eq!(d.position(&about, &Position::new("xiangqi", &["h2e2"])), "position fen rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1 moves h2e2\n");
        assert_eq!(d.go(&about, Limit::Time(1500), 1), "go time 1500\n");
        let clock = Limit::Clock { own: 90_500, other: 80_000, inc: 2_000, to_go: None, first: false };
        assert_eq!(d.go(&about, clock, 1), "go time 90500 increment 2000 opptime 80000\n");
        let got = events(&mut *d, "info depth 8 score 21 pv h9g7 h0g2\nnobestmove");
        assert!(matches!(&got[0], Event::Info(i) if i.score == Some(Score::Cp(21)) && i.depth == Some(8)));
        assert_eq!(got[1], Event::Best(Best::default()));
        assert_eq!(d.quit(), "quit\n");
        // Without milliseconds: whole seconds, rounded up.
        let mut s = Protocol::Ucci.dialect();
        let (about, ready) = greet(&mut *s, "id name Old\nucciok");
        assert_eq!(ready, Step::Ready(String::new()));
        assert_eq!(s.go(&about, Limit::Time(1500), 1), "go time 2\n");
        let clock = Limit::Clock { own: 90_500, other: 80_000, inc: 0, to_go: Some(12), first: true };
        assert_eq!(s.go(&about, clock, 1), "go time 90 movestogo 12 opptime 80\n");
    }

    /// What ElephantEye 3.31 itself printed (`tools/dev/images.sh elephanteye`): its name and
    /// version on two lines, `usemillisec` already on, lines of `info` without a line of play,
    /// and a `bestmove` with a `ponder`.
    #[test]
    fn a_ucci_session_as_elephanteye_wrote_it() {
        let mut d = Protocol::Ucci.dialect();
        let (about, ready) = greet(
            &mut *d,
            "id name ElephantEye\nid version 3.31\nid copyright 2004-2016 www.xqbase.com\noption usemillisec type check default true\noption bookfiles type string default /BOOK.DAT\noption hashsize type spin min 16 max 1024 default 16\noption idle type combo var none var small var medium var large default none\noption newgame type button\nucciok",
        );
        assert_eq!((about.name.as_str(), ready), ("ElephantEye", Step::Ready("setoption usemillisec true\n".into())));
        assert_eq!(about.option("hashsize").unwrap().min, Some(16));
        let got = events(&mut *d, "info time 635 nodes 1573904\ninfo depth 9 score -3 pv h9g7 g3g4 i9h9\nbestmove b9c7 ponder h0i2\nbye");
        assert_eq!(
            got,
            [
                Event::Info(Info { depth: Some(9), line: 1, score: Some(Score::Cp(-3)), nodes: None, pv: vec!["h9g7".into(), "g3g4".into(), "i9h9".into()] }),
                Event::Best(Best { mv: Some("b9c7".into()), ponder: Some("h0i2".into()) }),
            ]
        );
    }
}
