[← README](../../README.md) · [Docs index](../README.md)

# Peers

A **Peer** is another of your own machines whose Helper is paired with this one. You pair two machines once, with a 6-digit code. From then on their Helpers talk only to each other's key, over TLS. Peers are off until you turn them on in *Settings → Peers*.

In this version, Peers pair, find each other and say hello: each shows the other's name, Bragd version and whether it can be reached. Sharing the Position store and the background queue between Peers builds on this pairing.

| Page | What it covers |
|---|---|
| [Pairing your machines](peers.md) | Turning Peers on, showing and typing a code, finding a machine, unpairing |
| [How Peers talk](protocol.md) | The pairing code, SPAKE2, TLS with pinned keys, mDNS and the messages |

---
[← Previous: The Helper](../knowledge/helper.md) · [Next: Pairing your machines →](peers.md)
