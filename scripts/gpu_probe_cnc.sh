#!/usr/bin/env bash
# Ground-truth probe: map each GPU (index+UUID) to the compute processes on it
# and to the systemd unit that owns each PID. Read-only.
set -u
echo "--- GPU UUIDs by index ---"
nvidia-smi --query-gpu=index,name,uuid,memory.used,memory.total --format=csv,noheader
echo "--- compute procs (gpu_uuid, pid, mem, name) ---"
nvidia-smi --query-compute-apps=gpu_uuid,pid,used_memory,process_name --format=csv,noheader
echo "--- PID -> systemd unit ---"
for p in $(nvidia-smi --query-compute-apps=pid --format=csv,noheader); do
  unit=$(ps -o unit= -p "$p" 2>/dev/null | tr -d ' ')
  [ -z "$unit" ] && unit=$(cat /proc/"$p"/cgroup 2>/dev/null | head -1)
  echo "PID $p -> ${unit:-unknown}"
done
