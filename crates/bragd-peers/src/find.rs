//! Where a Helper listens for Peers, and how they find it: mDNS on the home network
//! (`_bragd._tcp`). Only the home network's addresses are listened on, unless the user picks an
//! interface: private IPv4 ranges, link-local IPv4, the 100.64.0.0/10 range Tailscale uses,
//! and IPv6 unique local addresses. Never a public address by itself.

use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};

const SERVICE: &str = "_bragd._tcp.local.";

/// Whether `ip` belongs to a home or private network.
pub fn is_lan(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_private() || v4.is_link_local() || (a == 100 && (64..128).contains(&b))
        }
        IpAddr::V6(v6) => (v6.segments()[0] & 0xfe00) == 0xfc00,
    }
}

/// An interface the user can pick, with its addresses.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Interface {
    pub name: String,
    pub addrs: Vec<IpAddr>,
}

/// The machine's interfaces, loopback and IPv6 link-local addresses left out (a Peer cannot
/// use them).
pub fn interfaces() -> Vec<Interface> {
    let mut out: Vec<Interface> = vec![];
    for i in if_addrs::get_if_addrs().unwrap_or_default() {
        let ip = i.ip();
        if ip.is_loopback() || matches!(ip, IpAddr::V6(v6) if v6.is_unicast_link_local()) {
            continue;
        }
        match out.iter_mut().find(|o| o.name == i.name) {
            Some(o) => o.addrs.push(ip),
            None => out.push(Interface { name: i.name, addrs: vec![ip] }),
        }
    }
    out
}

/// The addresses to listen on: every address of `interface`, or with none named, the home
/// network's.
pub fn listen_addrs(interface: &str, port: u16) -> Vec<SocketAddr> {
    interfaces()
        .into_iter()
        .filter(|i| interface.is_empty() || i.name == interface)
        .flat_map(|i| i.addrs)
        .filter(|ip| !interface.is_empty() || is_lan(*ip))
        .map(|ip| SocketAddr::new(ip, port))
        .collect()
}

/// This Helper announced on the home network, until dropped.
pub struct Advert(ServiceDaemon);

impl Drop for Advert {
    fn drop(&mut self) {
        let _ = self.0.shutdown();
    }
}

/// Announces the Helper `name` with key `id` on `addrs`.
pub fn advertise(name: &str, id: &str, addrs: &[SocketAddr]) -> Result<Advert, String> {
    let Some(port) = addrs.first().map(SocketAddr::port) else { return Err("no address".into()) };
    let ips: Vec<IpAddr> = addrs.iter().map(SocketAddr::ip).collect();
    let d = ServiceDaemon::new().map_err(|e| e.to_string())?;
    // An instance name of its own on each machine, even when two share a host name.
    let instance = format!("{name}-{}", &id[..8.min(id.len())]);
    let host = format!("{instance}.local.");
    let info = ServiceInfo::new(SERVICE, &instance, &host, &ips[..], port, &[("key", id), ("name", name)][..]).map_err(|e| e.to_string())?;
    d.register(info).map_err(|e| e.to_string())?;
    Ok(Advert(d))
}

/// A Helper found on the home network.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Found {
    pub name: String,
    /// Its key, as hex.
    pub key: String,
    pub addr: String,
}

/// The Helpers that answer within `wait`, not counting the one with key `me`.
pub fn look(wait: Duration, me: &str) -> Result<Vec<Found>, String> {
    let d = ServiceDaemon::new().map_err(|e| e.to_string())?;
    let events = d.browse(SERVICE).map_err(|e| e.to_string())?;
    let until = Instant::now() + wait;
    let mut found: Vec<Found> = vec![];
    while let Ok(e) = events.recv_timeout(until.saturating_duration_since(Instant::now())) {
        let ServiceEvent::ServiceResolved(s) = e else { continue };
        let (Some(key), Some(name)) = (s.get_property_val_str("key"), s.get_property_val_str("name")) else { continue };
        let Some(ip) = s.get_addresses().iter().map(|a| a.to_ip_addr()).min_by_key(|ip| !ip.is_ipv4()) else { continue };
        if key == me || found.iter().any(|f| f.key == key) {
            continue;
        }
        found.push(Found { name: name.into(), key: key.into(), addr: SocketAddr::new(ip, s.get_port()).to_string() });
    }
    let _ = d.shutdown();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_home_network_is_private_addresses_only() {
        for lan in ["192.168.1.20", "10.0.0.5", "172.16.3.4", "169.254.1.1", "100.101.102.103", "fd7a:115c:a1e0::1"] {
            assert!(is_lan(lan.parse().unwrap()), "{lan}");
        }
        for not in ["8.8.8.8", "100.128.0.1", "172.32.0.1", "2001:db8::1", "127.0.0.1"] {
            assert!(!is_lan(not.parse().unwrap()), "{not}");
        }
    }
}
