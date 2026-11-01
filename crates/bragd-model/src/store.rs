//! Translations of Comments, kept on this machine in `translations.sqlite` beside the Position
//! store, keyed by the original text and the language translated into. Keying by the text
//! means a Comment that turns up in many Games, or a Game imported again, is translated once.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

/// A Comment machine-translated into one language.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Translation {
    pub text: String,
    /// The model that wrote it, as the server named it.
    pub model: String,
    /// When, in seconds since 1970.
    pub time: i64,
}

/// The store's format (`PRAGMA user_version`).
pub const FORMAT: bragd_core::format::Sqlite = bragd_core::format::Sqlite { file: "translations.sqlite", first: 1, schema: SCHEMA, migrations: &[] };

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS translations (
    original TEXT NOT NULL,
    lang TEXT NOT NULL,
    text TEXT NOT NULL,
    model TEXT NOT NULL,
    time INTEGER NOT NULL,
    PRIMARY KEY (original, lang)
) WITHOUT ROWID;
";

pub struct Translations {
    conn: Connection,
}

impl Translations {
    /// `translations.sqlite` in Bragd's data folder.
    pub fn path() -> Option<PathBuf> {
        Some(bragd_core::dirs::app_data()?.join("translations.sqlite"))
    }

    /// The store at `path`, made with its table when it is new (parent folders too).
    pub fn open(path: &Path) -> rusqlite::Result<Translations> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|_| rusqlite::Error::InvalidPath(dir.to_path_buf()))?;
        }
        let mut conn = Connection::open(path)?;
        FORMAT.open(&mut conn)?;
        Ok(Translations { conn })
    }

    /// `original`'s translation into `lang`, if one is kept.
    pub fn get(&self, original: &str, lang: &str) -> rusqlite::Result<Option<Translation>> {
        self.conn
            .query_row(
                "SELECT text, model, time FROM translations WHERE original = ?1 AND lang = ?2",
                (original, lang),
                |row| Ok(Translation { text: row.get(0)?, model: row.get(1)?, time: row.get(2)? }),
            )
            .optional()
    }

    /// Keeps `t` as `original`'s translation into `lang`, replacing one kept before.
    pub fn put(&self, original: &str, lang: &str, t: &Translation) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO translations (original, lang, text, model, time) VALUES (?1, ?2, ?3, ?4, ?5)",
            (original, lang, &t.text, &t.model, t.time),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tr(text: &str, time: i64) -> Translation {
        Translation { text: text.into(), model: "m".into(), time }
    }

    #[test]
    fn store_keeps_one_translation_per_text_and_language() {
        let dir = std::env::temp_dir().join(format!("bragd-model-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("sub").join("translations.sqlite");
        let s = Translations::open(&path).unwrap();
        assert_eq!(s.get("Gut", "en").unwrap(), None);
        s.put("Gut", "en", &tr("Good", 1)).unwrap();
        s.put("Gut", "da", &tr("Godt", 2)).unwrap();
        s.put("Gut", "en", &tr("Well played", 3)).unwrap();
        drop(s);
        let s = Translations::open(&path).unwrap();
        assert_eq!(s.get("Gut", "en").unwrap(), Some(tr("Well played", 3)));
        assert_eq!(s.get("Gut", "da").unwrap(), Some(tr("Godt", 2)));
        assert_eq!(s.get("gut", "en").unwrap(), None, "the text is the key as it is");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
