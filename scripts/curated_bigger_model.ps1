# Curated corpus + bigger model. Tests whether MLP capacity is the
# bottleneck now that the corpus is cleaner. (hidden=256 didn't help on
# the noisy baseline corpus, but may help on curated data.)

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$run_dir = "J:\antcolony\bench\curated-bigger"
$weights = Join-Path $run_dir "mlp_weights_h256.json"
$corpus  = "J:\antcolony\bench\curated-tournament\trajectories_filtered.jsonl"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"

New-Item -ItemType Directory -Path $run_dir -Force | Out-Null
$logfile = Join-Path $run_dir "run.log"
function Log { param($m); $stamp = Get-Date -Format 'HH:mm:ss'; "[$stamp] $m" | Tee-Object -FilePath $logfile -Append }

Log "=== Train hidden=256 MLP on curated corpus ==="
$train = python "J:\antcolony\scripts\train_mlp_brain.py" `
    --trajectories $corpus --out $weights `
    --hidden 256 --epochs 150 --lr 1e-3 --device cuda 2>&1 | Select-Object -Last 8
foreach ($l in $train) { Log "  $l" }

Log "=== Eval h256 vs original 7 ==="
foreach ($opp in @("heuristic","defender","aggressor","economist","breeder","forager","conservative")) {
    $eval_dir = Join-Path $run_dir "eval_vs_$opp"
    & $bench_exe --left "mlp:$weights" --right $opp --matches 20 --max-ticks 10000 --out $eval_dir 2>&1 | Out-Null
}
Log "=== Done ==="
