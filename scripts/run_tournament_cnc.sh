#!/usr/bin/env bash
# CPU-bound PvP round-robin tournament on cnc (16GB P100 UUID 17bd0d20).
#
# Rationale for full-fleet kick: the tournament is CPU-sim-bound (no HAC
# gradient — inference only), so all 48 cnc cores are more valuable than
# the GPU. Stopping every inference service frees RAM and eliminates
# scheduling contention so rayon fills all cores cleanly.
#
# Build (CPU, no --features cuda):
#   cargo build --release -p antcolony-trainer --bin tournament
#
# Checkpoint-presence prerequisite:
#   Pull sp1-terminal and sp2 checkpoints from /opt/antcolony-archive/ first:
#     cp /opt/antcolony-archive/phase3-sp1-terminal/hac_best.safetensors \
#        bench/phase3-sp1-terminal/hac_best.safetensors
#     cp /opt/antcolony-archive/phase3-sp2/league_best.safetensors \
#        bench/phase3-sp2/league_best.safetensors
#   Verify all contender paths exist before running.
#
# Service restore: EXIT/TERM/INT/HUP trap guarantees restoration even if the
# session is dropped or the binary panics. restore() is idempotent.

set -uo pipefail

SERVICES="openclaw-inference-workhorse openclaw-inference-scout openclaw-inference-embed aether-vision aether-serve"

restore() {
  echo "=== restoring services $(date -Is) ==="
  sudo systemctl start $SERVICES
  sleep 3
  echo "post-restore state: $(systemctl is-active $SERVICES | tr '\n' ' ')"
  echo "=== restore done $(date -Is) ==="
}
# Cover signal kills too (bash skips the EXIT trap on an unhandled SIGTERM/INT/
# HUP) so a killed runner or dropped session still restores services.
# restore() is idempotent (systemctl start on a running unit no-ops).
trap restore EXIT
trap 'restore; exit 143' TERM
trap 'restore; exit 130' INT
trap 'restore; exit 129' HUP

echo "=== pre-stop state: $(systemctl is-active $SERVICES | tr '\n' ' ') ==="
echo "=== stopping full fleet to free cores for CPU-bound tournament $(date -Is) ==="
sudo systemctl stop $SERVICES
sleep 3   # let the inference processes release CPU + RAM
echo "=== post-stop state: $(systemctl is-active $SERVICES | tr '\n' ' ') ==="

# CPU-only tournament: no GPU pin, no CUDA libs (bin built without --features cuda).
export RAYON_NUM_THREADS=$(nproc)
cd /opt/antcolony-cuda || exit 97

echo "=== tournament start $(date -Is) RAYON_NUM_THREADS=${RAYON_NUM_THREADS} out=bench/tournament ==="
./target/release/tournament \
  --contenders sota=hac:bench/phase3-a1-combat/hac_best.safetensors,v1=mlp:bench/iterative-fsp/round_1/mlp_weights_v1.json,sp1=hac:bench/phase3-sp1/hac_best.safetensors,sp1term=hac:bench/phase3-sp1-terminal/hac_best.safetensors,sp2=hac:bench/phase3-sp2/league_best.safetensors,gradclip=hac:bench/phase3-a1-gradclip/hac_best.safetensors \
  --add-archetypes --mpe 15 --anchor v1 --out bench/tournament
code=$?
echo "=== tournament done $(date -Is) exit=$code ==="
echo "$code" > /opt/antcolony-cuda/run_tournament.done
exit $code
