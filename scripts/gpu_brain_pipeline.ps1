# GPU MLP brain pipeline — end-to-end.
#
# 1. Generate fresh trajectories from self-play (with the new
#    forage_weight + nurse_weight sim hooks live)
# 2. Filter by graded outcome
# 3. Python+CUDA: train MLP, save JSON weights
# 4. Eval trained MLP brain vs HeuristicBrain
#
# Run: powershell -NoProfile -ExecutionPolicy Bypass -File scripts\gpu_brain_pipeline.ps1

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$matches_hh = 100
$matches_hr = 100
$max_ticks = 10000     # long enough for forage/nurse hooks to manifest
$min_outcome = 0.55
$hidden = 64
$epochs = 80
$lr = 1e-3
$eval_matches = 30

$run_dir = "J:\antcolony\bench\gpu-brain-run"
$traj_hh = Join-Path $run_dir "trajectories_hh.jsonl"
$traj_hr = Join-Path $run_dir "trajectories_hr.jsonl"
$traj_all = Join-Path $run_dir "trajectories_all.jsonl"
$traj_filtered = Join-Path $run_dir "trajectories_filtered.jsonl"
$weights = Join-Path $run_dir "mlp_weights.json"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"

New-Item -ItemType Directory -Path $run_dir -Force | Out-Null
$logfile = Join-Path $run_dir "run.log"
function Log { param($m); $stamp = Get-Date -Format 'HH:mm:ss'; "[$stamp] $m" | Tee-Object -FilePath $logfile -Append }

Log "=== A. Generate trajectories ==="
Log "A1. heuristic vs heuristic ($matches_hh matches @ $max_ticks ticks)"
& $bench_exe --left heuristic --right heuristic --matches $matches_hh `
    --max-ticks $max_ticks --dump-trajectories $traj_hh 2>&1 | Out-Null
$hh = (Get-Content $traj_hh | Measure-Object -Line).Lines
Log "  -> $hh records"

Log "A2. heuristic vs random  ($matches_hr matches @ $max_ticks ticks)"
& $bench_exe --left heuristic --right random --right-seed 42 --matches $matches_hr `
    --max-ticks $max_ticks --dump-trajectories $traj_hr 2>&1 | Out-Null
$hr = (Get-Content $traj_hr | Measure-Object -Line).Lines
Log "  -> $hr records"

Get-Content $traj_hh, $traj_hr | Set-Content -Path $traj_all -Encoding utf8
$total = (Get-Content $traj_all | Measure-Object -Line).Lines
Log "  combined: $total records"

Log "=== B. Filter (outcome >= $min_outcome) ==="
$kept = New-Object System.Collections.ArrayList
$drop = 0
Get-Content $traj_all | ForEach-Object {
    try {
        $r = $_ | ConvertFrom-Json
        if ($r.outcome_for_this_colony -ge $min_outcome) {
            [void]$kept.Add($_)
        } else { $drop++ }
    } catch { $drop++ }
}
$kept | Set-Content -Path $traj_filtered -Encoding utf8
$kept_count = $kept.Count
$pct = [math]::Round($kept_count / [math]::Max($total,1) * 100, 1)
Log "  kept $kept_count / dropped $drop  ($pct% kept)"

Log "=== C. Python+CUDA train MLP ==="
$train_out = python "J:\antcolony\scripts\train_mlp_brain.py" `
    --trajectories $traj_filtered --out $weights `
    --hidden $hidden --epochs $epochs --lr $lr --device cuda 2>&1
foreach ($l in $train_out) { Log "  $l" }
if ($LASTEXITCODE -ne 0) { Log "ABORT: training failed (exit $LASTEXITCODE)"; exit 1 }

Log "=== D. Evaluate MLP vs heuristic baseline ==="
$baseline_dir = Join-Path $run_dir "eval_baseline_hr"
& $bench_exe --left heuristic --right random --right-seed 42 --matches $eval_matches --max-ticks $max_ticks --out $baseline_dir 2>&1 | Out-Null
Log "  baseline (heuristic vs random) -> $baseline_dir/SUMMARY.md"

$eval_dir = Join-Path $run_dir "eval_mlp"
& $bench_exe --left heuristic --right "mlp:$weights" --matches $eval_matches --max-ticks $max_ticks --out $eval_dir 2>&1 | Out-Null
Log "  mlp     (heuristic vs mlp)    -> $eval_dir/SUMMARY.md"

Log "=== Done. Inspect $run_dir/eval_*/SUMMARY.md ==="
