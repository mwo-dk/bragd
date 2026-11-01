[← README](../../README.md) · [Docs index](../README.md)

# Peers

A **Peer** is another of your own machines whose Helper is paired with this one. You pair two machines once, with a 6-digit code. From then on their Helpers talk only to each other's key, over TLS. Peers are off until you turn them on in *Settings → Peers*.

Paired machines swap what is new in their Position stores, and an idle one takes the other's waiting background analysis and translations. Libraries, Slices and Settings stay on each machine.

| Page | What it covers |
|---|---|
| [Pairing your machines](peers.md) | Turning Peers on, showing and typing a code, finding a machine, unpairing |
| [Sharing between Peers](sharing.md) | The Position store's sync, the shared queue, and a Peer's language model |
| [How Peers talk](protocol.md) | The pairing code, SPAKE2, TLS with pinned keys, mDNS and the messages |

---
[← Previous: The Helper](../knowledge/helper.md) · [Next: Pairing your machines →](peers.md)
