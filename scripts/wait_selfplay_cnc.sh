#!/usr/bin/env bash
# Block until the ~2h SP1 self-play run finishes (run_selfplay.done), then dump
# the win-rate curve + self-play health + restore tail. 2.5h poll budget.
set -u
LOG=/opt/antcolony-cuda/run_selfplay.log
DONE=/opt/antcolony-cuda/run_selfplay.done
for i in $(seq 1 300); do
  if [ -f "$DONE" ]; then
    echo "SELFPLAY_DONE_CODE=$(cat "$DONE")"
    echo "=== curve / evals / self-play health ==="
    grep -E 'phase3 eval|self.play|self_play|new best|phase3 best|FINAL vs|final per-archetype|curve |snapshot saved' "$LOG" | tail -90
    echo "=== restore tail ==="
    tail -8 "$LOG"
    exit 0
  fi
  sleep 30
done
echo "TIMEOUT"; tail -20 "$LOG"
