//! The Helper starts on a Windows without the Visual C++ Redistributable: a fresh Windows 11 has
//! no vcruntime140.dll, and a Helper that imports it never starts.

#[cfg(windows)]
#[test]
fn helper_does_not_import_the_visual_cpp_runtime() {
    let exe = std::fs::read(env!("CARGO_BIN_EXE_bragd-helper")).unwrap().to_ascii_lowercase();
    assert!(!exe.windows(b"vcruntime140".len()).any(|w| w == b"vcruntime140"), "bragd-helper.exe imports vcruntime140.dll");
}
