//! Commentary: prose about a position or a move, written by a language model in one language
//! from the Evaluation and the Annotation, and labelled as machine-written (`GLOSSARY.md`). It is
//! kept apart from Annotations, in a table of its own, by position, move, language and model.
//!
//! Peers merge it as they merge the rest: for one position, move, language and model the store
//! keeps the row greater by `(time, text)`, so the latest one wins and any order gives the same
//! store.

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::{Result, Store, sync};

/// One model's prose about one move (or, with `mv` empty, one position) in one language.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commentary {
    pub family: String,
    pub variant: String,
    /// The position's key: for a move, the position it was played in.
    pub key: u64,
    /// The move in the engine's notation, as an Annotation's; empty for the position itself.
    pub mv: String,
    /// The locale code it is written in.
    pub language: String,
    /// The model that wrote it.
    pub model: String,
    pub text: String,
    /// When it was written, in seconds since 1970.
    pub time: i64,
}

impl Store {
    /// Merges `c`: kept if the store has none for its position, move, language and model, or
    /// if it is greater than that one by `(time, text)`. True when the store changed.
    pub fn put_commentary(&mut self, c: &Commentary) -> Result<bool> {
        let tx = self.conn.transaction()?;
        let changed = put(&tx, c)?;
        tx.commit()?;
        Ok(changed)
    }

    /// What Bragd shows for a move in a language: the latest Commentary, of whichever model.
    pub fn commentary(&self, family: &str, variant: &str, key: u64, mv: &str, language: &str) -> Result<Option<Commentary>> {
        let mut all = self.select_commentary("WHERE family = ?1 AND variant = ?2 AND key = ?3 AND move = ?4 AND language = ?5 ORDER BY time DESC, model LIMIT 1", params![family, variant, key as i64, mv, language])?;
        Ok(all.pop().map(|(_, c)| c))
    }

    /// Every Commentary in the store, by position, move, language and model.
    pub fn commentaries(&self) -> Result<Vec<Commentary>> {
        Ok(self.select_commentary("ORDER BY family, variant, key, move, language, model", [])?.into_iter().map(|(_, c)| c).collect())
    }

    /// Commentary rows, each with its change number (`sync`).
    pub(crate) fn select_commentary(&self, rest: &str, p: impl rusqlite::Params) -> Result<Vec<(u64, Commentary)>> {
        let mut st = self.conn.prepare(&format!("SELECT family, variant, key, move, language, model, text, time, seq FROM commentary {rest}"))?;
        let rows = st.query_map(p, |r| {
            Ok((
                r.get::<_, i64>(8)? as u64,
                Commentary { family: r.get(0)?, variant: r.get(1)?, key: r.get::<_, i64>(2)? as u64, mv: r.get(3)?, language: r.get(4)?, model: r.get(5)?, text: r.get(6)?, time: r.get(7)? },
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

/// The merge rule for Commentary: insert, or replace one that is less by `(time, text)`.
pub(crate) fn put(tx: &Connection, c: &Commentary) -> Result<bool> {
    let seq = sync::next(tx)?;
    let changed = tx
        .prepare_cached(
            "INSERT INTO commentary (family, variant, key, move, language, model, text, time, seq) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
             ON CONFLICT (family, variant, key, move, language, model) DO UPDATE SET text = excluded.text, time = excluded.time, seq = excluded.seq \
             WHERE (excluded.time, excluded.text) > (commentary.time, commentary.text)",
        )?
        .execute(params![c.family, c.variant, c.key as i64, c.mv, c.language, c.model, c.text, c.time, seq])?;
    sync::taken(tx, seq, changed > 0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn said(key: u64, mv: &str, language: &str, model: &str, time: i64, text: &str) -> Commentary {
        Commentary { family: "chess".into(), variant: "chess".into(), key, mv: mv.into(), language: language.into(), model: model.into(), text: text.into(), time }
    }

    /// Every order of the rows, and every row twice, gives the same store: per position, move,
    /// language and model the latest wins, and a tie goes to the greater text.
    #[test]
    fn commentary_merges_the_same_in_every_order() {
        let rows = [
            said(5, "e2e4", "da", "qwen3", 10, "Et godt træk."),
            said(5, "e2e4", "da", "qwen3", 20, "Hvid tager centrum."),
            said(5, "e2e4", "da", "qwen3", 20, "Hvid åbner."),
            said(5, "e2e4", "da", "gemma", 5, "Sort må svare."),
            said(5, "e2e4", "en-GB", "qwen3", 1, "White takes the centre."),
            said(5, "", "da", "qwen3", 1, "Stillingen er lige."),
            said(6, "e2e4", "da", "qwen3", 1, "Et andet sted."),
        ];
        // Every permutation of seven rows is 5,040 stores: each order's own, checked against the first.
        let mut want = None;
        let mut order: Vec<usize> = (0..rows.len()).collect();
        let mut seen = 0;
        permute(&mut order, 0, &mut |o| {
            seen += 1;
            let mut s = Store::open_in_memory().unwrap();
            for &i in o {
                s.put_commentary(&rows[i]).unwrap();
            }
            for &i in o {
                assert!(!s.put_commentary(&rows[i]).unwrap(), "a second merge changes nothing");
            }
            let got = s.commentaries().unwrap();
            match &want {
                None => want = Some(got),
                Some(w) => assert_eq!(&got, w, "order {o:?}"),
            }
        });
        assert_eq!(seen, 5040);
        let want = want.unwrap();
        assert_eq!(want.len(), 5, "two models, two languages, the position, another key");
        assert!(want.contains(&rows[2]), "of two at the same time, the greater text: 'å' sorts after 't'");
        let mut s = Store::open_in_memory().unwrap();
        for r in &rows {
            s.put_commentary(r).unwrap();
        }
        assert_eq!(s.commentary("chess", "chess", 5, "e2e4", "da").unwrap(), Some(rows[2].clone()), "the latest of any model shows");
        assert_eq!(s.commentary("chess", "chess", 5, "e2e4", "en-GB").unwrap(), Some(rows[4].clone()));
        assert_eq!(s.commentary("chess", "chess", 5, "e2e4", "fr").unwrap(), None, "kept per language");
        assert_eq!(s.commentary("chess", "chess", 5, "", "da").unwrap(), Some(rows[5].clone()));
        assert_eq!(s.commentary("go", "chess", 5, "e2e4", "da").unwrap(), None);
    }

    fn permute(v: &mut Vec<usize>, k: usize, f: &mut impl FnMut(&[usize])) {
        if k == v.len() {
            return f(v);
        }
        for i in k..v.len() {
            v.swap(k, i);
            permute(v, k + 1, f);
            v.swap(k, i);
        }
    }
}
