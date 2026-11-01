[← README](../../README.md) · [Docs index](../README.md) · [Peers](README.md)

# How Peers talk

This page is for those who want to know what goes over the wire. Using Peers needs none of it: see [Pairing your machines](peers.md). Why it is built this way is in [ADR 0006](../adr/0006-peers-pair-by-spake2-and-pin-keys-in-tls.md).

## Contents

- [Keys](#keys)
- [The connection](#the-connection)
- [Pairing](#pairing)
- [Messages](#messages)
- [Finding each other](#finding-each-other)
- [Limits](#limits)
- [Questions](#questions)

## Keys

Each machine makes one Ed25519 key the first time Peers are used, from the system's random source. Its private half is `peers/key.pk8` in the Helper's folder, readable by the user alone. Its public half is the machine's id, written as 64 hex digits. The machines paired with this one are in `peers/peers.json`: each one's key, name, last address, when it was paired and when it last answered.

## The connection

Peers talk TLS 1.3 over TCP, port 47247, through rustls with ring. There are no certificates and no CA. Each side presents its Ed25519 key as a raw public key (RFC 7250), and proves it holds the private key with the handshake signature. Ed25519 is the only signature accepted, and TLS 1.2 is not offered.

| Side | Accepts |
|---|---|
| The machine dialled | A client key it has pinned. While it shows a code, any client key, and that connection may only pair |
| The machine dialling | Exactly the key it pinned for that Peer. When pairing, any key: the code then binds it |

A key that is not accepted ends the connection in the handshake, before either side says anything.

## Pairing

Machine B shows a code: six digits, uniform from the system's random source, good for 120 seconds and 5 tries. Machine A types it.

1. A connects to B with TLS. Each side now has the other's key from the handshake.
2. A sends its SPAKE2 message (Ed25519 group), made from the code, with A's key and B's key as the two identities.
3. B counts a try, and answers with its own SPAKE2 message.
4. Both derive the same key K only if they used the same code and saw the same two keys. A sends HMAC-SHA-256(K, `bragd peer client` ‖ E), where E is 32 bytes exported from this TLS session under the label `EXPORTER-bragd-peer-pairing`.
5. B checks it. Wrong: B says so and the try is spent. Right: B removes the code, pins A's key, and answers with HMAC-SHA-256(K, `bragd peer server` ‖ E).
6. A checks B's proof and pins B's key.

Someone who records a pairing cannot test codes against it afterwards: that is what SPAKE2 is for. Someone pretending to be B, or A, gets one guess per try. Because the identities are the TLS keys and the proofs carry the TLS exporter, a code typed on one connection pairs only the two keys on that connection.

## Messages

Each side sends one JSON line and the other answers with one.

| Sent | Answer | When |
|---|---|---|
| `{"op":"hello","name":…,"version":…}` | `{"re":"hello","name":…,"version":…}` | Between paired machines: **Check again**, and when Settings → Peers opens |
| `{"op":"pair","name":…,"port":…,"spake":…}` | `{"re":"spake","spake":…}` | Pairing, step 2 |
| `{"op":"confirm","mac":…}` | `{"re":"paired","name":…,"mac":…}` | Pairing, step 4 |
| `{"op":"unpaired"}` | `{"re":"done"}` | The sender has unpaired: the receiver forgets it too |
| `{"op":"call","body":…}` | `{"re":"reply","body":…}` | Between paired machines: a request of the Helpers' own, below |

A refusal is `{"re":"refused","why":…}`, with `why` one of `no_code`, `wrong_code` or `not_paired`. Bytes are hex.

### The Helpers' requests

A `call`'s body is one of these, and the reply's body is `{"ok":…}` or `{"error":…}`. They carry the [sync and the shared queue](sharing.md).

| Body | `ok` | What for |
|---|---|---|
| `{"op":"changes","store":…,"since":…}` | The rows changed after change number `since` in that store, oldest first, at most 2,000 and about 1 MiB; the store's id, the last number sent, and whether there are more | The Position store's sync. A `store` that is not the answering store's id gets everything from the start |
| `{"op":"working"}` | The jobs running there, and whether a language model is ready | *Settings → Peers* |
| `{"op":"take","lane":…}` | A job of `analysis` or `model`, lent to the asker, or `null` | The shared queue |
| `{"op":"renew","id":…,"done":…,"total":…}` | `true` to go on, `false` when the owner wants it back | Every minute while the job runs |
| `{"op":"give_back","id":…,"outcome":…}` | `null` | `outcome` is `done`, `translated` (with the text), `cannot` (with why) or `stopped` |

## Finding each other

A Helper with Peers on announces itself by mDNS as `_bragd._tcp.local.`, with its name and key in the TXT record. **Look around** lists what answers within three seconds. mDNS only says where a machine is; whether it is the machine you paired is decided by its key, in TLS.

## Limits

| What | Limit |
|---|---|
| A code | 120 seconds, 5 tries, one use |
| A line | 64 KiB |
| A reply to a `call` | 4 MiB |
| A batch of the sync | 2,000 rows and about 1 MiB; 50 batches from one Peer per sync |
| A job lent | 10 minutes, renewed every minute while it runs |
| A connection | 5 seconds, handshake included |
| Dialling | 3 seconds per address |
| Connections served at once | 16; more are closed at once |

## Questions

#### Why not certificates?

A certificate would only wrap the same key, and a CA would add a party that could vouch for someone else. Two of your own machines need to know one key each.

#### What if someone takes my machine's key file?

They could pose as that machine to its Peers. Unpair it on every other machine: they then refuse its key. The file is readable only by your user, like the rest of the Helper's folder.

#### Why SPAKE2 and not just the code as a password?

A code sent or hashed over the network could be guessed offline from a recording: a million guesses take no time. With SPAKE2 the recording is useless, and each guess needs a live try, five at most.

---
[← Previous: Sharing between Peers](sharing.md) · [Next: Engines →](../engines/README.md)
