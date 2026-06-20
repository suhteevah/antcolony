#!/usr/bin/env bash
set -u
LOG=/opt/antcolony-cuda/eval_combat.log
echo "now: $(date -Is)"
echo "started: $(head -1 "$LOG" | grep -oE '[0-9T:+-]{20,}')"
echo "alive: $(pgrep -c eval_winrate)"
echo "done sentinel: $(cat /opt/antcolony-cuda/eval.done 2>/dev/null || echo 'not yet')"
pid=$(pgrep eval_winrate | head -1)
[ -n "$pid" ] && echo "elapsed/cpu: $(ps -o etime=,%cpu= -p "$pid")"
echo "--- archetypes scored so far (ANSI-stripped) ---"
sed -r 's/\x1b\[[0-9;]*m//g' "$LOG" | grep -aoE "archetype=.[a-z]+. win_rate=[0-9.]+ played=[0-9]+" || echo "(none finished yet)"
echo "raw eval-line count: $(grep -ac 'eval vs archetype' "$LOG")"
