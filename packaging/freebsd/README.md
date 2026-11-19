# FreeBSD port

`games/bragd` builds the window and the Helper with `USES=cargo` from the GitHub tag, and installs
`bin/bragd-gui`, `bin/bragd-helper`, the menu entry and its icon. The built frontend comes from
the release's `bragd-frontend-vX.Y.Z.tar.gz`: npm cannot fetch during a ports build.

| File | What |
|---|---|
| `games/bragd/Makefile` | The port |
| `games/bragd/pkg-descr` | Its description |
| `update-port.sh` | Brings the port to a release in a FreeBSD machine: `distinfo`, `Makefile.crates`, a built and checked package, and `out/port.diff` |

`Makefile.crates` and `distinfo` are written by `update-port.sh` for a release; until Bragd's
first release there is nothing to sum, so they are not here yet.

The desktop archive for FreeBSD (`bragd-desktop-vX.Y.Z-x86_64-unknown-freebsd.tar.gz`) is built by
`.github/workflows/bsd.yml`; this port is for the ports tree.

## Updating it for a release

In a FreeBSD 14.5 or newer machine, as root, with `pkg install git portlint rust`:

```sh
sh packaging/freebsd/update-port.sh vX.Y.Z
```

Commit the port's `Makefile`, `Makefile.crates` and `distinfo` back here.

## Submitting

By hand, as a new port, once Bragd has a release:

1. Make the diff: `update-port.sh` leaves it in `out/port.diff`. Add `SUBDIR += bragd` (sorted)
   to `games/Makefile` in the same diff.
2. File it at <https://bugs.freebsd.org/bugzilla/enter_bug.cgi?product=Ports%20%26%20Packages>,
   component *Individual Port(s)*, summary `[NEW PORT] games/bragd: Play, study and keep board
   games`, with the diff attached.
3. Answer the committer's review. Once it is in the tree, an update is a
   `games/bragd: Update to X.Y.Z` report with the diff and *maintainer-approval* `+`.

The port was drafted with an AI assistant: read every line, build it yourself, and say so if
FreeBSD's rules on the day ask for it.
