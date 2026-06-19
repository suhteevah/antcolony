#!/usr/bin/env bash
# Sample the 16GB card's used memory + the A2 run's latest progress a few times
# to judge whether the tight (~15.6GB peak) envs=4 run is stable or creeping.
set -u
GPU_UUID=GPU-17bd0d20-0ddd-ad47-0db1-7857ffc89096
LOG=/opt/antcolony-cuda/run_a2_autofit.log
echo "alive=$(pgrep -c phase3_train)"
for i in $(seq 1 8); do
  u=$(nvidia-smi --id=$GPU_UUID --query-gpu=memory.used,utilization.gpu --format=csv,noheader)
  echo "sample $i: $u"
  sleep 4
done
echo "--- latest progress ---"
grep -E 'phase3 iter|phase3 eval|eval vs archetype|long run done|OUT_OF_MEMORY|Error' "$LOG" | tail -8
echo "--- sentinels ---"
ls /opt/antcolony-cuda/run_a2.done /opt/antcolony-cuda/run_a2.nofit 2>/dev/null || echo "still running"
