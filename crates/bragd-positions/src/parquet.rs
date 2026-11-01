//! The store exported to Parquet and imported from it: for backups, moving a store by hand,
//! and reading it in DuckDB. Each row carries its Edition whole, so a file reads on its own.
//! An import merges by the store's one rule (`Store::put_all`).

use std::path::Path;

use crate::{Merged, Result, Store};

/// Writes every Evaluation in `store` to the Parquet file `out` (Snappy-compressed): the rows
/// written. Written beside it first (`<out>.part`) and then put in place, so a failed export
/// leaves a file that was there before whole.
///
/// Columns, in this order: `family` (string), `variant` (string), `key` (int64, the u64 key's
/// bits), `edition` (string, `Edition::id`), `engine`, `version` (string), `network`, `image`
/// (optional string), `kind` (string: cp, mate, winrate, equity), `score` (double), `lead`
/// (double), `depth` (int32), `line` (string), `time` (int64).
pub fn to_parquet(store: &Store, out: &Path) -> Result<u64> {
    let _ = (store, out);
    todo!()
}

/// Merges every row of the Parquet file `path` (as `to_parquet` writes it) into `store`, in
/// one transaction: nothing is merged when a row cannot be read.
pub fn from_parquet(store: &mut Store, path: &Path) -> Result<Merged> {
    let _ = (store, path);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{dir, eval, sf};
    use crate::{Edition, Evaluation, Score};

    fn rows() -> Vec<Evaluation> {
        vec![
            eval(u64::MAX, sf("Stockfish 17"), 20, 100, Score::Cp(-30)),
            eval(3, sf("Stockfish 17.1"), 9, 100, Score::Mate(4)),
            Evaluation { family: "go".into(), variant: "go-19".into(), key: 1 << 63, edition: Edition { engine: "katago".into(), version: "KataGo 1.18.2".into(), network: Some("ab12".into()), image: None }, score: Score::WinRate { win: 0.625, lead: -1.5 }, depth: 400, line: "Q16 D4".into(), time: 5 },
            Evaluation { family: "backgammon".into(), variant: "backgammon".into(), key: 0, edition: Edition { engine: "gnubg".into(), version: "GNU Backgammon 1.08.003".into(), network: None, image: None }, score: Score::Equity(0.125), depth: 2, line: String::new(), time: -1 },
        ]
    }

    #[test]
    fn a_store_exports_and_imports_back_whole() {
        let d = dir("parquet");
        let mut a = Store::open(&d.join("a.sqlite")).unwrap();
        a.put_all(&rows()).unwrap();
        let out = d.join("positions.parquet");
        assert_eq!(to_parquet(&a, &out).unwrap(), 4);
        let mut b = Store::open(&d.join("b.sqlite")).unwrap();
        assert_eq!(from_parquet(&mut b, &out).unwrap(), Merged { read: 4, changed: 4 });
        assert_eq!(b.all().unwrap(), a.all().unwrap());
        // Imported again: read, and nothing changes.
        assert_eq!(from_parquet(&mut b, &out).unwrap(), Merged { read: 4, changed: 0 });
    }

    /// An import is a merge: a deeper row in the store stays, a deeper row in the file wins.
    #[test]
    fn an_import_keeps_the_deeper_row() {
        let d = dir("parquet-merge");
        let mut a = Store::open(&d.join("a.sqlite")).unwrap();
        a.put(&eval(1, sf("Stockfish 17"), 30, 1, Score::Cp(5))).unwrap();
        a.put(&eval(2, sf("Stockfish 17"), 10, 1, Score::Cp(5))).unwrap();
        let out = d.join("a.parquet");
        to_parquet(&a, &out).unwrap();
        let mut b = Store::open(&d.join("b.sqlite")).unwrap();
        b.put(&eval(1, sf("Stockfish 17"), 20, 9, Score::Cp(7))).unwrap();
        b.put(&eval(2, sf("Stockfish 17"), 40, 9, Score::Cp(7))).unwrap();
        assert_eq!(from_parquet(&mut b, &out).unwrap(), Merged { read: 2, changed: 1 });
        let depths: Vec<(u64, u32)> = b.all().unwrap().iter().map(|e| (e.key, e.depth)).collect();
        assert_eq!(depths, [(1, 30), (2, 40)]);
    }

    #[test]
    fn a_failed_export_leaves_the_old_file_and_a_bad_file_merges_nothing() {
        let d = dir("parquet-fail");
        let mut a = Store::open(&d.join("a.sqlite")).unwrap();
        a.put_all(&rows()).unwrap();
        let out = d.join("a.parquet");
        to_parquet(&a, &out).unwrap();
        let before = std::fs::read(&out).unwrap();
        a.conn().execute_batch("DROP TABLE evaluations").unwrap();
        assert!(to_parquet(&a, &out).is_err());
        assert_eq!(std::fs::read(&out).unwrap(), before);
        assert!(!d.join("a.parquet.part").exists());
        let bad = d.join("bad.parquet");
        std::fs::write(&bad, b"not parquet").unwrap();
        let mut b = Store::open(&d.join("b.sqlite")).unwrap();
        assert!(from_parquet(&mut b, &bad).is_err());
        assert!(b.all().unwrap().is_empty());
    }
}
