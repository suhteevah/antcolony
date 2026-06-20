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

echo "=== run start (self-play SP1 WARM-START validation) $(date -Is) reward=combat.toml out=bench/phase3-sp1-warmstart ==="
# SP1 self-play WARM-START run (anti-forgetting fix): the first SP1 run started
# the policy FRESH and forgot the archetype bench (0.314 final). This time:
#   --warm-start-POLICY loads the 0.874 SOTA into the TRAINING policy (starts
#     competent, refines instead of relearning),
#   --warm-start-snapshot also seeds it into the opponent pool,
#   --archetype-mix 0.6 anchors more matches to the fixed bench (keep general
#     skill in the gradient).
# 300-iter PILOT (~50min): forgetting was obvious by iter 200 last time, so this
# conclusively shows whether warm-start holds/climbs the bench before committing
# a full 1000-iter run. Distinct out dir so the first run's data is untouched.
./target/release/phase3_train \
  --iters 300 --envs 8 --rollout-cycles 96 --ant-chunk-size 8192 \
  --eval-every 50 --matches-per-eval 5 --max-grad-norm 0.5 \
  --self-play --snapshot-every 25 --pool-cap 8 \
  --opponent-sampling pfsp --archetype-mix 0.6 \
  --warm-start-policy bench/phase3-a1-combat/hac_best.safetensors \
  --warm-start-snapshot bench/phase3-a1-combat/hac_best.safetensors \
  --reward assets/reward/terminal.toml --out bench/phase3-sp1-terminal
code=$?
echo "=== run done $(date -Is) exit=$code ==="
echo "$code" > /opt/antcolony-cuda/run_selfplay.done
exit $code
