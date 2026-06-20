#!/usr/bin/env bash
# Head-to-head eval of two HAC checkpoints on cnc. CPU-only (Device::Cpu) — no
# GPU/card-free needed; binary linked --features cuda so cuda libs must load.
# Usage: run_h2h_cnc.sh <ckptA> <ckptB> [mpe]. Sentinel: h2h.done
set -uo pipefail
cd /opt/antcolony-cuda || exit 97
_nvlibs=$(echo /opt/ml-venv/lib/python3.13/site-packages/nvidia/*/lib | tr ' ' ':')
export LD_LIBRARY_PATH=/usr/local/cuda-12.8/targets/x86_64-linux/lib:${_nvlibs}
export RAYON_NUM_THREADS=3
A="${1:?need ckptA}"; B="${2:?need ckptB}"; MPE="${3:-50}"
echo "=== h2h start $(date -Is) A=$A B=$B mpe=$MPE ==="
./target/release/eval_h2h "$A" "$B" "$MPE"
code=$?
echo "=== h2h done $(date -Is) exit=$code ==="
echo "$code" > /opt/antcolony-cuda/h2h.done
