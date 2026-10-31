//! Whether the machine runs on its battery: the Helper pauses heavy work (downloads, imports,
//! merging Duplicates) until it is plugged in again.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Whether the machine runs on its battery now. Asked at most every half minute.
pub fn on_battery() -> bool {
    static LAST: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, known)) = *last
        && at.elapsed() < Duration::from_secs(30)
    {
        return known;
    }
    let now = battery_now();
    *last = Some((Instant::now(), now));
    now
}

#[cfg(target_os = "linux")]
fn battery_now() -> bool {
    on_battery_in(Path::new("/sys/class/power_supply"))
}

/// Linux's power supplies under `root` (`/sys/class/power_supply`): on the battery when there
/// is one and no mains adapter is online; without an adapter listed, when a battery discharges.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn on_battery_in(root: &Path) -> bool {
    let read = |p: &Path, f: &str| std::fs::read_to_string(p.join(f)).map(|s| s.trim().to_string()).unwrap_or_default();
    let supplies: Vec<_> = std::fs::read_dir(root).into_iter().flatten().flatten().map(|e| e.path()).collect();
    let mains: Vec<_> = supplies.iter().filter(|p| read(p, "type") == "Mains").collect();
    let battery = supplies.iter().any(|p| read(p, "type") == "Battery");
    if !mains.is_empty() {
        return battery && !mains.iter().any(|p| read(p, "online") == "1");
    }
    supplies.iter().any(|p| read(p, "type") == "Battery" && read(p, "status") == "Discharging")
}

#[cfg(target_os = "macos")]
fn battery_now() -> bool {
    bragd_engines::tools::command("/usr/bin/pmset")
        .args(["-g", "batt"])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("'Battery Power'"))
}

#[cfg(windows)]
fn battery_now() -> bool {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    // Safety: a plain struct the call fills in.
    let mut s: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    // ACLineStatus: 0 offline, 1 online, 255 unknown.
    unsafe { GetSystemPowerStatus(&mut s) != 0 && s.ACLineStatus == 0 }
}

// shortcut: the BSDs and illumos count as on mains; port their readers (sysctl, envstat, kstat) when Bragd ships there.
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn battery_now() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supply(root: &Path, name: &str, files: &[(&str, &str)]) {
        let d = root.join(name);
        std::fs::create_dir_all(&d).unwrap();
        for (f, v) in files {
            std::fs::write(d.join(f), format!("{v}\n")).unwrap();
        }
    }

    #[test]
    fn linux_power_supplies() {
        let d = crate::tests::Dir::new("power");
        assert!(!on_battery_in(&d.0), "a desktop has no battery");
        supply(&d.0, "BAT0", &[("type", "Battery"), ("status", "Discharging")]);
        assert!(on_battery_in(&d.0), "no adapter listed, discharging");
        supply(&d.0, "AC", &[("type", "Mains"), ("online", "1")]);
        assert!(!on_battery_in(&d.0));
        supply(&d.0, "AC", &[("type", "Mains"), ("online", "0")]);
        assert!(on_battery_in(&d.0));
    }
}
