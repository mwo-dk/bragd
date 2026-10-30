//! Writing the app's own files safely.

use std::fs;
use std::io;
use std::path::Path;

/// Seconds since 1970.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Writes `bytes` to `path` atomically: to a file beside it first, flushed to the disk, then
/// renamed over it, so a crash never leaves a half-written or empty file. `own`: readable by
/// the user alone (on Unix).
pub fn replace(path: &Path, bytes: &[u8], own: bool) -> io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    // Its own name, so two windows saving at once never write into one file beside it.
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_file_name(format!(
        ".{}.bragd-{}-{n}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    let done = (|| {
        fs::create_dir_all(dir)?;
        let mut f = fs::File::create(&tmp)?;
        #[cfg(unix)]
        if own {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        #[cfg(not(unix))]
        let _ = own;
        io::Write::write_all(&mut f, bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if done.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    done
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fs_replace_leaves_only_the_file() {
        let dir = crate::dirs::cache_dir().unwrap().join("fs-replace");
        let _ = fs::remove_dir_all(&dir);
        let p = dir.join("a.json");
        replace(&p, b"one", true).unwrap();
        replace(&p, b"two", true).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            1,
            "no temporary file left"
        );
    }
}
