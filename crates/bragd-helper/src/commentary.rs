//! What Commentary is written from: the Position store's facts about a move, gathered here so
//! that the window (Analyse, one move) and the Helper (a whole Slice, `Work::SliceCommentary`)
//! build the same prompt. Commentary explains the facts and invents none, so a move the store
//! knows nothing about has none.

use bragd_core::config::Config;
use bragd_core::i18n::tr;
use bragd_engines::protocol::Score;
use bragd_engines::seam::{self, Notation};
use bragd_games::State;
use bragd_library::library::Library;
use bragd_positions::{Annotation, Mark, Motif, Store};

type Res<T> = Result<T, String>;

/// The most moves of a line the facts give.
const LINE: usize = 10;

/// What Commentary on a move (or, before the first, on a position) is written from: the store's
/// facts about it, as the prompt gives them, and where the Commentary is kept.
#[derive(Clone, Debug, PartialEq)]
pub struct Facts {
    pub family: String,
    pub variant: String,
    /// The position the move was played in.
    pub key: u64,
    /// The move in the Engine's notation, as its Annotation is kept; empty for the position.
    pub mv: String,
    pub text: String,
}

/// The Game's Family, its positions in their Family's notation (before the first entry, then
/// after each) and their keys in the Position store.
pub fn positions(variant: &str, initial: Option<&str>, entries: &[String]) -> Res<(String, Vec<String>, Vec<u64>)> {
    let family = bragd_games::variant(variant).map_err(|e| e.to_string())?.family().key();
    // shortcut: each prefix replayed from the start, n²/2 moves; replay once with every
    // position kept if a long go Game makes this slow.
    let texts = (0..=entries.len()).map(|i| bragd_games::replay(variant, initial, &entries[..i]).map(|s| s.position)).collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    let keys = bragd_library::library::position_keys(family, &texts);
    Ok((family.to_string(), texts, keys))
}

/// An Annotation worded from Bragd's own texts; None when it says nothing.
pub fn worded(mark: Option<Mark>, motifs: &[Motif], better: &[(String, String)]) -> Option<String> {
    let mut words: Vec<String> = mark.map(|m| tr(&format!("annotation.mark.{}", m.id()), &[])).into_iter().collect();
    words.extend(motifs.iter().map(|m| tr(&format!("annotation.motif.{}", m.id()), &[])));
    let bad = matches!(mark, Some(Mark::Blunder | Mark::Mistake | Mark::Inaccuracy)) || motifs.contains(&Motif::MissedMate);
    if bad && !better.is_empty() {
        let line: Vec<&str> = better.iter().map(|(_, n)| n.as_str()).collect();
        words.push(tr("annotation.better", &[("line", &line.join(" "))]));
    }
    (!words.is_empty()).then(|| words.join(" "))
}

/// Move `index` as the move list writes it, `12. Nf3`; a reply has no number in the move list,
/// so here it is the move's, as `3… Qg5`.
pub fn label(state: &State, index: usize) -> String {
    let number = match state.numbers.get(index).map_or("", String::as_str) {
        "" if index > 0 => state.numbers[index - 1].replace('.', "…"),
        n => n.to_string(),
    };
    format!("{number} {}", state.history.get(index).map_or("", String::as_str)).trim().to_string()
}

/// A Game as its facts are read from the store, by one Engine.
struct Game<'a> {
    store: &'a Store,
    engine: String,
    notation: Notation,
    variant: &'a str,
    initial: Option<&'a str>,
    entries: &'a [String],
    /// Each entry in the Engine's notation.
    moves: Vec<String>,
    family: String,
    texts: Vec<String>,
    keys: Vec<u64>,
    state: State,
}

impl<'a> Game<'a> {
    /// The Game read by the Engine `engine` (or the Variant's); None without one.
    fn new(cfg: &Config, store: &'a Store, variant: &'a str, initial: Option<&'a str>, entries: &'a [String], engine: Option<String>) -> Res<Option<Game<'a>>> {
        let Some(key) = engine.filter(|k| !k.is_empty()).or_else(|| bragd_engines::local::default_for(cfg, variant)) else { return Ok(None) };
        let chosen = bragd_engines::local::resolve(cfg, &key, variant)?;
        let notation = seam::notation(variant, chosen.protocols[0], bragd_engines::known::known(&key).map_or(key.as_str(), |k| k.name));
        let (pos, _) = seam::position(variant, initial, entries, notation)?;
        let (family, texts, keys) = positions(variant, initial, entries)?;
        let state = bragd_games::replay(variant, initial, entries).map_err(|e| e.to_string())?;
        Ok(Some(Game { store, engine: key, notation, variant, initial, entries, moves: pos.moves, family, texts, keys, state }))
    }

    /// The Annotation of entry `at`.
    fn note(&self, at: usize) -> Res<Option<Annotation>> {
        let Some(mv) = self.moves.get(at) else { return Ok(None) };
        self.store.annotation(&self.family, self.variant, self.keys[at], mv, &self.engine).map_err(|e| e.to_string())
    }

    /// A line in the Variant's notation, from the position after `upto` entries.
    fn words(&self, upto: usize, line: &str) -> String {
        let pv: Vec<String> = line.split_whitespace().take(LINE).map(String::from).collect();
        let l = seam::line(self.variant, self.initial, &self.entries[..upto], self.notation, &pv);
        if l.is_empty() { pv.join(" ") } else { l.into_iter().map(|(_, n)| n).collect::<Vec<_>>().join(" ") }
    }

    /// The facts about entry `n - 1`, the last of the first `n` (the position itself for 0):
    /// the position before it, the move, the Evaluation with its second line, the score after
    /// it, and the Annotation worded. None when the store has none of them.
    fn facts(&self, n: usize) -> Res<Option<Facts>> {
        let at = n.saturating_sub(1);
        let mv = if n > 0 { self.moves.get(at).cloned().unwrap_or_default() } else { String::new() };
        let shown = |s: bragd_positions::Score| Score::from(s).shown();
        let before = self.store.shown(&self.family, self.variant, self.keys[at], &self.engine).map_err(|e| e.to_string())?;
        let after = if n > 0 { self.store.shown(&self.family, self.variant, self.keys[n], &self.engine).map_err(|e| e.to_string())? } else { None };
        let note = if n > 0 { self.note(at)? } else { None };
        if before.is_none() && after.is_none() && note.is_none() {
            return Ok(None);
        }
        let variant = self.variant;
        let mut text = vec![format!("Game: {}, variant {variant}.", self.family)];
        text.push(format!("Position{}: {}", if n > 0 { " before the move" } else { "" }, self.texts[at]));
        if n > 0 {
            let side = bragd_games::replay(variant, self.initial, &self.entries[..at]).ok().and_then(|s| s.to_move).map(|s| format!(", by {}", s.name)).unwrap_or_default();
            text.push(format!("Move played: {}{side}.", label(&self.state, at)));
        }
        if let Some(e) = &before {
            text.push(format!("Engine: {} (depth {}).", e.edition.version, e.depth));
            text.push(format!("Evaluation{}: {}. Best line: {}.", if n > 0 { " before the move" } else { "" }, shown(e.score), self.words(at, &e.line)));
            if let Some(s) = &e.second {
                text.push(format!("Second choice: {}. Its line: {}.", shown(s.score), self.words(at, &s.line)));
            }
        }
        if let Some(e) = &after {
            text.push(format!("Evaluation after the move: {} (depth {}). Best reply: {}.", shown(e.score), e.depth, self.words(n, &e.line)));
        }
        if let Some(a) = &note {
            let better: Vec<String> = a.better.split_whitespace().take(LINE).map(String::from).collect();
            let better = if better.is_empty() { vec![] } else { seam::line(variant, self.initial, &self.entries[..at], self.notation, &better) };
            if let Some(words) = worded(a.mark, &a.motifs, &better) {
                text.push(format!("Annotation: {}{} {words}", label(&self.state, at), a.mark.map_or("", Mark::sign)));
            }
        }
        text.push("Scores are from the view of the side that moved first: + is good for it.".into());
        Ok(Some(Facts { family: self.family.clone(), variant: variant.into(), key: self.keys[at], mv, text: text.join("\n") }))
    }
}

/// The facts the store holds about the Game's last move by the Engine `engine` (or the
/// Variant's), or about its position before the first move.
pub fn facts(cfg: &Config, store: &Store, variant: &str, initial: Option<&str>, entries: &[String], engine: Option<String>) -> Res<Option<Facts>> {
    match Game::new(cfg, store, variant, initial, entries, engine)? {
        Some(g) => g.facts(entries.len()),
        None => Ok(None),
    }
}

/// The facts about every move of the Game that the Variant's Engine has judged: those with an
/// Annotation in the store. Only these are worth a Commentary.
pub fn judged(cfg: &Config, store: &Store, variant: &str, initial: Option<&str>, entries: &[String]) -> Res<Vec<Facts>> {
    let Some(g) = Game::new(cfg, store, variant, initial, entries, None)? else { return Ok(vec![]) };
    let mut out = vec![];
    for at in 0..entries.len() {
        if g.note(at)?.is_some()
            && let Some(f) = g.facts(at + 1)?
        {
            out.push(f);
        }
    }
    Ok(out)
}

/// The judged moves of the Games of the Slice `slice` of `family` (empty: any) that have no
/// Commentary in `language` yet, each once: what a Slice job asks the model to write. `stop` is
/// asked between Games; `progress` hears the Games done of all.
#[allow(clippy::too_many_arguments, reason = "the job's fields, then its stop and progress, as the other jobs take them")]
pub fn slice(cfg: &Config, store: &Store, lib: &Library, slice: &str, family: &str, language: &str, stop: &dyn Fn() -> bool, progress: &mut dyn FnMut(u64, u64)) -> Res<Vec<Facts>> {
    let ids = lib.slice_games(slice).map_err(|e| e.to_string())?;
    let mut seen = std::collections::HashSet::new();
    let mut out = vec![];
    for (i, &id) in ids.iter().enumerate() {
        if stop() {
            break;
        }
        progress(i as u64, ids.len() as u64);
        let s = lib.stored(id).map_err(|e| e.to_string())?;
        if !family.is_empty() && s.family != family {
            continue;
        }
        let Some((initial, entries)) = lib.played(id).map_err(|e| e.to_string())? else { continue };
        // A Game whose Variant has no Engine here, or that cannot be played again, has no facts.
        let Ok(found) = judged(cfg, store, &s.variant, initial.as_deref(), &entries) else { continue };
        for f in found {
            let written = store.commentary(&f.family, &f.variant, f.key, &f.mv, language).map_err(|e| e.to_string())?.is_some();
            if !written && seen.insert((f.variant.clone(), f.key, f.mv.clone())) {
                out.push(f);
            }
        }
    }
    progress(ids.len() as u64, ids.len() as u64);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bragd_positions::{Commentary, Edition, Evaluation, Second};
    use std::path::PathBuf;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bragd-helper-commentary-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// An Engine `factsfake` added for chess, by a path that is there, and chess's default.
    fn with_engine(d: &std::path::Path) -> Config {
        let p = d.join("factsfake");
        std::fs::write(&p, "").unwrap();
        let mut cfg = Config::default();
        cfg.engines.added.insert("factsfake".into(), "chess".into());
        cfg.engines.paths.insert("factsfake".into(), p.display().to_string());
        cfg.engines.default.insert("chess".into(), "factsfake".into());
        cfg
    }

    fn edition() -> Edition {
        Edition { engine: "factsfake".into(), version: "Facts 1".into(), network: None, image: None }
    }

    fn note(key: u64, mv: &str, mark: Mark, better: &str) -> Annotation {
        Annotation { family: "chess".into(), variant: "chess".into(), key, mv: mv.into(), edition: edition(), mark: Some(mark), better: better.into(), motifs: vec![], depth: 22, time: 1 }
    }

    fn entries(m: &[&str]) -> Vec<String> {
        m.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn an_annotation_is_worded_in_the_readers_language() {
        let better = [("g1f3".to_string(), "Nf3".to_string()), ("b8c6".to_string(), "Nc6".to_string())];
        assert_eq!(worded(Some(Mark::Blunder), &[Motif::Hangs], &better).unwrap(), "Blunder. It leaves a piece hanging. Better was Nf3 Nc6.");
        // A good move names no better one, and a move with nothing to say has no note.
        assert_eq!(worded(Some(Mark::Good), &[Motif::Fork], &better).unwrap(), "Good move. It forks two pieces.");
        assert_eq!(worded(None, &[], &better), None);
        assert_eq!(worded(None, &[Motif::MissedMate], &better).unwrap(), "It misses a forced mate. Better was Nf3 Nc6.");
    }

    /// Commentary's facts come from the store only: none before the Engine has judged the move,
    /// then the position, the move, the Evaluation with its second line, the score after and the
    /// Annotation, in the Variant's notation.
    #[test]
    fn commentary_facts_are_the_stores() {
        let d = dir("facts");
        let cfg = with_engine(&d);
        let entries = entries(&["e2e4", "e7e5", "g1f3"]);
        let (_, _, keys) = positions("chess", None, &entries).unwrap();
        let mut store = Store::open(&d.join("positions.sqlite")).unwrap();
        assert!(facts(&cfg, &store, "chess", None, &entries, None).unwrap().is_none(), "nothing judged yet");
        let second = Some(Second { score: bragd_positions::Score::Cp(-40), line: "d2d4 e5d4".into() });
        store.put(&Evaluation { family: "chess".into(), variant: "chess".into(), key: keys[2], edition: edition(), score: bragd_positions::Score::Cp(35), depth: 22, line: "b1c3 g8f6".into(), time: 1, second }).unwrap();
        store.put(&Evaluation { family: "chess".into(), variant: "chess".into(), key: keys[3], edition: edition(), score: bragd_positions::Score::Cp(20), depth: 20, line: "b8c6".into(), time: 1, second: None }).unwrap();
        store.put_annotation(&note(keys[2], "g1f3", Mark::Inaccuracy, "b1c3")).unwrap();
        let f = facts(&cfg, &store, "chess", None, &entries, Some("factsfake".into())).unwrap().unwrap();
        assert_eq!((f.family.as_str(), f.key, f.mv.as_str()), ("chess", keys[2], "g1f3"));
        for want in ["rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq", "Move played: 2. Nf3, by White.", "Facts 1 (depth 22)", "+0.35. Best line: Nc3 Nf6.", "Second choice: −0.40. Its line: d4 exd4.", "after the move: +0.20 (depth 20). Best reply: Nc6.", "Annotation: 2. Nf3?! Inaccuracy. Better was Nc3."] {
            assert!(f.text.contains(want), "{want:?} not in\n{}", f.text);
        }
        // Before the first move: the position itself, which nothing judged here.
        assert!(facts(&cfg, &store, "chess", None, &[], None).unwrap().is_none());
        // Every judged move of the Game, and only those: the last move has the facts above.
        assert_eq!(judged(&cfg, &store, "chess", None, &entries).unwrap(), [f]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// A Slice's job: each judged move of its Games once, of the Family asked, without the ones
    /// that already have Commentary in the language.
    #[test]
    fn a_slice_gathers_its_judged_moves_not_yet_written() {
        let d = dir("slice");
        let cfg = with_engine(&d);
        let pgn = d.join("g.pgn");
        // The same opening twice: a move judged once is asked about once.
        std::fs::write(&pgn, "[White \"A\"]\n[Black \"B\"]\n\n1. e4 e5 2. Nf3 Nc6 1-0\n\n[White \"C\"]\n[Black \"D\"]\n\n1. e4 e5 2. Nf3 Nf6 0-1\n").unwrap();
        let mut lib = Library::open(&d.join("l.bragd")).unwrap();
        lib.import(&pgn, "mine", &|| false, &mut |_| {}).unwrap();
        lib.save_slice("All", &bragd_library::slice::Query::default()).unwrap();
        let (_, _, keys) = positions("chess", None, &entries(&["e2e4", "e7e5", "g1f3"])).unwrap();
        let mut store = Store::open(&d.join("positions.sqlite")).unwrap();
        store.put_annotation(&note(keys[1], "e7e5", Mark::Good, "")).unwrap();
        store.put_annotation(&note(keys[2], "g1f3", Mark::Good, "")).unwrap();
        store.put_commentary(&Commentary { family: "chess".into(), variant: "chess".into(), key: keys[2], mv: "g1f3".into(), language: "da".into(), model: "m".into(), text: "Springeren ud.".into(), time: 1 }).unwrap();
        let ask = |family: &str, language: &str| {
            let mut seen = (0, 0);
            let found = slice(&cfg, &store, &lib, "All", family, language, &|| false, &mut |n, of| seen = (n, of)).unwrap();
            assert_eq!(seen, (2, 2), "both Games read");
            found.into_iter().map(|f| f.mv).collect::<Vec<_>>()
        };
        assert_eq!(ask("chess", "en-GB"), ["e7e5", "g1f3"]);
        assert_eq!(ask("", "da"), ["e7e5"], "Nf3 has Danish Commentary already");
        assert!(ask("go", "en-GB").is_empty(), "no go Games in it");
        assert!(slice(&cfg, &store, &lib, "All", "", "da", &|| true, &mut |_, _| {}).unwrap().is_empty(), "stopped at once");
        std::fs::remove_dir_all(&d).unwrap();
    }
}
