//! The system's own word that its network changed, so a pull that failed for want of a network
//! goes again within seconds of the network coming back, not at the next look.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// How many times the system said its network changed (`watch`).
static CHANGES: AtomicU64 = AtomicU64::new(0);

/// How often a rest looks for a *Retry now* or a change.
const STEP: Duration = Duration::from_millis(250);

/// A new address comes before its route and name servers: a moment for them.
const SETTLE: Duration = Duration::from_secs(3);

/// Rests `d` in steps; true when `wake` says so (*Retry now*) or, with `net_down` true, when
/// the network changed in the meantime (after a moment to settle). `nap` does the waiting.
pub(crate) fn rest(d: Duration, wake: &dyn Fn() -> bool, net_down: &dyn Fn() -> bool, changes: &AtomicU64, nap: &dyn Fn(Duration)) -> bool {
    let (mut left, seen) = (d, changes.load(Ordering::SeqCst));
    while !left.is_zero() {
        if wake() {
            return true;
        }
        let step = STEP.min(left);
        nap(step);
        left -= step;
        if changes.load(Ordering::SeqCst) != seen && net_down() {
            nap(SETTLE);
            return true;
        }
    }
    false
}

/// `rest` on the real clock and the system's signal.
pub fn rest_for(d: Duration, wake: &dyn Fn() -> bool, net_down: &dyn Fn() -> bool) -> bool {
    rest(d, wake, net_down, &CHANGES, &std::thread::sleep)
}

/// Starts listening, once, in a thread of its own: an address or a route added (Linux: netlink;
/// macOS: the routing socket), the address table changed (Windows). Elsewhere (the BSDs,
/// illumos) nothing is heard and a pull waits out its time.
pub fn watch() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = std::thread::Builder::new().name("network-watch".into()).spawn(|| {
            let _ = listen(&|| {
                CHANGES.fetch_add(1, Ordering::SeqCst);
            });
        });
    });
}

/// Calls `changed` on every address or route the kernel adds, until the socket fails.
#[cfg(target_os = "linux")]
fn listen(changed: &dyn Fn()) -> std::io::Result<()> {
    // SAFETY: a socket of our own, a zeroed address with its family and groups set, and a
    // buffer as long as the length given.
    unsafe {
        let fd = libc::socket(libc::AF_NETLINK, libc::SOCK_RAW | libc::SOCK_CLOEXEC, libc::NETLINK_ROUTE);
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut sa: libc::sockaddr_nl = std::mem::zeroed();
        sa.nl_family = libc::AF_NETLINK as u16;
        sa.nl_groups = (libc::RTMGRP_IPV4_IFADDR | libc::RTMGRP_IPV6_IFADDR | libc::RTMGRP_IPV4_ROUTE | libc::RTMGRP_IPV6_ROUTE) as u32;
        if libc::bind(fd, (&raw const sa).cast(), std::mem::size_of::<libc::sockaddr_nl>() as u32) < 0 {
            let e = std::io::Error::last_os_error();
            libc::close(fd);
            return Err(e);
        }
        let mut buf = [0u8; 16384];
        loop {
            let n = libc::recv(fd, buf.as_mut_ptr().cast(), buf.len(), 0);
            if n < 0 {
                let e = std::io::Error::last_os_error();
                // Interrupted, or more came than the socket holds: something changed.
                match e.raw_os_error() {
                    Some(libc::EINTR) => continue,
                    Some(libc::ENOBUFS) => {
                        changed();
                        continue;
                    }
                    _ => {
                        libc::close(fd);
                        return Err(e);
                    }
                }
            }
            if added(&buf[..n as usize]) {
                changed();
            }
        }
    }
}

/// Whether the netlink messages in `b` add an address or a route (not only take one away).
#[cfg(any(target_os = "linux", test))]
fn added(mut b: &[u8]) -> bool {
    const NEWADDR: u16 = 20;
    const NEWROUTE: u16 = 24;
    while b.len() >= 16 {
        let len = u32::from_ne_bytes([b[0], b[1], b[2], b[3]]) as usize;
        if matches!(u16::from_ne_bytes([b[4], b[5]]), NEWADDR | NEWROUTE) {
            return true;
        }
        // Each message is padded to 4 bytes.
        let step = (len + 3) & !3;
        if step < 16 || step > b.len() {
            break;
        }
        b = &b[step..];
    }
    false
}

/// Calls `changed` on every address added or interface changed, until the socket fails.
#[cfg(target_os = "macos")]
fn listen(changed: &dyn Fn()) -> std::io::Result<()> {
    // SAFETY: a socket of our own and a buffer as long as the length given.
    unsafe {
        let fd = libc::socket(libc::PF_ROUTE, libc::SOCK_RAW, libc::AF_UNSPEC);
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut buf = [0u8; 8192];
        loop {
            let n = libc::read(fd, buf.as_mut_ptr().cast(), buf.len());
            if n < 0 {
                let e = std::io::Error::last_os_error();
                if e.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                libc::close(fd);
                return Err(e);
            }
            // rt_msghdr: its length (2 bytes), version, then type. Routes cloned for each new
            // connection are not looked at, so asking the registry wakes nothing.
            if n >= 4 && matches!(buf[3] as libc::c_int, libc::RTM_NEWADDR | libc::RTM_IFINFO) {
                changed();
            }
        }
    }
}

/// Calls `changed` every time the table of IP addresses changes, until that call fails.
#[cfg(windows)]
fn listen(changed: &dyn Fn()) -> std::io::Result<()> {
    loop {
        // SAFETY: no handle and no overlapped: the call waits until the table changes.
        let r = unsafe { windows_sys::Win32::NetworkManagement::IpHelper::NotifyAddrChange(std::ptr::null_mut(), std::ptr::null()) };
        if r != 0 {
            return Err(std::io::Error::from_raw_os_error(r as i32));
        }
        changed();
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn listen(_: &dyn Fn()) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Only netlink messages that add an address or a route say the network may be back.
    #[test]
    fn netlink_additions_are_told_apart() {
        let msg = |kind: u16| {
            let mut m = vec![0u8; 20];
            m[..4].copy_from_slice(&18u32.to_ne_bytes());
            m[4..6].copy_from_slice(&kind.to_ne_bytes());
            m
        };
        assert!(added(&msg(20)) && added(&msg(24)), "a new address, a new route");
        assert!(!added(&msg(21)) && !added(&msg(25)) && !added(&[]), "only taken away");
        assert!(added(&[msg(21), msg(24)].concat()), "a new route after a removed address");
    }

    /// A signal ends an hour's rest after three naps and a moment to settle; a signal while the
    /// network is not down, or none, rests the whole hour; Retry now ends it at once. Naps are
    /// counted, never slept.
    #[test]
    fn a_network_change_ends_the_rest_early() {
        let hour = Duration::from_secs(3600);
        // Rests with a signal on the third nap; how many naps, and the last one.
        let run = |wake: bool, down: bool, signal: bool| {
            let (changes, naps) = (AtomicU64::new(0), RefCell::new(vec![]));
            let nap = |d| {
                naps.borrow_mut().push(d);
                if signal && naps.borrow().len() == 3 {
                    changes.fetch_add(1, Ordering::SeqCst);
                }
            };
            let r = rest(hour, &|| wake, &|| down, &changes, &nap);
            (r, naps.borrow().len(), naps.borrow().last().copied())
        };
        assert_eq!(run(false, true, true), (true, 4, Some(SETTLE)), "three steps, the change seen after the third, a moment to settle");
        assert_eq!(run(false, false, true), (false, 14400, Some(STEP)), "a change while the network is not down is not a wake");
        assert_eq!(run(false, true, false), (false, 14400, Some(STEP)), "no change, the whole hour in quarter seconds");
        assert_eq!(run(true, true, true), (true, 0, None), "Retry now needs no rest");
    }
}
