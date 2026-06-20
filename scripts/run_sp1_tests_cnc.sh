#!/usr/bin/env bash
# Venue-compliant verification of the SP1 self-play branch: run the full
# antcolony-trainer test suite (incl. the ~609s phase3_smoke + the new self-play
# tests + the OFF-path determinism guard) on cnc, NOT on kokonoe. Tests use
# Device::Cpu, so no GPU is needed — but the binary is linked --features cuda,
# so the cuda runtime libs must be on the path to load. Sentinel: sp1_tests.done
set -uo pipefail
cd /opt/antcolony-cuda || exit 97
export PATH=/usr/local/cuda-12.8/bin:$PATH
export NVCC_PREPEND_FLAGS="-ccbin g++-13"
export CUDA_COMPUTE_CAP=60
_nvlibs=$(echo /opt/ml-venv/lib/python3.13/site-packages/nvidia/*/lib | tr ' ' ':')
export LD_LIBRARY_PATH=/usr/local/cuda-12.8/targets/x86_64-linux/lib:${_nvlibs}
export RAYON_NUM_THREADS=3
echo "=== sp1 tests start $(date -Is) ==="
cargo test --release -p antcolony-trainer --features cuda 2>&1 | tail -50
code=${PIPESTATUS[0]}
echo "=== sp1 tests done $(date -Is) exit=$code ==="
echo "$code" > /opt/antcolony-cuda/sp1_tests.done
