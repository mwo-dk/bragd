//! Fetching over HTTPS, behind a trait so the tablebase planner, the downloader and the Lichess
//! client are tested without the network.

use std::io::{Read, Write};

/// Something that fetches URLs.
pub trait Fetch: Sync {
    /// The body of `url`, refused past `max` bytes.
    fn get(&self, url: &str, max: u64) -> std::io::Result<Vec<u8>>;
    /// The body of `url` written into `to`, `progress` told the bytes so far after each piece
    /// (false stops it). The bytes written.
    fn download(&self, url: &str, to: &mut dyn Write, progress: &mut dyn FnMut(u64) -> bool) -> std::io::Result<u64>;
}

/// The real thing, over HTTPS.
pub struct Http;

const AGENT: &str = concat!("Bragd/", env!("CARGO_PKG_VERSION"), " (https://github.com/mwo-dk/bragd)");

fn io(e: ureq::Error) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

impl Fetch for Http {
    fn get(&self, url: &str, max: u64) -> std::io::Result<Vec<u8>> {
        let mut r = ureq::get(url).header("User-Agent", AGENT).call().map_err(io)?;
        r.body_mut().with_config().limit(max).read_to_vec().map_err(io)
    }

    fn download(&self, url: &str, to: &mut dyn Write, progress: &mut dyn FnMut(u64) -> bool) -> std::io::Result<u64> {
        let r = ureq::get(url).header("User-Agent", AGENT).call().map_err(io)?;
        let mut body = r.into_body().into_reader();
        let (mut buf, mut done) = (vec![0u8; 1 << 16], 0u64);
        loop {
            let n = body.read(&mut buf)?;
            if n == 0 {
                return Ok(done);
            }
            to.write_all(&buf[..n])?;
            done += n as u64;
            if !progress(done) {
                return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "stopped"));
            }
        }
    }
}
