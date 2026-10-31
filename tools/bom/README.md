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

`cyclonedx-npm` runs through `npx` at the version pinned in `generate.sh`, so the frontend's
lockfile does not carry it. All output is CycloneDX 1.6 JSON (cargo-cyclonedx writes 1.5, and
`bom.py` lifts it). Timestamps are the commit's and serial numbers derive from the version, so a
commit always gives the same bytes.

## Where it runs

- **CI:** the *bills of materials* job of the Dependencies workflow (`deps.yml`), on every pull
  request that changes a dependency, on `master` and weekly, so a failure shows before a release.
  Until `gui/` exists it is skipped with a notice.
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
