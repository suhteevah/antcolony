#!/usr/bin/env bash
# Phase-3 grad-clip convergence run on cnc's 16GB P100 (UUID 17bd0d20).
# Frees the card by stopping workhorse + aether-vision, runs the A1 init-fix
# config (same as the 0.629 keeper) WITH grad-norm clipping 0.5, then ALWAYS
# restarts the services via an EXIT trap. Detach-safe: restoration does not
# depend on the launching session. Writes run.done with the binary exit code.
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
trap restore EXIT

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

echo "=== run start $(date -Is) reward=default.toml out=bench/phase3-a1-gradclip ==="
./target/release/phase3_train \
  --iters 100 --envs 8 --rollout-cycles 96 --ant-chunk-size 8192 \
  --eval-every 25 --matches-per-eval 5 --max-grad-norm 0.5 \
  --reward assets/reward/default.toml --out bench/phase3-a1-gradclip
code=$?
echo "=== run done $(date -Is) exit=$code ==="
echo "$code" > /opt/antcolony-cuda/run.done
exit $code
