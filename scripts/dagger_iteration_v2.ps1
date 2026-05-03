# DAgger v2 — same as v1 but uses DAgger v1 weights as base + accumulates corpus.

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$archetypes = @("heuristic","defender","aggressor","economist","breeder","forager","conservative")
$matches_per_pair = 8
$max_ticks = 10000

$run_dir = "J:\antcolony\bench\dagger-v2-run"
$base_weights = "J:\antcolony\bench\dagger-run\mlp_weights_dagger.json"
$traj_v2 = Join-Path $run_dir "trajectories_v2.jsonl"
$traj_combined = Join-Path $run_dir "trajectories_combined.jsonl"
$traj_filtered = Join-Path $run_dir "trajectories_filtered.jsonl"
$weights = Join-Path $run_dir "mlp_weights_v2.json"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"

New-Item -ItemType Directory -Path $run_dir -Force | Out-Null
$logfile = Join-Path $run_dir "run.log"
function Log { param($m); $stamp = Get-Date -Format 'HH:mm:ss'; "[$stamp] $m" | Tee-Object -FilePath $logfile -Append }

if (Test-Path $traj_v2) { Remove-Item $traj_v2 }

Log "=== Iteration v2: DAgger v1 MLP plays each archetype ==="
foreach ($opp in $archetypes) {
    $tmp = Join-Path $run_dir "tmp_mlp_vs_$opp.jsonl"
    & $bench_exe --left "mlp:$base_weights" --right $opp --matches $matches_per_pair `
        --max-ticks $max_ticks --dump-trajectories $tmp 2>&1 | Out-Null
    if (Test-Path $tmp) {
        Get-Content $tmp | Add-Content -Path $traj_v2 -Encoding utf8
        Remove-Item $tmp
    }
}
Log "  v2 trajectories: $((Get-Content $traj_v2 | Measure-Object -Line).Lines)"

Log "=== Combine: DAgger v1 corpus + v2 trajectories ==="
Get-Content "J:\antcolony\bench\dagger-run\trajectories_filtered.jsonl", $traj_v2 `
    | Set-Content -Path $traj_combined -Encoding utf8

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
    --hidden 64 --epochs 100 --lr 1e-3 --device cuda 2>&1 | Select-Object -Last 5
foreach ($l in $train) { Log "  $l" }

Log "=== Eval DAgger v2 ==="
foreach ($opp in $archetypes) {
    $eval_dir = Join-Path $run_dir "eval_v2_vs_$opp"
    & $bench_exe --left "mlp:$weights" --right $opp --matches 20 --max-ticks $max_ticks --out $eval_dir 2>&1 | Out-Null
}
Log "=== v2 Done ==="
