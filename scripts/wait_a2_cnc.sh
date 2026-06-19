#!/usr/bin/env bash
# Block until the A2 long run finishes (run_a2.done) or no-fit (run_a2.nofit),
# then dump the win-rate curve + restore tail. Standalone (no PS->ssh quoting).
set -u
LOG=/opt/antcolony-cuda/run_a2_autofit.log
for i in $(seq 1 200); do
  if [ -f /opt/antcolony-cuda/run_a2.done ]; then
    echo "A2_RUN_DONE_CODE=$(cat /opt/antcolony-cuda/run_a2.done)"
    echo "=== per-archetype evals + curve ==="
    grep -E 'eval vs archetype|phase3 eval|new best|phase3 best|FINAL vs|final per-archetype|curve |fit:' "$LOG" | tail -90
    echo "=== tail ==="
    tail -6 "$LOG"
    exit 0
  fi
  if [ -f /opt/antcolony-cuda/run_a2.nofit ]; then
    echo "A2_NOFIT"; grep -E 'PROBE|peak' "$LOG" | tail -10; exit 0
  fi
  sleep 30
done
echo "TIMEOUT"; tail -20 "$LOG"
