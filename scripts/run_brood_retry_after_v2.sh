#!/usr/bin/env bash
# Wait for the v2 sprite batch to finish, then kick off brood-clean retry.
set -e
cd "$(dirname "$0")/.."

while pgrep -f "lasius_niger_sprites.py" > /dev/null 2>&1; do
  sleep 20
done
echo "[brood-wrapper] v2 batch exited, starting brood retry in 5s..."
sleep 5
python scripts/brood_retry.py 2>&1 | tee scripts/.brood-retry.log
echo "[brood-wrapper] done"
