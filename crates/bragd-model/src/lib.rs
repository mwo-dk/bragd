//! The language model: an OpenAI-compatible address set in Settings → Language model, off by
//! default. Local servers found running are offered; an address off this machine needs the
//! user's consent once, and its key lives in the OS keychain only. Comments are translated one
//! at a time, when the user asks: never in bulk.

pub mod client;
pub mod key;
pub mod lang;
pub mod store;

use bragd_core::config::ModelConfig;
use bragd_core::t;
use serde::Serialize;

pub use client::Found;
pub use store::{Translation, Translations};

/// The language model as Settings shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub url: String,
    pub model: String,
    /// The address is this machine: nothing sent there leaves it.
    pub on_this_machine: bool,
    /// A key is kept in the keychain for the address.
    pub key_saved: bool,
    /// The address is off this machine and the user has not agreed to it yet.
    pub consent_needed: bool,
    /// An address and a model are set, and consent given where it is needed.
    pub ready: bool,
}

/// Whether `m` can be asked: an address and a model, and the user's consent to an address off
/// this machine.
pub fn ready(m: &ModelConfig) -> bool {
    !m.url.trim().is_empty() && !m.name.trim().is_empty() && (client::on_this_machine(&m.url) || m.consent.trim() == m.url.trim())
}

pub fn status(m: &ModelConfig) -> Status {
    let set = !m.url.trim().is_empty();
    let local = client::on_this_machine(&m.url);
    Status {
        url: m.url.clone(),
        model: m.name.clone(),
        on_this_machine: local,
        key_saved: set && key::get(&m.url).is_some(),
        consent_needed: set && !local && m.consent.trim() != m.url.trim(),
        ready: ready(m),
    }
}

/// A Comment as Replay shows it: whether to offer *Translate*, and the translation kept for the
/// reader's language.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CommentView {
    /// A model is ready and the Comment is not known to be in the reader's language already.
    pub translatable: bool,
    pub translation: Option<Translation>,
}

/// `text` for a reader of `reader` (a locale code), with the store at `store`.
pub fn comment_view(m: &ModelConfig, store: Option<&Translations>, text: &str, reader: &str) -> CommentView {
    let text = text.trim();
    if text.is_empty() || lang::in_language(text, reader) {
        return CommentView::default();
    }
    CommentView {
        translatable: ready(m),
        translation: store.and_then(|s| s.get(text, reader).ok().flatten()),
    }
}

/// `text` translated into the language of `reader` by the model of `m`, kept nowhere: a Peer asks
/// this for another machine, which keeps it.
pub fn ask(m: &ModelConfig, text: &str, reader: &str) -> Result<Translation, String> {
    let text = text.trim();
    if !ready(m) {
        return Err(t!("model.error.not_ready"));
    }
    let language = bragd_core::i18n::find(reader).map_or(reader, |l| l.name);
    let system = format!(
        "Translate the user's text into {language} (language code {reader}). It is a comment from the record of a board game. \
         Keep move notation, square names, numbers, symbols and people's names exactly as they are. \
         Reply with the translation only, without quotes or explanations."
    );
    let out = client::chat(&m.url, key::get(&m.url).as_deref(), &m.name, &system, text)?;
    let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    Ok(Translation { text: out, model: m.name.clone(), time })
}

/// `text` translated into the language of `reader` by the model of `m`, and kept in `store`. A
/// translation already kept is given without asking the model.
pub fn translate(m: &ModelConfig, store: &Translations, text: &str, reader: &str) -> Result<Translation, String> {
    let text = text.trim();
    if let Some(t) = store.get(text, reader).map_err(|e| e.to_string())? {
        return Ok(t);
    }
    let t = ask(m, text, reader)?;
    store.put(text, reader, &t).map_err(|e| e.to_string())?;
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DE: &str = "Weiß hätte hier besser den Springer ziehen sollen, denn danach ist die Stellung verloren.";
    const EN: &str = "White should have moved the knight here, since after this the position is lost.";

    fn cfg(url: &str, name: &str, consent: &str) -> ModelConfig {
        ModelConfig { url: url.into(), name: name.into(), consent: consent.into() }
    }

    fn temp_store(tag: &str) -> (std::path::PathBuf, Translations) {
        let dir = std::env::temp_dir().join(format!("bragd-model-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let s = Translations::open(&dir.join("t.sqlite")).unwrap();
        (dir, s)
    }

    #[test]
    fn model_is_off_by_default_and_a_cloud_one_waits_for_consent() {
        key::mock();
        assert!(!ready(&ModelConfig::default()));
        assert!(!status(&ModelConfig::default()).consent_needed);
        assert!(ready(&cfg("http://localhost:11434/v1", "qwen3", "")));
        assert!(!ready(&cfg("http://localhost:11434/v1", "", "")), "no model chosen");
        let cloud = "https://api.example.com/v1";
        let s = status(&cfg(cloud, "gpt", ""));
        assert!(s.consent_needed && !s.ready && !s.on_this_machine);
        assert!(!ready(&cfg(cloud, "gpt", "https://other.example.com/v1")), "consent is per address");
        assert!(ready(&cfg(cloud, "gpt", cloud)));
    }

    #[test]
    fn model_comment_in_the_readers_language_offers_nothing() {
        let m = cfg("http://localhost:1/v1", "m", "");
        assert_eq!(comment_view(&m, None, EN, "en-GB"), CommentView::default());
        assert!(comment_view(&m, None, DE, "en-GB").translatable);
        assert!(!comment_view(&ModelConfig::default(), None, DE, "en-GB").translatable, "no model, no button");
        assert!(comment_view(&m, None, "+-", "en-GB").translatable, "a scrap may be in any language");
    }

    #[test]
    fn model_translates_once_and_keeps_it_per_language() {
        key::mock();
        let (url, seen) = client::tests::fake(vec![(200, r#"{"choices":[{"message":{"content":"White should have moved the knight."}}]}"#.into())]);
        let m = cfg(&url, "qwen3", "");
        let (dir, s) = temp_store("translate");
        let t = translate(&m, &s, DE, "en-GB").unwrap();
        assert_eq!((t.text.as_str(), t.model.as_str()), ("White should have moved the knight.", "qwen3"));
        // Kept: the second time the model is not asked (the fake answers only once).
        assert_eq!(translate(&m, &s, DE, "en-GB").unwrap(), t);
        assert_eq!(comment_view(&m, Some(&s), DE, "en-GB").translation, Some(t));
        assert_eq!(comment_view(&m, Some(&s), DE, "da").translation, None, "kept per language");
        let seen = seen.lock().unwrap();
        assert!(seen[0].2.contains("(language code en-GB)"), "{}", seen[0].2);
        assert_eq!(seen.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A Comment of a Game imported into a Library comes back with it and translates.
    #[test]
    fn a_library_games_comment_translates() {
        key::mock();
        let (dir, s) = temp_store("library");
        let file = dir.join("opera.pgn");
        std::fs::write(&file, include_str!("../../bragd-library/tests/annotated.pgn")).unwrap();
        let mut lib = bragd_library::library::Library::open(&dir.join("n.bragd")).unwrap();
        lib.import(&file, "mine", &|| false, &mut |_| {}).unwrap();
        let comment = lib.stored(1).unwrap().notes.into_iter().find(|n| n.ply == 4).unwrap().comment;
        let (url, seen) = client::tests::fake(vec![(200, r#"{"choices":[{"message":{"content":"White should have moved the knight here."}}]}"#.into())]);
        let m = cfg(&url, "qwen3", "");
        assert!(comment_view(&m, Some(&s), &comment, "en-GB").translatable);
        assert_eq!(translate(&m, &s, &comment, "en-GB").unwrap().text, "White should have moved the knight here.");
        assert!(seen.lock().unwrap()[0].2.contains("Springer"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A Peer asks its model for another machine, which keeps the translation: `ask` keeps
    /// nothing, and the translation goes over the wire as JSON.
    #[test]
    fn model_ask_keeps_nothing_and_travels() {
        key::mock();
        let (url, _seen) = client::tests::fake(vec![(200, r#"{"choices":[{"message":{"content":"White should have moved the knight."}}]}"#.into())]);
        let t = ask(&cfg(&url, "qwen3", ""), DE, "en-GB").unwrap();
        assert_eq!((t.text.as_str(), t.model.as_str()), ("White should have moved the knight.", "qwen3"));
        let back: Translation = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
        assert_eq!(ask(&ModelConfig::default(), DE, "en-GB").unwrap_err(), t!("model.error.not_ready"));
    }

    #[test]
    fn model_not_ready_asks_nothing() {
        let (dir, s) = temp_store("off");
        assert_eq!(translate(&cfg("https://api.example.com/v1", "gpt", ""), &s, DE, "en-GB").unwrap_err(), t!("model.error.not_ready"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
