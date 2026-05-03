# DAgger v3 — REPLACE the corpus per iteration instead of accumulating.
# Tests the hypothesis that v2's regression came from stale-loop training
# (model retrained on data its previous self generated).

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$archetypes = @("heuristic","defender","aggressor","economist","breeder","forager","conservative")
$matches_per_pair = 16   # bigger sample per pairing since we don't accumulate
$max_ticks = 10000

$run_dir = "J:\antcolony\bench\dagger-v3-run"
# Use DAgger v1 weights (the best we have) as the brain that generates new data.
$base_weights = "J:\antcolony\bench\dagger-run\mlp_weights_dagger.json"
$traj_v3 = Join-Path $run_dir "trajectories_v3.jsonl"
$traj_filtered = Join-Path $run_dir "trajectories_filtered.jsonl"
$weights = Join-Path $run_dir "mlp_weights_v3.json"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"

New-Item -ItemType Directory -Path $run_dir -Force | Out-Null
$logfile = Join-Path $run_dir "run.log"
function Log { param($m); $stamp = Get-Date -Format 'HH:mm:ss'; "[$stamp] $m" | Tee-Object -FilePath $logfile -Append }

if (Test-Path $traj_v3) { Remove-Item $traj_v3 }

Log "=== DAgger v3 (replacement, not accumulation): MLP plays each archetype ==="
foreach ($opp in $archetypes) {
    $tmp = Join-Path $run_dir "tmp_mlp_vs_$opp.jsonl"
    & $bench_exe --left "mlp:$base_weights" --right $opp --matches $matches_per_pair `
        --max-ticks $max_ticks --dump-trajectories $tmp 2>&1 | Out-Null
    if (Test-Path $tmp) {
        Get-Content $tmp | Add-Content -Path $traj_v3 -Encoding utf8
        Remove-Item $tmp
    }
}
Log "  v3 trajectories (no accumulation): $((Get-Content $traj_v3 | Measure-Object -Line).Lines)"

$kept = New-Object System.Collections.ArrayList
Get-Content $traj_v3 | ForEach-Object {
    try {
        $r = $_ | ConvertFrom-Json
        if ($r.outcome_for_this_colony -ge 0.55) { [void]$kept.Add($_) }
    } catch {}
}
$kept | Set-Content -Path $traj_filtered -Encoding utf8
Log "  filtered: $($kept.Count) records"

Log "=== Retrain on FRESH corpus only ==="
$train = python "J:\antcolony\scripts\train_mlp_brain.py" `
    --trajectories $traj_filtered --out $weights `
    --hidden 64 --epochs 100 --lr 1e-3 --device cuda 2>&1 | Select-Object -Last 5
foreach ($l in $train) { Log "  $l" }

Log "=== Eval v3 ==="
foreach ($opp in $archetypes) {
    $eval_dir = Join-Path $run_dir "eval_v3_vs_$opp"
    & $bench_exe --left "mlp:$weights" --right $opp --matches 20 --max-ticks $max_ticks --out $eval_dir 2>&1 | Out-Null
}
Log "=== v3 Done ==="
