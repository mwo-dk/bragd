[← README](../../README.md) · [Docs index](../README.md) · [Getting started](README.md)

# Installing Bragd

Bragd is a desktop app for FreeBSD and the other BSDs, Linux, macOS and Windows. Each release puts the same version on every channel at once, or it does not go out. Every installer carries two programs: `bragd-gui`, the window, and `bragd-helper`, the [Helper](../knowledge/helper.md) that the window starts beside itself.

Bragd has not had its first release yet. Until it has, the channels below are empty, and the install scripts under [Any system](#any-system) build it from source.

## How to reach it

Pick your system, then the channel you already use for other software. The update check later names the command for that same channel ([Privacy](../reference/privacy.md)).

### FreeBSD and the other BSDs

Desktop tarballs for FreeBSD, NetBSD and OpenBSD are planned, but not yet available. The desktop app needs a WebKitGTK build per system, and on FreeBSD a fix in Tauri that is not released yet.

### Linux

| Channel | File or command | Systems |
|---|---|---|
| GitHub release | `Bragd_X.Y.Z_amd64.deb`, `Bragd-X.Y.Z-1.x86_64.rpm`, `Bragd_X.Y.Z_amd64.AppImage` | x64; the same three for arm64 (`arm64`, `aarch64`) |
| Flatpak | `Bragd_X.Y.Z_x86_64.flatpak` from the release, then `flatpak install --user Bragd_X.Y.Z_x86_64.flatpak`. The app id is `io.github.mwo_dk.Bragd`. It is not on Flathub | x64 |
| Nix | `nix run github:mwo-dk/bragd`, or add the flake to your profile | x64, arm64 |
| Homebrew | `brew tap mwo-dk/bragd` then `brew install --cask bragd`: the cask takes the AppImage | x64 |

### Any system

`install/install.sh` (Linux and macOS) and `install/install.ps1` (Windows) build Bragd from the source tree. They ask before they install Rust, Node.js or the system libraries the window needs (WebKitGTK on Linux), and `--yes` (`-Yes` in PowerShell) answers yes to every question.

| Script | Installs to |
|---|---|
| `./install/install.sh` | `~/.local/bin/bragd-gui` and a menu entry on Linux (`PREFIX` changes the folder); `~/Applications/Bragd.app` on macOS |
| `install\install.ps1` | `%LOCALAPPDATA%\Programs\Bragd`, on your user `PATH`, with a Start Menu shortcut |

The scripts copy only `bragd-gui` for now, not `bragd-helper`. Without the Helper beside it, the window shows the Helper as out of reach, and Sources, imports and Disk watch do not work.

### macOS

| Channel | File or command |
|---|---|
| GitHub release | `Bragd_X.Y.Z_aarch64.dmg` (Apple Silicon) or `Bragd_X.Y.Z_x64.dmg` (Intel) |
| Homebrew | `brew tap mwo-dk/bragd` then `brew install --cask bragd` |

### Windows

| Channel | File or command |
|---|---|
| GitHub release | `Bragd_X.Y.Z_x64-setup.exe` or `Bragd_X.Y.Z_x64_en-GB.msi`, and the same for `arm64` |
| WinGet | `winget install mwo-dk.Bragd` |

## What it needs

- A 64-bit machine, x64 or arm64.
- On Linux, WebKitGTK 4.1, which draws the window. The packages bring it in.
- Room for what you keep: Libraries, downloads, tablebases and engine images. [Disk watch](../knowledge/disk-watch.md) shows how much each takes.
- Podman or docker only if you want Bragd to run an Engine you have not installed ([Engine container images](../engines/container-images.md)).

## Checking the download

Every installer on the release page has a `.sha256` file beside it, and `SHA256SUMS` lists them all. On Linux or macOS, in the folder you downloaded to:

```sh
sha256sum -c SHA256SUMS --ignore-missing
```

On macOS without GNU coreutils, use `shasum -a 256 -c SHA256SUMS --ignore-missing`.

## Questions

#### Does Bragd update itself?

No. It checks for a newer release and shows the command that upgrades your copy, such as `brew upgrade --cask bragd` or `winget upgrade mwo-dk.Bragd`. Your package manager stays in charge. See [Privacy](../reference/privacy.md).

#### Which file do I take on Linux?

The `.deb` on Debian and Ubuntu, the `.rpm` on Fedora and openSUSE, and the AppImage anywhere else. The Flatpak bundle keeps Bragd in its own sandbox. The Nix flake suits NixOS.

#### Why are there two programs?

`bragd-helper` fetches Sources, imports and finds Duplicates in the background, for every open window. The window starts the copy beside its own program when none runs ([The Helper](../knowledge/helper.md)).

#### Can I run Bragd without installing it?

The AppImage runs where it is: make it executable and start it. Everything else installs.

#### Where does Bragd keep my settings?

In `config.toml` in your system's config folder, and your data in its data folder. [Files on disk](../reference/files.md) lists every place.

---
[← Previous: Getting started](README.md) · [Next: The install wizard →](first-start.md)
