[← README](../../README.md) · [Docs index](../README.md) · [Getting started](README.md)

# Installing Bragd

Bragd is a desktop app for FreeBSD, NetBSD, OpenBSD, Linux, macOS and Windows. Each release puts the same version on every channel at once, or it does not go out. Every installer carries two programs: `bragd-gui`, the window, and `bragd-helper`, the [Helper](../knowledge/helper.md) that the window starts beside itself.

Bragd has not had its first release yet. Until it has, the channels below are empty, and the install scripts under [Any system](#any-system) build it from source.

## How to reach it

Pick your system, then the channel you already use for other software. The update check later names the command for that same channel ([Privacy](../reference/privacy.md)).

### FreeBSD, NetBSD and OpenBSD

| Channel | File | Systems |
|---|---|---|
| GitHub release | `bragd-desktop-vX.Y.Z-x86_64-unknown-freebsd.tar.gz` | FreeBSD 14.5 and newer, x64 |
| GitHub release | `bragd-desktop-vX.Y.Z-x86_64-unknown-netbsd.tar.gz` | NetBSD 10, x64 |
| GitHub release | `bragd-desktop-vX.Y.Z-x86_64-unknown-openbsd.tar.gz` | OpenBSD 7.9, x64 |

Each archive holds `bragd-gui`, `bragd-helper`, a menu entry (`bragd.desktop`) with its icon (`bragd.png`), the licence and the notices. Each is built in a virtual machine of its system, against that system's own packages. Install the window's libraries first, then put the two programs side by side, as root:

| System | Libraries |
|---|---|
| FreeBSD | `pkg install webkit2-gtk_41 gtk3 libsoup3 librsvg2-rust` |
| NetBSD | `pkgin install webkit-gtk41`, on a system with the X sets |
| OpenBSD | `pkg_add webkitgtk41 librsvg` |

```sh
tar xzf bragd-desktop-vX.Y.Z-x86_64-unknown-freebsd.tar.gz
cd bragd-desktop-vX.Y.Z-x86_64-unknown-freebsd
install -m 755 bragd-gui bragd-helper /usr/local/bin/
install -m 644 bragd.desktop /usr/local/share/applications/
install -m 644 bragd.png /usr/local/share/icons/hicolor/128x128/apps/
```

On NetBSD, `/usr/pkg` takes the place of `/usr/local`.

| Channel | State |
|---|---|
| FreeBSD port `games/bragd` | Prepared in `packaging/freebsd/`, not yet in the ports tree |
| pkgsrc `wip/bragd` (NetBSD and others) | Prepared in `packaging/pkgsrc/`, not yet in pkgsrc-wip |
| OpenBSD port `games/bragd` | Prepared in `packaging/openbsd/`, not yet in openbsd-wip |

The ports build from the release's source and `bragd-frontend-vX.Y.Z.tar.gz`, the built frontend, since npm cannot fetch during a ports build.

What differs on the BSDs:

| | |
|---|---|
| [Calls](../online/play-with-friends.md#calls) | Off while the system's WebKitGTK is built without WebRTC, as the three package it now. Bragd checks at start; a notice says so once, and Chat and Games with Friends work |
| [Engine container images](../engines/container-images.md) | Not used: they are Linux images. Install an Engine from the system's packages, such as `stockfish`, and Bragd finds it on your `PATH` |
| [The Helper](../knowledge/helper.md) | Runs as everywhere else |
| Rust | Each system's packaged Rust must build Bragd: FreeBSD's quarterly packages, NetBSD's pkgin and OpenBSD's `pkg_add` (`tools/msrv-check.py`) |

### Linux

| Channel | File or command | Systems |
|---|---|---|
| GitHub release | `Bragd_X.Y.Z_amd64.deb`, `Bragd-X.Y.Z-1.x86_64.rpm`, `Bragd_X.Y.Z_amd64.AppImage` | x64; the same three for arm64 (`arm64`, `aarch64`) |
| Flatpak | `Bragd_X.Y.Z_x86_64.flatpak` from the release, then `flatpak install --user Bragd_X.Y.Z_x86_64.flatpak`. The app id is `io.github.mwo_dk.Bragd`. It is not on Flathub | x64 |
| Nix | `nix run github:mwo-dk/bragd`, or add the flake to your profile | x64, arm64 |
| Homebrew | `brew tap mwo-dk/bragd` then `brew install --cask bragd`: the cask takes the AppImage | x64 |
| AUR | `bragd` (from source) and `bragd-bin` (the `.deb`), prepared in `packaging/aur/`; not on the AUR yet: `makepkg -si` in a clone's folder | x64, arm64 |

### Any system

`install/install.sh` (Linux, macOS and FreeBSD) and `install/install.ps1` (Windows) build Bragd from the source tree. They ask before they install Rust, Node.js or the system libraries the window needs (WebKitGTK on Linux), and `--yes` (`-Yes` in PowerShell) answers yes to every question. On FreeBSD, install what the build needs first, as root: `pkg install bash rust node24 npm-node24 pkgconf webkit2-gtk_41`.

| Script | Installs to |
|---|---|
| `./install/install.sh` | `~/.local/bin/bragd-gui`, `~/.local/bin/bragd-helper` and a menu entry on Linux and FreeBSD (`PREFIX` changes the folder); `~/Applications/Bragd.app` on macOS, with `bragd-helper` inside it and both linked from `~/.local/bin` |
| `install\install.ps1` | `bragd-gui.exe` and `bragd-helper.exe` in `%LOCALAPPDATA%\Programs\Bragd`, on your user `PATH`, with a Start Menu shortcut. Entries such as `%USERPROFILE%\bin` in your `PATH` stay as they were |

The window starts the Helper from beside its own program, so the scripts put `bragd-helper` next to `bragd-gui`.

### macOS

| Channel | File or command |
|---|---|
| GitHub release | `Bragd_X.Y.Z_aarch64.dmg` (Apple Silicon) or `Bragd_X.Y.Z_x64.dmg` (Intel) |
| Homebrew | `brew tap mwo-dk/bragd` then `brew install --cask bragd` |

### Windows

| Channel | File or command |
|---|---|
| GitHub release | `Bragd_X.Y.Z_x64-setup.exe` or `Bragd_X.Y.Z_x64_en-US.msi`, and the same for `arm64` |
| WinGet | `winget install mwo-dk.Bragd` |

## What it needs

- A 64-bit machine, x64 or arm64.
- On Linux and the BSDs, WebKitGTK 4.1, which draws the window. The Linux packages bring it in; on the BSDs install it from the system's packages.
- On Windows, Windows 10 or 11 and nothing more. Both programs carry their Visual C++ runtime, so the Visual C++ Redistributable is not needed.
- Room for what you keep: Libraries, downloads, tablebases and engine images. [Disk watch](../knowledge/disk-watch.md) shows how much each takes.
- Podman or docker only if you want Bragd to run an Engine you have not installed ([Engine container images](../engines/container-images.md)).

## Checking the download

Every installer and archive on the release page has a `.sha256` file beside it, and `SHA256SUMS` lists them all. On Linux or macOS, in the folder you downloaded to:

```sh
sha256sum -c SHA256SUMS --ignore-missing
```

On macOS without GNU coreutils, use `shasum -a 256 -c SHA256SUMS --ignore-missing`. On the BSDs, `sha256 -q <file>` (FreeBSD) or `cksum -a sha256 <file>` (NetBSD, OpenBSD) prints the sum to compare with the `.sha256` file.

## Questions

#### Does Bragd update itself?

No. It checks for a newer release and shows the command that upgrades your copy, such as `brew upgrade --cask bragd` or `winget upgrade mwo-dk.Bragd`. Your package manager stays in charge. See [Privacy](../reference/privacy.md).

#### Which file do I take on Linux?

The `.deb` on Debian and Ubuntu, the `.rpm` on Fedora and openSUSE, and the AppImage anywhere else. The Flatpak bundle keeps Bragd in its own sandbox. The Nix flake suits NixOS.

#### Why does the Homebrew cask clear the quarantine flag?

Bragd's macOS app is not yet signed and notarised by Apple. macOS marks everything downloaded with a quarantine flag, and Gatekeeper refuses an unnotarised app that carries one: it says the app is damaged and offers only to move it to the Bin. So after installing, the cask runs `xattr -cr` on `Bragd.app` to clear the flag. That also means Gatekeeper does not check the app when it first opens. The cask's download is still checked against its SHA-256. Once releases are notarised, the cask will stop clearing the flag.

#### Why are there two programs?

`bragd-helper` fetches Sources, imports and finds Duplicates in the background, for every open window. The window starts the copy beside its own program when none runs ([The Helper](../knowledge/helper.md)).

#### Why can I not call a Friend on FreeBSD?

The WebKitGTK that FreeBSD, NetBSD and OpenBSD package is built without WebRTC, which carries a Call's sound and picture. Bragd checks for it at start, shows no Call buttons without it, and says so once. Should a system's WebKitGTK gain WebRTC, Calls work there with no change to Bragd. Chat and Games with Friends work as on any other system.

#### Can I run Bragd without installing it?

The AppImage runs where it is: make it executable and start it. Everything else installs.

#### Where does Bragd keep my settings?

In `config.toml` in your system's config folder, and your data in its data folder. [Files on disk](../reference/files.md) lists every place.

---
[← Previous: Getting started](README.md) · [Next: The install wizard →](first-start.md)
