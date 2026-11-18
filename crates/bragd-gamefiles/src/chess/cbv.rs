//! ChessBase archives (`.cbv`): a database packed into one file. This reads the archive's
//! list of members (`plan`), picks those a database's games need (`.cbh`, `.cbg`, `.cba`,
//! `.cbp`, `.cbt`, `.cbj`, or an archived PGN database's `.pgn`), and unpacks each to a writer
//! (`unpack`). Where they are unpacked to is the caller's.
//!
//! Written from the specification of facts in the MIT-licensed `cbvault` (github.com/itshak/
//! cbvault, `docs/format-spec-uncbv.md`), no code of its or of any GPL program read: an 8-byte
//! head (`08 00`, the member count, the record length), a table of members (a name of 128
//! bytes, its place, packed and unpacked lengths), then each member's blocks: a payload length,
//! a word not needed, and the payload, whose first byte says how it is packed (stored, LZ,
//! Huffman, or Huffman then LZ).
//!
//! Every byte is untrusted: lengths are checked against the source before use; a member is
//! streamed to its writer (never held whole) and unpacked to no more than its stated length; a
//! stated length far past what its packed bytes could honestly hold, and archives over
//! `MAX_TOTAL` in all, are refused before a byte is unpacked; and a damaged member is an error.

use std::io::{self, Read, SeekFrom, Write};

use crate::Source;

/// The most one archive unpacks to, all its members together: the biggest real databases'
/// `.cbg` files run to a few GB.
const MAX_TOTAL: u64 = 16 << 30;
/// A member stated larger than this many times its packed bytes is refused (real ones pack a
/// few times smaller; a crafted one of runs and back-references could claim 1370 times), unless
/// it is no larger than `SMALL`.
const MAX_RATIO: u64 = 100;
const SMALL: u64 = 64 << 20;
/// The members a database's games need.
const NEEDED: [&str; 6] = ["cbh", "cbg", "cba", "cbp", "cbt", "cbj"];

fn bad(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("not a ChessBase archive: {what}"))
}

/// A member of the archive: its name, where its blocks start, their length and what it unpacks to.
#[derive(Clone, Debug)]
pub struct Member {
    pub name: String,
    at: u64,
    pub packed: u64,
    pub size: u64,
}

fn members(f: &mut dyn Source) -> io::Result<Vec<Member>> {
    let len = crate::len(f)?;
    let mut head = [0u8; 8];
    f.read_exact(&mut head).map_err(|_| bad("too short"))?;
    if head[..2] != [8, 0] {
        return Err(bad("head"));
    }
    let (count, rec) = (u16::from_le_bytes([head[2], head[3]]) as u64, head[4] as u64);
    let pool = 8 + count * rec;
    if rec < 140 || pool > len {
        return Err(bad("table"));
    }
    let mut table = vec![0u8; (count * rec) as usize];
    f.read_exact(&mut table)?;
    let mut at = pool;
    let mut out = vec![];
    for r in table.chunks_exact(rec as usize) {
        let u32_at = |i: usize| u32::from_le_bytes([r[i], r[i + 1], r[i + 2], r[i + 3]]) as u64;
        let end = r[..128].iter().position(|&c| c == 0).unwrap_or(128);
        let name = super::scan::text(&r[..end]).into_owned();
        let (packed, size) = (u32_at(132), u32_at(136));
        if at + packed > len {
            return Err(bad("member past the end"));
        }
        out.push(Member { name, at, packed, size });
        at += packed;
    }
    Ok(out)
}

/// Where a member's bytes go: its file, through a window that keeps the last bytes for LZ's
/// back-references (at most 4098 back), so a member of any length is held in no more than
/// `FLUSH` bytes. More than `max` bytes in all is an error.
struct Out<W: Write> {
    w: W,
    win: Vec<u8>,
    written: u64,
    max: u64,
}

/// The bytes kept for back-references, and how many gather before they are written.
const WINDOW: usize = 1 << 13;
const FLUSH: usize = 1 << 16;

impl<W: Write> Out<W> {
    fn new(w: W, max: u64) -> Self {
        Out { w, win: Vec::with_capacity(FLUSH + 8192), written: 0, max }
    }

    fn len(&self) -> u64 {
        self.written + self.win.len() as u64
    }

    /// After each token: over the stated length is an error, a full buffer is written out.
    fn check(&mut self) -> io::Result<()> {
        if self.len() > self.max {
            return Err(bad("member longer than stated"));
        }
        if self.win.len() >= FLUSH {
            let cut = self.win.len() - WINDOW;
            self.w.write_all(&self.win[..cut])?;
            self.win.drain(..cut);
            self.written += cut as u64;
        }
        Ok(())
    }

    fn finish(mut self) -> io::Result<(W, u64)> {
        self.w.write_all(&self.win)?;
        let n = self.len();
        Ok((self.w, n))
    }
}

/// What an archive holds for its databases (`plan`).
#[derive(Clone, Debug)]
pub struct Plan {
    /// `cbh` for ChessBase databases, `pgn` for an archive of PGN databases.
    pub ending: &'static str,
    /// How many databases: the `n` of `db<n>.<ending>`, from 0, in the archive's order.
    pub databases: usize,
    /// The members to unpack, each by the name it is to have: `db<n>.<its ending>`.
    pub members: Vec<(String, Member)>,
    /// Their packed bytes in all.
    pub packed: u64,
}

/// What the archive `r` holds: its databases and the members their games need, weighed before
/// a byte is unpacked. Members in a folder (`name.ext\file`), pictures and the rest are left.
pub fn plan(r: &mut dyn Source) -> io::Result<Plan> {
    let list = members(r)?;
    // Members at the root only (`name.ext`), never a folder's (`name.ext\file`).
    let root = |m: &Member| !m.name.contains(['\\', '/']);
    let stems_of = |ext: &str| -> Vec<&str> {
        list.iter().filter(|m| root(m) && m.name.to_ascii_lowercase().ends_with(ext)).map(|m| &m.name[..m.name.len() - 4]).collect()
    };
    // A PGN database is backed up as an archive too: its `.pgn` file, read as one.
    let (mut ending, mut stems) = ("cbh", stems_of(".cbh"));
    if stems.is_empty() {
        (ending, stems) = ("pgn", stems_of(".pgn"));
    }
    if stems.is_empty() {
        return Err(bad("no .cbh or .pgn in it"));
    }
    // Each database's files, as `db<n>.<ext>` (`n` its place among the `.cbh` members).
    let mut wanted: Vec<(String, Member)> = vec![];
    for m in list.iter().filter(|m| root(m)) {
        if let Some((s, ext)) = m.name.rsplit_once('.')
            && let Some(n) = stems.iter().position(|stem| s.eq_ignore_ascii_case(stem))
            && (if ending == "pgn" { ext.eq_ignore_ascii_case("pgn") } else { NEEDED.contains(&ext.to_ascii_lowercase().as_str()) })
        {
            let name = format!("db{n}.{}", ext.to_ascii_lowercase());
            wanted.retain(|w| w.0 != name);
            wanted.push((name, m.clone()));
        }
    }
    if wanted.iter().any(|(_, m)| m.size > SMALL.max(m.packed.saturating_mul(MAX_RATIO))) {
        return Err(bad("a member says it is far larger than its packed bytes can hold"));
    }
    let (total, packed) = wanted.iter().fold((0, 0), |(t, p), (_, m)| (t + m.size, p + m.packed));
    if total > MAX_TOTAL || total > SMALL.max(packed.saturating_mul(MAX_RATIO)) {
        return Err(bad("larger unpacked than any real database"));
    }
    Ok(Plan { ending, databases: stems.len(), members: wanted, packed })
}

/// Member `m` of the archive `r` (as `plan` lists it), unpacked to `to`; `stop` is asked and
/// `progress` told its packed bytes read after each block.
pub fn unpack(r: &mut dyn Source, m: &Member, to: impl Write, stop: &dyn Fn() -> bool, progress: &mut dyn FnMut(u64)) -> io::Result<()> {
    r.seek(SeekFrom::Start(m.at))?;
    unpack_blocks(r, m.packed, m.size, to, stop, progress)
}

/// A member's `packed` bytes of blocks from `r`, unpacked to `to`, `size` long; `stop` is asked
/// and `progress` told the packed bytes read after each block.
fn unpack_blocks(r: impl Read, packed: u64, size: u64, to: impl Write, stop: &dyn Fn() -> bool, progress: &mut dyn FnMut(u64)) -> io::Result<()> {
    let mut r = io::BufReader::with_capacity(1 << 16, r.take(packed));
    let mut out = Out::new(to, size);
    let (mut left, mut payload) = (packed, vec![0u8; 1 << 16]);
    while left >= 4 {
        if stop() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "stopped"));
        }
        let mut head = [0u8; 4];
        r.read_exact(&mut head)?;
        let n = u16::from_le_bytes([head[0], head[1]]) as usize;
        if 4 + n as u64 > left {
            return Err(bad("block past the member"));
        }
        left -= 4 + n as u64;
        let payload = &mut payload[..n];
        r.read_exact(payload)?;
        progress(packed - left);
        let Some((&mode, data)) = payload.split_first() else { continue };
        match mode {
            0 => out.win.extend_from_slice(data),
            1 => lz(data, &mut out)?,
            2 => out.win.extend(huffman(data)?),
            3 => lz(&huffman(data)?, &mut out)?,
            _ => return Err(bad("block mode")),
        }
        out.check()?;
    }
    if out.finish()?.1 != size {
        return Err(bad("member shorter than stated"));
    }
    Ok(())
}

/// A Huffman block: its length (2 bytes, big-endian), a code for each byte value in order (4
/// bits of length, then the path, a bit at a time from the top), then the coded bytes.
fn huffman(d: &[u8]) -> io::Result<Vec<u8>> {
    let want = u16::from_be_bytes([*d.first().ok_or_else(|| bad("huffman"))?, *d.get(1).ok_or_else(|| bad("huffman"))?]) as usize;
    let mut pos = 16usize;
    let mut bits = |n: usize| -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            let byte = *d.get(pos / 8)?;
            v = v << 1 | (byte >> (7 - pos % 8) & 1) as u32;
            pos += 1;
        }
        Some(v)
    };
    // The tree: per node its two children (0 none), or the byte it ends in (`LEAF` + value).
    const LEAF: u32 = 1 << 16;
    let mut tree: Vec<[u32; 2]> = vec![[0, 0]];
    let mut sum = 0u32;
    for value in 0..256u32 {
        let n = bits(4).ok_or_else(|| bad("huffman table"))? as usize;
        if n == 0 {
            continue;
        }
        let path = bits(n).ok_or_else(|| bad("huffman table"))?;
        sum += 1 << (15 - n);
        let mut node = 0usize;
        for k in (0..n).rev() {
            let side = (path >> k & 1) as usize;
            if k == 0 {
                if tree[node][side] != 0 {
                    return Err(bad("huffman table"));
                }
                tree[node][side] = LEAF + value;
                break;
            }
            match tree[node][side] {
                0 => {
                    tree.push([0, 0]);
                    tree[node][side] = (tree.len() - 1) as u32;
                }
                c if c >= LEAF => return Err(bad("huffman table")),
                _ => {}
            }
            node = tree[node][side] as usize;
        }
    }
    if sum != 1 << 15 {
        return Err(bad("huffman table"));
    }
    let mut out = Vec::with_capacity(want);
    let mut node = 0usize;
    while out.len() < want {
        let Some(bit) = bits(1) else { break };
        match tree[node][bit as usize] {
            0 => return Err(bad("huffman code")),
            c if c >= LEAF => {
                out.push((c - LEAF) as u8);
                node = 0;
            }
            c => node = c as usize,
        }
    }
    Ok(out)
}

/// An LZ stream, added to `out` (back-references reach into what the member has so far):
/// groups of a 2-byte control word (little-endian) and up to 16 tokens, its bits from the top:
/// 0 a literal byte, 1 a coded token (a tag byte: its top half the kind, its low half part of
/// the length or distance).
fn lz<W: Write>(d: &[u8], o: &mut Out<W>) -> io::Result<()> {
    let mut i = 0;
    let byte = |i: usize| d.get(i).copied().ok_or_else(|| bad("lz past the end"));
    while i < d.len() {
        if i + 1 == d.len() {
            o.win.push(d[i]);
            break;
        }
        let control = u16::from_le_bytes([d[i], d[i + 1]]);
        i += 2;
        for k in (0..16).rev() {
            if i >= d.len() {
                break;
            }
            if control >> k & 1 == 0 {
                o.win.push(d[i]);
                i += 1;
                continue;
            }
            let tag = byte(i)?;
            let (high, low) = ((tag >> 4) as usize, (tag & 15) as usize);
            match high {
                0 => {
                    let b = byte(i + 1)?;
                    o.win.extend(std::iter::repeat_n(b, low + 3));
                    i += 2;
                }
                1 => {
                    let (b1, b2) = (byte(i + 1)? as usize, byte(i + 2)?);
                    o.win.extend(std::iter::repeat_n(b2, low + (b1 << 4) + 0x13));
                    i += 3;
                }
                _ => {
                    let b1 = byte(i + 1)? as usize;
                    let back = (b1 << 4) + low + 3;
                    let n = if high == 2 { byte(i + 2)? as usize + 0x10 } else { high };
                    i += if high == 2 { 3 } else { 2 };
                    // The window always holds the last `WINDOW` bytes or all there are.
                    let from = o.win.len().checked_sub(back).ok_or_else(|| bad("lz distance"))?;
                    for k in 0..n {
                        o.win.push(o.win[from + k]);
                    }
                }
            }
            o.check()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lz_vec(d: &[u8]) -> io::Result<Vec<u8>> {
        let mut o = Out::new(vec![], u64::MAX);
        lz(d, &mut o)?;
        Ok(o.finish()?.0)
    }

    fn unpack_vec(b: &[u8], size: u64) -> io::Result<Vec<u8>> {
        let mut out = vec![];
        unpack_blocks(b, b.len() as u64, size, &mut out, &|| false, &mut |_| {})?;
        Ok(out)
    }

    /// A stored block, an LZ block (a literal run, a short run, a long run, back-references
    /// near and far) and a Huffman block of four values: each unpacks to what it says.
    #[test]
    fn blocks_unpack() {
        let block = |payload: &[u8]| [&(payload.len() as u16).to_le_bytes()[..], &[0, 0], payload].concat();
        // LZ: control 0b0001_1111 << 11: three literals "abc", then five tokens.
        let lz_data = [
            &[0b0000_0000, 0b0001_1111][..],
            b"abc",
            &[0x02, b'x'],             // short run: 5 x
            &[0x10, 0x00, b'y'],       // long run: 19 y
            &[0x30, 0x00],             // short back-reference: 3 bytes from 3 back
            &[0x23, 0x00, 0x00],       // long back-reference: 16 bytes from 6 back
            &[0x40, 0x01],             // 4 bytes from 19 back
        ]
        .concat();
        let mut want = b"abc".to_vec();
        want.extend([b'x'; 5]);
        want.extend([b'y'; 19]);
        want.extend(b"yyy");
        for _ in 0..16 {
            want.push(want[want.len() - 6]);
        }
        for _ in 0..4 {
            want.push(want[want.len() - 19]);
        }
        assert_eq!(lz_vec(&lz_data).unwrap(), want);
        // Huffman: 'a' 0, 'b' 10, 'c' 110, 'd' 111; "abcd" is 0 10 110 111.
        let mut bits: Vec<u8> = vec![];
        let mut push = |v: u32, n: usize| (0..n).rev().for_each(|k| bits.push((v >> k & 1) as u8));
        for value in 0..256u32 {
            match value as u8 {
                b'a' => (push(1, 4), push(0, 1)),
                b'b' => (push(2, 4), push(2, 2)),
                b'c' => (push(3, 4), push(6, 3)),
                b'd' => (push(3, 4), push(7, 3)),
                _ => (push(0, 4), ()),
            };
        }
        for (v, n) in [(0, 1), (2, 2), (6, 3), (7, 3)] {
            push(v, n);
        }
        let mut huff = vec![0u8, 4];
        huff.extend(bits.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |b, (i, &v)| b | v << (7 - i))));
        assert_eq!(huffman(&huff).unwrap(), b"abcd");
        let member = [block(&[&[0][..], b"st"].concat()), block(&[&[1][..], &lz_data].concat()), block(&[&[2][..], &huff].concat())].concat();
        let all = [&b"st"[..], &want, b"abcd"].concat();
        assert_eq!(unpack_vec(&member, all.len() as u64).unwrap(), all);
        assert!(unpack_vec(&member, all.len() as u64 + 1).is_err(), "shorter than stated");
        // A table that is no prefix code, a distance before the start, junk: errors, no panic.
        assert!(huffman(&[0, 4, 0x10]).is_err());
        assert!(lz_vec(&[0x00, 0x80, 0x30, 0x05]).is_err());
        let mut x = 7u32;
        for len in 0..300 {
            let junk: Vec<u8> = (0..len)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    x as u8
                })
                .collect();
            let _ = unpack_vec(&junk, 1000);
            let _ = huffman(&junk);
            let _ = lz_vec(&junk);
        }
    }

    /// An LZ block of nothing but the longest runs: 65,500 bytes that unpack to 86 MB.
    fn bomb() -> Vec<u8> {
        let mut payload = vec![1u8];
        while payload.len() + 50 <= 65_535 {
            payload.extend([0xff, 0xff]);
            for _ in 0..16 {
                payload.extend([0x1f, 0xff, b'z']);
            }
        }
        [&(payload.len() as u16).to_le_bytes()[..], &[0, 0], &payload].concat()
    }

    /// A crafted archive cannot take the memory, the disk or the time: a member is streamed
    /// through a window of fixed size, never past its stated length, stops when told, and one
    /// that claims gigabytes from a few packed bytes is refused before a byte is written.
    #[test]
    fn a_bomb_is_bounded_and_refused() {
        let block = bomb();
        let mut o = Out::new(io::sink(), u64::MAX);
        lz(&block[5..], &mut o).unwrap();
        assert!(o.len() > 80 << 20);
        assert_eq!(o.win.capacity(), FLUSH + 8192, "the window never grew");
        // Past its stated length: an error at that length, the rest never unpacked.
        let three = block.repeat(3);
        let mut counted = 0u64;
        let r = unpack_blocks(&three[..], three.len() as u64, 10 << 20, CountWriter(&mut counted), &|| false, &mut |_| {});
        assert!(r.is_err() && counted <= 10 << 20);
        // Stopped: at once.
        let r = unpack_blocks(&three[..], three.len() as u64, u32::MAX as u64, io::sink(), &|| true, &mut |_| {});
        assert_eq!(r.unwrap_err().kind(), io::ErrorKind::Interrupted);
        // An archive whose one member claims 4 GB from 65 KB: refused before a byte is unpacked.
        let rec = 140usize;
        let mut file = vec![8u8, 0, 1, 0, rec as u8, 0, 3, 0];
        let mut r = vec![0u8; rec];
        r[..9].copy_from_slice(b"Games.cbh");
        r[132..136].copy_from_slice(&(block.len() as u32).to_le_bytes());
        r[136..140].copy_from_slice(&u32::MAX.to_le_bytes());
        file.extend(r);
        file.extend(&block);
        assert!(plan(&mut io::Cursor::new(file)).unwrap_err().to_string().contains("far larger"));
        // Three members each claiming 60 MB from 65 KB, under what one member alone may claim:
        // together far past what their packed bytes can hold, refused all the same.
        let mut file = vec![8u8, 0, 3, 0, rec as u8, 0, 3, 0];
        for (i, name) in ["Games.cbh", "Games.cbg", "Games.cba"].iter().enumerate() {
            let mut r = vec![0u8; rec];
            r[..9].copy_from_slice(name.as_bytes());
            r[128..132].copy_from_slice(&((8 + 3 * rec + i * block.len()) as u32).to_le_bytes());
            r[132..136].copy_from_slice(&(block.len() as u32).to_le_bytes());
            r[136..140].copy_from_slice(&(60u32 << 20).to_le_bytes());
            file.extend(r);
        }
        file.extend(block.repeat(3));
        let e = plan(&mut io::Cursor::new(file)).unwrap_err().to_string();
        assert!(e.contains("larger unpacked"), "{e}");
    }

    struct CountWriter<'a>(&'a mut u64);
    impl Write for CountWriter<'_> {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            *self.0 += b.len() as u64;
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// The test archive (made by ChessBase from `cbh-test/Games.*`, a picture beside it)
    /// plans as one database and unpacks to the very files it was made from; cut short, it
    /// is refused.
    #[test]
    fn an_archive_unpacks_to_its_database() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/chess/cbh-test");
        let bytes = std::fs::read(dir.join("Games.cbv")).unwrap();
        let mut r = io::Cursor::new(&bytes[..]);
        let p = plan(&mut r).unwrap();
        assert_eq!((p.ending, p.databases), ("cbh", 1));
        let mut names: Vec<&str> = p.members.iter().map(|m| m.0.as_str()).collect();
        names.sort();
        assert_eq!(names, ["db0.cba", "db0.cbg", "db0.cbh", "db0.cbp", "db0.cbt"]);
        for (name, m) in &p.members {
            let mut out = vec![];
            unpack(&mut r, m, &mut out, &|| false, &mut |_| {}).unwrap();
            assert_eq!(out, std::fs::read(dir.join(name.replace("db0", "Games"))).unwrap(), "{name}");
        }
        let cut = &bytes[..bytes.len() - 10];
        let ok = plan(&mut io::Cursor::new(cut)).and_then(|p| p.members.iter().try_for_each(|(_, m)| unpack(&mut io::Cursor::new(cut), m, io::sink(), &|| false, &mut |_| {})));
        assert!(ok.is_err());
    }
}
