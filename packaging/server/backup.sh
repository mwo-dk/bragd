#!/usr/bin/env bash
# Back up the Bragd server's Postgres into BRAGD_BACKUP_DIR (the Storage Box, mounted) and delete
# backups older than 30 days. Run it daily from a timer; `--check` tests it on throwaway files with
# a stand-in pg_dump. `--failed` raises the alarm: bragd-backup-failed.service runs it when a
# backup fails.
#
#   BRAGD_SERVER_DATABASE_URL=postgres://… BRAGD_BACKUP_DIR=/mnt/storagebox/bragd packaging/server/backup.sh
set -euo pipefail
KEEP_DAYS=30

prune() {
  find "$1" -maxdepth 1 -name 'bragd-*.dump' -type f -mtime "+$KEEP_DAYS" -delete
  # A run killed before its own clean-up leaves a part; none is younger than a day for long.
  find "$1" -maxdepth 1 -name 'bragd-*.dump.part' -type f -mtime +1 -delete
}

backup() {
  : "${BRAGD_SERVER_DATABASE_URL:?}" "${BRAGD_BACKUP_DIR:?}"
  # The password goes to pg_dump in its environment, not on its command line, where every local
  # user could read it in /proc while the dump runs.
  local url=$BRAGD_SERVER_DATABASE_URL
  if [[ $url =~ ^(postgres(ql)?://[^:/@]+):([^@]*)@(.*)$ ]]; then
    url="${BASH_REMATCH[1]}@${BASH_REMATCH[4]}"
    local pass=${BASH_REMATCH[3]}
    PGPASSWORD=$(printf '%b' "${pass//%/\\x}")
    export PGPASSWORD
  fi
  out="$BRAGD_BACKUP_DIR/bragd-$(date -u +%Y%m%dT%H%M%SZ).dump"
  trap 'rm -f "$out.part"' EXIT
  pg_dump --format=custom --file="$out.part" "$url"
  mv "$out.part" "$out"
  trap - EXIT
  prune "$BRAGD_BACKUP_DIR"
}

failed() {
  logger -p daemon.crit -t bragd-backup "The Bragd server's backup failed: see journalctl -u bragd-backup.service" || true
  if [ -n "${BRAGD_BACKUP_ALERT:-}" ]; then
    sh -c "$BRAGD_BACKUP_ALERT"
  fi
}

fail() { echo "backup.sh --check: $1" >&2; exit 1; }

check() {
  d=$(mktemp -d); trap 'rm -rf "$d"' EXIT
  touch -d '31 days ago' "$d/bragd-old.dump"
  touch -d '29 days ago' "$d/bragd-new.dump"
  touch -d '3 days ago' "$d/bragd-killed.dump.part"
  touch -d '40 days ago' "$d/other.txt"
  prune "$d"
  [ ! -e "$d/bragd-old.dump" ] && [ ! -e "$d/bragd-killed.dump.part" ] && [ -e "$d/bragd-new.dump" ] && [ -e "$d/other.txt" ] || fail "pruning is wrong"

  # A stand-in pg_dump: writes its part, notes its arguments and password, and fails when asked.
  mkdir "$d/bin" "$d/out"
  cat >"$d/bin/pg_dump" <<'EOF'
#!/usr/bin/env bash
for a in "$@"; do case $a in --file=*) f=${a#--file=} ;; esac; done
echo data >"$f"
printf '%s\n' "$*" >"$(dirname "$f")/../argv"
printf '%s\n' "${PGPASSWORD:-}" >"$(dirname "$f")/../password"
[ -z "${FAIL:-}" ]
EOF
  chmod +x "$d/bin/pg_dump"
  local run=(env PATH="$d/bin:$PATH" BRAGD_BACKUP_DIR="$d/out" BRAGD_SERVER_DATABASE_URL='postgres://bragd:s%40cret@db.example:5432/postgres?sslmode=require' "$0")
  "${run[@]}" || fail "a good dump failed"
  ls "$d/out"/bragd-*.dump >/dev/null 2>&1 || fail "no dump was kept"
  grep -q 's%40cret\|s@cret' "$d/argv" && fail "the password is on pg_dump's command line"
  [ "$(cat "$d/password")" = 's@cret' ] || fail "pg_dump was not given the password"
  rm "$d/out"/bragd-*.dump
  FAIL=1 "${run[@]}" 2>/dev/null && fail "a failed dump succeeded"
  [ -z "$(ls -A "$d/out")" ] || fail "a failed dump left $(ls "$d/out")"

  # A stand-in logger too: the check writes nothing to this machine's log.
  printf '#!/bin/sh\ntouch "%s/logged"\n' "$d" >"$d/bin/logger"
  chmod +x "$d/bin/logger"
  PATH="$d/bin:$PATH" BRAGD_BACKUP_ALERT="touch '$d/alerted'" "$0" --failed
  [ -e "$d/logged" ] || fail "a failure was not logged"
  [ -e "$d/alerted" ] || fail "a failure raised no alarm"
  echo "backup.sh --check: ok"
}

case "${1:-}" in
  --check) check ;;
  --failed) failed ;;
  *) backup ;;
esac
