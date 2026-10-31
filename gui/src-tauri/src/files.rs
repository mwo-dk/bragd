//! Every file dialog Bragd shows goes through `pick`: Coxswain's picker (`coxswain-gui --pick`)
//! when Settings → Look → File picker chooses it, or chooses Automatic and a `coxswain-gui`
//! that knows `--pick` is on PATH; otherwise the system's own dialog.
//!
//! Coxswain's picker prints each chosen path on a line of its own and exits 0, or exits 1 when
//! the user cancels.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;

use bragd_core::config::Config;
use bragd_engines::tools;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

/// The program Coxswain's picker is.
pub const COXSWAIN: &str = "coxswain-gui";

/// What a dialog asks for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ask {
    pub title: String,
    /// A folder, not a file.
    pub folder: bool,
    /// More than one may be chosen.
    pub multiple: bool,
    /// A file to save, with this name offered.
    pub save: Option<String>,
    /// Each filter's label and its extensions, without the dot.
    pub filters: Vec<(String, Vec<String>)>,
    /// The folder it opens in.
    pub start: Option<PathBuf>,
}

/// Which picker a dialog uses.
#[derive(Clone, Debug, PartialEq)]
pub enum Picker {
    Coxswain(PathBuf),
    System,
}

/// The picker the setting `choice` (`auto`, `coxswain` or `system`) gives, `found` being a
/// `coxswain-gui` that knows `--pick`. Coxswain chosen but not found falls back to the system's.
pub fn which(choice: &str, found: Option<PathBuf>) -> Picker {
    match (choice, found) {
        ("system", _) | (_, None) => Picker::System,
        (_, Some(p)) => Picker::Coxswain(p),
    }
}

/// A `coxswain-gui` on PATH whose `--help` names `--pick`; looked for once.
pub fn coxswain() -> Option<PathBuf> {
    static FOUND: OnceLock<Option<PathBuf>> = OnceLock::new();
    FOUND.get_or_init(|| tools::find(COXSWAIN).filter(|p| knows_pick(p))).clone()
}

/// Whether the program at `p` says it knows `--pick` in its `--help`.
pub fn knows_pick(p: &Path) -> bool {
    let mut c = tools::command(p);
    c.arg("--help");
    tools::capture(c, std::time::Duration::from_secs(10), 256 * 1024).is_ok_and(|o| {
        let said = [o.stdout, o.stderr].concat();
        String::from_utf8_lossy(&said).contains("--pick")
    })
}

/// Coxswain's picker's arguments for `ask`.
pub fn args(ask: &Ask) -> Vec<String> {
    let mut a = vec!["--pick".to_string()];
    if ask.folder {
        a.push("--folder".into());
    }
    if ask.multiple {
        a.push("--multiple".into());
    }
    if let Some(name) = &ask.save {
        a.extend(["--save".into(), name.clone()]);
    }
    for (label, exts) in &ask.filters {
        // Its own separators are taken out of a label, so the filter reads as one.
        let label: String = label.chars().map(|c| if matches!(c, ':' | ',') { ' ' } else { c }).collect();
        a.extend(["--filter".into(), format!("{label}:{}", exts.join(","))]);
    }
    if let Some(dir) = &ask.start {
        a.extend(["--start".into(), dir.display().to_string()]);
    }
    if !ask.title.is_empty() {
        a.extend(["--title".into(), ask.title.clone()]);
    }
    a
}

/// Runs Coxswain's picker at `program`: the paths chosen, none when the user cancelled.
pub fn run_coxswain(program: &Path, ask: &Ask) -> Result<Vec<PathBuf>, String> {
    let out = tools::command(program).args(args(ask)).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).output().map_err(|e| format!("{}: {e}", program.display()))?;
    match out.status.code() {
        Some(0) => Ok(String::from_utf8_lossy(&out.stdout).lines().map(str::trim_end).filter(|l| !l.is_empty()).map(PathBuf::from).collect()),
        Some(1) => Ok(vec![]),
        _ => {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(format!("{}: {}", program.display(), err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("it failed")))
        }
    }
}

/// The system's own dialog for `ask`.
fn system(app: &AppHandle, ask: &Ask) -> Result<Vec<PathBuf>, String> {
    let mut d = app.dialog().file();
    if !ask.title.is_empty() {
        d = d.set_title(&ask.title);
    }
    for (label, exts) in &ask.filters {
        let exts: Vec<&str> = exts.iter().map(String::as_str).collect();
        d = d.add_filter(label, &exts);
    }
    if let Some(dir) = &ask.start {
        d = d.set_directory(dir);
    }
    let picked = match (&ask.save, ask.folder, ask.multiple) {
        (Some(name), _, _) => d.set_file_name(name).blocking_save_file().into_iter().collect(),
        (None, true, true) => d.blocking_pick_folders().unwrap_or_default(),
        (None, true, false) => d.blocking_pick_folder().into_iter().collect(),
        (None, false, true) => d.blocking_pick_files().unwrap_or_default(),
        (None, false, false) => d.blocking_pick_file().into_iter().collect(),
    };
    picked.into_iter().map(|f| f.into_path().map_err(|e| e.to_string())).collect()
}

/// Shows a dialog for `ask` with the picker Settings chooses: the paths chosen, none when the
/// user cancelled. A Coxswain that cannot be started gives way to the system's dialog.
pub fn pick(app: &AppHandle, cfg: &Config, ask: &Ask) -> Result<Vec<PathBuf>, String> {
    let found = if cfg.look.file_picker == "system" { None } else { coxswain() };
    match which(&cfg.look.file_picker, found) {
        Picker::Coxswain(p) => match run_coxswain(&p, ask) {
            Err(e) if !p.is_file() => {
                eprintln!("{e}; using the system's file dialog");
                system(app, ask)
            }
            r => r,
        },
        Picker::System => system(app, ask),
    }
}

/// A path for a field of the page: an engine's program, an opening book or the tablebase
/// folder, `what` says which. Only the path is handed back; nothing is opened.
#[tauri::command(async)]
pub fn pick_path(what: String, app: AppHandle, ctx: tauri::State<'_, crate::Ctx>) -> Result<Option<String>, String> {
    let tr = |k: &str| bragd_core::i18n::tr(k, &[]);
    let ask = match what.as_str() {
        "engine" => Ask { title: tr("files.pick_engine"), ..Default::default() },
        "book" => Ask { title: tr("files.pick_book"), filters: vec![(tr("files.books"), vec!["bin".into()])], ..Default::default() },
        "tablebases" => Ask { title: tr("files.pick_tablebases"), folder: true, ..Default::default() },
        _ => return Err(format!("nothing to pick for {what}")),
    };
    Ok(pick_one(&app, &ctx.cfg(), &ask)?.map(|p| p.display().to_string()))
}

/// `pick` for one path.
pub fn pick_one(app: &AppHandle, cfg: &Config, ask: &Ask) -> Result<Option<PathBuf>, String> {
    Ok(pick(app, cfg, ask)?.into_iter().next())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setting_chooses_the_picker() {
        let p = PathBuf::from("/usr/bin/coxswain-gui");
        assert_eq!(which("auto", Some(p.clone())), Picker::Coxswain(p.clone()));
        assert_eq!(which("coxswain", Some(p.clone())), Picker::Coxswain(p.clone()));
        assert_eq!(which("system", Some(p)), Picker::System);
        assert_eq!(which("auto", None), Picker::System);
        assert_eq!(which("coxswain", None), Picker::System, "not found: the system's");
    }

    #[test]
    fn coxswains_arguments() {
        let ask = Ask { title: "Open a database".into(), multiple: true, filters: vec![("Game files: PGN, SGF".into(), vec!["pgn".into(), "sgf".into()])], start: Some("/home/me/Games".into()), ..Default::default() };
        assert_eq!(args(&ask), ["--pick", "--multiple", "--filter", "Game files  PGN  SGF:pgn,sgf", "--start", "/home/me/Games", "--title", "Open a database"]);
        let save = Ask { save: Some("games.pgn".into()), ..Default::default() };
        assert_eq!(args(&save), ["--pick", "--save", "games.pgn"]);
        assert_eq!(args(&Ask { folder: true, ..Default::default() }), ["--pick", "--folder"]);
    }

    /// Writes the script `text` to `p` from a child process of its own, so no file handle of
    /// it leaks into a program another test starts meanwhile ("Text file busy").
    #[cfg(unix)]
    fn script(p: &Path, text: &str) {
        use std::io::Write;
        let mut sh = tools::own("/bin/sh").arg("-c").arg("cat > \"$0\" && chmod 755 \"$0\"").arg(p).stdin(Stdio::piped()).spawn().unwrap();
        sh.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
        assert!(sh.wait().unwrap().success());
    }

    /// A stand-in picker: it prints its arguments to a file, then answers as `answer` says.
    #[cfg(unix)]
    fn fake(d: &Path, name: &str, answer: &str) -> PathBuf {
        let p = d.join(name);
        script(&p, &format!("#!/bin/sh\nif [ \"$1\" = --help ]; then echo 'usage: coxswain-gui [--pick [--folder] [--multiple]]'; exit 0; fi\nprintf '%s\\n' \"$@\" > '{}'\n{answer}\n", d.join("args").display()));
        p
    }

    #[cfg(unix)]
    #[test]
    fn coxswains_picker_answers() {
        let d = std::env::temp_dir().join(format!("bragd-picker-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let ask = Ask { title: "Pick".into(), multiple: true, ..Default::default() };
        // Two paths chosen, one with a space.
        let p = fake(&d, "chose", "printf '/a/one.pgn\\n/b/two games.pgn\\n'; exit 0");
        assert!(knows_pick(&p));
        assert_eq!(run_coxswain(&p, &ask).unwrap(), [PathBuf::from("/a/one.pgn"), PathBuf::from("/b/two games.pgn")]);
        assert_eq!(std::fs::read_to_string(d.join("args")).unwrap(), "--pick\n--multiple\n--title\nPick\n");
        // Cancelled.
        let p = fake(&d, "cancelled", "exit 1");
        assert_eq!(run_coxswain(&p, &ask).unwrap(), Vec::<PathBuf>::new());
        // Failed: its last words.
        let p = fake(&d, "failed", "echo 'no display' >&2; exit 3");
        assert!(run_coxswain(&p, &ask).unwrap_err().ends_with("no display"));
        // An older Coxswain without --pick is not used.
        let old = d.join("old");
        script(&old, "#!/bin/sh\necho 'usage: coxswain-gui [FILE]'\n");
        assert!(!knows_pick(&old));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
