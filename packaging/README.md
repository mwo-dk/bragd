# Releases and channels

A release is every installer, each with its `.sha256`, one `SHA256SUMS` over all of them, and
every channel, or it does not go out (CLAUDE.md, Shipping).

| Channel | What | Made by | Secret |
|---|---|---|---|
| GitHub release | `.deb`, `.rpm`, AppImage (x64, arm64); `.dmg` (Apple Silicon, x64); NSIS `.exe` and `.msi` (x64, arm64); the Flatpak bundle; the FreeBSD, NetBSD and OpenBSD desktop archives (x64, [bsd/](#the-bsds)); the built frontend for the ports; SBOM, CBOM, notices | `release.yml` | none |
| Homebrew | the `bragd` cask in [mwo-dk/homebrew-bragd](https://github.com/mwo-dk/homebrew-bragd): the `.dmg` on macOS, the AppImage on Linux (`taps/generate.sh`) | `release.yml`, job `taps` | `TAP_TOKEN` |
| WinGet | `mwo-dk.Bragd` in [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs), the NSIS installers, by Komac; the first version by hand from [winget/](winget/README.md) | `winget.yml`, Mondays | `WINGET_TOKEN` |
| Flatpak | `io.github.mwo_dk.Bragd` (`flatpak/`), built, linted and started by `flatpak.yml`; its bundle is a release file. Not on Flathub. | `flatpak.yml` in `release.yml` | none |
| Nix | the flake at the root (`flake.nix`, `nix/package.nix`): `nix run github:mwo-dk/bragd` | `nix.yml` in `release.yml` | none |
| BSD ports and pkgsrc | `games/bragd` for FreeBSD ([freebsd/](freebsd/README.md)) and OpenBSD ([openbsd/](openbsd/README.md)), `wip/bragd` for pkgsrc ([pkgsrc/](pkgsrc/README.md)); built from the tag and `bragd-frontend-vX.Y.Z.tar.gz`. Submitted by hand | by hand | none |
| AUR | `bragd` and `bragd-bin` ([aur/](aur/README.md)). Published by hand | by hand | none |
| Scripts | `install/install.sh` (Linux, macOS) and `install/install.ps1` (Windows) build from source | by hand | none |

Without its secret a channel's job leaves a notice and does nothing.

## The week

| When (UTC) | What |
|---|---|
| Every night, 02:17 | `nightly.yml`: Flatpak, Nix and the BSDs on master when it changed; red when the version in `Cargo.toml` has no published release (`unreleased.sh`) |
| Saturday, 03:47 | `images.yml`: the engine images built again ([images/](images/README.md)) |
| Saturday, 06:17 | `release.yml`, rehearsal: tomorrow's release built and checked, nothing published. Only by hand until 1.0.0 |
| Sunday, 06:17 | `release-pr.yml`: the release pull request, with auto-merge on; its merge releases. Only by hand until 1.0.0 |
| Monday, 05:23 | `coxswain-sync.yml`: the upstream sync issue |
| Monday, 06:00 | `winget.yml`: the newest release to WinGet |
| Monday, 06:17 | `deps.yml`: advisories, licences, bills of materials |

## The release pull request

`release-pr.py` makes it from the `changelog.d/` rows: minor when a row is marked
`<!-- feature -->`, patch otherwise; from a 0.x version it makes 1.0.0, since nothing is released
before it. It refuses, changing nothing:

| Exit | Why | Do |
|---|---|---|
| 2 | `changelog.d/` has no rows | nothing: no release this week |
| 3 | the version in `Cargo.toml` has no published release (it does not hold before the first release) | finish that release: fix what failed, then `gh workflow run release.yml` on master; or `--unreleased-ok`, the owner's decision only |
| 4 | the new version's milestone `X.Y.Z` has open issues (not those labelled `post-release` or titled "After the release…"); a version with no milestone of its own is held by every open milestone above the current version up to the next minor | close them or move them on; or `--open-issues-ok`, the owner's decision only |
| 5 | a feature row has no tour card | add the card to `crates/bragd-core/src/tour.rs` |

### Tour cards

Every user-facing feature ships a tour card (CLAUDE.md, Every feature). A feature's
`changelog.d/` row names its card after the feature marker:

```markdown
| **What a user gets.** In plain words. | [Page](area/page.md#anchor) | <!-- feature --> <!-- tour: card-id -->
```

`card-id` must appear in quotes (`"card-id"`) in `crates/bragd-core/src/tour.rs`, where the cards
live, keyed by the version they arrive in. A feature row with no marker, or a marker whose card
is not there, stops the release pull request with exit 5 and names the file.

## The BSDs

`bsd.yml` builds the window and the Helper for FreeBSD 14.5, NetBSD 10.1 and OpenBSD 7.9, each in
a virtual machine of that system (vmactions), with the system's own Rust, WebKitGTK and GTK
packages, starts `bragd-gui --version` there, and packs the result with `bsd/pack.sh`. In a
release, the `upload` job packs those binaries again with the release's tag and notices as
`bragd-desktop-vX.Y.Z-x86_64-unknown-<system>.tar.gz`, each with its `.sha256`. Pull requests
run `bsd.yml` only when `bsd/` or the workflow changes; every pull request runs
`tools/msrv-check.py`, which fails when a locked dependency needs a newer Rust than one of the
three systems packages.

| File | What |
|---|---|
| `bsd/pack.sh` | The archive: `bragd-gui`, `bragd-helper`, `bragd.desktop`, `bragd.png`, licence, README, notices |
| `bsd/bragd.desktop` | The menu entry in the archive |

On the BSDs, Calls are off (their WebKitGTK has no WebRTC; the app checks at start) and Engine container images are not
used (they are Linux images); the app says so where it applies
([install page](../docs/getting-started/install.md#freebsd-netbsd-and-openbsd)).

## Checking a release

`release-assets.sh` lists every file a release carries, from `release.yml`'s matrices. A new build
target goes into both; the `verify` job refuses to publish a draft that does not match the list
exactly. When a Release run ends, `release-check.yml` checks the published release: the files
against the list and `SHA256SUMS`, the `.deb` installed with apt and started on x64 and arm64, the
`.dmg` mounted on both Macs, the NSIS installer run silently on both Windows machines, the
cask's version, and the WinGet pull request within its week.

No job that builds holds a token that can write: a dependency's build script or install script
runs on the same machine as every later step of its job. `boms` and `desktop` build read-only and
hand their files on as artifacts; `upload`, which builds nothing, is the one job that writes them
to the draft. `tools/workflow-shell-check.py` fails any workflow job that builds with a write
permission or a `*TOKEN*` secret. A job that builds a container image (docker, podman, buildah)
may not even reach a read-only token: no secret, no `github.token` and no registry login. The
engine images (`images.yml`) build into an OCI archive, and a `push` job copies it to ghcr.io with
skopeo.

A job that failed on GitHub's side (a runner lost, an upload refused) is run again with
`gh run rerun <run id> --failed`. `discard` deleted the run's draft, and `upload` makes it again
(`release-draft.sh`, which leaves another run's draft alone). A failed rehearsal keeps its
`rehearsal/vX.Y.Z` branch for the same reason; the next rehearsal replaces it, and publishing that
version deletes it.

`retry.sh` runs a command again on a passing fault on the far side (an HTTP 5xx or 429, a reset
connection): five tries, a longer wait each time. `pkg-retry.sh` does the same for Flathub's
downloads, on any failure.

## Secrets

**RELEASE_PR_TOKEN:** a fine-grained token for `mwo-dk/bragd` with *Contents* and *Pull requests*
read and write: `gh secret set RELEASE_PR_TOKEN -R mwo-dk/bragd`. A pull request made with
`GITHUB_TOKEN` starts no checks, so it could never merge.

**TAP_TOKEN:** a fine-grained token limited to `mwo-dk/homebrew-bragd`, with *Contents* read and
write: `gh secret set TAP_TOKEN -R mwo-dk/bragd`.

**WINGET_TOKEN:** a classic token with `public_repo`, from the account that owns the fork
`mwo-dk/winget-pkgs`: `gh secret set WINGET_TOKEN -R mwo-dk/bragd`.

**APPLE_CERTIFICATE**, **APPLE_CERTIFICATE_PASSWORD**, **APPLE_ID**, **APPLE_PASSWORD**,
**APPLE_TEAM_ID** (optional): Developer ID signing and notarisation of the `.dmg`. Without them
the app is ad-hoc signed.

## Not yet

- **nixpkgs `bragd`**: the flake is ready; a nixpkgs package is submitted by hand once the app
  has a release.
