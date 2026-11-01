//! Rating lists: published lists of players with their federation IDs and ratings. The Helper
//! downloads them (as Sources) and imports each into one SQLite file per machine,
//! `rating-lists.sqlite` in the data folder, replacing what that list had before. Check-in and
//! the Club page find players there by name or ID.

use std::io::{self, BufRead, BufReader, Read, Seek};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, params};
use serde::Serialize;

/// A player as a rating list has them.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Player {
    /// The list's own ID for the player: the FIDE ID on FIDE's list, the member number on a
    /// federation's.
    pub id: String,
    /// As the list writes it, often `Surname, Given`.
    pub name: String,
    /// The three-letter federation (FIDE's lists), or the club (a federation's).
    pub federation: String,
    pub club: String,
    pub title: String,
    /// Standard, rapid and blitz ratings, where the list has them.
    pub rating: Option<u32>,
    pub rapid: Option<u32>,
    pub blitz: Option<u32>,
    pub born: Option<u16>,
    /// The FIDE ID a federation's list gives beside its own.
    pub fide_id: String,
}

/// A player found, with the list they are on.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Found {
    pub list: String,
    #[serde(flatten)]
    pub player: Player,
}

/// A list and what was imported of it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Imported {
    pub list: String,
    pub players: u64,
    /// Unix time.
    pub at: u64,
}

/// How a list's file is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// FIDE's combined list: a zip holding one fixed-width text file, its columns named by its
    /// header line.
    FideTxt,
    /// The Danish Chess Union's member list: `;`-separated, every field quoted, Latin-1.
    DsuCsv,
    /// The English Chess Federation's rating list: comma-separated, with a header.
    EcfCsv,
}

/// A rating list Bragd can fetch: its id (the Source's), its name, where it is, the file it is
/// kept as, about how big one download is, and how it is written.
#[derive(Debug, PartialEq)]
pub struct List {
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    pub file: &'static str,
    pub size: u64,
    pub format: Format,
}

/// Every rating list Bragd can fetch, in the order the Sources page lists them. Where each comes
/// from, its terms and why others are missing: `docs/reference/rating-lists.md`.
pub const LISTS: &[List] = &[
    List { id: "fide-ratings", name: "FIDE", url: "https://ratings.fide.com/download/players_list.zip", file: "players_list.zip", size: 45_000_000, format: Format::FideTxt },
    List { id: "dsu-ratings", name: "DSU", url: "https://turnering.skak.dk/ClubAndMembers/AllMemberReport?memType=ActiveMembersAjax&format=csv", file: "dsu-members.csv", size: 550_000, format: Format::DsuCsv },
    List { id: "ecf-ratings", name: "ECF", url: "https://rating.englishchess.org.uk/api/rating-list/csv", file: "ecf-ratings.csv", size: 4_500_000, format: Format::EcfCsv },
];

/// What a parser hands each player to.
pub type Each<'a> = &'a mut dyn FnMut(Player) -> io::Result<()>;

/// The lines of `r`, each read as UTF-8, or as Latin-1 when it is not.
fn lines(r: &mut dyn BufRead, mut each: impl FnMut(&str) -> io::Result<()>) -> io::Result<()> {
    let mut buf = vec![];
    loop {
        buf.clear();
        if r.read_until(b'\n', &mut buf)? == 0 {
            return Ok(());
        }
        while buf.last().is_some_and(|b| matches!(b, b'\n' | b'\r')) {
            buf.pop();
        }
        match std::str::from_utf8(&buf) {
            Ok(s) => each(s)?,
            Err(_) => each(&buf.iter().map(|&b| b as char).collect::<String>())?,
        }
    }
}

/// The fields of one CSV line: `sep`-separated, a field in quotes may hold `sep` and `""`.
fn fields(line: &str, sep: char) -> Vec<String> {
    let (mut out, mut cur, mut quoted) = (vec![], String::new(), false);
    let mut cs = line.chars().peekable();
    while let Some(c) = cs.next() {
        match c {
            '"' if quoted && cs.peek() == Some(&'"') => {
                cur.push('"');
                cs.next();
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// A rating, a birth year: a positive number, none when blank or 0.
fn number<T: std::str::FromStr + PartialEq + Default>(s: &str) -> Option<T> {
    s.trim().parse().ok().filter(|n| *n != T::default())
}

/// The players in `r`, written as `format`.
pub fn parse(format: Format, r: &mut dyn BufRead, each: Each) -> io::Result<()> {
    match format {
        Format::FideTxt => {
            // Each column runs from its name in the header to the next name.
            let mut cols: Vec<(String, usize)> = vec![];
            lines(r, |line| {
                if cols.is_empty() {
                    let mut at = 0;
                    for w in line.split(' ') {
                        if !w.is_empty() && w != "Number" {
                            cols.push((w.to_string(), at));
                        }
                        at += w.len() + 1;
                    }
                    return Ok(());
                }
                let get = |name: &str| -> &str {
                    let Some(i) = cols.iter().position(|(n, _)| n == name) else { return "" };
                    let from = cols[i].1.min(line.len());
                    let to = cols.get(i + 1).map_or(line.len(), |c| c.1).clamp(from, line.len());
                    line.get(from..to).unwrap_or("").trim()
                };
                each(Player {
                    id: get("ID").into(),
                    name: get("Name").into(),
                    federation: get("Fed").into(),
                    title: get("Tit").into(),
                    rating: number(get("SRtng")),
                    rapid: number(get("RRtng")),
                    blitz: number(get("BRtng")),
                    born: number(get("B-day")),
                    fide_id: get("ID").into(),
                    ..Player::default()
                })
            })
        }
        Format::DsuCsv | Format::EcfCsv => {
            let sep = if format == Format::DsuCsv { ';' } else { ',' };
            let mut head: Vec<String> = vec![];
            lines(r, |line| {
                let f = fields(line, sep);
                if head.len() < 2 {
                    // DSU's first line is a lone `""`.
                    head = f;
                    return Ok(());
                }
                let at = |name: &str, last: bool| {
                    let i = if last { head.iter().rposition(|h| h == name) } else { head.iter().position(|h| h == name) };
                    i.and_then(|i| f.get(i)).map_or("", |s| s.trim())
                };
                each(if format == Format::DsuCsv {
                    // `Nr` twice: the DSU member number first, the FIDE ID last.
                    Player {
                        id: at("Nr", false).into(),
                        name: at("Navn", false).into(),
                        federation: "DEN".into(),
                        club: at("Klub", false).into(),
                        title: at("Titel", false).into(),
                        rating: number(at("Rat.", false)),
                        rapid: number(at("Hur.", false)),
                        blitz: number(at("Lyn", false)),
                        born: number(at("Født", false)),
                        fide_id: at("Nr", true).into(),
                    }
                } else {
                    Player {
                        id: at("ECF_code", false).into(),
                        name: at("full_name", false).into(),
                        federation: at("nation", false).into(),
                        club: at("club_name", false).into(),
                        title: at("title", false).into(),
                        rating: number(at("revised_standard", false)),
                        rapid: number(at("revised_rapid", false)),
                        blitz: number(at("revised_blitz", false)),
                        born: None,
                        fide_id: at("FIDE_no", false).into(),
                    }
                })
            })
        }
    }
}

/// The players in the downloaded file `file` of `list`: the first text file in it when it is a
/// zip.
pub fn read(list: &List, file: &Path, each: Each) -> io::Result<()> {
    let mut f = std::fs::File::open(file)?;
    let mut magic = [0; 4];
    let zipped = f.read(&mut magic)? == 4 && magic == *b"PK\x03\x04";
    f.seek(io::SeekFrom::Start(0))?;
    if !zipped {
        return parse(list.format, &mut BufReader::new(f), each);
    }
    let mut z = zip::ZipArchive::new(f).map_err(io::Error::other)?;
    let txt = z.file_names().flatten().map(|n| n.into_owned()).find(|n| n.to_ascii_lowercase().ends_with(".txt"));
    let entry = match txt {
        Some(n) => z.by_name(&n),
        None => z.by_index(0),
    }
    .map_err(io::Error::other)?;
    parse(list.format, &mut BufReader::new(entry), each)
}

/// The rating lists file: in the data folder.
pub fn path() -> Option<PathBuf> {
    Some(bragd_core::dirs::app_data()?.join("rating-lists.sqlite"))
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS players (
    list TEXT NOT NULL,
    id TEXT NOT NULL,
    name TEXT NOT NULL,
    key TEXT NOT NULL,
    federation TEXT NOT NULL,
    club TEXT NOT NULL,
    title TEXT NOT NULL,
    rating INTEGER,
    rapid INTEGER,
    blitz INTEGER,
    born INTEGER,
    fide_id TEXT NOT NULL,
    PRIMARY KEY (list, id)
);
CREATE INDEX IF NOT EXISTS players_id ON players (id);
CREATE INDEX IF NOT EXISTS players_fide ON players (fide_id);
CREATE TABLE IF NOT EXISTS lists (list TEXT PRIMARY KEY, players INTEGER NOT NULL, at INTEGER NOT NULL);
-- The words of each name's `key`, so a search reads the players it finds, not all of them.
CREATE VIRTUAL TABLE IF NOT EXISTS names USING fts5 (key, content = 'players', content_rowid = 'rowid', tokenize = 'unicode61', prefix = '2 3');
";

fn open(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let db = Connection::open(path)?;
    db.busy_timeout(std::time::Duration::from_secs(30))?;
    db.execute_batch(SCHEMA)?;
    Ok(db)
}

/// A name as search compares it: lower case, the accents most European names carry taken off,
/// and anything that is not a letter or digit a space.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        let f = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ą' | 'ă' => "a",
            'æ' => "ae",
            'ç' | 'ć' | 'č' => "c",
            'ď' | 'đ' | 'ð' => "d",
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => "e",
            'ģ' | 'ğ' => "g",
            'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' | 'ı' => "i",
            'ķ' => "k",
            'ļ' | 'ł' => "l",
            'ñ' | 'ń' | 'ņ' | 'ň' => "n",
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ő' => "o",
            'œ' => "oe",
            'ř' => "r",
            'ś' | 'š' | 'ş' | 'ș' => "s",
            'ß' => "ss",
            'ť' | 'ţ' | 'ț' => "t",
            'þ' => "th",
            'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' | 'ų' => "u",
            'ý' | 'ÿ' => "y",
            'ź' | 'ż' | 'ž' => "z",
            c if c.is_alphanumeric() => {
                out.push(c);
                continue;
            }
            _ => " ",
        };
        out.push_str(f);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Replaces what the list `list` held with the players `fill` hands on, in one transaction: a
/// list that fails half-way leaves the one before it whole. How many were stored.
pub fn import_with(path: &Path, list: &str, fill: impl FnOnce(Each) -> io::Result<()>) -> io::Result<u64> {
    let sql = |e: rusqlite::Error| io::Error::other(e);
    let mut db = open(path).map_err(sql)?;
    let tx = db.transaction().map_err(sql)?;
    tx.execute("DELETE FROM players WHERE list = ?1", [list]).map_err(sql)?;
    let mut n = 0;
    {
        let mut q = tx.prepare("INSERT OR REPLACE INTO players (list, id, name, key, federation, club, title, rating, rapid, blitz, born, fide_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)").map_err(sql)?;
        fill(&mut |p: Player| {
            if p.id.is_empty() || p.name.trim().is_empty() {
                return Ok(());
            }
            q.execute(params![list, p.id, p.name.trim(), fold(&p.name), p.federation, p.club, p.title, p.rating, p.rapid, p.blitz, p.born, p.fide_id]).map_err(sql)?;
            n += 1;
            Ok(())
        })?;
    }
    tx.execute("INSERT INTO names (names) VALUES ('rebuild')", []).map_err(sql)?;
    tx.execute("INSERT OR REPLACE INTO lists (list, players, at) VALUES (?1, ?2, ?3)", params![list, n as i64, bragd_core::fs::now() as i64]).map_err(sql)?;
    tx.commit().map_err(sql)?;
    Ok(n)
}

/// `import_with` for players at hand.
pub fn import(path: &Path, list: &str, players: impl IntoIterator<Item = Player>) -> io::Result<u64> {
    import_with(path, list, |each| players.into_iter().try_for_each(each))
}

/// The lists imported so far.
pub fn lists(path: &Path) -> rusqlite::Result<Vec<Imported>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut q = db.prepare("SELECT list, players, at FROM lists ORDER BY list")?;
    let rows = q.query_map([], |r| Ok(Imported { list: r.get(0)?, players: r.get::<_, i64>(1)? as u64, at: r.get::<_, i64>(2)? as u64 }))?;
    rows.collect()
}

/// The most players one search gives.
pub const LIMIT: usize = 50;

/// Players whose ID (or FIDE ID) is `query`, or whose name has a word starting with each word
/// of it, in any order and without regard to accents: the highest rated first.
pub fn search(path: &Path, query: &str) -> rusqlite::Result<Vec<Found>> {
    let q = query.trim();
    if q.is_empty() || !path.exists() {
        return Ok(vec![]);
    }
    search_on(&Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?, q)
}

fn search_on(db: &Connection, q: &str) -> rusqlite::Result<Vec<Found>> {
    const COLS: &str = "SELECT list, id, name, federation, club, title, rating, rapid, blitz, born, fide_id FROM players";
    let row = |r: &rusqlite::Row| {
        Ok(Found {
            list: r.get(0)?,
            player: Player { id: r.get(1)?, name: r.get(2)?, federation: r.get(3)?, club: r.get(4)?, title: r.get(5)?, rating: r.get(6)?, rapid: r.get(7)?, blitz: r.get(8)?, born: r.get(9)?, fide_id: r.get(10)? },
        })
    };
    if q.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && q.chars().any(|c| c.is_ascii_digit()) {
        let mut s = db.prepare(&format!("{COLS} WHERE id = ?1 OR fide_id = ?1 ORDER BY list LIMIT {LIMIT}"))?;
        return s.query_map([q], row)?.collect();
    }
    // Every word, as the start of a word of the name: `fold` leaves letters and digits only.
    let words: Vec<String> = fold(q).split(' ').filter(|w| !w.is_empty()).map(|w| format!("\"{w}\"*")).collect();
    if words.is_empty() {
        return Ok(vec![]);
    }
    let mut s = db.prepare(&format!("{COLS} WHERE rowid IN (SELECT rowid FROM names WHERE names MATCH ?1) ORDER BY rating IS NULL, rating DESC, name LIMIT {LIMIT}"))?;
    s.query_map([words.join(" ")], row)?.collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: &str, name: &str, rating: Option<u32>) -> Player {
        Player { id: id.into(), name: name.into(), rating, ..Player::default() }
    }

    #[test]
    fn names_fold_without_accents_or_punctuation() {
        assert_eq!(fold("Møller, Søren-Åge"), "moller soren age");
        assert_eq!(fold("Straße  Łukasz"), "strasse lukasz");
        assert_eq!(fold("Æble"), "aeble");
    }

    #[test]
    fn players_are_found_by_name_words_in_any_order_or_by_id() {
        let d = std::env::temp_dir().join(format!("bragd-lists-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let db = d.join("rating-lists.sqlite");
        assert!(search(&db, "carlsen").unwrap().is_empty(), "no file yet: nothing found");
        import(&db, "fide", [p("1503014", "Carlsen, Magnus", Some(2830)), p("1401234", "Møller, Søren", Some(1900)), p("1409999", "Moller, Bo", None), p("", "No id", None)]).unwrap();
        import(&db, "dsu", [Player { fide_id: "1401234".into(), ..p("12345", "Søren Møller", Some(1950)) }]).unwrap();
        let names = |q: &str| search(&db, q).unwrap().into_iter().map(|f| format!("{}:{}", f.list, f.player.name)).collect::<Vec<_>>();
        assert_eq!(names("magnus carlsen"), ["fide:Carlsen, Magnus"]);
        // Accents or none, rated before unrated.
        assert_eq!(names("moller"), ["dsu:Søren Møller", "fide:Møller, Søren", "fide:Moller, Bo"]);
        assert_eq!(names("Møller bo"), ["fide:Moller, Bo"]);
        // An ID finds the player on their own list and on the list that gives it as a FIDE ID.
        assert_eq!(names("1401234"), ["dsu:Søren Møller", "fide:Møller, Søren"]);
        assert_eq!(names("12345"), ["dsu:Søren Møller"]);
        assert!(names("%").is_empty());
        // A list imported again replaces what it had.
        assert_eq!(import(&db, "fide", [p("1503014", "Carlsen, Magnus", Some(2840))]).unwrap(), 1);
        assert_eq!(names("moller"), ["dsu:Søren Møller"]);
        let l = lists(&db).unwrap();
        assert_eq!(l.iter().map(|i| (i.list.as_str(), i.players)).collect::<Vec<_>>(), [("dsu", 1), ("fide", 1)]);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A name search asks the name index, not every player: on FIDE's whole list (1.95 M
    /// players, 311 MB) a scan took some 7.8 M SQLite steps for each search.
    #[test]
    fn a_name_search_does_not_read_every_player() {
        let d = std::env::temp_dir().join(format!("bragd-lists-work-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let db = d.join("rating-lists.sqlite");
        let many = (0..20_000).map(|i| p(&i.to_string(), &format!("Player{i}, Name{}", i % 97), Some(1000 + i % 1500)));
        import(&db, "fide", many.chain([p("1503014", "Carlsen, Magnus", Some(2830))])).unwrap();
        let mut c = Connection::open(&db).unwrap();
        let steps = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let counted = steps.clone();
        c.progress_handler(1, Some(move || {
            counted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            false
        }))
        .unwrap();
        let found = search_on(&c, "magnus carl").unwrap();
        assert_eq!(found.iter().map(|f| f.player.name.as_str()).collect::<Vec<_>>(), ["Carlsen, Magnus"]);
        // A scan is four steps or more for each of the 20,001 players.
        let steps = steps.load(std::sync::atomic::Ordering::Relaxed);
        assert!(steps < 2_000, "{steps} steps");
        let _ = std::fs::remove_dir_all(&d);
    }

    fn parsed(format: Format, bytes: &[u8]) -> Vec<Player> {
        let mut out = vec![];
        parse(format, &mut &bytes[..], &mut |p| {
            out.push(p);
            Ok(())
        })
        .unwrap();
        out
    }

    /// FIDE's header and two lines as FIDE writes them, columns where its header puts them.
    const FIDE: &str = "ID Number      Name                                                         Fed Sex Tit  WTit OTit           FOA SRtng SGm SK RRtng RGm Rk BRtng BGm BK B-day Flag\r\n\
1503014        Carlsen, Magnus                                              NOR M   GM                           2830  0   10 2780  0   20 2880  0   20 1990      \r\n\
10688862       Abdel Maabod, Hoda                                           EGY F                                0     0   0  0     0   0  0     0   0  2009  w   \r\n";

    #[test]
    fn the_fide_list_is_read_by_its_header_columns() {
        let ps = parsed(Format::FideTxt, FIDE.as_bytes());
        assert_eq!(ps.len(), 2);
        let c = &ps[0];
        assert_eq!((c.id.as_str(), c.name.as_str(), c.federation.as_str(), c.title.as_str()), ("1503014", "Carlsen, Magnus", "NOR", "GM"));
        assert_eq!((c.rating, c.rapid, c.blitz, c.born, c.fide_id.as_str()), (Some(2830), Some(2780), Some(2880), Some(1990), "1503014"));
        // Unrated: no rating rather than 0.
        assert_eq!((ps[1].rating, ps[1].title.as_str(), ps[1].born), (None, "", Some(2009)));
    }

    #[test]
    fn the_dsu_list_is_read_in_latin_1_with_both_numbers() {
        let mut bytes = b"\"\"\r\n\"Nr\";\"F\xf8dt\";\"Alder\";\"Navn\";\"Klub\";\"Titel\";\"Rat.\";\"Hur.\";\"Lyn\";\"Fide\";\"Nr\";\"K\"\r\n".to_vec();
        bytes.extend_from_slice(b"\"100602738\";\"2004\";\"22\";\"S\xf8ren M\xf8ller\";\"Skakklubben; \xc5rhus\";\"FM\";\"2262\";\"2180\";\"\";\"2262\";\"12923044\";\"10\"\r\n");
        let ps = parsed(Format::DsuCsv, &bytes);
        assert_eq!(ps.len(), 1);
        let p = &ps[0];
        assert_eq!((p.id.as_str(), p.name.as_str(), p.club.as_str(), p.title.as_str(), p.federation.as_str()), ("100602738", "Søren Møller", "Skakklubben; Århus", "FM", "DEN"));
        assert_eq!((p.rating, p.rapid, p.blitz, p.born, p.fide_id.as_str()), (Some(2262), Some(2180), None, Some(2004), "12923044"));
    }

    #[test]
    fn the_ecf_list_is_read_with_quoted_names() {
        let csv = "ECF_code,full_name,member_no,FIDE_no,gender,nation,original_standard,standard_original_category,revised_standard,standard_revised_category,original_rapid,rapid_original_category,revised_rapid,rapid_revised_category,original_blitz,blitz_original_category,revised_blitz,blitz_revised_category,club_code,club_name,title\n\
100057D,\"Kennaugh, Charles W\",24424,400815,M,ENG,2110,,2118,K,,,,,,,,,7ATT,Atticus,FM\n";
        let ps = parsed(Format::EcfCsv, csv.as_bytes());
        let p = &ps[0];
        assert_eq!((p.id.as_str(), p.name.as_str(), p.federation.as_str(), p.club.as_str(), p.title.as_str(), p.fide_id.as_str()), ("100057D", "Kennaugh, Charles W", "ENG", "Atticus", "FM", "400815"));
        assert_eq!((p.rating, p.rapid), (Some(2118), None));
    }

    #[test]
    fn a_zipped_list_is_read_from_its_text_file_into_the_lists() {
        use std::io::Write;
        let d = std::env::temp_dir().join(format!("bragd-lists-zip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let zip = d.join("players_list.zip");
        let mut w = zip::ZipWriter::new(std::fs::File::create(&zip).unwrap());
        w.start_file("players_list_foa.txt", zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(FIDE.as_bytes()).unwrap();
        w.finish().unwrap();
        let db = d.join("rating-lists.sqlite");
        assert_eq!(import_with(&db, "fide-ratings", |each| read(&LISTS[0], &zip, each)).unwrap(), 2);
        assert_eq!(search(&db, "carlsen").unwrap()[0].player.rating, Some(2830));
        let _ = std::fs::remove_dir_all(&d);
    }
}

