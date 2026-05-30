#!/usr/bin/env bash
# Probe peak GPU memory for a candidate (envs, cycles) on the 16GB P100.
# Runs 1 iter, no periodic eval, samples GPU1 memory until iter 0 completes
# or OOMs, then kills the proc (skips the slow final eval).
# Usage: probe_mem_cnc.sh ENVS CYCLES [CHUNK]
set -uo pipefail
ENVS=$1; CYCLES=$2; CHUNK=${3:-8192}
export CUDA_VISIBLE_DEVICES=GPU-17bd0d20-0ddd-ad47-0db1-7857ffc89096
_nvlibs=$(echo /opt/ml-venv/lib/python3.13/site-packages/nvidia/*/lib | tr ' ' ':')
export LD_LIBRARY_PATH=/usr/local/cuda-12.8/targets/x86_64-linux/lib:${_nvlibs}
export RAYON_NUM_THREADS=3
cd /opt/antcolony-cuda || exit 97
rm -f probe.log

./target/release/phase3_train --iters 1 --envs "$ENVS" --rollout-cycles "$CYCLES" \
  --ant-chunk-size "$CHUNK" --eval-every 0 --matches-per-eval 1 \
  --out bench/probe > probe.log 2>&1 &
PID=$!

peak=0
result=RUNNING
for _ in $(seq 1 90); do
  kill -0 "$PID" 2>/dev/null || { result=EXITED; break; }
  used=$(nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits -i 1 2>/dev/null | tr -d ' ')
  if [ -n "$used" ] && [ "$used" -gt "$peak" ] 2>/dev/null; then peak=$used; fi
  if grep -q "phase3 iter" probe.log 2>/dev/null; then result=ITER0_OK; break; fi
  if grep -qi "out of memory" probe.log 2>/dev/null; then result=OOM; break; fi
  sleep 2
done

echo "RESULT=$result ENVS=$ENVS CYCLES=$CYCLES CHUNK=$CHUNK PEAK_MIB=$peak"
grep -iE "phase3 iter|out of memory|error" probe.log | head -3
kill "$PID" 2>/dev/null
pkill -f 'phase3_train --iters 1' 2>/dev/null
exit 0
