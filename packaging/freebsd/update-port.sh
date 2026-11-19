#!/bin/sh
# Bring games/bragd to the release $1 (vX.Y.Z), in a FreeBSD machine with git and portlint:
# regenerate distinfo and Makefile.crates, build and check the package, copy the port back here,
# and write out/port.diff against the port as it is in the ports tree (all new until it is there).
#
#   sh packaging/freebsd/update-port.sh v1.0.0
set -eu
v="${1#v}"
here=$(cd "$(dirname "$0")" && pwd)
out="$here/out"
mkdir -p "$out"

# The quarterly ports tree: with main, USES=cargo can want a newer lang/rust than the packages
# have for days after each Rust release, and builds Rust from source. The diff is made against
# main, where a port is submitted.
if [ ! -d /usr/ports/Mk ]; then
  branch="$(date +%Y)Q$(( ($(date +%m | sed 's/^0//') - 1) / 3 + 1 ))"
  git clone -q --depth 1 -b "$branch" https://git.FreeBSD.org/ports.git /usr/ports ||
    git clone -q --depth 1 https://git.FreeBSD.org/ports.git /usr/ports
fi
git -C /usr/ports fetch -q --depth 1 origin main:refs/remotes/origin/main
port=/usr/ports/games/bragd
rm -rf "$port"
cp -R "$here/games/bragd" "$port"
cd "$port"
sed -i '' "s/^DISTVERSION=.*/DISTVERSION=	$v/" Makefile
rm -f Makefile.crates distinfo
make makesum BATCH=yes
make cargo-crates BATCH=yes | grep -v '^===>' >Makefile.crates
make makesum BATCH=yes
make clean BATCH=yes
make stage check-plist package BATCH=yes
make clean BATCH=yes
git -C /usr/ports add games/bragd
portlint -AC

cp Makefile Makefile.crates distinfo "$here/games/bragd/"
git -C /usr/ports diff --staged origin/main -- games/bragd >"$out/port.diff"
echo "games/bragd $v: built and checked; the diff is in $out/port.diff"
