#!/usr/bin/env bash
# Phase-3 A2 (larger net, ~95M) convergence run on cnc's 16GB P100.
# Same config as the A1 0.629 grad-clip run (r6 reward, envs=8/cycles=96/
# iters=100/grad-clip 0.5) but --sizing a2, so the comparison isolates "does
# more capacity help the combat axis?". ant-chunk-size dropped 8192->4096 for
# memory safety (chunking is gradient-identical, so this does NOT change the
# result, only peak per-forward memory). Frees the 16GB card by stopping
# workhorse + aether-vision, ALWAYS restores them via an EXIT trap.
set -uo pipefail

GPU_UUID=GPU-17bd0d20-0ddd-ad47-0db1-7857ffc89096
SERVICES="openclaw-inference-workhorse aether-vision"

restore() {
  echo "=== restoring services $(date -Is) ==="
  sudo systemctl start $SERVICES
  sleep 3
  echo "post-restore state: $(systemctl is-active $SERVICES | tr '\n' ' ')"
  echo "=== restore done $(date -Is) ==="
}
# Cover signal kills too (bash skips the EXIT trap on an unhandled SIGTERM/INT/
# HUP) so a killed trainer or a dropped nohup session still restores services.
# restore() is idempotent (systemctl start on a running unit is a no-op).
trap restore EXIT
trap 'restore; exit 143' TERM
trap 'restore; exit 130' INT
trap 'restore; exit 129' HUP

echo "=== pre-stop state: $(systemctl is-active $SERVICES | tr '\n' ' ') ==="
echo "=== stopping services to free 16GB card $(date -Is) ==="
sudo systemctl stop $SERVICES
for i in $(seq 1 20); do
  used=$(nvidia-smi --id=$GPU_UUID --query-gpu=memory.used --format=csv,noheader,nounits 2>/dev/null | tr -d ' ')
  echo "card used=${used}MiB (try $i)"
  [ "${used:-99999}" -lt 1500 ] && break
  sleep 2
done

export CUDA_VISIBLE_DEVICES=$GPU_UUID
_nvlibs=$(echo /opt/ml-venv/lib/python3.13/site-packages/nvidia/*/lib | tr ' ' ':')
export LD_LIBRARY_PATH=/usr/local/cuda-12.8/targets/x86_64-linux/lib:${_nvlibs}
export RAYON_NUM_THREADS=3
cd /opt/antcolony-cuda || exit 97

echo "=== A2 run start $(date -Is) sizing=a2 reward=default.toml out=bench/phase3-a2 ==="
# Longer training budget than A1's 100 iters: A2 (~95M) needs more to converge.
# eval protocol (mpe=5) kept identical to the A1 run for apples-to-apples.
./target/release/phase3_train \
  --sizing a2 --iters 250 --envs 8 --rollout-cycles 96 --ant-chunk-size 4096 \
  --eval-every 50 --matches-per-eval 5 --max-grad-norm 0.5 \
  --reward assets/reward/default.toml --out bench/phase3-a2
code=$?
echo "=== A2 run done $(date -Is) exit=$code ==="
echo "$code" > /opt/antcolony-cuda/run_a2.done
exit $code
