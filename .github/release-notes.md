Bragd is a desktop app for playing, studying and keeping board games: chess and its Variants, xiangqi and janggi, shogi, go, draughts, tafl, backgammon, ur and senet.

## Download

Every file comes from this release's **Assets** list below.

| You have | File |
|---|---|
| Linux (Debian, Ubuntu) | `Bragd_…_amd64.deb` |
| Linux (Fedora, openSUSE) | `Bragd-…x86_64.rpm` |
| Linux (any distribution) | `Bragd_…_amd64.AppImage`, or `Bragd_…_x86_64.flatpak` |
| Linux on ARM64 (Raspberry Pi OS 64-bit, Debian, Ubuntu) | `Bragd_…_arm64.deb` |
| Linux on ARM64 (Fedora, Asahi) | `Bragd-…aarch64.rpm` |
| Linux on ARM64 (any distribution) | `Bragd_…_aarch64.AppImage` |
| Mac with Apple Silicon (M1 or newer) | `Bragd_…_aarch64.dmg` |
| Mac with Intel | `Bragd_…_x64.dmg` |
| Windows 10/11 | `Bragd_…_x64-setup.exe` (or the `.msi`) |
| Windows 10/11 on ARM | `Bragd_…_arm64-setup.exe` (or the `.msi`) |

Or from a package manager: `brew install --cask mwo-dk/bragd/bragd` (macOS, Linux), `winget install mwo-dk.Bragd` (Windows), `nix run github:mwo-dk/bragd` (Linux).

`SHA256SUMS` lists every file's checksum. Check a download with `sha256sum -c --ignore-missing SHA256SUMS`.

## What it is made of

`bragd-….sbom.cdx.json` lists every component (a CycloneDX SBOM), `bragd-….cbom.cdx.json` the cryptography it uses (a CBOM), and `THIRD-PARTY-NOTICES.md` every component's licence; it also ships inside the app.

## The app is not code-signed

Bragd is free, and the builds are not signed with a paid certificate, so your system warns you the first time:

- **Windows:** SmartScreen says "Windows protected your PC". Click **More info**, then **Run anyway**.
- **macOS:** the app "can't be opened" or "is damaged". Drag Bragd to Applications first, then run this once in Terminal: `xattr -cr /Applications/Bragd.app`.
- **Linux:** no warning. Make the AppImage executable (`chmod +x`) and run it.

## Build it yourself

```sh
git clone https://github.com/mwo-dk/bragd.git
cd bragd
./install/install.sh          # Linux, macOS or FreeBSD
```

On Windows:

```powershell
git clone https://github.com/mwo-dk/bragd.git
cd bragd
powershell -ExecutionPolicy Bypass -File install\install.ps1
```

The script offers to install Rust and Node.js if they are missing, and it asks before it does.
