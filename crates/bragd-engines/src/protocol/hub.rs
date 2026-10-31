//! Hub, the draughts protocol of Scan: `key=value` words, a value with spaces in double quotes.
//! `hub` is answered with its `id`, its `param`s and `wait`; `init` with `ready`. A position is
//! one letter for the side to move and one for each of the 50 squares (`w`, `b`, `W`, `B`
//! kings, `e` empty).

use super::{About, Best, Dialect, Event, Info, Limit, Position, Protocol, Score, Start, Step};
use crate::options::EngineOption;

/// International draughts' start: White to move, twenty men each.
pub const START: &str = "Wbbbbbbbbbbbbbbbbbbbbeeeeeeeeeewwwwwwwwwwwwwwwwwwww";

/// A Hub line: its command, then its `key=value` words (a bare word has an empty value).
pub fn fields(line: &str) -> (String, Vec<(String, String)>) {
    let mut words = vec![];
    let (mut word, mut quoted) = (String::new(), false);
    for c in line.trim().chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            c => word.push(c),
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    let mut it = words.into_iter();
    let cmd = it.next().unwrap_or_default();
    let kv = it.map(|w| match w.split_once('=') {
        Some((k, v)) => (k.to_string(), v.to_string()),
        None => (w, String::new()),
    });
    (cmd, kv.collect())
}

fn get<'a>(kv: &'a [(String, String)], key: &str) -> Option<&'a str> {
    kv.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// A value with a space goes in double quotes.
fn quoted(v: &str) -> String {
    if v.contains(char::is_whitespace) || v.is_empty() { format!("\"{v}\"") } else { v.to_string() }
}

#[derive(Default)]
pub struct Hub;

impl Dialect for Hub {
    fn protocol(&self) -> Protocol {
        Protocol::Hub
    }

    fn hello(&self) -> String {
        "hub\n".into()
    }

    fn greeted(&mut self, line: &str, about: &mut About) -> Step {
        let (cmd, kv) = fields(line);
        match cmd.as_str() {
            "id" => {
                let name = [get(&kv, "name"), get(&kv, "version")].into_iter().flatten().collect::<Vec<_>>().join(" ");
                about.name = name;
            }
            "param" if about.options.len() < 1_000 => {
                if let Some(name) = get(&kv, "name") {
                    let kind = match get(&kv, "type").unwrap_or("string") {
                        "int" => "spin",
                        "bool" => "check",
                        "enum" => "combo",
                        _ => "string",
                    };
                    about.options.push(EngineOption {
                        name: name.into(),
                        kind: kind.into(),
                        default: get(&kv, "value").unwrap_or_default().into(),
                        min: get(&kv, "min").and_then(|v| v.parse().ok()),
                        max: get(&kv, "max").and_then(|v| v.parse().ok()),
                        vars: get(&kv, "values").map(|v| v.split_whitespace().map(String::from).collect()).unwrap_or_default(),
                    });
                }
            }
            "wait" => return Step::Reply("init\n".into()),
            "ready" => return Step::Ready(String::new()),
            _ => {}
        }
        Step::More
    }

    fn set_option(&self, name: &str, value: &str) -> String {
        format!("set-param name={} value={}\n", quoted(name), quoted(value))
    }

    fn position(&mut self, about: &About, p: &Position) -> String {
        let mut out = String::new();
        if about.option("variant").is_some() && p.variant == "draughts" {
            out.push_str(&self.set_option("variant", "normal"));
        }
        out.push_str("new-game\n");
        let pos = match &p.start {
            Start::Setup(s) => s.as_str(),
            Start::Initial => START,
        };
        out.push_str(&format!("pos pos={pos}"));
        if !p.moves.is_empty() {
            out.push_str(&format!(" moves={}", quoted(&p.moves.join(" "))));
        }
        out.push('\n');
        out
    }

    fn go(&mut self, _about: &About, limit: Limit, _lines: u32) -> String {
        let level = match limit {
            Limit::Time(ms) => format!("move-time={:.3}", ms as f64 / 1000.0),
            Limit::Depth(d) => format!("depth={d}"),
            Limit::Nodes(n) => format!("nodes={n}"),
        };
        format!("level {level}\ngo think\n")
    }

    fn read(&mut self, line: &str) -> Vec<Event> {
        let (cmd, kv) = fields(line);
        match cmd.as_str() {
            "info" => {
                // A score in men (`0.15`) or in hundredths of one (`15`).
                let score = get(&kv, "score").and_then(|s| if s.contains('.') { s.parse::<f64>().ok().map(|v| (v * 100.0).round() as i32) } else { s.parse().ok() });
                vec![Event::Info(Info {
                    depth: get(&kv, "depth").and_then(|d| d.parse().ok()),
                    line: 1,
                    score: score.map(Score::Cp),
                    nodes: get(&kv, "nodes").and_then(|d| d.parse().ok()),
                    pv: get(&kv, "pv").map(|v| v.split_whitespace().map(String::from).collect()).unwrap_or_default(),
                })]
            }
            "done" => vec![Event::Best(Best { mv: get(&kv, "move").filter(|m| !m.is_empty()).map(String::from), ponder: get(&kv, "ponder").filter(|m| !m.is_empty()).map(String::from) })],
            "error" => vec![Event::Refused(get(&kv, "message").unwrap_or(line).to_string())],
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

    #[test]
    fn a_hub_session_with_scan() {
        let mut d = Protocol::Hub.dialect();
        let mut about = About::default();
        assert_eq!(d.hello(), "hub\n");
        let said = ["id name=Scan version=3.1 author=\"Fabien Letouzey\" country=France", "param name=variant value=normal type=enum values=\"normal killer bt frisian losing\"", "param name=tt-size value=24 type=int min=16 max=30", "wait"];
        let steps: Vec<Step> = said.iter().map(|l| d.greeted(l, &mut about)).collect();
        assert_eq!(steps.last(), Some(&Step::Reply("init\n".into())));
        assert_eq!(d.greeted("ready", &mut about), Step::Ready(String::new()));
        assert_eq!(about.name, "Scan 3.1");
        assert_eq!(about.option("variant").unwrap().vars, ["normal", "killer", "bt", "frisian", "losing"]);
        assert_eq!(about.option("tt-size").map(|o| (o.kind.as_str(), o.max)), Some(("spin", Some(30))));
        assert_eq!(
            d.position(&about, &Position::new("draughts", &["32-28", "19-23"])),
            format!("set-param name=variant value=normal\nnew-game\npos pos={START} moves=\"32-28 19-23\"\n")
        );
        assert_eq!(d.go(&about, Limit::Time(1500), 1), "level move-time=1.500\ngo think\n");
        let got: Vec<Event> = ["info depth=11 mean-depth=10.2 score=0.12 nodes=12345 time=0.10 nps=1.2 pv=\"28x19 14x23\"", "done move=28x19 ponder=14x23"].iter().flat_map(|l| d.read(l)).collect();
        assert_eq!(got[0], Event::Info(Info { depth: Some(11), line: 1, score: Some(Score::Cp(12)), nodes: Some(12345), pv: vec!["28x19".into(), "14x23".into()] }));
        assert_eq!(got[1], Event::Best(Best { mv: Some("28x19".into()), ponder: Some("14x23".into()) }));
        assert_eq!(d.set_option("tt-size", "22"), "set-param name=tt-size value=22\n");
    }
}
