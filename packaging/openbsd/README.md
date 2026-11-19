# OpenBSD port

[`games/bragd/`](games/bragd/) is laid out as in [openbsd-wip](https://github.com/jasperla/openbsd-wip)
and the ports tree. It builds the window and the Helper with `MODULES = devel/cargo` from the
GitHub tag, with the system's WebKitGTK 4.1 (`www/webkitgtk4,webkitgtk41`), GTK 3, libsoup 3 and
librsvg. The built frontend comes from the release's `bragd-frontend-vX.Y.Z.tar.gz`: npm cannot
fetch during the build.

| File | What |
|---|---|
| `games/bragd/Makefile` | The port |
| `games/bragd/pkg/DESCR`, `games/bragd/pkg/PLIST` | Description and the installed files |
| `games/bragd/crates.inc`, `games/bragd/distinfo` | The crates of the release's `Cargo.lock` with their licences, and every file's checksums. Written for a release (below); not here before Bragd's first release |

The OpenBSD desktop archive (`bragd-desktop-vX.Y.Z-x86_64-unknown-openbsd.tar.gz`) is built by
`.github/workflows/bsd.yml`; this port is for the ports tree.

## Updating it for a release

On OpenBSD 7.9 or -current with the ports tree in `/usr/ports` and openbsd-wip on
`PORTSDIR_PATH` (`echo 'PORTSDIR_PATH=${PORTSDIR}:${PORTSDIR}/openbsd-wip' | doas tee -a /etc/mk.conf`):

```sh
doas pkg_add rust cargo-generate-vendor
cp -R packaging/openbsd/games/bragd /usr/ports/openbsd-wip/games/
cd /usr/ports/openbsd-wip/games/bragd
# set GH_TAGNAME in the Makefile to vX.Y.Z, then:
touch crates.inc
make makesum
make modcargo-gen-crates | grep '^MODCARGO' > crates.inc
make makesum
make modcargo-gen-crates-licenses | grep '^MODCARGO' > crates.inc.new && mv crates.inc.new crates.inc
make package
make port-lib-depends-check      # add what it names to WANTLIB
make update-plist && git diff pkg/PLIST
/usr/ports/infrastructure/bin/portcheck
```

Copy `Makefile`, `crates.inc`, `distinfo` and `pkg/` back here.

## Submitting

By hand, once Bragd has a release:

1. Fork <https://github.com/jasperla/openbsd-wip>, add `games/bragd`, and open a pull request
   with what it is, how you tested it, and that you are upstream.
2. Once it builds cleanly there, mail ports@openbsd.org with the subject `NEW: games/bragd`, the
   `DESCR`, the homepage and how you tested it, with the port as a tarball
   (`tar czf bragd.tgz games/bragd` in openbsd-wip).
3. Answer the review. A developer who gives an OK imports it.

The port was drafted with an AI assistant: read every line until you could have written it,
build it yourself, and say in one sentence that an assistant helped.
