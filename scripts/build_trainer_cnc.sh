#!/usr/bin/env bash
# Build antcolony-trainer (+ sim) with CUDA on cnc-server's P100s.
# nvcc 12.8 rejects cnc's default gcc 15, so pin the host compiler to g++-13.
# P100 = compute capability 6.0 (sm_60). Run detached via nohup; writes the
# exit code to build.done as a completion sentinel.
set -uo pipefail
cd /opt/antcolony-cuda || exit 97

export PATH=/usr/local/cuda-12.8/bin:$PATH
export NVCC_PREPEND_FLAGS="-ccbin g++-13"
export CUDA_COMPUTE_CAP=60

echo "=== build start $(date -Is) ==="
echo "nvcc: $(nvcc --version | tail -1)"
echo "rustc: $(rustc --version)"
echo "ccbin: $(g++-13 --version | head -1)"
echo "===================================="

cargo build --release -p antcolony-trainer --features cuda -j2
code=$?

echo "=== build done $(date -Is) exit=$code ==="
echo "$code" > /opt/antcolony-cuda/build.done
