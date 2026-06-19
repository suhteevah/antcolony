#!/usr/bin/env bash
# A2 (~95M net) auto-fit + long run on cnc's 16GB P100, in ONE card-free window.
# A2 OOM'd at envs=8/cycles=96 (the rollout forward batches all ants/tick and is
# NOT chunked, so the big net's per-tick activation exceeds 16GB). This script
# frees the 16GB card once, finds the largest (envs,cycles,chunk) that survives
# a 2-iter no-eval smoke, then launches the full 250-iter run at that config.
# Services ALWAYS restored via signal-covering traps. Sentinels: run_a2.done
# (binary exit of the long run) or run_a2.nofit (no config fit).
set -uo pipefail

GPU_UUID=GPU-17bd0d20-0ddd-ad47-0db1-7857ffc89096
SERVICES="openclaw-inference-workhorse aether-vision"
DIR=/opt/antcolony-cuda
BIN=$DIR/target/release/phase3_train

restore() {
  echo "=== restoring services $(date -Is) ==="
  sudo systemctl start $SERVICES
  sleep 3
  echo "post-restore: $(systemctl is-active $SERVICES | tr '\n' ' ')"
}
trap restore EXIT
trap 'restore; exit 143' TERM
trap 'restore; exit 130' INT
trap 'restore; exit 129' HUP

echo "=== stopping services to free 16GB card $(date -Is) ==="
sudo systemctl stop $SERVICES
for i in $(seq 1 20); do
  used=$(nvidia-smi --id=$GPU_UUID --query-gpu=memory.used --format=csv,noheader,nounits 2>/dev/null | tr -d ' ')
  echo "card used=${used}MiB"; [ "${used:-99999}" -lt 1500 ] && break; sleep 2
done

export CUDA_VISIBLE_DEVICES=$GPU_UUID
_nvlibs=$(echo /opt/ml-venv/lib/python3.13/site-packages/nvidia/*/lib | tr ' ' ':')
export LD_LIBRARY_PATH=/usr/local/cuda-12.8/targets/x86_64-linux/lib:${_nvlibs}
export RAYON_NUM_THREADS=3
cd "$DIR" || exit 97

# Candidate configs, largest first: "envs cycles chunk"
CONFIGS=(
  "6 96 2048"
  "4 96 2048"
  "4 64 2048"
  "3 64 1024"
  "2 48 1024"
)

peak_sampler() {  # $1 = pid to watch; prints peak MiB
  local pid=$1 peak=0 u
  while kill -0 "$pid" 2>/dev/null; do
    u=$(nvidia-smi --id=$GPU_UUID --query-gpu=memory.used --format=csv,noheader,nounits 2>/dev/null | tr -d ' ')
    [ -n "$u" ] && [ "$u" -gt "$peak" ] && peak=$u
    sleep 1
  done
  echo "$peak"
}

FIT=""
for cfg in "${CONFIGS[@]}"; do
  read -r E C K <<<"$cfg"
  echo "=== PROBE envs=$E cycles=$C chunk=$K $(date -Is) ==="
  "$BIN" --sizing a2 --iters 2 --envs "$E" --rollout-cycles "$C" --ant-chunk-size "$K" \
    --eval-every 99 --matches-per-eval 1 --max-grad-norm 0.5 \
    --reward assets/reward/default.toml --out "$DIR/bench/phase3-a2-probe" \
    > "$DIR/probe_${E}_${C}_${K}.log" 2>&1 &
  ppid=$!
  pk=$(peak_sampler "$ppid")
  wait "$ppid"; pcode=$?
  echo "PROBE envs=$E cycles=$C chunk=$K -> exit=$pcode peak=${pk}MiB"
  if [ "$pcode" -eq 0 ]; then FIT="$cfg"; FIT_PEAK="$pk"; break; fi
done

if [ -z "$FIT" ]; then
  echo "=== NO A2 CONFIG FIT 16GB $(date -Is) ==="
  echo "nofit" > "$DIR/run_a2.nofit"
  exit 0
fi

read -r E C K <<<"$FIT"
echo "=== A2 LONG RUN fit: envs=$E cycles=$C chunk=$K (probe peak ${FIT_PEAK}MiB) $(date -Is) ==="
"$BIN" --sizing a2 --iters 250 --envs "$E" --rollout-cycles "$C" --ant-chunk-size "$K" \
  --eval-every 50 --matches-per-eval 5 --max-grad-norm 0.5 \
  --reward assets/reward/default.toml --out "$DIR/bench/phase3-a2"
code=$?
echo "=== A2 long run done $(date -Is) exit=$code (config envs=$E cycles=$C chunk=$K) ==="
echo "$code" > "$DIR/run_a2.done"
exit $code
