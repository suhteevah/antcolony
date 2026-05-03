# DAgger on top of the curated-tournament MLP.
# Hypothesis: curated-BC (41.9%) + self-play (DAgger v1 trick that gave
# baseline-BC its +5pp lift) = stacked gain.
#
# Pipeline:
# 1. Load curated MLP weights as the starting brain
# 2. Play 8 matches each vs the original 7 archetypes (~14k trajectories)
# 3. Combine with curated tournament corpus, filter winners
# 4. Retrain MLP
# 5. Eval vs original 7

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$run_dir   = "J:\antcolony\bench\dagger-on-curated"
$curated_traj    = "J:\antcolony\bench\curated-tournament\trajectories_filtered.jsonl"
$curated_weights = "J:\antcolony\bench\curated-tournament\mlp_weights_curated.json"
$traj_new        = Join-Path $run_dir "trajectories_self_play.jsonl"
$traj_combined   = Join-Path $run_dir "trajectories_combined.jsonl"
$traj_filtered   = Join-Path $run_dir "trajectories_filtered.jsonl"
$weights         = Join-Path $run_dir "mlp_weights_doc.json"
$bench_exe       = "J:\antcolony\target\release\examples\matchup_bench.exe"

New-Item -ItemType Directory -Path $run_dir -Force | Out-Null
$logfile = Join-Path $run_dir "run.log"
function Log { param($m); $stamp = Get-Date -Format 'HH:mm:ss'; "[$stamp] $m" | Tee-Object -FilePath $logfile -Append }

if (Test-Path $traj_new) { Remove-Item $traj_new }

Log "=== DAgger on curated: curated MLP plays each archetype ==="
foreach ($opp in @("heuristic","defender","aggressor","economist","breeder","forager","conservative")) {
    $tmp = Join-Path $run_dir "tmp_curated_vs_$opp.jsonl"
    & $bench_exe --left "mlp:$curated_weights" --right $opp --matches 8 `
        --max-ticks 10000 --dump-trajectories $tmp 2>&1 | Out-Null
    if (Test-Path $tmp) {
        Get-Content $tmp | Add-Content -Path $traj_new -Encoding utf8
        Remove-Item $tmp
    }
}
Log "  self-play records: $((Get-Content $traj_new | Measure-Object -Line).Lines)"

Log "=== Combine: curated corpus + self-play ==="
Get-Content $curated_traj, $traj_new | Set-Content -Path $traj_combined -Encoding utf8

$kept = New-Object System.Collections.ArrayList
Get-Content $traj_combined | ForEach-Object {
    try {
        $r = $_ | ConvertFrom-Json
        if ($r.outcome_for_this_colony -ge 0.55) { [void]$kept.Add($_) }
    } catch {}
}
$kept | Set-Content -Path $traj_filtered -Encoding utf8
Log "  filtered: $($kept.Count) records"

Log "=== Retrain ==="
$train = python "J:\antcolony\scripts\train_mlp_brain.py" `
    --trajectories $traj_filtered --out $weights `
    --hidden 64 --epochs 100 --lr 1e-3 --device cuda 2>&1 | Select-Object -Last 6
foreach ($l in $train) { Log "  $l" }

Log "=== Eval DAgger-on-curated MLP vs original 7 ==="
foreach ($opp in @("heuristic","defender","aggressor","economist","breeder","forager","conservative")) {
    $eval_dir = Join-Path $run_dir "eval_vs_$opp"
    & $bench_exe --left "mlp:$weights" --right $opp --matches 20 --max-ticks 10000 --out $eval_dir 2>&1 | Out-Null
}
Log "=== Done ==="
