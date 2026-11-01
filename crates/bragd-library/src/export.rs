//! A Library's games exported to Parquet, for DuckDB to scan. The export is a copy: DuckDB never
//! owns the data, and the Library stays the store (ADR 0002).

use std::path::Path;
use std::sync::Arc;

use parquet::basic::Compression;
use parquet::data_type::{ByteArray, ByteArrayType, Int32Type, Int64Type};
use parquet::file::properties::WriterProperties;
use parquet::file::writer::SerializedFileWriter;
use parquet::schema::parser::parse_message_type;
use rusqlite::types::Value;

use crate::library::{Library, Result};

/// The columns, as SQL selects them from `games` and as Parquet holds them, in the same order.
const COLUMNS: [(&str, &str); 20] = [
    ("id", "required int64 id"),
    ("family", "required binary family (STRING)"),
    ("variant", "required binary variant (STRING)"),
    ("white", "required binary white (STRING)"),
    ("black", "required binary black (STRING)"),
    ("white_elo", "optional int32 white_elo"),
    ("black_elo", "optional int32 black_elo"),
    ("date", "required binary date (STRING)"),
    ("event", "required binary event (STRING)"),
    ("site", "required binary site (STRING)"),
    ("eco", "required binary eco (STRING)"),
    ("opening", "required binary opening (STRING)"),
    ("result", "required binary result (STRING)"),
    ("headers", "required binary headers (STRING)"),
    ("moves", "required binary moves (STRING)"),
    ("start", "optional binary start (STRING)"),
    ("ply_count", "required int32 ply_count"),
    ("zobrist_final", "required int64 zobrist_final"),
    ("zobrist_chain_hash", "required int64 zobrist_chain_hash"),
    ("(SELECT group_concat(source, ',') FROM (SELECT DISTINCT source FROM game_sources WHERE game_id = games.id))", "required binary sources (STRING)"),
];

/// Games in one row group.
const ROWS: usize = 64 * 1024;

/// Writes every Game of `lib` to the Parquet file `out` (Snappy-compressed), with its Sources'
/// names; the number of rows written. Written beside it first, then put in place, so a failed
/// export leaves a file that was there before whole.
pub fn to_parquet(lib: &Library, out: &Path) -> Result<u64> {
    let mut part = out.as_os_str().to_owned();
    part.push(".part");
    let part = std::path::PathBuf::from(part);
    let written = write_parquet(lib, &part).and_then(|n| Ok(std::fs::rename(&part, out).map(|_| n)?));
    if written.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    written
}

fn write_parquet(lib: &Library, out: &Path) -> Result<u64> {
    let fields: Vec<&str> = COLUMNS.iter().map(|c| c.1).collect();
    let schema = Arc::new(parse_message_type(&format!("message games {{ {}; }}", fields.join("; ")))?);
    let props = Arc::new(WriterProperties::builder().set_compression(Compression::SNAPPY).build());
    let mut writer = SerializedFileWriter::new(std::fs::File::create(out)?, schema, props)?;
    let select: Vec<&str> = COLUMNS.iter().map(|c| c.0).collect();
    let mut st = lib.conn().prepare(&format!("SELECT {} FROM games ORDER BY id", select.join(", ")))?;
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
fn write_group(writer: &mut SerializedFileWriter<std::fs::File>, batch: &[Vec<Value>]) -> Result<()> {
    let mut group = writer.next_row_group()?;
    let mut i = 0;
    while let Some(mut col) = group.next_column()? {
        let values = batch.iter().map(|r| &r[i]);
        let defs: Vec<i16> = batch.iter().map(|r| i16::from(!matches!(r[i], Value::Null))).collect();
        let optional = COLUMNS[i].1.starts_with("optional");
        let defs = optional.then_some(defs.as_slice());
        if COLUMNS[i].1.contains(" int64 ") {
            let v: Vec<i64> = values.filter_map(|v| if let Value::Integer(n) = v { Some(*n) } else { None }).collect();
            col.typed::<Int64Type>().write_batch(&v, defs, None)?;
        } else if COLUMNS[i].1.contains(" int32 ") {
            let v: Vec<i32> = values.filter_map(|v| if let Value::Integer(n) = v { Some(*n as i32) } else { None }).collect();
            col.typed::<Int32Type>().write_batch(&v, defs, None)?;
        } else {
            let v: Vec<ByteArray> = values
                .filter_map(|v| match v {
                    Value::Text(s) => Some(ByteArray::from(s.as_str())),
                    // A required text column never misses a value (sources: none is "").
                    Value::Null if !optional => Some(ByteArray::from("")),
                    _ => None,
                })
                .collect();
            col.typed::<ByteArrayType>().write_batch(&v, defs, None)?;
        }
        col.close()?;
        i += 1;
    }
    group.close()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::tests::{CENTURY, Dir, pgn};
    use parquet::file::reader::{FileReader, SerializedFileReader};

    #[test]
    fn a_library_exports_and_reads_back() {
        let d = Dir::new("parquet");
        let p = d.0.join("games.pgn");
        let text = [
            pgn(&[("White", "Byrne, Donald"), ("WhiteElo", "2400")], CENTURY, "0-1"),
            pgn(&[("White", "Byrne, Donald")], CENTURY, "0-1"),
            pgn(&[], "1. e4 e5", "*"),
            pgn(&[("FEN", "4k3/8/8/8/8/8/8/4K2R w K - 0 1"), ("SetUp", "1")], "1. O-O Kd7", "*"),
        ]
        .concat();
        std::fs::write(&p, text).unwrap();
        let mut lib = Library::open(&d.0.join("lib.bragd")).unwrap();
        lib.import(&p, "twic", &|| false, &mut |_| {}).unwrap();
        assert_eq!(lib.count().unwrap(), 3);
        let out = d.0.join("games.parquet");
        assert_eq!(to_parquet(&lib, &out).unwrap(), 3);
        let r = SerializedFileReader::new(std::fs::File::open(&out).unwrap()).unwrap();
        assert_eq!(r.metadata().file_metadata().num_rows(), 3);
        let rows: Vec<String> = r.get_row_iter(None).unwrap().map(|row| row.unwrap().to_string()).collect();
        assert_eq!(rows.len(), 3);
        assert!(rows[0].contains("Byrne, Donald") && rows[0].contains("white_elo: 2400"), "{}", rows[0]);
        assert!(rows[1].contains("white_elo: null") && rows[1].contains("start: null"), "{}", rows[1]);
        assert!(rows[2].contains("4k3/8/8/8/8/8/8/4K2R"), "{}", rows[2]);
        // A failed export leaves the file that was there whole, and nothing beside it.
        let before = std::fs::read(&out).unwrap();
        lib.conn().execute_batch("DROP TABLE game_sources").unwrap();
        assert!(to_parquet(&lib, &out).is_err());
        assert_eq!(std::fs::read(&out).unwrap(), before);
        assert_eq!(std::fs::read_dir(&d.0).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().ends_with(".part")).count(), 0);
    }
}
