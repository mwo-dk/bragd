#!/bin/sh
# A package install in CI that fetches from far away (Flathub's runtimes and SDK extensions):
# four tries, 30, 60 and 120 s apart. Unlike retry.sh it tries again on any failure, since a
# mirror half-way through a sync looks like a real dependency error; a real one still fails,
# after the fourth try. POSIX sh.
#
#   sh packaging/pkg-retry.sh flatpak install --user -y flathub org.gnome.Platform//51
#
# PKG_TRIES and PKG_WAIT (the first wait, in seconds) are for testing it.
set -u
tries=${PKG_TRIES:-4}
wait=${PKG_WAIT:-30}
i=1
while :; do
  "$@"
  status=$?
  [ "$status" = 0 ] && exit 0
  [ "$i" -ge "$tries" ] && break
  echo "::warning::$1 failed (try $i of $tries): trying again in ${wait}s"
  sleep "$wait"
  wait=$((wait * 2))
  i=$((i + 1))
done
echo "::error::Could not fetch with '$*' after $tries tries"
exit "$status"
