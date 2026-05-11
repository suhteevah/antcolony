#!/usr/bin/env bash
# Queue manager for Phase 1 2yr smoke on cnc.
# Runs ALL species 2-at-a-time, in process order, on this host only.
# Designed to be launched under nohup and survive ssh disconnects.
#
# State files in $OUTROOT/_logs/:
#   queue.pid        — this script's PID
#   queue.log        — script's own activity log
#   queue_pids.json  — {species: pid} as launched
#   queue.done       — touched when all species finish
#   <species>.log.{out,err} — each smoke run's stdio

set -o pipefail

OUTROOT=/opt/antcolony/runs/phase1-2yr
BIN=/opt/antcolony/target/release/examples/smoke_10yr_ai
YEARS=2
SEED=42
MAX_CONCURRENT=2
POLL_SECONDS=60

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
JSON=$OUTROOT/_logs/queue_pids.json
DONEFILE=$OUTROOT/_logs/queue.done

echo $$ > "$PIDFILE"
rm -f "$DONEFILE"

ts()  { date +'%Y-%m-%dT%H:%M:%S'; }
log() { echo "[$(ts)] $*" | tee -a "$LOGFILE"; }

# Parallel indexed arrays — simpler than associative under bash semantics.
LAUNCHED_SP=()
LAUNCHED_PID=()

write_json() {
  {
    echo "{"
    local i n=${#LAUNCHED_SP[@]}
    for (( i = 0; i < n; i++ )); do
      local sep=","
      [[ $i -eq $((n - 1)) ]] && sep=""
      printf '  "%s": %s%s\n' "${LAUNCHED_SP[$i]}" "${LAUNCHED_PID[$i]}" "$sep"
    done
    echo "}"
  } > "$JSON"
}

launch_one() {
  local sp=$1
  local out=$OUTROOT/$sp
  mkdir -p "$out"
  rm -f "$out/daily.csv" "$out/summary.json" 2>/dev/null || true
  nohup "$BIN" \
    --years "$YEARS" \
    --no-mlp \
    --species "$sp" \
    --seed "$SEED" \
    --out "$out" \
    > "$OUTROOT/_logs/$sp.log.out" \
    2> "$OUTROOT/_logs/$sp.log.err" \
    < /dev/null &
  local pid=$!
  LAUNCHED_SP+=("$sp")
  LAUNCHED_PID+=("$pid")
  log "launched $sp pid=$pid"
  write_json
}

log "queue start: ${#SPECIES[@]} species, max-concurrent=$MAX_CONCURRENT, years=$YEARS, seed=$SEED"
log "binary: $BIN ($(stat -c %y "$BIN" 2>/dev/null || echo missing))"

# Process the queue.
qi=0   # next species index into SPECIES
running_pids=()
running_sps=()

count_alive() {
  local alive=0
  local new_pids=() new_sps=()
  local i n=${#running_pids[@]}
  for (( i = 0; i < n; i++ )); do
    if kill -0 "${running_pids[$i]}" 2>/dev/null; then
      new_pids+=("${running_pids[$i]}")
      new_sps+=("${running_sps[$i]}")
      alive=$((alive + 1))
    else
      log "finished ${running_sps[$i]} pid=${running_pids[$i]}"
    fi
  done
  running_pids=("${new_pids[@]}")
  running_sps=("${new_sps[@]}")
  echo "$alive"
}

while [[ $qi -lt ${#SPECIES[@]} || ${#running_pids[@]} -gt 0 ]]; do
  # Fill slots from queue.
  while [[ ${#running_pids[@]} -lt $MAX_CONCURRENT && $qi -lt ${#SPECIES[@]} ]]; do
    sp="${SPECIES[$qi]}"
    qi=$((qi + 1))
    launch_one "$sp"
    # Pull the PID we just appended.
    last=$((${#LAUNCHED_PID[@]} - 1))
    running_pids+=("${LAUNCHED_PID[$last]}")
    running_sps+=("$sp")
  done

  sleep $POLL_SECONDS

  count_alive >/dev/null
done

log "all species complete"
touch "$DONEFILE"
