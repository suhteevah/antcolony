#!/usr/bin/env bash
# Wait for the sprite batch to finish, then kick off queen retry.
# Detects "done" by waiting for the process to exit and last log line to contain
# "corpse" (last sprite in the batch).
set -e
cd "$(dirname "$0")/.."

LOG=scripts/.sprites-run.log
while pgrep -f "lasius_niger_sprites.py" > /dev/null 2>&1; do
  sleep 20
done
echo "[retry-wrapper] main batch exited, starting queen retry in 5s..."
sleep 5
python scripts/queen_retry.py 2>&1 | tee scripts/.queen-retry.log
echo "[retry-wrapper] done"
