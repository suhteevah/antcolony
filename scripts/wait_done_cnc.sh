#!/usr/bin/env bash
# Generic completion waiter. $1 = .done sentinel path, $2 = run log path.
# Blocks until the sentinel appears, then dumps the curve + tail. Standalone.
set -u
DONE="${1:?need done-file}"
LOG="${2:?need log-file}"
for i in $(seq 1 200); do
  if [ -f "$DONE" ]; then
    echo "RUN_DONE_CODE=$(cat "$DONE")"
    echo "=== curve + per-archetype + best ==="
    grep -E 'phase3 eval|eval vs archetype|new best|phase3 best|FINAL vs|final per-archetype|curve |BEST checkpoint' "$LOG" | tail -80
    echo "=== tail ==="
    tail -6 "$LOG"
    exit 0
  fi
  sleep 30
done
echo "TIMEOUT"; tail -20 "$LOG"
