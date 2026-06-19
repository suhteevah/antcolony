#!/usr/bin/env bash
# Poll for the grad-clip run's completion sentinel, then dump the win-rate
# curve + restore tail. Standalone (avoids PowerShell->ssh quoting hazards).
set -u
LOG=/opt/antcolony-cuda/run.log
DONE=/opt/antcolony-cuda/run.done
for i in $(seq 1 150); do
  if [ -f "$DONE" ]; then
    echo "RUN_DONE_CODE=$(cat "$DONE")"
    echo "=== per-archetype evals ==="
    grep -E 'eval vs archetype|new best checkpoint|phase3 best|mean_win_rate' "$LOG" | tail -80
    echo "=== run tail ==="
    tail -8 "$LOG"
    exit 0
  fi
  sleep 30
done
echo "TIMEOUT"
tail -20 "$LOG"
