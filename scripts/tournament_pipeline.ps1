# Tournament-style data generation + GPU MLP training + eval.
#
# 1. Round-robin all archetype pairings (7x7 = 49), N matches each,
#    dump every trajectory.
# 2. Filter by graded outcome (winning side only).
# 3. Python+CUDA train MLP on diverse corpus.
# 4. Eval MLP vs each archetype separately so we can see where it
#    beats vs where it loses.

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$archetypes = @("heuristic","defender","aggressor","economist","breeder","forager","conservative")
$matches_per_pair = 8
$max_ticks = 10000
$min_outcome = 0.55
$hidden = 64
$epochs = 100
$lr = 1e-3
$eval_matches = 20

$run_dir = "J:\antcolony\bench\tournament-run"
$traj_all = Join-Path $run_dir "trajectories_all.jsonl"
$traj_filtered = Join-Path $run_dir "trajectories_filtered.jsonl"
$weights = Join-Path $run_dir "mlp_weights.json"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"

New-Item -ItemType Directory -Path $run_dir -Force | Out-Null
$logfile = Join-Path $run_dir "run.log"
function Log { param($m); $stamp = Get-Date -Format 'HH:mm:ss'; "[$stamp] $m" | Tee-Object -FilePath $logfile -Append }

# Wipe any prior trajectory file to start fresh.
if (Test-Path $traj_all) { Remove-Item $traj_all }

Log "=== A. Tournament round-robin ($($archetypes.Count) brains x $($archetypes.Count) opponents x $matches_per_pair matches) ==="
$total_matches = 0
foreach ($l in $archetypes) {
    foreach ($r in $archetypes) {
        $tmp_jsonl = Join-Path $run_dir "tmp_${l}_vs_${r}.jsonl"
        & $bench_exe --left $l --right $r --matches $matches_per_pair `
            --max-ticks $max_ticks --dump-trajectories $tmp_jsonl 2>&1 | Out-Null
        if (Test-Path $tmp_jsonl) {
            Get-Content $tmp_jsonl | Add-Content -Path $traj_all -Encoding utf8
            $cnt = (Get-Content $tmp_jsonl | Measure-Object -Line).Lines
            Remove-Item $tmp_jsonl
            $total_matches += $matches_per_pair
            Log "  $l vs $r : $matches_per_pair matches, $cnt trajectory records"
        } else {
            Log "  $l vs $r : FAILED (no trajectory file written)"
        }
    }
}
$total_records = (Get-Content $traj_all | Measure-Object -Line).Lines
Log "  Tournament complete: $total_matches matches, $total_records total trajectory records"

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
$pct = [math]::Round($kept_count / [math]::Max($total_records,1) * 100, 1)
Log "  kept $kept_count / dropped $drop ($pct% kept)"

Log "=== C. GPU MLP training ==="
$train_out = python "J:\antcolony\scripts\train_mlp_brain.py" `
    --trajectories $traj_filtered --out $weights `
    --hidden $hidden --epochs $epochs --lr $lr --device cuda 2>&1
foreach ($l in $train_out) { Log "  $l" }
if ($LASTEXITCODE -ne 0) { Log "ABORT: training failed (exit $LASTEXITCODE)"; exit 1 }

Log "=== D. Tournament-style eval: MLP vs each archetype ==="
$results = @{}
foreach ($opp in $archetypes) {
    $eval_dir = Join-Path $run_dir "eval_mlp_vs_$opp"
    & $bench_exe --left "mlp:$weights" --right $opp --matches $eval_matches --max-ticks $max_ticks --out $eval_dir 2>&1 | Out-Null
    if (Test-Path (Join-Path $eval_dir "SUMMARY.md")) {
        $sum = Get-Content (Join-Path $eval_dir "SUMMARY.md") -Raw
        # Extract win counts from the SUMMARY's Win record table.
        $left_wins = if ($sum -match "Left\s+\|\s+`mlp[^|]+`\s+\|\s+(\d+)") { [int]$matches[1] } else { 0 }
        $right_wins = if ($sum -match "Right\s+\|\s+`$opp`\s+\|\s+(\d+)") { [int]$matches[1] } else { 0 }
        $results[$opp] = @{left=$left_wins; right=$right_wins}
        Log "  mlp vs $opp : MLP $left_wins / $opp $right_wins (out of $eval_matches)"
    }
}

Log "=== E. Final report ==="
$rpt = @()
$rpt += "# MLP Tournament Eval Report"
$rpt += ""
$rpt += "MLP brain trained on $kept_count filtered trajectories from $total_matches tournament matches."
$rpt += ""
$rpt += "| Opponent | MLP wins | Opp wins | MLP win-rate |"
$rpt += "|----------|---------:|---------:|-------------:|"
foreach ($opp in $archetypes) {
    if ($results.ContainsKey($opp)) {
        $r = $results[$opp]
        $tot = $r.left + $r.right
        $wr = if ($tot -gt 0) { "{0:F1}%" -f (100.0 * $r.left / $tot) } else { "n/a" }
        $rpt += "| $opp | $($r.left) | $($r.right) | $wr |"
    }
}
$rpt | Set-Content -Path (Join-Path $run_dir "REPORT.md") -Encoding utf8
Log "Wrote REPORT.md"
Log "=== Done. ==="
