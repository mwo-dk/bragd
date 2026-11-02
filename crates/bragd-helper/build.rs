// The Visual C++ runtime goes into the Helper, as tauri-build puts it into the app, so the Helper
// starts on a Windows without the Visual C++ Redistributable. The Universal CRT ships with Windows.
fn main() {
    static_vcruntime::metabuild();
}
