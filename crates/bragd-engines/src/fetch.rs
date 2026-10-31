//! Fetching over HTTPS, behind a trait so the tablebase planner, the downloader and the Lichess
//! client are tested without the network.

use std::io::{Read, Write};
use std::sync::LazyLock;
use std::time::Duration;

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

/// The client every download uses: the system's certificate store, Bragd's user agent, and
/// timeouts, so a connection that dies mid-body (Wi-Fi gone, laptop asleep) fails instead of
/// blocking its thread for good. `status_as_error: false` hands 4xx/5xx back as replies.
pub fn agent(status_as_error: bool) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .timeout_per_read(Some(Duration::from_secs(120)))
        .http_status_as_error(status_as_error)
        .tls_config(tls)
        .user_agent(AGENT)
        .build()
        .into()
}

static CLIENT: LazyLock<ureq::Agent> = LazyLock::new(|| agent(true));

fn io(e: ureq::Error) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

impl Fetch for Http {
    fn get(&self, url: &str, max: u64) -> std::io::Result<Vec<u8>> {
        let mut r = CLIENT.get(url).call().map_err(io)?;
        r.body_mut().with_config().limit(max).read_to_vec().map_err(io)
    }

    fn download(&self, url: &str, to: &mut dyn Write, progress: &mut dyn FnMut(u64) -> bool) -> std::io::Result<u64> {
        let r = CLIENT.get(url).call().map_err(io)?;
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
