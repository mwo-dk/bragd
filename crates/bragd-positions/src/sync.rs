//! Peers swap what is new in their stores (`docs/peers/sync.md`).
//!
//! Every change to a row gives it the store's next change number (`seq`, schema 4). A Peer asks
//! for the rows changed since the last number it got from this store (`Store::changes`), a
//! bounded batch at a time, and merges them by the store's merge rule (`Store::merge_changes`).
//! The number it got so far, its watermark, is kept by the Peer that asks, in its own store and
//! in the same transaction as the merge, so a batch is never half taken, and a store deleted
//! and made again asks its Peers for everything.
//!
//! A row merged from one Peer that changes this store gets a number here too, so it goes on to
//! this store's other Peers: three machines converge though only two of them ever talk. It also
//! goes back once to the Peer it came from, where it changes nothing.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{Annotation, Evaluation, Merged, Result, Store};

/// Schema 4: a change number on every row, the counter, this store's id, and what was taken
/// from each Peer.
pub(crate) const MIGRATE_4: &str = "
ALTER TABLE evaluations ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
ALTER TABLE annotations ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
UPDATE evaluations SET seq = n.s
    FROM (SELECT family, variant, key, edition, row_number() OVER (ORDER BY family, variant, key, edition) AS s FROM evaluations) AS n
    WHERE n.family = evaluations.family AND n.variant = evaluations.variant AND n.key = evaluations.key AND n.edition = evaluations.edition;
UPDATE annotations SET seq = n.s + (SELECT count(*) FROM evaluations)
    FROM (SELECT family, variant, key, move, edition, row_number() OVER (ORDER BY family, variant, key, move, edition) AS s FROM annotations) AS n
    WHERE n.family = annotations.family AND n.variant = annotations.variant AND n.key = annotations.key AND n.move = annotations.move AND n.edition = annotations.edition;
CREATE INDEX evaluations_seq ON evaluations (seq);
CREATE INDEX annotations_seq ON annotations (seq);
-- The last change number given.
CREATE TABLE changes (n INTEGER NOT NULL);
INSERT INTO changes (n) SELECT (SELECT count(*) FROM evaluations) + (SELECT count(*) FROM annotations);
-- This store's id, made the first time a Peer asks: a store made again is another store.
CREATE TABLE store (id TEXT NOT NULL);
-- What was taken from each Peer: its store's id and the last change number, when, and how many rows.
CREATE TABLE pulled (
    peer TEXT PRIMARY KEY,
    store TEXT NOT NULL,
    seq INTEGER NOT NULL,
    time INTEGER NOT NULL,
    rows INTEGER NOT NULL
) WITHOUT ROWID;
";

/// The next change number, for a row about to be written.
pub(crate) fn next(tx: &rusqlite::Connection) -> Result<i64> {
    Ok(tx.prepare_cached("SELECT n + 1 FROM changes")?.query_row([], |r| r.get(0))?)
}

/// The counter moves on to `seq` when the row written with it changed the store.
pub(crate) fn taken(tx: &rusqlite::Connection, seq: i64, changed: bool) -> Result<bool> {
    if changed {
        tx.prepare_cached("UPDATE changes SET n = ?1")?.execute([seq])?;
    }
    Ok(changed)
}

/// A batch of rows changed in a store since a change number.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Changes {
    /// The store's id.
    pub store: String,
    pub evaluations: Vec<Evaluation>,
    pub annotations: Vec<Annotation>,
    /// The change number of the last row in the batch: where the next one starts.
    pub last: u64,
    /// More rows changed after `last`.
    pub more: bool,
}

/// What was taken from a Peer so far.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pulled {
    /// The Peer's id (its key).
    pub peer: String,
    /// Its store's id, and the change number taken up to.
    pub store: String,
    pub seq: u64,
    /// When, in seconds since 1970.
    pub time: u64,
    /// Rows read from it, all told.
    pub rows: u64,
}

/// About what a row takes on the wire, for the batch's bound.
fn weight_e(e: &Evaluation) -> usize {
    160 + e.family.len() + e.variant.len() + e.line.len() + e.edition.id().len() + e.second.as_ref().map_or(0, |s| s.line.len() + 40)
}

fn weight_a(a: &Annotation) -> usize {
    160 + a.family.len() + a.variant.len() + a.mv.len() + a.better.len() + a.edition.id().len() + a.motifs.len() * 12
}

impl Store {
    /// This store's id, made the first time it is asked.
    pub fn id(&mut self) -> Result<String> {
        let tx = self.conn.transaction()?;
        let id = match tx.query_row("SELECT id FROM store", [], |r| r.get::<_, String>(0)).optional()? {
            Some(id) => id,
            None => {
                let id: String = tx.query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))?;
                tx.execute("INSERT INTO store (id) VALUES (?1)", [&id])?;
                id
            }
        };
        tx.commit()?;
        Ok(id)
    }

    /// The rows changed after change number `since` of the store `store`, oldest change first:
    /// at most `rows` of them and about `bytes`, and always one when there is one. From the
    /// start when `store` is not this store's id (it was made again, or the Peer has none).
    pub fn changes(&mut self, store: &str, since: u64, rows: usize, bytes: usize) -> Result<Changes> {
        let id = self.id()?;
        let since = if store == id { since as i64 } else { 0 };
        let limit = rows as i64 + 1;
        let evaluations = self.select_seq("WHERE v.seq > ?1 ORDER BY v.seq LIMIT ?2", params![since, limit])?;
        let annotations = self.select_annotations_seq("WHERE a.seq > ?1 ORDER BY a.seq LIMIT ?2", params![since, limit])?;
        let mut out = Changes { store: id, last: since as u64, ..Changes::default() };
        let (mut e, mut a) = (evaluations.into_iter().peekable(), annotations.into_iter().peekable());
        let mut weight = 0;
        loop {
            let take_e = match (e.peek(), a.peek()) {
                (None, None) => break,
                (Some((se, _)), Some((sa, _))) => se < sa,
                (Some(_), None) => true,
                (None, Some(_)) => false,
            };
            let w = if take_e { e.peek().map_or(0, |(_, x)| weight_e(x)) } else { a.peek().map_or(0, |(_, x)| weight_a(x)) };
            let taken = out.evaluations.len() + out.annotations.len();
            if taken >= rows || (taken > 0 && weight + w > bytes) {
                out.more = true;
                break;
            }
            weight += w;
            if let Some((seq, x)) = if take_e { e.next().map(|(s, x)| (s, Ok(x))) } else { a.next().map(|(s, x)| (s, Err(x))) } {
                out.last = seq;
                match x {
                    Ok(x) => out.evaluations.push(x),
                    Err(x) => out.annotations.push(x),
                }
            }
        }
        Ok(out)
    }

    /// Merges a batch from `peer` by the merge rule, and keeps how far it is taken, in one
    /// transaction: nothing when a row is refused.
    pub fn merge_changes(&mut self, peer: &str, c: &Changes) -> Result<Merged> {
        let tx = self.conn.transaction()?;
        let m = crate::merge(&tx, &c.evaluations, &c.annotations)?;
        tx.execute(
            "INSERT INTO pulled (peer, store, seq, time, rows) VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT (peer) DO UPDATE SET store = excluded.store, seq = excluded.seq, time = excluded.time, rows = pulled.rows + excluded.rows",
            params![peer, c.store, c.last as i64, bragd_core::fs::now() as i64, m.read as i64],
        )?;
        tx.commit()?;
        Ok(m)
    }

    /// What was taken from `peer` so far; None before the first batch.
    pub fn pulled(&self, peer: &str) -> Result<Option<Pulled>> {
        Ok(self
            .conn
            .query_row("SELECT peer, store, seq, time, rows FROM pulled WHERE peer = ?1", [peer], |r| {
                Ok(Pulled { peer: r.get(0)?, store: r.get(1)?, seq: r.get::<_, i64>(2)? as u64, time: r.get::<_, i64>(3)? as u64, rows: r.get::<_, i64>(4)? as u64 })
            })
            .optional()?)
    }

    /// Forgets what was taken from `peer` (unpaired): paired again, it starts from the start.
    pub fn forget_peer(&mut self, peer: &str) -> Result<()> {
        self.conn.execute("DELETE FROM pulled WHERE peer = ?1", [peer])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Score;
    use crate::tests::{dir, eval, note, sf};

    fn store(name: &str) -> Store {
        Store::open(&dir(name).join("positions.sqlite")).unwrap()
    }

    /// `to` takes everything new in `from`, a batch of `rows` at a time: the batches, and the
    /// rows read.
    fn pull(to: &mut Store, from: &mut Store, peer: &str, rows: usize) -> (u32, u64) {
        let (mut batches, mut read) = (0, 0);
        loop {
            let at = to.pulled(peer).unwrap();
            let c = from.changes(at.as_ref().map_or("", |p| &p.store), at.as_ref().map_or(0, |p| p.seq), rows, 1 << 20).unwrap();
            read += to.merge_changes(peer, &c).unwrap().read;
            batches += 1;
            if !c.more {
                return (batches, read);
            }
        }
    }

    #[test]
    fn a_peer_takes_only_what_changed_since_the_last_time_in_bounded_batches() {
        let (mut a, mut b) = (store("inc-a"), store("inc-b"));
        for k in 0..5 {
            a.put(&eval(k, sf("Stockfish 17"), 20, 1, Score::Cp(k as i32))).unwrap();
        }
        a.put_annotation(&note(1, "e2e4", sf("Stockfish 17"), 20, 1, None)).unwrap();
        assert_eq!(pull(&mut b, &mut a, "a", 2), (3, 6), "six rows, two at a time");
        assert_eq!((b.all().unwrap(), b.annotations().unwrap()), (a.all().unwrap(), a.annotations().unwrap()));
        assert_eq!(pull(&mut b, &mut a, "a", 2), (1, 0), "nothing new: nothing read");
        // A shallower row changes nothing in A, so it is not sent; a deeper one is.
        a.put(&eval(1, sf("Stockfish 17"), 10, 9, Score::Cp(0))).unwrap();
        a.put(&eval(2, sf("Stockfish 17"), 30, 9, Score::Cp(0))).unwrap();
        assert_eq!(pull(&mut b, &mut a, "a", 100), (1, 1));
        assert_eq!(b.evaluations("chess", "chess", 2).unwrap()[0].depth, 30);
        assert_eq!(b.pulled("a").unwrap().unwrap().rows, 7);
    }

    #[test]
    fn a_batch_is_bounded_by_its_size_too() {
        let mut a = store("bytes");
        for k in 0..4 {
            a.put(&eval(k, sf("Stockfish 17"), 20, 1, Score::Cp(0))).unwrap();
        }
        let c = a.changes("", 0, 100, 1).unwrap();
        assert_eq!((c.evaluations.len(), c.more), (1, true), "one row at least, then the bound");
        let all = a.changes("", 0, 100, 1 << 20).unwrap();
        assert_eq!((all.evaluations.len(), all.more), (4, false));
    }

    /// A store made again is another store: its Peers take it from the start.
    #[test]
    fn a_store_made_again_is_taken_from_the_start() {
        let d = dir("again");
        let (p, mut b) = (d.join("a.sqlite"), store("again-b"));
        let mut a = Store::open(&p).unwrap();
        for k in 0..3 {
            a.put(&eval(k, sf("Stockfish 17"), 20, 1, Score::Cp(0))).unwrap();
        }
        pull(&mut b, &mut a, "a", 10);
        drop(a);
        std::fs::remove_file(&p).unwrap();
        let mut a = Store::open(&p).unwrap();
        a.put(&eval(9, sf("Stockfish 17"), 20, 1, Score::Cp(0))).unwrap();
        // The new store's number 1 is below B's watermark, but it is another store.
        assert_eq!(pull(&mut b, &mut a, "a", 10).1, 1);
        assert_eq!(b.all().unwrap().len(), 4);
    }

    /// Three machines, A and C each paired only with B, each analysing on its own, with rows
    /// that disagree: after the exchanges all three hold the same store, the one a single merge
    /// of every row gives.
    #[test]
    fn three_peers_converge_through_the_one_in_the_middle() {
        let (mut a, mut b, mut c) = (store("three-a"), store("three-b"), store("three-c"));
        let rows_a = [eval(1, sf("Stockfish 17"), 20, 1, Score::Cp(10)), eval(2, sf("Stockfish 17"), 30, 1, Score::Cp(20)), eval(5, sf("Stockfish 17.1"), 12, 4, Score::Mate(3))];
        let rows_b = [eval(1, sf("Stockfish 17"), 25, 2, Score::Cp(11)), eval(3, sf("Stockfish 17"), 18, 2, Score::Cp(-4))];
        let rows_c = [eval(1, sf("Stockfish 17"), 22, 3, Score::Cp(12)), eval(2, sf("Stockfish 17"), 28, 3, Score::Cp(21)), eval(4, sf("Stockfish 17"), 8, 3, Score::Cp(0))];
        let notes_a = [note(1, "e2e4", sf("Stockfish 17"), 20, 1, None)];
        let notes_c = [note(1, "e2e4", sf("Stockfish 17"), 24, 1, Some(crate::Mark::Good))];
        a.merge(&rows_a, &notes_a).unwrap();
        b.put_all(&rows_b).unwrap();
        c.merge(&rows_c, &notes_c).unwrap();
        let mut want = store("three-want");
        want.merge(rows_a.iter().chain(&rows_b).chain(&rows_c), notes_a.iter().chain(&notes_c)).unwrap();
        // Rounds until nothing moves; small batches, so each round takes several.
        let mut rounds = 0;
        loop {
            let moved = pull(&mut b, &mut a, "a", 2).1 + pull(&mut a, &mut b, "b", 2).1 + pull(&mut c, &mut b, "b", 2).1 + pull(&mut b, &mut c, "c", 2).1;
            rounds += 1;
            assert!(rounds < 10, "never settles");
            if moved == 0 {
                break;
            }
        }
        for (name, s) in [("a", &a), ("b", &b), ("c", &c)] {
            assert_eq!(s.all().unwrap(), want.all().unwrap(), "{name}");
            assert_eq!(s.annotations().unwrap(), want.annotations().unwrap(), "{name}");
        }
        assert_eq!(a.all().unwrap().len(), 5, "keys 1 to 4, and 5 by another Edition");
    }

    #[test]
    fn rows_from_before_schema_4_are_numbered_once() {
        let d = dir("v3");
        let p = d.join("positions.sqlite");
        {
            let mut conn = rusqlite::Connection::open(&p).unwrap();
            let v3 = bragd_core::format::Sqlite { migrations: &[], ..crate::FORMAT };
            v3.open(&mut conn).unwrap();
            conn.execute_batch(
                "INSERT INTO editions (n, id, engine, version) VALUES (1, 'x', 'stockfish', 'Stockfish 17');
                 INSERT INTO evaluations (family, variant, key, edition, kind, score, lead, depth, line, time) VALUES ('chess', 'chess', 2, 1, 'cp', 1, 0, 9, '', 1), ('chess', 'chess', 1, 1, 'cp', 1, 0, 9, '', 1);
                 INSERT INTO annotations (family, variant, key, move, edition, mark, better, motifs, depth, time) VALUES ('chess', 'chess', 1, 'e2e4', 1, '', '', '', 9, 1);",
            )
            .unwrap();
        }
        let mut s = Store::open(&p).unwrap();
        let c = s.changes("", 0, 10, 1 << 20).unwrap();
        assert_eq!((c.evaluations.iter().map(|e| e.key).collect::<Vec<_>>(), c.annotations.len(), c.last), (vec![1, 2], 1, 3));
        s.put(&eval(7, sf("Stockfish 17"), 20, 1, Score::Cp(0))).unwrap();
        assert_eq!(s.changes(&c.store, 3, 10, 1 << 20).unwrap().evaluations.len(), 1, "the counter goes on after them");
    }
}
