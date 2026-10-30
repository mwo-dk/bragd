//! Makes a Library from game files, for the screenshots' demo home (docs/screenshots/demo-home.sh):
//!
//!     cargo run -p bragd-library --example demo_library -- <library.bragd> <source> <file>...
//!
//! Each file is imported as coming from `<source>`, and a Slice "White wins" is kept. No network,
//! nothing else touched.

use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [out, source, files @ ..] = args.as_slice() else {
        return Err("usage: demo_library <library.bragd> <source> <file>...".into());
    };
    if let Some(dir) = Path::new(out).parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut lib = bragd_library::library::Library::open(Path::new(out))?;
    for f in files {
        let p = lib.import(Path::new(f), source, &|| false, &mut |_| {})?;
        println!("{f}: {} read, {} added, {} merged", p.read, p.added, p.merged);
    }
    // A Slice to show in the sidebar.
    lib.save_slice("White wins", &bragd_library::slice::Query { result: Some("1-0".into()), ..Default::default() })?;
    println!("{out}: {} Games", lib.count()?);
    Ok(())
}
