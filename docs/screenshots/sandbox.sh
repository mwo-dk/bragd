#!/usr/bin/env bash
# Run a command as user "demo" with a throwaway home, for screenshots that show nothing
# personal. The real /home, /tmp, /mnt and /run/media are hidden, the host name is "demo", the
# environment is cleared, and HOME and the XDG folders point into the fake home; podman and
# docker cannot run in it. Needs bubblewrap (bwrap) and a release build (target/release/bragd-gui).
#
#   docs/screenshots/sandbox.sh bragd-gui
#   SANDBOX_ARGS="--unshare-net" docs/screenshots/sandbox.sh bragd-gui    # extra bwrap arguments
set -euo pipefail
repo="$(cd "$(dirname "$0")/../.." && pwd)"
demo="${DEMO_HOME:-${XDG_RUNTIME_DIR:-/tmp}/bragd-demo-home}"
[ -d "$demo" ] || "$repo/docs/screenshots/demo-home.sh" "$demo"

# No containers in here (CLAUDE.md, Testing on a machine someone uses): a podman run in the
# sandbox would start its pause process in the sandbox's namespaces and point the user's
# rootless podman at it. Every podman and docker on the PATH is covered with an empty file.
no_containers=()
for b in $(type -ap podman docker 2>/dev/null) /usr/bin/podman /usr/bin/docker; do
  [ -e "$b" ] && no_containers+=(--ro-bind /dev/null "$(readlink -f "$b")")
done

# Mounted disks are hidden where the machine has them: a CI runner has no /run/media, and the
# read-only root cannot get one.
hide=()
for d in /mnt /run/media; do
  [ -d "$d" ] && hide+=(--tmpfs "$d")
done

# shellcheck disable=SC2086 # SANDBOX_ARGS is split into arguments on purpose
exec bwrap \
  --ro-bind / / --dev /dev --proc /proc \
  "${no_containers[@]}" \
  --tmpfs /tmp --tmpfs /home --tmpfs /opt "${hide[@]}" \
  --bind "$demo" /home/demo \
  --ro-bind "$repo/target/release" /opt/bragd \
  --bind "$XDG_RUNTIME_DIR" "$XDG_RUNTIME_DIR" \
  --unshare-uts --hostname demo \
  ${SANDBOX_ARGS:-} \
  --chdir /home/demo \
  --clearenv \
  --setenv HOME /home/demo --setenv USER demo --setenv LOGNAME demo \
  --setenv XDG_CONFIG_HOME /home/demo/.config --setenv XDG_CACHE_HOME /home/demo/.cache \
  --setenv XDG_DATA_HOME /home/demo/.local/share \
  --setenv PATH "/opt/bragd:/usr/local/bin:/usr/bin:/bin" \
  --setenv LANG "${LANG:-C.UTF-8}" \
  --setenv XDG_RUNTIME_DIR "$XDG_RUNTIME_DIR" \
  --setenv WAYLAND_DISPLAY "${WAYLAND_DISPLAY:-}" --setenv DISPLAY "${DISPLAY:-}" \
  "$@"
