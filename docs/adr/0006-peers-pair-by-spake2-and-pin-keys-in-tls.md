# Peers pair by a 6-digit code through SPAKE2 and then trust only each other's pinned key in TLS

A user's machines become Peers by a 6-digit code that one shows and the other types. Each Helper has an Ed25519 key. Pairing runs SPAKE2 (Ed25519 group) inside a TLS 1.3 connection in which each side has shown its key as a raw public key (RFC 7250). The SPAKE2 identities are those two keys, and the key confirmation is an HMAC over a TLS exporter, so the code binds exactly the two keys on that connection. A code lasts 120 seconds and 5 tries, and is used once. After pairing, each side keeps the other's key, and the Helpers talk only TLS 1.3 with that key: rustls with ring, Ed25519 handshake signatures only, no certificates and no CA. While no code is shown, a key not pinned is refused in the handshake. Peers are found by mDNS (`_bragd._tcp`) or a typed address, and listen only while the user has turned them on, on the home network's private addresses or the interface picked. The design is in [How Peers talk](../peers/protocol.md).

We chose this over three others:

- **The code as a shared secret**, such as TLS-PSK or a hash of the code. A recording of one pairing would let anyone try all million codes offline in moments. SPAKE2 makes each guess a live try, and the tries are capped.
- **Self-signed certificates with a fingerprint the user compares.** This is safe only if people really compare 64 hex digits. A code typed once is easier, and is checked by the machine.
- **A CA of Bragd's own, or a rendezvous server.** It adds a party that could vouch for a stranger, and something that must run. Peers are two of the user's own machines; each needs to know one key.

It is hard to reverse because the key and the pinned keys are kept on every machine, and the pairing exchange is the wire format between versions. A change to either means pairing again on every machine.

In return we carry a key file per machine, which only the user can read (a stolen one lets the thief pose as that machine until it is unpaired), a fixed port (47247), and three more crates that do cryptography: `spake2`, `curve25519-dalek` beneath it, and direct use of `ring`, `hmac` and `sha2`, all in `tools/bom/crypto.toml`. SPAKE2 is not quantum-safe; neither is the TLS key exchange it sits in. Pairing is a short, rare event, and a stolen recording gives no code without breaking the group itself.
