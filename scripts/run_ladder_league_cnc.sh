#!/usr/bin/env bash
# scripts/run_ladder_league_cnc.sh
# Ladder League on cnc. Training uses a P100 (CUDA build); the sim is the CPU
# bottleneck so RAYON fills the cores. This is a GPU-training run, so it FREES a
# P100 (stops the resident inference fleet) and pins the freed card — an
# EXIT/TERM/INT/HUP trap guarantees the services come back even on a dropped
# session or a panic. restore() is idempotent.
#
# ⚠ PROBE THE CARD MAPPING LIVE before trusting the UUID pin (it has flipped
# between sessions): scripts/gpu_probe_cnc.sh. As of 2026-06-22 the 16GB P100 is
# UUID GPU-17bd0d20-0ddd-ad47-0db1-7857ffc89096 and hosts the workhorse.
#
# Coordinate the window via openclaw main first. Overnight / full-access slot:
# RAYON_NUM_THREADS defaults to all cores. Daytime: prefer nproc-1.
set -uo pipefail

# Inference + aether services to stop so the GPU + cores are free. The 16GB card
# hosts the workhorse; the 12GB hosts scout+embed. Free all for a clean run.
SERVICES="openclaw-inference-workhorse openclaw-inference-scout openclaw-inference-embed aether-vision aether-serve"

restore() {
  echo "=== restoring services $(date -Is) ==="
  sudo systemctl start $SERVICES 2>/dev/null || true
  sleep 3
  echo "post-restore: $(systemctl is-active $SERVICES 2>/dev/null | tr '\n' ' ')"
  echo "=== restore done $(date -Is) ==="
}
trap restore EXIT
trap 'restore; exit 143' TERM
trap 'restore; exit 130' INT
trap 'restore; exit 129' HUP

echo "=== pre-stop: $(systemctl is-active $SERVICES 2>/dev/null | tr '\n' ' ') ==="
echo "=== stopping fleet to free the P100 + cores $(date -Is) ==="
sudo systemctl stop $SERVICES 2>/dev/null || true
sleep 3
echo "=== post-stop: $(systemctl is-active $SERVICES 2>/dev/null | tr '\n' ' ') ==="
nvidia-smi --query-gpu=index,name,memory.used,memory.total --format=csv,noheader || true

# Pin the freed 16GB P100 by UUID (probe live to confirm). CUDA runtime libs:
# libcudart/cublas in the toolkit, libnvrtc/libcurand only via pip nvidia pkgs.
export CUDA_VISIBLE_DEVICES="${CUDA_VISIBLE_DEVICES:-GPU-17bd0d20-0ddd-ad47-0db1-7857ffc89096}"
export RAYON_NUM_THREADS="${RAYON_NUM_THREADS:-$(nproc)}"
export LD_LIBRARY_PATH="/usr/local/cuda-12.8/targets/x86_64-linux/lib:$(echo /opt/ml-venv/lib/python3.13/site-packages/nvidia/*/lib | tr ' ' ':')"

cd /opt/antcolony-cuda || exit 97
echo "=== ladder_league start $(date -Is) RAYON=${RAYON_NUM_THREADS} CVD=${CUDA_VISIBLE_DEVICES} ==="
./target/release/ladder_league \
  --sota bench/phase3-a1-combat/hac_best.safetensors \
  --contender sp1term=hac:bench/phase3-sp1-terminal/hac_best.safetensors \
  --contender sp1=hac:bench/phase3-sp1/hac_best.safetensors \
  --contender gradclip=hac:bench/phase3-a1-gradclip/hac_best.safetensors \
  --contender sp2=hac:bench/phase3-sp2/league_best.safetensors \
  --reward assets/reward/terminal.toml \
  --iters-per-round 150 --gate-mpe 50 --gate-margin 0.55 \
  --keepbest-arch-floor 0.70 --archetype-mix 0.30 \
  --no-improve-stop 2 --max-rounds 8 --out bench/ladder-league
code=$?
echo "=== ladder_league done $(date -Is) exit=$code ==="
echo "$code" > /opt/antcolony-cuda/run_ladder_league.done
exit $code
