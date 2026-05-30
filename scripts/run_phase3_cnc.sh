#!/usr/bin/env bash
# Run phase3_train on cnc pinned to the freed 16GB P100 (by UUID, so it can't
# accidentally land on the 12GB card that still hosts the 14b workhorse).
# All args pass through to the binary, e.g.:
#   bash run_phase3_cnc.sh --iters 2 --envs 4 --rollout-cycles 8 \
#       --eval-every 1 --matches-per-eval 1 --ant-chunk-size 4096 --out bench/smoke
set -uo pipefail
export CUDA_VISIBLE_DEVICES=GPU-17bd0d20-0ddd-ad47-0db1-7857ffc89096
# CUDA runtime libs are split: cudart/cublas under the toolkit, but nvrtc +
# curand ship only via the pip nvidia packages under /opt/ml-venv. Put both
# on the runtime linker path.
_nvlibs=$(echo /opt/ml-venv/lib/python3.13/site-packages/nvidia/*/lib | tr ' ' ':')
export LD_LIBRARY_PATH=/usr/local/cuda-12.8/targets/x86_64-linux/lib:${_nvlibs}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
cd /opt/antcolony-cuda || exit 97
exec ./target/release/phase3_train "$@"
