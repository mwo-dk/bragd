//! Evaluation networks Bragd's engine images leave out: Pikafish's network may not be used
//! commercially, so Bragd does not ship it, and Lc0's networks are large, many, and published
//! apart from the program under terms of their own. It is downloaded only
//! once the user has read its licence and agreed, from the engine's own release, checked by
//! its digest, into Bragd's data folder; an image of the engine is then given it read-only. An
//! installed engine brings its own and is not affected.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::fetch::Fetch;

/// A network an engine needs, and where it comes from.
#[derive(Debug, PartialEq)]
pub struct Network {
    /// The engine it is for (`known::Known::key`).
    pub engine: &'static str,
    /// The file, as the engine names it.
    pub file: &'static str,
    /// What is downloaded, its size and SHA-256: the engine's release archive (7z) the file is
    /// in, or, when not `packed`, the file itself.
    pub archive: &'static str,
    pub archive_size: u64,
    pub archive_sha256: &'static str,
    /// The file's own size and SHA-256.
    pub size: u64,
    pub sha256: &'static str,
    /// The download is a 7z archive the file is taken out of.
    pub packed: bool,
    /// The engine option that names the file.
    pub option: &'static str,
    /// The licence the user agrees to, as the release ships it.
    pub licence: &'static str,
}

/// Pikafish's network, from the release Bragd's image is built from (`packaging/images/pikafish`).
pub const PIKAFISH: Network = Network {
    engine: "pikafish",
    file: "pikafish.nnue",
    archive: "https://github.com/official-pikafish/Pikafish/releases/download/Pikafish-2026-09-06/Pikafish.2026-09-06.7z",
    archive_size: 53_603_424,
    archive_sha256: "41952bbfe2520faceb5902c69e6ab4845cc999841d2b49a95cc1be7867a25e5b",
    size: 50_706_378,
    sha256: "7d13d73569a9b571ba0eb20cf1596247bc2a42738967e61afef6482b231e900e",
    packed: true,
    option: "EvalFile",
    licence: include_str!("../licences/pikafish-nnue.md"),
};

/// Lc0's network 791556 (192x15), the one Lc0's own CPU releases bundle: one gzip file, from
/// the project's network storage, named by the SHA-256 of what it unpacks to.
pub const LC0: Network = Network {
    engine: "lc0",
    file: "791556.pb.gz",
    archive: "https://storage.lczero.org/files/networks/f404e156ceb2882470fd8c032b8754af0fa0b71168328912eaef14671a256e34",
    archive_size: 18_648_209,
    archive_sha256: "b1c7047582a8ad37620849bf328935cb46f2ccbbecb7f2696c30c6b913ed690d",
    size: 18_648_209,
    sha256: "b1c7047582a8ad37620849bf328935cb46f2ccbbecb7f2696c30c6b913ed690d",
    packed: false,
    option: "WeightsFile",
    licence: include_str!("../licences/lc0-network.md"),
};

/// Every network Bragd downloads.
pub const NETWORKS: &[Network] = &[PIKAFISH, LC0];

/// The network the engine `key` needs, if any.
pub fn for_engine(key: &str) -> Option<&'static Network> {
    NETWORKS.iter().find(|n| n.engine == key)
}

/// Where a container sees its network.
pub const CONTAINER_NETWORK: &str = "/network.nnue";

/// A writer that hashes what it writes.
struct Hashing<W: Write> {
    out: W,
    hash: Sha256,
}

impl<W: Write> Write for Hashing<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.out.write(buf)?;
        self.hash.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

fn hex(digest: &[u8]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

impl Network {
    /// Where it is kept, under `dir` (Bragd's data folder's `networks`).
    pub fn path(&self, dir: &Path) -> PathBuf {
        dir.join(self.engine).join(self.file)
    }

    /// Downloads the release archive into `dir`, checks it, takes the network out of it, checks
    /// that, and keeps only the network; a network that is not `packed` is downloaded and checked
    /// as it is. `progress` hears the bytes so far of the archive's;
    /// false stops. Nothing is left behind but a whole, checked network.
    pub fn download(&self, dir: &Path, net: &dyn Fetch, progress: &mut dyn FnMut(u64, u64) -> bool) -> Result<PathBuf, String> {
        let folder = dir.join(self.engine);
        fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        let (archive, part, to) = (folder.join(".release.7z.part"), folder.join(format!(".{}.part", self.file)), self.path(dir));
        let got = if self.packed { self.fetch_and_take(net, &archive, &part, progress) } else { self.fetch(net, &part, progress) };
        let _ = fs::remove_file(&archive);
        match got {
            Ok(()) => fs::rename(&part, &to).map_err(|e| format!("{}: {e}", to.display()))?,
            Err(e) => {
                let _ = fs::remove_file(&part);
                return Err(e);
            }
        }
        Ok(to)
    }

    /// Downloads what `archive` names into `to` and checks it by its digest.
    fn fetch(&self, net: &dyn Fetch, to: &Path, progress: &mut dyn FnMut(u64, u64) -> bool) -> Result<(), String> {
        let mut w = Hashing { out: File::create(to).map_err(|e| e.to_string())?, hash: Sha256::new() };
        let total = self.archive_size;
        // Never more than its size: a server that keeps sending is cut off.
        net.download(self.archive, &mut w, &mut |done| done <= total && progress(done, total)).map_err(|e| format!("{}: {e}", self.archive))?;
        w.out.sync_all().map_err(|e| e.to_string())?;
        if hex(&w.hash.finalize()) != self.archive_sha256 {
            return Err(format!("{} is not the release Bragd knows: its digest differs", self.archive));
        }
        Ok(())
    }

    fn fetch_and_take(&self, net: &dyn Fetch, archive: &Path, part: &Path, progress: &mut dyn FnMut(u64, u64) -> bool) -> Result<(), String> {
        let mut w = Hashing { out: File::create(archive).map_err(|e| e.to_string())?, hash: Sha256::new() };
        let total = self.archive_size;
        // Never more than the release's size: a server that keeps sending is cut off.
        net.download(self.archive, &mut w, &mut |done| done <= total && progress(done, total)).map_err(|e| format!("{}: {e}", self.archive))?;
        w.out.sync_all().map_err(|e| e.to_string())?;
        if hex(&w.hash.finalize()) != self.archive_sha256 {
            return Err(format!("{} is not the release Bragd knows: its digest differs", self.archive));
        }
        let mut reader = sevenz_rust2::ArchiveReader::open(archive, sevenz_rust2::Password::empty()).map_err(|e| e.to_string())?;
        let mut out = Hashing { out: File::create(part).map_err(|e| e.to_string())?, hash: Sha256::new() };
        let mut found = false;
        reader
            .for_each_entries(|entry, r| {
                if entry.name() != self.file {
                    // Read past it: the archive is solid, so what comes before has to be unpacked.
                    io::copy(r, &mut io::sink())?;
                    return Ok(true);
                }
                io::copy(&mut r.take(self.size + 1), &mut out)?;
                found = true;
                Ok(false)
            })
            .map_err(|e| e.to_string())?;
        out.out.sync_all().map_err(|e| e.to_string())?;
        if !found || hex(&out.hash.finalize()) != self.sha256 {
            return Err(format!("{} in {} is not the network Bragd knows", self.file, self.archive));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A release served from memory: `body` for its URL, counting what was asked.
    struct Release {
        url: &'static str,
        body: Vec<u8>,
    }

    impl Fetch for Release {
        fn get(&self, _: &str, _: u64) -> io::Result<Vec<u8>> {
            unreachable!("a network comes by download")
        }

        fn download(&self, url: &str, to: &mut dyn Write, progress: &mut dyn FnMut(u64) -> bool) -> io::Result<u64> {
            if url != self.url {
                return Err(io::Error::other("404"));
            }
            for (i, piece) in self.body.chunks(7).enumerate() {
                to.write_all(piece)?;
                if !progress(((i + 1) * 7).min(self.body.len()) as u64) {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "stopped"));
                }
            }
            Ok(self.body.len() as u64)
        }
    }

    /// `tests/release.7z`: a solid LZMA2 7z of `Copying.txt`, `NNUE-License.md` and
    /// `pikafish.nnue` (the text `not a real network\n`), as Pikafish's release lays them out.
    const FIXTURE: &[u8] = include_bytes!("../tests/release.7z");

    /// A network that is one file, not packed: downloaded, checked, kept as it is.
    #[test]
    fn a_network_that_is_one_file_is_kept_as_it_comes() {
        let d = crate::test_dir("network-file");
        let body = b"gzip of weights".to_vec();
        let n = Network { archive: "https://example.org/net", archive_size: body.len() as u64, archive_sha256: Box::leak(sha(&body).into_boxed_str()), size: body.len() as u64, sha256: Box::leak(sha(&body).into_boxed_str()), ..LC0 };
        let at = n.download(&d, &Release { url: n.archive, body: body.clone() }, &mut |_, _| true).unwrap();
        assert_eq!(at, d.join("lc0/791556.pb.gz"));
        assert_eq!(fs::read(&at).unwrap(), body);
        let mut bad = body.clone();
        bad[0] ^= 1;
        fs::remove_file(&at).unwrap();
        assert!(n.download(&d, &Release { url: n.archive, body: bad }, &mut |_, _| true).unwrap_err().contains("digest differs"));
        assert_eq!(fs::read_dir(d.join("lc0")).unwrap().count(), 0, "nothing is left");
        assert_eq!(for_engine("lc0"), Some(&LC0));
        assert_eq!((LC0.size, LC0.sha256), (LC0.archive_size, LC0.archive_sha256), "one file: what comes is what is kept");
        fs::remove_dir_all(&d).unwrap();
    }

    fn sha(b: &[u8]) -> String {
        hex(&Sha256::digest(b))
    }

    fn fixture() -> Network {
        let nnue = b"not a real network\n";
        Network {
            archive: "https://example.org/release.7z",
            archive_size: FIXTURE.len() as u64,
            archive_sha256: Box::leak(sha(FIXTURE).into_boxed_str()),
            size: nnue.len() as u64,
            sha256: Box::leak(sha(nnue).into_boxed_str()),
            ..PIKAFISH
        }
    }

    #[test]
    fn the_network_is_taken_from_the_checked_release_and_nothing_else_is_kept() {
        let d = crate::test_dir("network");
        let n = fixture();
        let net = Release { url: n.archive, body: FIXTURE.to_vec() };
        let mut heard = vec![];
        let at = n.download(&d, &net, &mut |done, total| {
            heard.push((done, total));
            true
        }).unwrap();
        assert_eq!(at, d.join("pikafish/pikafish.nnue"));
        assert_eq!(fs::read(&at).unwrap(), b"not a real network\n");
        assert_eq!(heard.last(), Some(&(FIXTURE.len() as u64, FIXTURE.len() as u64)));
        let left: Vec<_> = fs::read_dir(d.join("pikafish")).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left, ["pikafish.nnue"], "the archive and the parts are gone");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_release_that_differs_is_refused_and_leaves_nothing() {
        let d = crate::test_dir("network-bad");
        let n = fixture();
        // One byte changed: the archive's digest differs.
        let mut body = FIXTURE.to_vec();
        let last = body.len() - 1;
        body[last] ^= 1;
        let e = n.download(&d, &Release { url: n.archive, body }, &mut |_, _| true).unwrap_err();
        assert!(e.contains("digest differs"), "{e}");
        // The right archive, but a network Bragd does not know.
        let other = Network { sha256: "00", ..fixture() };
        let e = other.download(&d, &Release { url: n.archive, body: FIXTURE.to_vec() }, &mut |_, _| true).unwrap_err();
        assert!(e.contains("not the network"), "{e}");
        // A server that sends more than the release is cut off.
        let short = Network { archive_size: 10, ..fixture() };
        assert!(short.download(&d, &Release { url: n.archive, body: FIXTURE.to_vec() }, &mut |_, _| true).is_err());
        // Stopped.
        assert!(n.download(&d, &Release { url: n.archive, body: FIXTURE.to_vec() }, &mut |_, _| false).is_err());
        assert_eq!(fs::read_dir(d.join("pikafish")).unwrap().count(), 0, "nothing is left");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn pikafish_needs_its_network_and_the_licence_is_the_releases() {
        assert_eq!(for_engine("pikafish"), Some(&PIKAFISH));
        assert_eq!(for_engine("stockfish"), None);
        assert!(PIKAFISH.licence.contains("No commercial use without permission"));
        assert!(PIKAFISH.archive.contains("Pikafish-2026-09-06"));
        assert_eq!(crate::known::known("pikafish").unwrap().tag, Some("2026-09-06"), "the network is the image's release's");
    }

    /// The real release, from a copy on disk named by `RELEASE_7Z`, gives the network Bragd
    /// knows. Ignored: it needs the 53 MB archive.
    #[test]
    #[ignore = "needs RELEASE_7Z, a copy of Pikafish's release archive"]
    fn the_real_release_gives_the_real_network() {
        let body = fs::read(std::env::var("RELEASE_7Z").unwrap()).unwrap();
        let d = crate::test_dir("network-real");
        let at = PIKAFISH.download(&d, &Release { url: PIKAFISH.archive, body }, &mut |_, _| true).unwrap();
        assert_eq!(fs::metadata(&at).unwrap().len(), PIKAFISH.size);
        fs::remove_dir_all(&d).unwrap();
    }
}
