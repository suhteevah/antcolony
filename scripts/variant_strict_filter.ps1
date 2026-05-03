# Strict-filter retrain: same variant tournament corpus, but keep only
# high-margin wins (outcome >= 0.80). Tests whether the BC-on-diverse-pool
# regression is from noisy/contested teachers vs from the diversity itself.

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$run_dir   = "J:\antcolony\bench\variant-tournament"
$traj      = Join-Path $run_dir "trajectories.jsonl"
$strict    = Join-Path $run_dir "trajectories_strict.jsonl"
$weights   = Join-Path $run_dir "mlp_weights_strict.json"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"

$logfile = Join-Path $run_dir "strict.log"
function Log { param($m); $stamp = Get-Date -Format 'HH:mm:ss'; "[$stamp] $m" | Tee-Object -FilePath $logfile -Append }

Log "=== Strict-filter retrain (outcome >= 0.80) ==="
$kept = New-Object System.Collections.ArrayList
Get-Content $traj | ForEach-Object {
    try {
        $r = $_ | ConvertFrom-Json
        if ($r.outcome_for_this_colony -ge 0.80) { [void]$kept.Add($_) }
    } catch {}
}
$kept | Set-Content -Path $strict -Encoding utf8
Log "  Strict-filter kept: $($kept.Count) records"

$train = python "J:\antcolony\scripts\train_mlp_brain.py" `
    --trajectories $strict --out $weights `
    --hidden 64 --epochs 100 --lr 1e-3 --device cuda 2>&1 | Select-Object -Last 6
foreach ($l in $train) { Log "  $l" }

Log "=== Eval strict-trained MLP ==="
foreach ($opp in @("heuristic","defender","aggressor","economist","breeder","forager","conservative")) {
    $eval_dir = Join-Path $run_dir "eval_strict_vs_$opp"
    & $bench_exe --left "mlp:$weights" --right $opp --matches 20 --max-ticks 10000 --out $eval_dir 2>&1 | Out-Null
}
Log "=== Strict done ==="
