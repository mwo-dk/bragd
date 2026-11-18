# AUR packages

Not on the AUR yet: publishing needs an AUR account and its SSH key, which only the owner can
set up. Until then, on Arch, clone the repository and run `makepkg -si` in one of these folders.

| Package | What |
|---|---|
| [`bragd`](bragd/PKGBUILD) | Builds the window and the Helper from the tagged source, with npm and cargo |
| [`bragd-bin`](bragd-bin/PKGBUILD) | The release's `.deb` for x64 or arm64, unpacked |

In a clone, `pkgver` is the version in the clone's `Cargo.toml`, so `makepkg -si` fetches that
release: before Bragd's first release there is none to fetch. The checksums are `SKIP` while
`pkgver` is read from the clone; the published PKGBUILD names the version and its sums
(`updpkgsums`).

## Publishing, once

1. Have an account at <https://aur.archlinux.org>, with an SSH key made for it
   (`ssh-keygen -t ed25519 -f ~/.ssh/aur -C bragd-aur`) under *My Account → SSH Public Key*.
2. For each package: `git clone ssh://aur@aur.archlinux.org/<package>.git`, copy its `PKGBUILD`
   in, write the released version into `pkgver`, run `updpkgsums` and
   `makepkg --printsrcinfo > .SRCINFO`, build it with `makepkg -si`, then commit and push.

Arch's `webkit2gtk-4.1` is built without WebRTC, so Calls are off with these packages: Bragd
says so once.
