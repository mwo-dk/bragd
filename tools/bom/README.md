# Bills of materials and licence notices

`generate.sh` makes what every release carries:

| File | Made from |
|---|---|
| `bragd-<version>.sbom.cdx.json` | `cargo cyclonedx` for `bragd-gui`, and `cyclonedx-npm` for `gui/` without dev dependencies |
| `bragd-<version>.cbom.cdx.json` | `crypto.toml`, checked against `cargo metadata` and the source |
| `THIRD-PARTY-NOTICES.md` | `cargo about` (Rust), the npm packages' licence files, and the `[[bundled]]` entries of `licenses.toml` |

```sh
cargo install --locked cargo-cyclonedx@0.5.7 cargo-about@0.8.4
(cd gui && npm ci)
tools/bom/generate.sh [OUT_DIR]        # default target/bom
```

`cyclonedx-npm` is a devDependency of `gui/`, pinned exactly, so `gui/package-lock.json` locks it
and everything it pulls in, and `npm audit` covers it. `generate.sh` runs that copy and never
fetches one. Its `glob` is overridden to 14 in `gui/package.json`: the version its unused XML
validator brings has a known advisory. All output is CycloneDX 1.6 JSON (cargo-cyclonedx writes 1.5, and
`bom.py` lifts it). Timestamps are the commit's and serial numbers derive from the version, so a
commit always gives the same bytes.

## Where it runs

- **CI:** the *bills of materials* job of the Dependencies workflow (`deps.yml`), on every pull
  request that changes a dependency, on `master` and weekly, so a failure shows before a release.
  CI installs the two cargo tools as prebuilt,
  checksummed binaries (`taiki-e/install-action`) instead of compiling them each run.
- **Release:** the `boms` job of `release.yml` uploads the three files to the draft release. The
  installers take `THIRD-PARTY-NOTICES.md` from there and bundle it through a `--config`
  override, so other builds need no such file. The release is published only once `boms` has
  passed.

## What it checks, and what to do when it fails

| Message | Do |
|---|---|
| `X is in the build but not in crypto.toml` | A dependency brought in a crate from `watch`. Find out what it is used for (`cargo tree -i X`) and add a `[[library]]`, with `[[asset]]`s for what Bragd really uses |
| `crypto.toml describes X, which is no longer in the build` | Remove its `[[library]]`, and assets nothing else provides |
| `"…" is no longer in <file>` | The code moved: update the `found` pattern to a line that still shows the use |
| `npm licences: … is not on the allowed list` | Read the package's licence. If it is acceptable, add it to `[npm].also_allow` with the reason; if the package metadata is wrong, add an override keyed by name and version |

`cargo deny` checks the Rust crates' licences on its own (`deny.toml`); `about.toml` accepts the
same list.

## Kept by hand

- **`crypto.toml`:** no scanner reads Rust's cryptography well enough yet. Each asset says what
  it is (CycloneDX `cryptoProperties`), why Bragd uses it (`purpose`) and where (`found`, turned
  into file and line evidence). Every crate that does cryptography is described there, in the
  pull request that brings it in (CLAUDE.md, Building and checking).
- **`licenses.toml`:** components that come without a package manager (the piece art, glyph
  outlines from fonts) and C code a crate compiles in under its own licence, whose texts
  cargo-about does not report. The art is listed for people in `docs/CREDITS.md`.

## Our engine images

Bragd ships no container image in a release, but it runs images of its own
(`packaging/images/<engine>`, built by `.github/workflows/images.yml`). Each image's licence, the
licences of what it holds, and where its source is, are listed here when its folder is added.

| Image | Licence | Holds | Source |
|---|---|---|---|
| `bragd-pikafish` | GPL-3.0-only | `/pikafish` and its `COPYING`. Not its evaluation network: that one's NNUE-License forbids commercial use without permission, so it is downloaded only with the user's consent, from the release, into Bragd's data folder (`bragd_engines::network`) | Pikafish 2026-09-06 release, by digest (`packaging/images/pikafish/Dockerfile`) |
| `bragd-arasan` | MIT | `/arasan`, its network `arasanv8-20261005.nnue` (from the release's `network/` folder, under the same MIT licence) and its `COPYING`. Fathom, its Syzygy prober, is MIT too | Arasan v26.1 release and Fathom at the commit it names, by digest (`packaging/images/arasan/Dockerfile`) |
| `bragd-berserk` | GPL-3.0-or-later | `/berserk`, with its network `berserk-9b84c340af7e.nn` built in (no licence of its own; Berserk's releases ship it inside the program), and its `COPYING` | Berserk 14 release and the network from Berserk's networks release, by digest (`packaging/images/berserk/Dockerfile`) |
| `bragd-ethereal` | GPL-3.0-or-later | `/ethereal` and its `COPYING`. Hand-crafted evaluation: no network | Ethereal v14.00 release, by digest (`packaging/images/ethereal/Dockerfile`) |
| `bragd-fruit` | GPL-2.0-or-later | `/fruit` and its `COPYING`. Not its opening book | Fruit 2.1, Debian's `fruit_2.1.dfsg.orig.tar.gz` from snapshot.debian.org, by digest (`packaging/images/fruit/Dockerfile`) |
| `bragd-koivisto` | GPL-3.0-or-later AND MIT | `/koivisto`, with its network `default.net` built in, its `COPYING`, and the network's MIT licence as `COPYING.network` | Koivisto v9.0 release and KoivistoNetworks at the commit it names, by digest (`packaging/images/koivisto/Dockerfile`) |
| `bragd-lc0` | GPL-3.0-or-later | `/lc0` (CPU, Eigen) and its `COPYING`. Not a network: Lc0 needs one, downloaded only with the user's consent | Lc0 v0.32.1 release, lczero-common at the commit it names, and Abseil through its meson wrap, by digest (`packaging/images/lc0/Dockerfile`) |
| `bragd-rubichess` | GPL-3.0-or-later | `/rubichess`, with its network `nn-bc638d5ec9-20240730.nnue` built in (no licence of its own; RubiChess's releases ship it inside the program), and its `COPYING` | RubiChess 20240817 release and the network from its author's NN repository at a fixed commit, by digest (`packaging/images/rubichess/Dockerfile`) |
| `bragd-scan` | GPL-3.0-only | `/scan/scan`, its `scan.ini`, its `license.txt`, and the international draughts evaluation and opening book from the release's `data/` folder, under the same licence. Not the endgame tables | Scan 3.1 release (`scan_31.zip`), by digest (`packaging/images/scan/Dockerfile`) |
| `bragd-gnubg` | GPL-3.0-or-later | `/gnubg` and its `COPYING`, with its data in `/usr/share/gnubg`: its neural network weights and the one- and two-sided bear-off databases its build makes, all part of the release | GNU Backgammon 1.08.003 release, by digest (`packaging/images/gnubg/Dockerfile`) |
