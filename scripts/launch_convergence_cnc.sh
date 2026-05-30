#!/usr/bin/env bash
# Launch the A1 full-horizon convergence run on cnc's freed 16GB P100.
# Pinned to that card by UUID; RAYON capped at 3 of cnc's 4 cores so the
# fleet keeps one. Longer horizon (rollout-cycles 96 = 480 ticks, 3x the
# failed run's 160) so the policy experiences combat + terminal reward;
# ant-chunk-size bounds the update memory that OOM'd the 8GB card.
# Writes exit code to run.done as a completion sentinel.
set -uo pipefail
export CUDA_VISIBLE_DEVICES=GPU-17bd0d20-0ddd-ad47-0db1-7857ffc89096
_nvlibs=$(echo /opt/ml-venv/lib/python3.13/site-packages/nvidia/*/lib | tr ' ' ':')
export LD_LIBRARY_PATH=/usr/local/cuda-12.8/targets/x86_64-linux/lib:${_nvlibs}
export RAYON_NUM_THREADS=3
cd /opt/antcolony-cuda || exit 97

echo "=== convergence run start $(date -Is) ==="
./target/release/phase3_train \
  --iters 100 --envs 8 --rollout-cycles 96 --ant-chunk-size 8192 \
  --eval-every 25 --matches-per-eval 5 \
  --reward assets/reward/default.toml --out bench/phase3-a1-fullhorizon
code=$?
echo "=== convergence run done $(date -Is) exit=$code ==="
echo "$code" > /opt/antcolony-cuda/run.done
