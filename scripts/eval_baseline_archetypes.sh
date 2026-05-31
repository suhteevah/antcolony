#!/usr/bin/env bash
# Per-archetype eval of the SOTA MlpBrain baseline, scored IDENTICALLY to the
# trainer's eval.rs (the harness that measured the 47.1% baseline AND the A1
# run): win=1, loss=0, draw=0.5, and TIMEOUT scored by worker share (more
# workers at tick 10000 => 1.0). matchup_bench is just the match runner; we
# re-score its per-match output to match eval.rs exactly.
#
# Answers: does the 47.1% baseline ALSO score ~0 vs defender/aggressor, or is
# that combat collapse specific to the A1 hierarchical brain?
#
# Usage: eval_baseline_archetypes.sh [WEIGHTS_JSON] [MATCHES]
set -uo pipefail
WEIGHTS="${1:-bench/iterative-fsp/round_1/mlp_weights_v1.json}"
MATCHES="${2:-30}"
MAXTICKS=10000   # match eval.rs / MatchEnv
cd "J:/antcolony" 2>/dev/null || cd "$(dirname "$0")/.."

echo "SOTA MLP per-archetype eval (eval.rs scoring: timeout => worker-share win)"
echo "  weights: $WEIGHTS   matches: $MATCHES/opp   max-ticks: $MAXTICKS"
cargo build --release --example matchup_bench -p antcolony-sim 2>&1 | tail -1

ARCHES=(heuristic defender aggressor economist breeder forager conservative)
results=""
printf "\n%-14s %s\n" "archetype" "mlp_win_rate (eval.rs scoring)"
printf -- "-------------- ------------------------------\n"
for a in "${ARCHES[@]}"; do
  pct=$(cargo run --release --example matchup_bench -p antcolony-sim -- \
        --left "mlp:$WEIGHTS" --right "$a" --matches "$MATCHES" --max-ticks "$MAXTICKS" 2>/dev/null \
      | grep -E '^  match ' \
      | awk '
          {
            st=$4; win=$5; wr=$7;
            sub(/.*=/,"",wr); split(wr,ab,"/"); A=ab[1]+0; B=ab[2]+0;
            if (win=="winner=Some(0)") s+=1;
            else if (win=="winner=Some(1)") s+=0;
            else if (st=="draw") s+=0.5;
            else { if (A>B) s+=1; else if (A<B) s+=0; else s+=0.5; }
            n++;
          }
          END { if(n>0) printf "%.1f", 100*s/n; else print "ERR" }')
  printf "%-14s %s%%\n" "$a" "$pct"
  results="$results $pct"
done
echo "----------------------------------------------"
echo "$results" | awk '{s=0;n=0; for(i=1;i<=NF;i++){if($i+0==$i){s+=$i;n++}}; if(n>0) printf "MEAN over %d opps: %.1f%%  (eval.rs baseline target ~47.1%%)\n", n, s/n}'
