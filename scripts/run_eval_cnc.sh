#!/usr/bin/env bash
# Honest win-rate eval of a saved A1 HAC checkpoint at mpe=50 (de-noised) on the
# eval.rs 7-archetype metric. CPU-only (Device::Cpu) — does NOT touch the GPU or
# the fleet's inference services, so no card-free/coordination needed. The bin is
# linked --features cuda though, so the cuda runtime libs must be on the path for
# it to load. RAYON capped to leave the fleet a core. Sentinel: eval.done
set -uo pipefail
cd /opt/antcolony-cuda || exit 97
_nvlibs=$(echo /opt/ml-venv/lib/python3.13/site-packages/nvidia/*/lib | tr ' ' ':')
export LD_LIBRARY_PATH=/usr/local/cuda-12.8/targets/x86_64-linux/lib:${_nvlibs}
export RAYON_NUM_THREADS=3
CKPT="${1:-bench/phase3-a1-combat/hac_best.safetensors}"
MPE="${2:-50}"
echo "=== eval start $(date -Is) ckpt=$CKPT mpe=$MPE ==="
./target/release/eval_winrate "$CKPT" "$MPE"
code=$?
echo "=== eval done $(date -Is) exit=$code ==="
echo "$code" > /opt/antcolony-cuda/eval.done
