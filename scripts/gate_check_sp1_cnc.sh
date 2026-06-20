#!/usr/bin/env bash
# Pre-flight gates for the overnight SP1 1000-iter run. Read-only.
set -u
echo "=== now ==="; date -Is
echo "=== nightdrive eviction timers (the real inference-evictor) ==="
systemctl list-timers --all --no-pager 2>/dev/null | grep -iE 'nightdrive|NEXT' | grep -iE 'nightdrive|NEXT'
echo "--- any nightdrive unit next-fire ---"
systemctl list-timers --all --no-pager 2>/dev/null | grep -i nightdrive || echo "no nightdrive timers active"
echo "=== GPU mapping (verify 16GB UUID 17bd0d20 = workhorse, not flipped) ==="
nvidia-smi --query-gpu=index,name,uuid,memory.used,memory.total --format=csv,noheader
echo "--- compute procs -> systemd unit ---"
nvidia-smi --query-compute-apps=gpu_uuid,pid,used_memory --format=csv,noheader | while IFS=, read -r uuid pid mem; do
  pid=$(echo "$pid" | tr -d ' ')
  unit=$(ps -o unit= -p "$pid" 2>/dev/null | tr -d ' ')
  echo "${uuid} | PID ${pid} (${mem# }) -> ${unit:-?}"
done
echo "=== warm-start keeper on cnc bench/? ==="
ls -la /opt/antcolony-cuda/bench/phase3-a1-combat/hac_best.safetensors 2>/dev/null || echo "MISSING from cnc bench/ -> restore from archive"
echo "=== archive copy (for restore if needed) ==="
ls -la /opt/antcolony-archive/checkpoints/phase3-a1-combat/hac_best.safetensors 2>/dev/null || echo "no archive copy!"
echo "=== phase3_train binary present? (needs --features cuda) ==="
ls -la /opt/antcolony-cuda/target/release/phase3_train 2>/dev/null || echo "no phase3_train binary - must build"
