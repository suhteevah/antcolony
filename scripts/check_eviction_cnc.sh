#!/usr/bin/env bash
# Verify there is no inference-eviction timer/script that would bounce the
# inference services during a ~90min training window. Read-only.
set -u
echo "=== openclaw-briefing exec lines ==="
systemctl cat openclaw-briefing.service 2>/dev/null | grep -iE 'ExecStart' | head
echo "=== briefing timer (if any) ==="
systemctl list-timers --all --no-pager 2>/dev/null | grep -i brief || echo "no briefing timer"
echo "=== ANY unit/script that stops|restarts inference (broad grep) ==="
sudo grep -rilE 'systemctl (stop|restart).*(inference|workhorse|aether-vision|scout|embed)' \
  /etc/systemd /opt /usr/local/bin /root 2>/dev/null | head
echo "(nothing listed above = none found)"
echo "=== timers next-fire in the next ~2h ==="
systemctl list-timers --all --no-pager 2>/dev/null | grep -E '2026-06-19 0[789]:|2026-06-19 1[01]:' | head -20
echo "=== now ==="
date -Is
