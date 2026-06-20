#!/usr/bin/env bash
set -u
LOG=/opt/antcolony-cuda/run_selfplay.log
echo "now: $(date -Is)"
echo "started: $(head -2 "$LOG" | grep -oE '[0-9T:+-]{20,}' | head -1)"
echo "done? $(cat /opt/antcolony-cuda/run_selfplay.done 2>/dev/null || echo 'still running')"
echo "alive: $(pgrep -c phase3_train)"
pid=$(pgrep phase3_train | head -1)
[ -n "$pid" ] && echo "elapsed: $(ps -o etime= -p "$pid" | tr -d ' ')"
echo "latest iter: $(grep -aoE 'phase3 iter . iter=[0-9]+' "$LOG" 2>/dev/null | tail -1)"
echo "snapshots saved: $(grep -ac 'snapshot saved' "$LOG")"
echo "--- 7-archetype eval curve so far (iter, mean) ---"
sed -r 's/\x1b\[[0-9;]*m//g' "$LOG" | grep -aoE 'phase3 eval . iter=[0-9]+ mean_win_rate=[0-9.]+' || echo '(no eval completed yet)'
echo "--- self-play health (winrate vs pool) ---"
sed -r 's/\x1b\[[0-9;]*m//g' "$LOG" | grep -aoE 'self_play_winrate=[0-9.]+' | tail -3 || echo '(not yet)'
echo "--- GPU 16GB ---"
nvidia-smi --id=GPU-17bd0d20-0ddd-ad47-0db1-7857ffc89096 --query-gpu=memory.used,utilization.gpu --format=csv,noheader
echo "--- nightdrive next-fire ---"
systemctl list-timers --all --no-pager 2>/dev/null | grep -i nightdrive | head -2
