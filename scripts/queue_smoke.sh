#!/usr/bin/env bash
# Queue manager for Phase 1 2yr smoke on cnc.
# Runs ALL species 2-at-a-time, in process order, on this host only.
# Designed to be launched under nohup and survive ssh disconnects.
#
# Uses `wait -n` (bash >= 4.3) for a robust dispatch loop — no
# associative-array juggling or empty-array edge cases.
#
# State files in $OUTROOT/_logs/:
#   queue.pid        — this script's PID
#   queue.log        — script's own activity log
#   queue_pids.txt   — flat "<pid> <species>" lines as launched
#   queue.done       — touched when all species finish

set -o pipefail

OUTROOT=/opt/antcolony/runs/phase1-2yr
BIN=/opt/antcolony/target/release/examples/smoke_10yr_ai
YEARS=2
SEED=42
MAX_CONCURRENT=2

SPECIES=(
  lasius_niger
  pogonomyrmex_occidentalis
  formica_rufa
  camponotus_pennsylvanicus
  tapinoma_sessile
  aphaenogaster_rudis
  formica_fusca
  tetramorium_immigrans
  brachyponera_chinensis
  temnothorax_curvinodis
)

mkdir -p "$OUTROOT/_logs"
LOGFILE=$OUTROOT/_logs/queue.log
PIDFILE=$OUTROOT/_logs/queue.pid
PIDLIST=$OUTROOT/_logs/queue_pids.txt
DONEFILE=$OUTROOT/_logs/queue.done

echo $$ > "$PIDFILE"
rm -f "$DONEFILE" "$PIDLIST"
: > "$PIDLIST"

ts()  { date +'%Y-%m-%dT%H:%M:%S'; }
log() { echo "[$(ts)] $*" | tee -a "$LOGFILE"; }

launch_one() {
  local sp=$1
  # Binary writes to <out>/<species>/daily.csv — pass OUTROOT (parent),
  # NOT $OUTROOT/$sp, or you get double-nested $OUTROOT/sp/sp/daily.csv.
  nohup "$BIN" \
    --years "$YEARS" \
    --no-mlp \
    --species "$sp" \
    --seed "$SEED" \
    --out "$OUTROOT" \
    > "$OUTROOT/_logs/$sp.log.out" \
    2> "$OUTROOT/_logs/$sp.log.err" \
    < /dev/null &
  local pid=$!
  echo "$pid $sp" >> "$PIDLIST"
  log "launched $sp pid=$pid"
}

log "queue start: ${#SPECIES[@]} species, max-concurrent=$MAX_CONCURRENT, years=$YEARS, seed=$SEED, out=$OUTROOT"
log "binary: $BIN ($(stat -c %y "$BIN" 2>/dev/null || echo missing))"

active=0
for sp in "${SPECIES[@]}"; do
  while (( active >= MAX_CONCURRENT )); do
    wait -n
    active=$((active - 1))
    log "slot freed (active=$active)"
  done
  launch_one "$sp"
  active=$((active + 1))
done

log "all dispatched; waiting on $active remaining"
wait
log "all species complete"
touch "$DONEFILE"
