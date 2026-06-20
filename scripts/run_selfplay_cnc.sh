#!/usr/bin/env bash
# Phase-3 SP1 self-play run on cnc's 16GB P100 (UUID 17bd0d20).
# Warm-starts from bench/phase3-a1-combat/hac_best.safetensors (0.874 combat
# keeper), uses combat.toml reward, and enables SP1 self-play with PFSP
# opponent sampling (pool-cap 8, snapshot every 25 iters).
# Frees the card by stopping workhorse + aether-vision, runs phase3_train,
# then ALWAYS restarts the services via an EXIT trap. Detach-safe: restoration
# does not depend on the launching session. Writes run.done with exit code.
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
# HUP) so a killed trainer or dropped session still restores services over the
# ~2h window. restore() is idempotent (systemctl start on a running unit no-ops).
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

echo "=== run start (self-play SP1) $(date -Is) reward=combat.toml out=bench/phase3-sp1 ==="
# SP1 self-play run: warm-starts from combat-reward best checkpoint, enables
# self-play with PFSP opponent sampling (archetype_mix=0.5, pool-cap=8,
# snapshot every 25 iters). Same iters/envs/rollout-cycles/eval cadence as
# run_combat_cnc.sh so comparisons isolate the self-play lever only.
./target/release/phase3_train \
  --iters 1000 --envs 8 --rollout-cycles 96 --ant-chunk-size 8192 \
  --eval-every 100 --matches-per-eval 5 --max-grad-norm 0.5 \
  --self-play --snapshot-every 25 --pool-cap 8 \
  --opponent-sampling pfsp --archetype-mix 0.5 \
  --warm-start-snapshot bench/phase3-a1-combat/hac_best.safetensors \
  --reward assets/reward/combat.toml --out bench/phase3-sp1
code=$?
echo "=== run done $(date -Is) exit=$code ==="
echo "$code" > /opt/antcolony-cuda/run_selfplay.done
exit $code
