//! Peers: your own machines' Helpers, paired with each other (`docs/peers/`, ADR 0006).
//!
//! One machine shows a 6-digit code (`Node::show_code`); the other types it
//! (`Node::pair_with`). SPAKE2 turns the code into a shared key, so someone listening learns
//! nothing they could test guesses against, and someone guessing gets `CODE_TRIES` tries in
//! `CODE_TTL`. Each side then keeps the other's Ed25519 key and from then on talks only TLS
//! 1.3 with exactly that key (`tls`). Peers find each other on the home network by mDNS
//! (`find`), or by an address typed in. Either side can unpair.

pub mod find;
pub mod keys;
pub mod node;
mod tls;

pub use node::{CODE_TRIES, CODE_TTL, Failed, Listening, Node, Peer};

/// The port a Helper listens on for Peers, and the one a typed address without a port gets.
pub const PORT: u16 = 47_247;

/// Why a Peer said no.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    /// No code is shown there: none yet, it expired, it was used, or its tries are gone.
    NoCode,
    /// The code typed is not the one shown.
    WrongCode,
    /// The machine has not paired with this one, or has unpaired.
    NotPaired,
    /// The address is this machine's own.
    Itself,
}

impl Why {
    /// Its text's key, `peers.error.<id>`.
    pub fn id(self) -> &'static str {
        match self {
            Why::NoCode => "no_code",
            Why::WrongCode => "wrong_code",
            Why::NotPaired => "not_paired",
            Why::Itself => "itself",
        }
    }
}

/// This machine's name, as the other machines show it.
pub fn machine_name() -> String {
    #[cfg(unix)]
    {
        let mut buf = [0u8; 256];
        // SAFETY: gethostname writes at most `buf.len()` bytes into `buf`.
        if unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } == 0 {
            let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
            let name = String::from_utf8_lossy(&buf[..end]).into_owned();
            if !name.is_empty() {
                return name.split('.').next().unwrap_or(&name).to_string();
            }
        }
    }
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Bragd".into())
}
