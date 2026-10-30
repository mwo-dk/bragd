//! Engines for every Game family that has them: the protocols they speak (UCI, UCCI, USI, GTP,
//! Hub and GNU Backgammon's command line), finding them installed, running them as sealed
//! container images that tidy up after themselves, reviewing whole Games, and Syzygy
//! tablebases with the Lichess tablebase API as a fallback.
//!
//! Texts here are plain English for now; they go through the app's translations once the
//! commands that show them are wired up.

pub mod fetch;
pub mod images;
pub mod known;
pub mod lichess;
pub mod options;
pub mod protocol;
pub mod review;
pub mod session;
pub mod syzygy;
pub mod tools;

pub use protocol::{About, Best, Event, Info, Limit, Position, Protocol, Score, Start};
pub use session::{Analysis, Engine, Program, Session, think};

/// A fresh folder of its own for a test, under the system's temporary folder.
#[cfg(test)]
pub(crate) fn test_dir(name: &str) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let d = std::env::temp_dir().join(format!("bragd-engines-{name}-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A shell script standing in for an engine or a container runtime: never a real one.
#[cfg(all(test, unix))]
pub(crate) fn fake_program(dir: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join(name);
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}
