# pkgsrc package

[`bragd/`](bragd/) is a package for `wip/bragd` in [pkgsrc-wip](https://pkgsrc.org/wip/). pkgsrc
builds on NetBSD and other systems; this package builds the window and the Helper with
`lang/rust/cargo.mk` from the GitHub tag, with pkgsrc's WebKitGTK 4.1, GTK 3, libsoup 3 and
librsvg. The built frontend comes from the release's `bragd-frontend-vX.Y.Z.tar.gz`: npm cannot
fetch during the build.

| File | What |
|---|---|
| `bragd/Makefile` | The recipe |
| `bragd/DESCR`, `bragd/PLIST` | Description and the installed files |
| `bragd/cargo-depends.mk`, `bragd/distinfo` | The crates of the release's `Cargo.lock` and every file's checksums. Written for a release (below); not here before Bragd's first release |

The NetBSD desktop archive (`bragd-desktop-vX.Y.Z-x86_64-unknown-netbsd.tar.gz`) is built by
`.github/workflows/bsd.yml`; this package is for pkgsrc.

## Updating it for a release

On NetBSD with the pkgsrc tree in `/usr/pkgsrc` and wip in `/usr/pkgsrc/wip`:

```sh
cp -R packaging/pkgsrc/bragd /usr/pkgsrc/wip/
cd /usr/pkgsrc/wip/bragd
# set DISTNAME in the Makefile to bragd-X.Y.Z, then:
touch cargo-depends.mk
make makesum                                  # the tag and the frontend
make print-cargo-depends > cargo-depends.mk   # the crates of its Cargo.lock
make makesum                                  # and theirs
make package install && bragd-gui --version
pkglint -Wall
```

Copy `Makefile`, `cargo-depends.mk` and `distinfo` back here.

## Submitting

By hand, once Bragd has a release:

1. Get wip access as <https://pkgsrc.org/wip/> says: mail your public SSH key, the user name you
   would like and a word about the package.
2. Build and check it as above, then add `SUBDIR+=	bragd` to `wip/Makefile`, in order.
3. Commit in wip's form, with its `COMMIT_MSG` file in the package:
   `bragd: add version X.Y.Z` and the first line of `DESCR`.
4. Later a pkgsrc developer may move it to `games/bragd`; ask on pkgsrc-users@NetBSD.org.

The package was drafted with an AI assistant. NetBSD presumes code written by one tainted
unless its rules on the day say otherwise: read every line, build it yourself, and follow them.
