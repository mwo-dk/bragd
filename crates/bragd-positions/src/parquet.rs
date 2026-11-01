//! The store exported to Parquet and imported from it: for backups, moving a store by hand,
//! and reading it in DuckDB. Each row carries its Edition whole, so a file reads on its own.
//! Evaluations and Annotations share one file: an Annotation's `kind` is `annotation`. An
//! import merges by the store's rules (`Store::merge`).

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parquet::basic::Compression;
use parquet::data_type::{ByteArray, ByteArrayType, DoubleType, Int32Type, Int64Type};
use parquet::file::properties::WriterProperties;
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::file::writer::SerializedFileWriter;
use parquet::record::{Field, Row, RowAccessor};
use parquet::schema::parser::parse_message_type;
use rusqlite::types::Value;

use crate::{Annotation, Edition, Error, Evaluation, Merged, Result, Score, Store, annotation_columns};

/// The columns, as SQL selects them from `evaluations` (`v`) and from `annotations` (`a`), each
/// with `editions` (`d`), and as Parquet holds them, in the same order.
const COLUMNS: [(&str, &str, &str); 21] = [
    ("v.family", "a.family", "required binary family (STRING)"),
    ("v.variant", "a.variant", "required binary variant (STRING)"),
    ("v.key", "a.key", "required int64 key"),
    ("d.id", "d.id", "required binary edition (STRING)"),
    ("d.engine", "d.engine", "required binary engine (STRING)"),
    ("d.version", "d.version", "required binary version (STRING)"),
    ("d.network", "d.network", "optional binary network (STRING)"),
    ("d.image", "d.image", "optional binary image (STRING)"),
    ("v.kind", "'annotation'", "required binary kind (STRING)"),
    ("v.score", "0.0", "required double score"),
    ("v.lead", "0.0", "required double lead"),
    ("v.depth", "a.depth", "required int32 depth"),
    ("v.line", "a.better", "required binary line (STRING)"),
    ("v.time", "a.time", "required int64 time"),
    ("NULL", "a.move", "optional binary move (STRING)"),
    ("NULL", "a.mark", "optional binary mark (STRING)"),
    ("NULL", "a.motifs", "optional binary motifs (STRING)"),
    ("v.kind2", "NULL", "optional binary kind2 (STRING)"),
    ("v.score2", "NULL", "optional double score2"),
    ("v.lead2", "NULL", "optional double lead2"),
    ("v.line2", "NULL", "optional binary line2 (STRING)"),
];

/// The export's format, in the file's key-value metadata under `bragd.format`. A file without
/// it is version 1. A newer one is refused; reading an older one is `from_parquet`'s to do.
pub const FORMAT: i64 = 1;
const FORMAT_KEY: &str = "bragd.format";

/// Rows in one row group.
const ROWS: usize = 64 * 1024;

/// Writes every Evaluation and Annotation in `store` to the Parquet file `out`
/// (Snappy-compressed): the rows written. Written beside it first (`<out>.part`) and then put in place, so a failed export
/// leaves a file that was there before whole.
///
/// Columns, in this order: `family` (string), `variant` (string), `key` (int64, the u64 key's
/// bits), `edition` (string, `Edition::id`), `engine`, `version` (string), `network`, `image`
/// (optional string), `kind` (string: cp, mate, winrate, equity), `score` (double), `lead`
/// (double), `depth` (int32), `line` (string), `time` (int64), then for an Annotation (its
/// `kind` `annotation`, `score` and `lead` 0, `line` the better line) `move`, `mark` and `motifs`
/// (optional strings, empty for an Evaluation), then `kind2`, `score2`, `lead2` and `line2`
/// (optional second-choice score kind, score, lead and line, empty for an Evaluation without one).
pub fn to_parquet(store: &Store, out: &Path) -> Result<u64> {
    let mut part = out.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    let written = write_parquet(store, &part).and_then(|n| {
        std::fs::rename(&part, out)?;
        Ok(n)
    });
    if written.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    written
}

fn write_parquet(store: &Store, out: &Path) -> Result<u64> {
    let fields: Vec<&str> = COLUMNS.iter().map(|c| c.2).collect();
    let schema = Arc::new(parse_message_type(&format!("message evaluations {{ {}; }}", fields.join("; ")))?);
    let props = Arc::new(
        WriterProperties::builder()
            .set_compression(Compression::SNAPPY)
            .set_key_value_metadata(Some(vec![parquet::file::metadata::KeyValue::new(FORMAT_KEY.into(), FORMAT.to_string())]))
            .build(),
    );
    let mut writer = SerializedFileWriter::new(File::create(out)?, schema, props)?;
    let select = |i: fn(&(&'static str, &'static str, &'static str)) -> &'static str| COLUMNS.iter().map(i).collect::<Vec<_>>().join(", ");
    let mut st = store.conn().prepare(&format!(
        "SELECT {} FROM evaluations v JOIN editions d ON d.n = v.edition \
         UNION ALL SELECT {} FROM annotations a JOIN editions d ON d.n = a.edition ORDER BY 1, 2, 3, 15, 4",
        select(|c| c.0),
        select(|c| c.1)
    ))?;
    let mut rows = st.query([])?;
    let mut batch: Vec<Vec<Value>> = Vec::with_capacity(ROWS);
    let mut total = 0u64;
    loop {
        let row = rows.next()?;
        if let Some(r) = row {
            batch.push((0..COLUMNS.len()).map(|i| r.get(i)).collect::<rusqlite::Result<_>>()?);
        }
        if batch.len() == ROWS || (row.is_none() && !batch.is_empty()) {
            write_group(&mut writer, &batch)?;
            total += batch.len() as u64;
            batch.clear();
        }
        if row.is_none() {
            break;
        }
    }
    writer.close()?;
    Ok(total)
}

/// One row group: each column's values, a NULL as no value (its definition level 0).
fn write_group(writer: &mut SerializedFileWriter<File>, batch: &[Vec<Value>]) -> Result<()> {
    let mut group = writer.next_row_group()?;
    let mut i = 0;
    while let Some(mut col) = group.next_column()? {
        let decl = COLUMNS[i].2;
        let defs: Vec<i16> = batch.iter().map(|r| i16::from(!matches!(r[i], Value::Null))).collect();
        let defs = decl.starts_with("optional").then_some(defs.as_slice());
        match decl.split_whitespace().nth(1) {
            Some("binary") => {
                let v: Vec<ByteArray> = batch.iter().filter_map(|r| if let Value::Text(s) = &r[i] { Some(ByteArray::from(s.as_str())) } else { None }).collect();
                col.typed::<ByteArrayType>().write_batch(&v, defs, None)?;
            }
            Some("int64") => {
                let v: Vec<i64> = batch.iter().filter_map(|r| if let Value::Integer(n) = r[i] { Some(n) } else { None }).collect();
                col.typed::<Int64Type>().write_batch(&v, defs, None)?;
            }
            Some("int32") => {
                let v: Vec<i32> = batch.iter().filter_map(|r| if let Value::Integer(n) = r[i] { Some(n as i32) } else { None }).collect();
                col.typed::<Int32Type>().write_batch(&v, defs, None)?;
            }
            _ => {
                let v: Vec<f64> = batch.iter().filter_map(|r| if let Value::Real(x) = r[i] { Some(x) } else { None }).collect();
                col.typed::<DoubleType>().write_batch(&v, defs, None)?;
            }
        }
        col.close()?;
        i += 1;
    }
    group.close()?;
    Ok(())
}

/// Merges every row of the Parquet file `path` (as `to_parquet` writes it) into `store`, in
/// one transaction: nothing is merged when a row cannot be read.
pub fn from_parquet(store: &mut Store, path: &Path) -> Result<Merged> {
    let reader = SerializedFileReader::new(File::open(path)?)?;
    let version = reader.metadata().file_metadata().key_value_metadata().and_then(|kv| kv.iter().find(|k| k.key == FORMAT_KEY)).map_or(Some(1), |k| k.value.as_deref()?.parse().ok());
    match version {
        Some(v) if v > FORMAT => return Err(Error::Format(bragd_core::format::Error::Newer { file: "*.parquet", version: v })),
        Some(v) if v >= 1 => {}
        v => return Err(Error::Format(bragd_core::format::Error::Unknown { file: "*.parquet", version: v.unwrap_or(0) })),
    }
    let (mut evaluations, mut annotations) = (vec![], vec![]);
    for row in reader.get_row_iter(None)? {
        let row = row?;
        if row.get_string(8)? == "annotation" {
            annotations.push(annotation(&row)?);
        } else {
            evaluations.push(evaluation(&row)?);
        }
    }
    store.merge(&evaluations, &annotations)
}

/// An optional column: None when its value is NULL; any other read error is an error.
fn optional(row: &Row, i: usize) -> Result<Option<String>> {
    match row.get_column_iter().nth(i) {
        Some((_, Field::Null)) => Ok(None),
        _ => row.get_string(i).map(|s| Some(s.clone())).map_err(Error::from),
    }
}

/// An optional double: 0.0 when NULL; any other read error is an error.
fn optional_double(row: &Row, i: usize) -> Result<f64> {
    match row.get_column_iter().nth(i) {
        Some((_, Field::Null)) => Ok(0.0),
        _ => row.get_double(i).map_err(Error::from),
    }
}

fn edition(row: &Row) -> Result<Edition> {
    Ok(Edition { engine: row.get_string(4)?.clone(), version: row.get_string(5)?.clone(), network: optional(row, 6)?, image: optional(row, 7)? })
}

fn depth(row: &Row) -> Result<u32> {
    u32::try_from(row.get_int(11)?).map_err(|_| Error::Bad("negative depth".into()))
}

/// One row read back as an Evaluation. The `edition` column is not read: `Edition::id` is
/// made from the fields it is written from.
fn evaluation(row: &Row) -> Result<Evaluation> {
    let kind2 = optional(row, 17)?.unwrap_or_default();
    let score2 = optional_double(row, 18)?;
    let lead2 = optional_double(row, 19)?;
    let line2 = optional(row, 20)?.unwrap_or_default();
    Ok(Evaluation {
        family: row.get_string(0)?.clone(),
        variant: row.get_string(1)?.clone(),
        key: row.get_long(2)? as u64,
        edition: edition(row)?,
        score: Score::from_columns(row.get_string(8)?, row.get_double(9)?, row.get_double(10)?)?,
        depth: depth(row)?,
        line: row.get_string(12)?.clone(),
        time: row.get_long(13)?,
        second: crate::Second::from_columns(&kind2, score2, lead2, line2)?,
    })
}

fn annotation(row: &Row) -> Result<Annotation> {
    let mv = optional(row, 14)?.ok_or_else(|| Error::Bad("an Annotation without its move".into()))?;
    let (mark, motifs) = annotation_columns(&optional(row, 15)?.unwrap_or_default(), &optional(row, 16)?.unwrap_or_default())?;
    Ok(Annotation {
        family: row.get_string(0)?.clone(),
        variant: row.get_string(1)?.clone(),
        key: row.get_long(2)? as u64,
        mv,
        edition: edition(row)?,
        mark,
        better: row.get_string(12)?.clone(),
        motifs,
        depth: depth(row)?,
        time: row.get_long(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{dir, eval, note, sf};
    use crate::{Edition, Evaluation, Mark, Motif, Score};

    fn rows() -> Vec<Evaluation> {
        vec![
            eval(u64::MAX, sf("Stockfish 17"), 20, 100, Score::Cp(-30)),
            eval(3, sf("Stockfish 17.1"), 9, 100, Score::Mate(4)),
            Evaluation { family: "go".into(), variant: "go-19".into(), key: 1 << 63, edition: Edition { engine: "katago".into(), version: "KataGo 1.18.2".into(), network: Some("ab12".into()), image: None }, score: Score::WinRate { win: 0.625, lead: -1.5 }, depth: 400, line: "Q16 D4".into(), time: 5, second: Some(crate::Second { score: Score::WinRate { win: 0.5, lead: 0.25 }, line: "D4 Q16".into() }) },
            Evaluation { family: "backgammon".into(), variant: "backgammon".into(), key: 0, edition: Edition { engine: "gnubg".into(), version: "GNU Backgammon 1.08.003".into(), network: None, image: None }, score: Score::Equity(0.125), depth: 2, line: String::new(), time: -1, second: Some(crate::Second { score: Score::Equity(-0.5), line: String::new() }) },
        ]
    }

    #[test]
    fn a_store_exports_and_imports_back_whole() {
        let d = dir("parquet");
        let mut a = Store::open(&d.join("a.sqlite")).unwrap();
        a.put_all(&rows()).unwrap();
        a.merge([], &[note(u64::MAX, "e2e4", sf("Stockfish 17"), 20, 100, Some(Mark::Blunder)), note(3, "a7a8q", sf("Stockfish 17.1"), 9, 100, None)]).unwrap();
        let out = d.join("positions.parquet");
        assert_eq!(to_parquet(&a, &out).unwrap(), 6);
        let mut b = Store::open(&d.join("b.sqlite")).unwrap();
        assert_eq!(from_parquet(&mut b, &out).unwrap(), Merged { read: 6, changed: 6 });
        assert_eq!(b.all().unwrap(), a.all().unwrap());
        assert_eq!(b.annotations().unwrap(), a.annotations().unwrap());
        assert_eq!(b.annotations().unwrap().iter().find(|n| n.mv == "e2e4").unwrap().motifs, [Motif::Hangs, Motif::Fork]);
        // Imported again: read, and nothing changes.
        assert_eq!(from_parquet(&mut b, &out).unwrap(), Merged { read: 6, changed: 0 });
    }

    #[test]
    fn a_file_from_a_newer_bragd_is_refused() {
        let d = dir("parquet-newer");
        let out = d.join("newer.parquet");
        let fields: Vec<&str> = COLUMNS.iter().map(|c| c.2).collect();
        let schema = Arc::new(parse_message_type(&format!("message evaluations {{ {}; }}", fields.join("; "))).unwrap());
        let kv = parquet::file::metadata::KeyValue::new(FORMAT_KEY.into(), (FORMAT + 1).to_string());
        let props = Arc::new(WriterProperties::builder().set_key_value_metadata(Some(vec![kv])).build());
        SerializedFileWriter::new(File::create(&out).unwrap(), schema, props).unwrap().close().unwrap();
        let mut store = Store::open(&d.join("s.sqlite")).unwrap();
        assert!(matches!(from_parquet(&mut store, &out), Err(Error::Format(bragd_core::format::Error::Newer { .. }))));
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
        b.put_annotation(&note(1, "e2e4", sf("Stockfish 17"), 40, 9, None)).unwrap();
        a.put_annotation(&note(1, "e2e4", sf("Stockfish 17"), 30, 1, Some(Mark::Mistake))).unwrap();
        a.put_annotation(&note(1, "d2d4", sf("Stockfish 17"), 30, 1, Some(Mark::Mistake))).unwrap();
        to_parquet(&a, &out).unwrap();
        assert_eq!(from_parquet(&mut b, &out).unwrap(), Merged { read: 4, changed: 2 });
        let depths: Vec<(u64, u32)> = b.all().unwrap().iter().map(|e| (e.key, e.depth)).collect();
        assert_eq!(depths, [(1, 30), (2, 40)]);
        let notes: Vec<(String, u32)> = b.annotations().unwrap().into_iter().map(|a| (a.mv, a.depth)).collect();
        assert_eq!(notes, [("d2d4".to_string(), 30), ("e2e4".to_string(), 40)]);
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
