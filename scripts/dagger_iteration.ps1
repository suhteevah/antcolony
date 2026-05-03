# DAgger iteration — train MLP, dump its own match trajectories,
# filter winners, retrain. Breaks the "model can only mimic teachers"
# ceiling.

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$archetypes = @("heuristic","defender","aggressor","economist","breeder","forager","conservative")
$matches_per_pair = 8
$max_ticks = 10000
$min_outcome = 0.55

$run_dir = "J:\antcolony\bench\dagger-run"
$base_weights = "J:\antcolony\bench\tournament-run\mlp_weights.json"
$traj_dagger = Join-Path $run_dir "trajectories_dagger.jsonl"
$traj_combined = Join-Path $run_dir "trajectories_combined.jsonl"
$traj_filtered = Join-Path $run_dir "trajectories_filtered.jsonl"
$weights = Join-Path $run_dir "mlp_weights_dagger.json"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"

New-Item -ItemType Directory -Path $run_dir -Force | Out-Null
$logfile = Join-Path $run_dir "run.log"
function Log { param($m); $stamp = Get-Date -Format 'HH:mm:ss'; "[$stamp] $m" | Tee-Object -FilePath $logfile -Append }

if (Test-Path $traj_dagger) { Remove-Item $traj_dagger }

Log "=== Iteration: MLP plays each archetype, $matches_per_pair matches each ==="
foreach ($opp in $archetypes) {
    $tmp = Join-Path $run_dir "tmp_mlp_vs_$opp.jsonl"
    & $bench_exe --left "mlp:$base_weights" --right $opp --matches $matches_per_pair `
        --max-ticks $max_ticks --dump-trajectories $tmp 2>&1 | Out-Null
    if (Test-Path $tmp) {
        Get-Content $tmp | Add-Content -Path $traj_dagger -Encoding utf8
        $cnt = (Get-Content $tmp | Measure-Object -Line).Lines
        Remove-Item $tmp
        Log "  mlp vs $opp : $cnt records"
    }
}
$dagger_count = (Get-Content $traj_dagger | Measure-Object -Line).Lines
Log "  DAgger trajectories: $dagger_count"

Log "=== Combine with tournament corpus ==="
Get-Content "J:\antcolony\bench\tournament-run\trajectories_filtered.jsonl", $traj_dagger `
    | Set-Content -Path $traj_combined -Encoding utf8
$combined_count = (Get-Content $traj_combined | Measure-Object -Line).Lines
Log "  Combined: $combined_count"

Log "=== Filter (outcome >= $min_outcome) ==="
$kept = New-Object System.Collections.ArrayList
$drop = 0
Get-Content $traj_combined | ForEach-Object {
    try {
        $r = $_ | ConvertFrom-Json
        if ($r.outcome_for_this_colony -ge $min_outcome) {
            [void]$kept.Add($_)
        } else { $drop++ }
    } catch { $drop++ }
}
$kept | Set-Content -Path $traj_filtered -Encoding utf8
Log "  kept $($kept.Count), dropped $drop"

Log "=== Retrain MLP on combined corpus (DAgger v1) ==="
$train_out = python "J:\antcolony\scripts\train_mlp_brain.py" `
    --trajectories $traj_filtered --out $weights `
    --hidden 64 --epochs 100 --lr 1e-3 --device cuda 2>&1
foreach ($l in $train_out) { Log "  $l" }

Log "=== Eval DAgger MLP vs each archetype ==="
$total_l = 0; $total_r = 0
foreach ($opp in $archetypes) {
    $eval_dir = Join-Path $run_dir "eval_dagger_vs_$opp"
    & $bench_exe --left "mlp:$weights" --right $opp --matches 20 --max-ticks $max_ticks --out $eval_dir 2>&1 | Out-Null
    if (Test-Path (Join-Path $eval_dir "SUMMARY.md")) {
        $sum = Get-Content (Join-Path $eval_dir "SUMMARY.md") -Raw
        # Extract from Win record table.
        if ($sum -match "Left[^|]+\|\s+`[^`]+`\s+\|\s+(\d+)\s+\|\s+([\d.]+)%") {
            $lw = [int]$matches[1]
            $total_l += $lw
        }
        if ($sum -match "Right\s+\|\s+`[^`]+`\s+\|\s+(\d+)\s+\|\s+([\d.]+)%") {
            $rw = [int]$matches[1]
            $total_r += $rw
        }
        Log "  dagger vs $opp : MLP $lw / $opp $rw / 20"
    }
}
$total = $total_l + $total_r
$wr = if ($total -gt 0) { [math]::Round(100.0 * $total_l / $total, 1) } else { 0 }
Log "=== DAgger total: $total_l / $total ($wr% win rate) ==="
