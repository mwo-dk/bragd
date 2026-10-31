//! Disk watch: everything Bragd keeps on disk, with its size, held against the limits the user
//! sets in `[disk]` of config.toml (`bragd_core::config::DiskConfig`): each Library, each
//! Source's downloads, the Helper's own files, the tablebases, the engine images Bragd pulled
//! and the caches. The Helper checks on a schedule and after each job (`Jobs::check_disk`);
//! a limit crossed gives a notice, and, for the Sources the user allowed it, an action of its
//! own (`sources::Auto`). A tablebase folder outside Bragd's data folder is shown, and counts
//! against nothing: Disk watch never deletes what it did not make.

use std::path::{Path, PathBuf};

use bragd_core::config::DiskConfig;
use serde::{Deserialize, Serialize};

/// What an item is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Library,
    Downloads,
    Helper,
    Tablebases,
    Image,
    Cache,
}

/// One thing Bragd keeps on disk.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// Its id, which `[disk.items]` sets a limit by: `library:main.bragd`, `downloads:twic`,
    /// `helper`, `tablebases`, `image:<name>`, `cache`.
    pub id: String,
    pub kind: Kind,
    /// The Library's file, the Source's id, the image: what `label` names.
    pub name: String,
    /// Where it is; none for an image, which the container runtime keeps.
    pub path: Option<PathBuf>,
    pub bytes: u64,
    /// Its limit; none when unset.
    pub limit: Option<u64>,
    /// Outside Bragd's data folder, chosen by the user: shown, never deleted, not in the total.
    pub external: bool,
}

impl Item {
    /// Its name as shown, in the language of the texts.
    pub fn label(&self) -> String {
        let key = format!("disk.item.{}", serde_json::to_value(self.kind).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default());
        bragd_core::i18n::tr(&key, &[("name", &self.name)])
    }
}

/// A limit crossed: a notice, by its text key and what fills it in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Crossed {
    /// The same for as long as the same limit stays crossed, so a dismissed notice stays so.
    pub id: String,
    /// The item it is about; none for the total and the free space.
    pub item: Option<String>,
    pub key: String,
    pub args: Vec<(String, String)>,
}

impl Crossed {
    /// The notice's text, in the language of the texts.
    pub fn text(&self) -> String {
        let args: Vec<(&str, &str)> = self.args.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        bragd_core::i18n::tr(&self.key, &args)
    }
}

/// The free space of the disk under a folder Bragd keeps things in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Free {
    pub path: PathBuf,
    /// None where the system does not say.
    pub bytes: Option<u64>,
}

/// Everything Bragd keeps on disk, and the limits crossed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub items: Vec<Item>,
    /// What the items not `external` take together.
    pub total: u64,
    pub limits: DiskConfig,
    pub free: Vec<Free>,
    pub crossed: Vec<Crossed>,
}

/// Where Bragd keeps things.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Places {
    /// Bragd's own data folder (`<data>/bragd`).
    pub data: PathBuf,
    /// The folder of the Libraries.
    pub libraries: PathBuf,
    /// The Helper's folder; its Sources' downloads are in `sources/` there.
    pub helper: PathBuf,
    /// Each Source's id and the folder of its downloads.
    pub sources: Vec<(String, PathBuf)>,
    pub tablebases: PathBuf,
    pub cache: PathBuf,
}

/// Bytes in `path`, a file or a folder and all in it; links are not followed.
pub fn size(path: &Path) -> u64 {
    let Ok(m) = std::fs::symlink_metadata(path) else { return 0 };
    if !m.is_dir() {
        return m.len();
    }
    std::fs::read_dir(path).into_iter().flatten().flatten().map(|e| size(&e.path())).sum()
}

/// `n` bytes as shown: `1.5 GB`, in powers of 1000 as disks are sold.
pub fn bytes(n: u64) -> String {
    let units = ["B", "kB", "MB", "GB", "TB"];
    let (mut v, mut u) = (n as f64, 0);
    while v >= 1000.0 && u + 1 < units.len() {
        v /= 1000.0;
        u += 1;
    }
    if u == 0 { format!("{n} B") } else { format!("{v:.1} {}", units[u]) }
}

/// The report for `places`, against `limits`; `images` are the engine images Bragd pulled, with
/// their sizes; `free` tells the free space under a folder.
pub fn report(places: &Places, limits: &DiskConfig, images: &[(String, u64)], free: &dyn Fn(&Path) -> Option<u64>) -> Report {
    let limit = |id: &str| limits.items.get(id).copied().filter(|&l| l > 0);
    let mut items = vec![];
    let mut item = |id: String, kind, name: String, path: Option<PathBuf>, bytes, external| {
        items.push(Item { limit: limit(&id), id, kind, name, path, bytes, external });
    };
    let mut libraries: Vec<PathBuf> = std::fs::read_dir(&places.libraries).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "bragd")).collect();
    libraries.sort();
    for lib in libraries {
        let name = lib.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let with = |end: &str| size(&PathBuf::from(format!("{}{end}", lib.display())));
        item(format!("library:{name}"), Kind::Library, name, Some(lib.clone()), with("") + with("-wal") + with("-shm"), false);
    }
    let mut downloads = 0;
    for (id, dir) in &places.sources {
        let n = size(dir);
        downloads += n;
        if n > 0 {
            item(format!("downloads:{id}"), Kind::Downloads, id.clone(), Some(dir.clone()), n, false);
        }
    }
    item("helper".into(), Kind::Helper, String::new(), Some(places.helper.clone()), size(&places.helper).saturating_sub(downloads), false);
    let external = !places.tablebases.starts_with(&places.data);
    item("tablebases".into(), Kind::Tablebases, String::new(), Some(places.tablebases.clone()), size(&places.tablebases), external);
    for (name, n) in images {
        item(format!("image:{name}"), Kind::Image, name.clone(), None, *n, false);
    }
    item("cache".into(), Kind::Cache, String::new(), Some(places.cache.clone()), size(&places.cache), false);

    let total = items.iter().filter(|i| !i.external).map(|i| i.bytes).sum();
    let mut crossed = vec![];
    for i in items.iter().filter(|i| i.limit.is_some_and(|l| i.bytes > l)) {
        let args = [("item", i.label()), ("size", bytes(i.bytes)), ("limit", bytes(i.limit.unwrap_or(0)))];
        crossed.push(Crossed { id: format!("disk:item:{}", i.id), item: Some(i.id.clone()), key: "notice.disk_item_over".into(), args: args.map(|(k, v)| (k.into(), v)).to_vec() });
    }
    if limits.total > 0 && total > limits.total {
        let args = [("size", bytes(total)), ("limit", bytes(limits.total))];
        crossed.push(Crossed { id: "disk:total".into(), item: None, key: "notice.disk_total_over".into(), args: args.map(|(k, v)| (k.into(), v)).to_vec() });
    }
    let mut folders = vec![places.data.clone()];
    if !places.libraries.starts_with(&places.data) {
        folders.push(places.libraries.clone());
    }
    let free: Vec<Free> = folders.into_iter().map(|path| Free { bytes: free(&path), path }).collect();
    for f in free.iter().filter(|f| limits.free_below > 0 && f.bytes.is_some_and(|b| b < limits.free_below)) {
        let args = [("free", bytes(f.bytes.unwrap_or(0))), ("path", f.path.display().to_string()), ("limit", bytes(limits.free_below))];
        crossed.push(Crossed { id: format!("disk:free:{}", f.path.display()), item: None, key: "notice.disk_free_low".into(), args: args.map(|(k, v)| (k.into(), v)).to_vec() });
    }
    Report { items, total, limits: limits.clone(), free, crossed }
}

/// The engine images Bragd pulled, with their sizes, from the container runtime `choice`
/// (`[engines] runtime`) names; none without a runtime.
pub fn pulled_images(choice: &str) -> Vec<(String, u64)> {
    let Some(rt) = bragd_engines::tools::container_runtime(choice) else { return vec![] };
    bragd_engines::known::images().into_iter().filter_map(|i| Some((bragd_engines::images::short(&i).to_string(), bragd_engines::images::image_size(&rt.path, &i)?))).collect()
}

/// Bytes free for this user on the disk that holds `path` (or the nearest folder above it
/// that is there); none where the system does not say.
pub fn free_space(path: &Path) -> Option<u64> {
    free_at(path.ancestors().find(|p| p.exists())?)
}

#[cfg(unix)]
// The field types differ between systems: a cast that is a no-op on one is needed on another.
#[allow(clippy::unnecessary_cast)]
fn free_at(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    // Safety: a plain struct the call fills in.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // Safety: a NUL-ended path and that struct.
    (unsafe { libc::statvfs(c.as_ptr(), &mut s) } == 0).then(|| s.f_bavail as u64 * s.f_frsize as u64)
}

#[cfg(windows)]
fn free_at(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let mut free = 0u64;
    // Safety: a NUL-ended wide path and a number the call fills in; the totals may be null.
    let ok = unsafe { windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut()) };
    (ok != 0).then_some(free)
}

#[cfg(not(any(unix, windows)))]
fn free_at(_: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn places(root: &Path) -> Places {
        let data = root.join("data/bragd");
        Places {
            libraries: data.join("libraries"),
            helper: data.join("helper"),
            sources: vec![("twic".into(), data.join("helper/sources/twic")), ("fics".into(), data.join("helper/sources/fics"))],
            tablebases: root.join("syzygy"),
            cache: root.join("cache/bragd"),
            data,
        }
    }

    pub fn fill(p: &Places) {
        let put = |path: PathBuf, n: usize| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, vec![0u8; n]).unwrap();
        };
        put(p.libraries.join("main.bragd"), 5000);
        put(p.libraries.join("main.bragd-wal"), 1000);
        put(p.libraries.join("notes.txt"), 7);
        put(p.helper.join("state.json"), 100);
        put(p.helper.join("sources/twic/twic1600g.zip"), 3000);
        put(p.tablebases.join("KQvK.rtbw"), 9000);
        put(p.cache.join("x"), 10);
    }

    #[test]
    fn the_report_lists_what_bragd_keeps_and_the_limits_crossed() {
        let d = crate::tests::Dir::new("disk");
        let p = places(&d.0);
        fill(&p);
        let images = [("bragd-stockfish:17.1".to_string(), 200_000u64)];
        let r = report(&p, &DiskConfig::default(), &images, &|_| Some(1 << 40));
        let got: Vec<(&str, u64, bool)> = r.items.iter().map(|i| (i.id.as_str(), i.bytes, i.external)).collect();
        assert_eq!(
            got,
            [("library:main.bragd", 6000, false), ("downloads:twic", 3000, false), ("helper", 100, false), ("tablebases", 9000, true), ("image:bragd-stockfish:17.1", 200_000, false), ("cache", 10, false)]
        );
        assert_eq!(r.total, 6000 + 3000 + 100 + 200_000 + 10, "the folder the user chose counts against nothing");
        assert!(r.crossed.is_empty(), "no limits, no notices");
        assert_eq!(r.items[1].label(), "Downloads of twic");
        let limits = DiskConfig { total: 200_000, free_below: 2 << 40, items: [("library:main.bragd".to_string(), 4000), ("downloads:twic".to_string(), 3000)].into() };
        let r = report(&p, &limits, &images, &|_| Some(1 << 40));
        let ids: Vec<&str> = r.crossed.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["disk:item:library:main.bragd", "disk:total", &format!("disk:free:{}", p.data.display())]);
        assert_eq!(r.crossed[0].text(), "Library main.bragd takes 6.0 kB, over your limit of 4.0 kB.");
        assert!(r.crossed[2].text().starts_with("Only 1.1 TB is free"), "{}", r.crossed[2].text());
        // Tablebases inside Bragd's data folder are Bragd's own.
        let inside = Places { tablebases: p.data.join("tablebases"), ..p.clone() };
        assert!(!report(&inside, &limits, &[], &|_| None).items.iter().any(|i| i.external));
    }

    #[test]
    fn free_space_and_sizes() {
        let d = crate::tests::Dir::new("free");
        assert!(free_space(&d.0.join("not/there/yet")).is_some_and(|b| b > 0), "the nearest folder that is there");
        assert_eq!(bytes(999), "999 B");
        assert_eq!(bytes(1_500_000_000), "1.5 GB");
        std::fs::write(d.0.join("a"), [0u8; 10]).unwrap();
        assert_eq!(size(&d.0), 10);
    }
}
