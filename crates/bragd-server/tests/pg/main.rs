//! The game service and the row-level security of `supabase/migrations/`, against plain
//! Postgres. One program, so the throwaway server is removed at the end whatever happened. The
//! game service runs on Linux only, so these run there.

#[cfg(target_os = "linux")]
mod support;
#[cfg(target_os = "linux")]
mod friends;
#[cfg(target_os = "linux")]
mod games;

/// A case: its name, and what it checks.
#[cfg(target_os = "linux")]
type Case = (&'static str, fn(&support::Pg));

#[cfg(target_os = "linux")]
fn main() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let filter = std::env::args().skip(1).find(|a| !a.starts_with('-'));
    let pg = support::Pg::start();
    let cases: Vec<Case> = [games::CASES, friends::CASES].concat();
    let mut failed = vec![];
    for (name, case) in cases {
        if filter.as_ref().is_some_and(|f| !name.contains(f.as_str())) {
            continue;
        }
        match catch_unwind(AssertUnwindSafe(|| case(&pg))) {
            Ok(()) => println!("test {name} ... ok"),
            Err(_) => {
                println!("test {name} ... FAILED");
                failed.push(name);
            }
        }
    }
    drop(pg);
    if !failed.is_empty() {
        eprintln!("failed: {failed:?}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {}
