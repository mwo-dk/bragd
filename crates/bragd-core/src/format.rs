//! The version of each file Bragd writes, and the migrations from its older versions
//! (`docs/reference/file-formats.md`). A file from a newer Bragd is refused and left as it is.
//! A format's migrations are a list in one place: entry `i` takes a file from the format's
//! first version plus `i` to the next one. The first versions are the 1.0 baselines.
//!
//! SQLite files keep their version in `PRAGMA user_version`; JSON and TOML files in a top-level
//! `format` key, where a file without one (or a JSON file that is not an object) is version 1.

use std::fmt;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::t;

/// Why a file was not opened. The file is left as it is.
#[derive(Debug)]
pub enum Error {
    /// Written by a newer Bragd.
    Newer { file: &'static str, version: i64 },
    /// A version this Bragd has no migration from: one older than 1.0, or not a number.
    Unknown { file: &'static str, version: i64 },
    Sql(rusqlite::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A file name as it is; otherwise the key of what the user calls it (`lib.name`).
        let name = |file: &str| if file.contains('.') { file.to_string() } else { t!(file) };
        match self {
            Error::Newer { file, .. } => write!(f, "{}", t!("format.newer", "file" => name(file))),
            Error::Unknown { file, version } => write!(f, "{}", t!("format.unknown", "file" => name(file), "version" => version)),
            Error::Sql(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sql(e)
    }
}

/// For a caller whose errors are SQLite's: the refusal travels as one, and still reads as itself.
impl From<Error> for rusqlite::Error {
    fn from(e: Error) -> Self {
        match e {
            Error::Sql(e) => e,
            e => rusqlite::Error::ToSqlConversionFailure(Box::new(e)),
        }
    }
}

/// An SQLite format.
pub struct Sqlite {
    /// How the file is named to the user: its file name, or the text key of its name.
    pub file: &'static str,
    /// The 1.0 baseline's version, made by `schema`.
    pub first: i64,
    pub schema: &'static str,
    /// SQL taking a file from `first + i` to `first + i + 1`.
    pub migrations: &'static [&'static str],
}

impl Sqlite {
    pub const fn version(&self) -> i64 {
        self.first + self.migrations.len() as i64
    }

    /// The file's version, refused when no migration leads from it: 0 for a new file.
    pub fn check(&self, conn: &Connection) -> Result<i64, Error> {
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if v > self.version() {
            return Err(Error::Newer { file: self.file, version: v });
        }
        if v != 0 && v < self.first {
            return Err(Error::Unknown { file: self.file, version: v });
        }
        Ok(v)
    }

    /// Makes a new file, or brings an older one up to date, in one transaction under the write
    /// lock, so two processes opening it at once make it once.
    pub fn open(&self, conn: &mut Connection) -> Result<(), Error> {
        if self.check(conn)? == self.version() {
            return Ok(());
        }
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let v = self.check(&tx)?;
        if v < self.version() {
            if v == 0 {
                tx.execute_batch(self.schema)?;
            }
            for m in &self.migrations[(v.max(self.first) - self.first) as usize..] {
                tx.execute_batch(m)?;
            }
            tx.pragma_update(None, "user_version", self.version())?;
        }
        tx.commit()?;
        Ok(())
    }
}

/// A JSON format.
pub struct Json {
    pub file: &'static str,
    /// Each takes the file from version `i + 1` to `i + 2`.
    pub migrations: &'static [fn(&mut serde_json::Value)],
}

impl Json {
    pub const fn version(&self) -> i64 {
        1 + self.migrations.len() as i64
    }

    /// `v` made current and stamped with its version, or refused (and left as it was).
    pub fn upgrade(&self, v: &mut serde_json::Value) -> Result<(), Error> {
        let n = match v.get("format") {
            None => 1,
            Some(f) => f.as_i64().ok_or(Error::Unknown { file: self.file, version: 0 })?,
        };
        if n > self.version() {
            return Err(Error::Newer { file: self.file, version: n });
        }
        if n < 1 {
            return Err(Error::Unknown { file: self.file, version: n });
        }
        for m in &self.migrations[(n - 1) as usize..] {
            m(v);
        }
        self.stamp(v);
        Ok(())
    }

    /// Writes the version into `v`, when it is an object.
    pub fn stamp(&self, v: &mut serde_json::Value) {
        if let Some(o) = v.as_object_mut() {
            o.insert("format".into(), self.version().into());
        }
    }
}

/// A TOML format.
pub struct Toml {
    pub file: &'static str,
    /// Each takes the file from version `i + 1` to `i + 2`.
    pub migrations: &'static [fn(&mut toml_edit::DocumentMut)],
}

impl Toml {
    pub const fn version(&self) -> i64 {
        1 + self.migrations.len() as i64
    }

    /// `text` made current, its comments kept, or refused. Text that is not TOML comes back as
    /// it is, for the reader to say what is wrong with it. The version is written only when a
    /// migration ran or the file had none.
    pub fn upgrade(&self, text: &str) -> Result<String, Error> {
        let Ok(mut doc) = text.parse::<toml_edit::DocumentMut>() else { return Ok(text.to_string()) };
        let n = match doc.get("format") {
            None => 1,
            Some(f) => f.as_integer().ok_or(Error::Unknown { file: self.file, version: 0 })?,
        };
        if n > self.version() {
            return Err(Error::Newer { file: self.file, version: n });
        }
        if n < 1 {
            return Err(Error::Unknown { file: self.file, version: n });
        }
        if n == self.version() && doc.contains_key("format") {
            return Ok(text.to_string());
        }
        for m in &self.migrations[(n - 1) as usize..] {
            m(&mut doc);
        }
        doc.insert("format", toml_edit::value(self.version()));
        // The version first, above the user's own lines.
        doc.sort_values_by(|a, _, b, _| (b.get() == "format").cmp(&(a.get() == "format")));
        Ok(doc.to_string())
    }
}

// ---------------------------------------------------------------- for the format tests

/// The schema of an SQLite file, as its tests snapshot it: each table, index, trigger and view
/// SQLite keeps, but not a virtual table's own tables, sorted so the text runs as it stands.
pub fn sqlite_schema(conn: &Connection) -> rusqlite::Result<String> {
    let mut st = conn.prepare(
        &format!("SELECT sql FROM sqlite_master m WHERE sql IS NOT NULL AND {OURS} ORDER BY CASE type WHEN 'table' THEN 0 WHEN 'index' THEN 1 WHEN 'view' THEN 2 ELSE 3 END, name"),
    )?;
    let all = st.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(all.into_iter().map(|s| format!("{s};\n")).collect())
}

/// Not SQLite's own tables, nor a virtual table's.
const OURS: &str = "m.name NOT LIKE 'sqlite\\_%' ESCAPE '\\'
    AND NOT EXISTS (SELECT 1 FROM sqlite_master v WHERE v.sql LIKE 'CREATE VIRTUAL TABLE%' AND substr(m.name, 1, length(v.name) + 1) = v.name || '_')";

/// Every row of an SQLite file as `INSERT`s, table by table and row by row in order, then each
/// full-text index made again from its rows: what a fixture's `data.sql` holds.
pub fn sqlite_data(conn: &Connection) -> rusqlite::Result<String> {
    use rusqlite::types::ValueRef;
    let mut out = String::new();
    let tables: Vec<(String, String)> = conn
        .prepare(&format!("SELECT name, sql FROM sqlite_master m WHERE type = 'table' AND {OURS} ORDER BY name"))?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (table, _) in tables.iter().filter(|(_, sql)| !sql.starts_with("CREATE VIRTUAL TABLE")) {
        let st = conn.prepare(&format!("SELECT * FROM \"{table}\""))?;
        let cols = st.column_names().map(|c| format!("\"{c}\"")).collect::<Vec<_>>();
        let order = (1..=cols.len()).map(|i| i.to_string()).collect::<Vec<_>>().join(", ");
        let mut st = conn.prepare(&format!("SELECT * FROM \"{table}\" ORDER BY {order}"))?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let values = (0..cols.len())
                .map(|i| {
                    Ok(match r.get_ref(i)? {
                        ValueRef::Null => "NULL".to_string(),
                        ValueRef::Integer(n) => n.to_string(),
                        ValueRef::Real(x) => format!("{x:?}"),
                        ValueRef::Text(t) => format!("'{}'", String::from_utf8_lossy(t).replace('\'', "''")),
                        ValueRef::Blob(b) => format!("X'{}'", b.iter().map(|b| format!("{b:02X}")).collect::<String>()),
                    })
                })
                .collect::<rusqlite::Result<Vec<_>>>()?;
            out += &format!("INSERT INTO \"{table}\" ({}) VALUES ({});\n", cols.join(", "), values.join(", "));
        }
    }
    for (table, sql) in &tables {
        if sql.starts_with("CREATE VIRTUAL TABLE") && sql.contains("fts5") {
            out += &format!("INSERT INTO \"{table}\" (\"{table}\") VALUES ('rebuild');\n");
        }
    }
    Ok(out)
}

/// The shape of a JSON value: one line per path with its type, sorted. A snapshot of it fails
/// when a field is added, removed or retyped.
pub fn shape(v: &serde_json::Value) -> String {
    fn walk(v: &serde_json::Value, path: &str, out: &mut std::collections::BTreeSet<String>) {
        use serde_json::Value::*;
        let kind = match v {
            Null => "null",
            Bool(_) => "bool",
            Number(n) if n.is_f64() => "float",
            Number(_) => "int",
            String(_) => "string",
            Array(a) => {
                a.iter().for_each(|x| walk(x, &format!("{path}[]"), out));
                "array"
            }
            Object(o) => {
                o.iter().for_each(|(k, x)| walk(x, &format!("{path}.{k}"), out));
                "object"
            }
        };
        out.insert(format!("{} {kind}\n", if path.is_empty() { "." } else { path }));
    }
    let mut out = std::collections::BTreeSet::new();
    walk(v, "", &mut out);
    out.into_iter().collect()
}

/// `tests/fixtures/formats/<format>` in the repository.
pub fn fixtures(format: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/formats").join(format)
}

/// Every committed version of `format`: (version, its folder), oldest first.
pub fn versions(format: &str) -> Vec<(i64, PathBuf)> {
    let mut all: Vec<_> = std::fs::read_dir(fixtures(format))
        .map(|d| d.flatten().filter_map(|e| Some((e.file_name().to_str()?.strip_prefix('v')?.parse().ok()?, e.path()))).collect())
        .unwrap_or_default();
    all.sort();
    all
}

/// Fails unless `actual` is the committed `name` of `format`'s current `version`. With
/// `BRAGD_WRITE_FIXTURES=1`, a snapshot not yet committed is written; one committed is never
/// written over.
pub fn snapshot(format: &str, version: i64, name: &str, actual: &str) {
    let path = fixtures(format).join(format!("v{version}")).join(name);
    match std::fs::read_to_string(&path) {
        Ok(want) => assert!(
            want == actual,
            "{format}'s {name} changed without a new version. A change to a stored format bumps its version, \
             adds a migration and a fixture (docs/reference/file-formats.md).\n--- committed {}\n{want}\n--- now\n{actual}",
            path.display()
        ),
        Err(_) if std::env::var_os("BRAGD_WRITE_FIXTURES").is_some() => {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, actual).unwrap();
        }
        Err(_) => panic!("{format} v{version} has no {}: run its test with BRAGD_WRITE_FIXTURES=1 and commit the file", path.display()),
    }
}

/// An SQLite file at `to` made from a fixture folder: its `schema.sql`, then its `data.sql`, at
/// the folder's version.
pub fn sqlite_fixture(dir: &Path, version: i64, to: &Path) -> rusqlite::Result<()> {
    let _ = std::fs::remove_file(to);
    let c = Connection::open(to)?;
    // The rows come table by table in name order, not in the order their keys need.
    c.pragma_update(None, "foreign_keys", false)?;
    let read = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap_or_else(|e| panic!("{}: {e}", dir.join(n).display()));
    c.execute_batch(&read("schema.sql"))?;
    c.execute_batch(&read("data.sql"))?;
    c.pragma_update(None, "user_version", version)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Sqlite = Sqlite { file: "t", first: 2, schema: "CREATE TABLE a (x);", migrations: &["ALTER TABLE a ADD COLUMN y DEFAULT 7;"] };

    fn db() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn a_new_sqlite_file_is_made_at_the_current_version() {
        let mut c = db();
        T.open(&mut c).unwrap();
        assert_eq!(T.check(&c).unwrap(), 3);
        c.execute("INSERT INTO a (x) VALUES (1)", []).unwrap();
        assert_eq!(c.query_row("SELECT y FROM a", [], |r| r.get::<_, i64>(0)).unwrap(), 7);
    }

    #[test]
    fn an_older_sqlite_file_is_migrated_and_a_newer_or_unknown_one_left_alone() {
        let mut c = db();
        c.execute_batch("CREATE TABLE a (x); INSERT INTO a VALUES (5); PRAGMA user_version = 2;").unwrap();
        T.open(&mut c).unwrap();
        assert_eq!(c.query_row("SELECT x + y FROM a", [], |r| r.get::<_, i64>(0)).unwrap(), 12);
        for (v, newer) in [(4, true), (1, false)] {
            let mut c = db();
            c.pragma_update(None, "user_version", v).unwrap();
            let e = T.open(&mut c).unwrap_err();
            assert_eq!(matches!(e, Error::Newer { version, .. } if version == v), newer, "{e:?}");
            assert_eq!(c.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
        }
    }

    const J: Json = Json { file: "j", migrations: &[|v| v["b"] = v["a"].take()] };

    #[test]
    fn json_is_migrated_and_stamped_or_refused() {
        let mut v = serde_json::json!({ "a": 1 });
        J.upgrade(&mut v).unwrap();
        assert_eq!(v, serde_json::json!({ "b": 1, "a": null, "format": 2 }));
        let mut v = serde_json::json!({ "b": 1, "format": 2 });
        J.upgrade(&mut v).unwrap();
        assert_eq!(v, serde_json::json!({ "b": 1, "format": 2 }));
        assert!(matches!(J.upgrade(&mut serde_json::json!({ "format": 3 })), Err(Error::Newer { version: 3, .. })));
        assert!(matches!(J.upgrade(&mut serde_json::json!({ "format": "x" })), Err(Error::Unknown { .. })));
        let mut list = serde_json::json!([1]);
        Json { file: "l", migrations: &[] }.upgrade(&mut list).unwrap();
        assert_eq!(list, serde_json::json!([1]));
    }

    #[test]
    fn toml_keeps_comments_and_puts_its_version_first() {
        let t = Toml { file: "t", migrations: &[] };
        assert_eq!(t.upgrade("# mine\n[a]\nb = 1\n").unwrap(), "format = 1\n# mine\n[a]\nb = 1\n");
        assert_eq!(t.upgrade("format = 1\n[a]\n").unwrap(), "format = 1\n[a]\n");
        assert!(matches!(t.upgrade("format = 2\n"), Err(Error::Newer { version: 2, .. })));
        assert_eq!(t.upgrade("not toml [").unwrap(), "not toml [");
    }

    #[test]
    fn the_schema_leaves_out_a_virtual_tables_own_tables() {
        let c = db();
        c.execute_batch("CREATE TABLE p (n TEXT); CREATE VIRTUAL TABLE w USING fts5 (n, content = 'p'); CREATE INDEX p_n ON p (n);").unwrap();
        assert_eq!(sqlite_schema(&c).unwrap(), "CREATE TABLE p (n TEXT);\nCREATE VIRTUAL TABLE w USING fts5 (n, content = 'p');\nCREATE INDEX p_n ON p (n);\n");
    }

    #[test]
    fn data_runs_back_into_the_same_rows_and_index() {
        let schema = "CREATE TABLE p (n TEXT, x REAL, b BLOB); CREATE VIRTUAL TABLE w USING fts5 (n, content = 'p');";
        let c = db();
        c.execute_batch(schema).unwrap();
        c.execute("INSERT INTO p VALUES ('it''s anna', 0.1, X'00ff'), (NULL, 3.0, NULL)", []).unwrap();
        let data = sqlite_data(&c).unwrap();
        assert_eq!(data, "INSERT INTO \"p\" (\"n\", \"x\", \"b\") VALUES (NULL, 3.0, NULL);\nINSERT INTO \"p\" (\"n\", \"x\", \"b\") VALUES ('it''s anna', 0.1, X'00FF');\nINSERT INTO \"w\" (\"w\") VALUES ('rebuild');\n");
        let d = db();
        d.execute_batch(schema).unwrap();
        d.execute_batch(&data).unwrap();
        assert_eq!(sqlite_data(&d).unwrap(), data);
        assert_eq!(d.query_row("SELECT x FROM w JOIN p ON p.rowid = w.rowid WHERE w MATCH 'anna'", [], |r| r.get::<_, f64>(0)).unwrap(), 0.1);
    }

    #[test]
    fn shape_names_every_path_and_type() {
        assert_eq!(shape(&serde_json::json!({ "a": [1, 2.5], "b": { "c": "x" } })), ". object\n.a array\n.a[] float\n.a[] int\n.b object\n.b.c string\n");
    }
}
