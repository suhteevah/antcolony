#!/usr/bin/env bash
# Phase-3 SP2 exploiter-league PILOT run on cnc's 16GB P100 (UUID 17bd0d20).
# Warm-starts all league agents from bench/phase3-a1-combat/hac_best.safetensors
# (0.874 combat keeper), uses terminal.toml reward, and runs a 15-step pilot
# to validate the league machinery before committing a full 40-step run.
# Frees the card by stopping workhorse + aether-vision, runs phase3_league,
# then ALWAYS restarts the services via an EXIT trap. Detach-safe: restoration
# does not depend on the launching session. Writes run_league.done with exit code.
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

echo "=== run start (SP2 exploiter-league 15-step PILOT) $(date -Is) reward=terminal.toml out=bench/phase3-sp2 ==="
# SP2 exploiter-league PILOT (15 steps). Validates the LeagueManager machinery:
#   --sota warm-starts all agents from the 0.874 SOTA (hac_best.safetensors);
#   --rollout-cycles 96 --ant-chunk-size 8192 proven SP1 values — each rollout
#     spans a full match so the policy experiences terminal reward without OOMing;
#   --eval-every-steps 5 triggers h2h vs SOTA + archetype bench every 5 steps
#     to catch forgetting early; success = league_best.safetensors > 0.5 h2h.
# If pilot passes → coordinate 40-step run via openclaw main + nightdrive-clear.
./target/release/phase3_league \
  --league-steps 15 --iters-main 25 --iters-exploiter 15 \
  --n-main-exploiters 1 --n-league-exploiters 1 --pool-cap 16 \
  --promote-winrate 0.70 --exploiter-max-iters 100 --main-snapshot-every 2 \
  --archetype-mix 0.5 --eval-every-steps 5 --success-mpe 20 \
  --rollout-cycles 96 --ant-chunk-size 8192 --max-grad-norm 0.5 \
  --sota bench/phase3-a1-combat/hac_best.safetensors \
  --reward assets/reward/terminal.toml --out bench/phase3-sp2
code=$?
echo "=== run done $(date -Is) exit=$code ==="
echo "$code" > /opt/antcolony-cuda/run_league.done
exit $code
