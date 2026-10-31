fn main() {
    // The Rules pages are compiled in (`rules.rs`): a changed page rebuilds the app.
    println!("cargo:rerun-if-changed=../../docs/rules");
    tauri_build::build()
}
