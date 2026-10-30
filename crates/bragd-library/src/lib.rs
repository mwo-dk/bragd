//! Bragd's game databases: Foreign databases read where they lie (`chess`, `go`, `shogi`,
//! `xiangqi`, `draughts`), Libraries of Bragd's own in SQLite with Duplicates merged
//! (`library`), Slices over them (`slice`), and a Library's games exported to Parquet (`export`).
//! Big downloads are read as a stream, game by game (`stream`), and Strip rules keep only the
//! Games wanted from them, as they come in and once stored (`strip`).

// The readers spell out their tuple types, as written where they were copied from.
#![allow(clippy::type_complexity)]

pub mod chess;
pub mod draughts;
pub mod export;
pub mod go;
pub mod library;
pub mod shogi;
pub mod slice;
pub mod stream;
pub mod strip;
pub mod xiangqi;

/// A time limit for a test, only against a hang: ten times longer in a debug build.
#[cfg(test)]
pub(crate) fn test_limit(normal: std::time::Duration) -> std::time::Duration {
    if cfg!(debug_assertions) { (normal * 10).max(std::time::Duration::from_secs(120)) } else { normal * 2 }
}
