# scripts/run_ladder_league_cnc.sh
# Ladder League on cnc. Training uses the P100 (CUDA build); the sim is the
# CPU bottleneck so RAYON fills the cores. Telegram ping is emitted by the
# binary's stdout being watched by the caller, or add notify here per-round.
set -uo pipefail

# Coordinate the window via openclaw main first. CPU-contention shape: prefer
# RAYON_NUM_THREADS=$(( $(nproc) - 1 )) on a daytime window to protect inference.
export RAYON_NUM_THREADS="${RAYON_NUM_THREADS:-$(nproc)}"
export CUDA_VISIBLE_DEVICES="${CUDA_VISIBLE_DEVICES:-GPU-17bd0d20-0000-0000-0000-000000000000}" # 16GB P100; PROBE LIVE first
# Split CUDA runtime libs (libnvrtc/libcurand ship via pip nvidia packages).
export LD_LIBRARY_PATH="/usr/local/cuda-12.8/targets/x86_64-linux/lib:$(echo /opt/ml-venv/lib/python3.13/site-packages/nvidia/*/lib | tr ' ' ':')"

cd /opt/antcolony-cuda || exit 97
echo "=== ladder_league start $(date -Is) RAYON=${RAYON_NUM_THREADS} ==="
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
