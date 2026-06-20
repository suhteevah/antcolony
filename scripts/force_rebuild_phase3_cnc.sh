#!/usr/bin/env bash
# Force a clean rebuild of phase3_train on cnc — the binary went stale on the
# --warm-start-policy addition (cargo hardlink-freshness trap: target/release
# bin hardlinked to deps/, mtime "immortal"). Remove both, rebuild, verify.
set -u
cd /opt/antcolony-cuda || exit 97
echo "before: warm-start-snapshot=$(strings target/release/phase3_train | grep -c warm-start-snapshot) warm-start-policy=$(strings target/release/phase3_train | grep -c warm-start-policy)"
ls -la --time-style=+%H:%M:%S target/release/phase3_train
rm -f target/release/phase3_train target/release/deps/phase3_train-*
export PATH=/usr/local/cuda-12.8/bin:$PATH
export CUDA_COMPUTE_CAP=60
echo "--- rebuilding ---"
cargo build --release -p antcolony-trainer --features cuda --bin phase3_train -j2 2>&1 | tail -3
echo "after: warm-start-policy=$(strings target/release/phase3_train | grep -c warm-start-policy)"
ls -la --time-style=+%H:%M:%S target/release/phase3_train
