//! Downloads for Sources: behind a trait, so tests use recorded files and never the network.
//! A download goes to `<name>.part` first and goes on from where it stopped; only a complete
//! one gets its name.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// What a server gave for a URL.
pub enum Got {
    /// 404 or 410: not there (yet). How TWIC says an issue is not out.
    Missing,
    /// The body; `resumed` when it starts at the byte asked for, not at the beginning; `size`:
    /// its length, when the server says (`Content-Length`).
    Body { resumed: bool, size: Option<u64>, body: Box<dyn Read + Send> },
}

/// Something that fetches URLs.
pub trait Fetcher: Send + Sync {
    /// `url` from byte `from` on (a range request when `from` > 0).
    fn get(&self, url: &str, from: u64) -> io::Result<Got>;
    /// Whether `url` is there, without its body where the server allows.
    fn exists(&self, url: &str) -> io::Result<bool> {
        Ok(!matches!(self.get(url, 0)?, Got::Missing))
    }
}

/// The real thing, over HTTPS with the system's certificate store.
pub struct Web(ureq::Agent);

impl Default for Web {
    fn default() -> Self {
        Web(bragd_engines::fetch::agent(false))
    }
}

fn status_error(url: &str, status: u16) -> io::Error {
    io::Error::other(format!("{url}: HTTP {status}"))
}

impl Fetcher for Web {
    fn get(&self, url: &str, from: u64) -> io::Result<Got> {
        let mut req = self.0.get(url);
        if from > 0 {
            req = req.header("Range", format!("bytes={from}-"));
        }
        let r = req.call().map_err(io::Error::other)?;
        let resumed = match r.status().as_u16() {
            404 | 410 => return Ok(Got::Missing),
            206 => true,
            200 => false,
            s => return Err(status_error(url, s)),
        };
        let size = r.body().content_length();
        Ok(Got::Body { resumed, size, body: Box::new(r.into_body().into_reader()) })
    }

    fn exists(&self, url: &str) -> io::Result<bool> {
        let r = self.0.head(url).call().map_err(io::Error::other)?;
        match r.status().as_u16() {
            404 | 410 => Ok(false),
            s if (200..400).contains(&s) => Ok(true),
            s => Err(status_error(url, s)),
        }
    }
}

/// A download over its cap. `size`: what the server said the file is, before anything was
/// written, so the user can be asked; none when it said nothing and the cap was passed as the
/// file came.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TooBig {
    pub url: String,
    pub size: Option<u64>,
    pub cap: u64,
}

impl std::fmt::Display for TooBig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.size {
            Some(size) => write!(f, "{} is {size} bytes, more than its cap of {} bytes", self.url, self.cap),
            None => write!(f, "{} is larger than its cap of {} bytes", self.url, self.cap),
        }
    }
}

impl std::error::Error for TooBig {}

impl TooBig {
    /// The `TooBig` inside an error a download gave, if that is what it is.
    pub fn of(e: &io::Error) -> Option<&TooBig> {
        e.get_ref().and_then(|e| e.downcast_ref::<TooBig>())
    }
}

/// Downloads `url` to `to`, going on from a `.part` left by one that stopped. Nothing past
/// `cap` bytes is downloaded: a file the server says is larger is refused before its first
/// byte, and one of no stated size is cut off as it passes the cap (`TooBig`). `stop` is
/// asked between pieces and leaves the `.part` for next time; `progress` hears the bytes so
/// far. `Ok(false)`: the server has no such file.
pub fn download(net: &dyn Fetcher, url: &str, to: &Path, cap: u64, stop: &dyn Fn() -> bool, progress: &mut dyn FnMut(u64)) -> io::Result<bool> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir)?;
    }
    let part = part_of(to);
    let had = fs::metadata(&part).map_or(0, |m| m.len());
    let (resumed, size, mut body) = match net.get(url, had)? {
        Got::Missing => return Ok(false),
        Got::Body { resumed, size, body } => (resumed, size, body),
    };
    let size = size.map(|n| if resumed { had + n } else { n });
    if let Some(size) = size.filter(|&n| n > cap) {
        return Err(io::Error::other(TooBig { url: url.into(), size: Some(size), cap }));
    }
    // A server that ignores the range sends it all again: start the file over.
    let mut out = if resumed { OpenOptions::new().append(true).open(&part)? } else { File::create(&part)? };
    let mut done = if resumed { had } else { 0 };
    let mut buf = vec![0u8; 1 << 16];
    loop {
        if stop() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "stopped"));
        }
        let n = body.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        done += n as u64;
        if done > cap {
            drop(out);
            let _ = fs::remove_file(&part);
            return Err(io::Error::other(TooBig { url: url.into(), size: None, cap }));
        }
        progress(done);
    }
    out.sync_all()?;
    drop(out);
    fs::rename(&part, to)?;
    Ok(true)
}

pub(crate) fn part_of(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".part");
    PathBuf::from(s)
}

/// The kinds of file a Library imports, by extension (`.pgn.gz`, `.pgn.zst` and `.pgn.bz2` too).
pub fn importable(name: &str) -> bool {
    let n = bragd_library::stream::unpacked_name(name).to_ascii_lowercase();
    [".pgn", ".sgf", ".kif", ".csa", ".pdn"].iter().any(|e| n.ends_with(e))
}

/// Whether the connection is metered: `Some` where the system says cheaply (NetworkManager on
/// Linux), `None` where it cannot be told.
pub fn metered() -> Option<bool> {
    // shortcut: NetworkManager only; macOS (NWPath.isExpensive) and Windows (NetworkCostType) need native APIs, add them when a user asks.
    if !cfg!(target_os = "linux") {
        return None;
    }
    let out = bragd_engines::tools::command("busctl")
        .args(["--system", "get-property", "org.freedesktop.NetworkManager", "/org/freedesktop/NetworkManager", "org.freedesktop.NetworkManager", "Metered"])
        .output()
        .ok()?;
    out.status.success().then_some(())?;
    nm_metered(&String::from_utf8_lossy(&out.stdout))
}

/// NetworkManager's `Metered` as busctl prints it (`u 4`): 1 yes, 3 guessed yes, 2 and 4 no,
/// 0 unknown.
fn nm_metered(s: &str) -> Option<bool> {
    match s.trim().strip_prefix("u ")?.trim() {
        "1" | "3" => Some(true),
        "2" | "4" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    /// Recorded files by URL; every request is noted. A URL not there is a 404.
    #[derive(Default)]
    pub struct Recorded {
        pub files: BTreeMap<String, Vec<u8>>,
        pub asked: Mutex<Vec<String>>,
        /// Honour ranges, as most servers do.
        pub ranges: bool,
        /// Say how long each body is (`Content-Length`).
        pub sizes: bool,
    }

    impl Fetcher for Recorded {
        fn get(&self, url: &str, from: u64) -> io::Result<Got> {
            self.asked.lock().unwrap().push(format!("GET {url} {from}"));
            let Some(b) = self.files.get(url) else { return Ok(Got::Missing) };
            let resumed = self.ranges && from > 0;
            let b = if resumed { b[from as usize..].to_vec() } else { b.clone() };
            let size = self.sizes.then_some(b.len() as u64);
            Ok(Got::Body { resumed, size, body: Box::new(io::Cursor::new(b)) })
        }
        fn exists(&self, url: &str) -> io::Result<bool> {
            self.asked.lock().unwrap().push(format!("HEAD {url}"));
            Ok(self.files.contains_key(url))
        }
    }

    #[test]
    fn a_download_goes_on_from_its_part() {
        let d = crate::tests::Dir::new("resume");
        let to = d.0.join("a.pgn");
        let net = Recorded { files: [("u".to_string(), b"0123456789".to_vec())].into(), ranges: true, ..Default::default() };
        fs::write(part_of(&to), b"0123").unwrap();
        assert!(download(&net, "u", &to, 100, &|| false, &mut |_| {}).unwrap());
        assert_eq!(fs::read(&to).unwrap(), b"0123456789");
        assert_eq!(net.asked.lock().unwrap().as_slice(), ["GET u 4"]);
        // A server that ignores the range: the file starts over, whole.
        let net = Recorded { ranges: false, ..net };
        fs::write(part_of(&to), b"xx").unwrap();
        assert!(download(&net, "u", &to, 100, &|| false, &mut |_| {}).unwrap());
        assert_eq!(fs::read(&to).unwrap(), b"0123456789");
        assert!(!download(&net, "nothing", &d.0.join("b.pgn"), 100, &|| false, &mut |_| {}).unwrap());
    }

    #[test]
    fn a_download_over_its_cap_is_refused_first_when_its_size_is_known() {
        let d = crate::tests::Dir::new("cap");
        let to = d.0.join("b.pgn");
        let files: BTreeMap<String, Vec<u8>> = [("u".to_string(), b"0123456789".to_vec())].into();
        // The server says how big: refused before a byte is written, with the size to ask about.
        let net = Recorded { files: files.clone(), sizes: true, ..Default::default() };
        let mut heard = vec![];
        let e = download(&net, "u", &to, 5, &|| false, &mut |n| heard.push(n)).unwrap_err();
        assert_eq!(TooBig::of(&e), Some(&TooBig { url: "u".into(), size: Some(10), cap: 5 }));
        assert!(heard.is_empty() && !to.exists() && !part_of(&to).exists());
        // At the cap exactly it comes.
        assert!(download(&net, "u", &to, 10, &|| false, &mut |_| {}).unwrap());
        // No size given: cut off as it passes the cap, nothing left behind.
        let to = d.0.join("c.pgn");
        let net = Recorded { files, sizes: false, ..Default::default() };
        let e = download(&net, "u", &to, 5, &|| false, &mut |_| {}).unwrap_err();
        assert_eq!(TooBig::of(&e).and_then(|t| t.size), None);
        assert!(TooBig::of(&e).is_some() && !to.exists() && !part_of(&to).exists());
        // A resumed download counts what it had.
        let to = d.0.join("d.pgn");
        let net = Recorded { ranges: true, sizes: true, ..net };
        fs::write(part_of(&to), b"0123").unwrap();
        let e = download(&net, "u", &to, 9, &|| false, &mut |_| {}).unwrap_err();
        assert_eq!(TooBig::of(&e).and_then(|t| t.size), Some(10));
    }

    /// With `BRAGD_OFFLINE` the Helper asks nobody: the real fetcher refuses before it dials.
    #[test]
    fn offline_asks_nobody() {
        let web = Web { offline: true, ..Web::default() };
        assert!(web.offline() && !Recorded::default().offline());
        let e = web.get("https://example.org/twic.zip", 0).err().expect("refused");
        assert!(e.to_string().contains("BRAGD_OFFLINE"), "{e}");
        assert!(web.exists("https://example.org/twic.zip").is_err());
    }

    #[test]
    fn importable_names_and_networkmanager_answers() {
        assert!(importable("twic1600.pgn") && importable("x.SGF") && importable("a.pgn.zst"));
        assert!(!importable("readme.txt") && !importable("a.zip"));
        assert_eq!(nm_metered("u 1\n"), Some(true));
        assert_eq!(nm_metered("u 4\n"), Some(false));
        assert_eq!(nm_metered("u 0\n"), None);
    }
}
