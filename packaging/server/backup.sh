#!/usr/bin/env bash
# Back up the Bragd server's Postgres into BRAGD_BACKUP_DIR (the Storage Box, mounted) and delete
# backups older than 30 days. Run it daily from a timer; `--check` tests the pruning on throwaway files.
#
#   BRAGD_SERVER_DATABASE_URL=postgres://… BRAGD_BACKUP_DIR=/mnt/storagebox/bragd packaging/server/backup.sh
set -euo pipefail
KEEP_DAYS=30

prune() { find "$1" -maxdepth 1 -name 'bragd-*.dump' -type f -mtime "+$KEEP_DAYS" -delete; }

if [ "${1:-}" = --check ]; then
  d=$(mktemp -d); trap 'rm -rf "$d"' EXIT
  touch -d '31 days ago' "$d/bragd-old.dump"
  touch -d '29 days ago' "$d/bragd-new.dump"
  touch -d '40 days ago' "$d/other.txt"
  prune "$d"
  [ ! -e "$d/bragd-old.dump" ] && [ -e "$d/bragd-new.dump" ] && [ -e "$d/other.txt" ] || { echo "backup.sh --check: pruning is wrong" >&2; exit 1; }
  echo "backup.sh --check: ok"
  exit 0
fi

: "${BRAGD_SERVER_DATABASE_URL:?}" "${BRAGD_BACKUP_DIR:?}"
out="$BRAGD_BACKUP_DIR/bragd-$(date -u +%Y%m%dT%H%M%SZ).dump"
pg_dump --format=custom --file="$out.part" "$BRAGD_SERVER_DATABASE_URL"
mv "$out.part" "$out"
prune "$BRAGD_BACKUP_DIR"
