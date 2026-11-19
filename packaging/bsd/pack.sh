#!/bin/sh
# Pack the desktop app for FreeBSD, NetBSD or OpenBSD as a release carries it:
# bragd-desktop-<tag>-<target>.tar.gz with its .sha256, holding bragd-gui, bragd-helper (the
# window starts it from beside itself), the menu entry and its icon, the licence and notes.
# Used by bsd.yml (every build) and release.yml (the release file), on the Linux runner: it
# needs sha256sum. POSIX sh.
#
#   sh packaging/bsd/pack.sh TAG TARGET BIN-DIR OUT-DIR
set -eu
tag=$1 target=$2 bin=$3 out=$4
name="bragd-desktop-$tag-$target"
mkdir -p "$out/$name"
cp "$bin/bragd-gui" "$bin/bragd-helper" packaging/bsd/bragd.desktop LICENSE README.md "$out/$name/"
cp assets/brand/icons/128x128.png "$out/$name/bragd.png"
# The notices come from the release's boms job; a build outside a release has none to carry.
[ ! -f THIRD-PARTY-NOTICES.md ] || cp THIRD-PARTY-NOTICES.md "$out/$name/"
cd "$out"
tar czf "$name.tar.gz" "$name"
rm -rf "$name"
sha256sum "$name.tar.gz" >"$name.tar.gz.sha256"
ls -l "$name.tar.gz" "$name.tar.gz.sha256"
