# 6-hour AI training experiment cycle.
#
# Pipeline:
#   A. Generate diverse trajectories (heuristic vs heuristic + heuristic vs random)
#   B. Filter by outcome score (keep "winning" trajectories only)
#   C. Convert filtered JSONL -> aether text corpus
#   D. Train multiple checkpoints at different scales
#   E. Evaluate each checkpoint vs heuristic
#   F. Write summary report
#
# Run from ANY shell:
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\ai_training_run.ps1

# Continue, not Stop — many native exes (aether-*) emit informational
# stderr lines that PS interprets as NativeCommandError. Stop here would
# abort mid-pipeline. We check $LASTEXITCODE manually after each call.
$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

# -------- Params (tune for time budget) --------
$matches_hh = 100   # heuristic vs heuristic matches
$matches_hr = 100   # heuristic vs random matches
$max_ticks = 500    # tick budget per match
$min_outcome = 0.55 # filter threshold for "winning" trajectories
$train_steps_short = 600
$train_steps_long  = 1500
$eval_matches = 30  # eval matches per trained checkpoint vs heuristic

# -------- Paths --------
$run_dir = "J:\antcolony\bench\ai-train-run"
$traj_hh = Join-Path $run_dir "trajectories_hh.jsonl"
$traj_hr = Join-Path $run_dir "trajectories_hr.jsonl"
$traj_all = Join-Path $run_dir "trajectories_all.jsonl"
$traj_filtered = Join-Path $run_dir "trajectories_filtered.jsonl"
$corpus_txt = Join-Path $run_dir "corpus.txt"
$report = Join-Path $run_dir "REPORT.md"

$aether_root = "J:\aether"
$aether_corpus = Join-Path $aether_root "scratch\antcolony_train_corpus.txt"
$aether_prepared = "scratch\antcolony_train_prepared"

$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"

New-Item -ItemType Directory -Path $run_dir -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $aether_root "scratch") -Force | Out-Null

function Log {
    param($msg)
    $stamp = Get-Date -Format 'HH:mm:ss'
    $line = "[$stamp] $msg"
    Write-Output $line
    Add-Content -Path (Join-Path $run_dir "run.log") -Value $line -Encoding utf8
}

# -------- A. Generate trajectories --------
Log "=== A. Data generation ==="
Log "A1. Heuristic vs Heuristic ($matches_hh matches @ $max_ticks ticks)"
& $bench_exe --left heuristic --right heuristic --matches $matches_hh `
    --max-ticks $max_ticks --dump-trajectories $traj_hh 2>&1 | Out-Null
$hh_lines = (Get-Content $traj_hh | Measure-Object -Line).Lines
Log "  -> $hh_lines records"

Log "A2. Heuristic vs Random ($matches_hr matches @ $max_ticks ticks)"
& $bench_exe --left heuristic --right random --right-seed 42 --matches $matches_hr `
    --max-ticks $max_ticks --dump-trajectories $traj_hr 2>&1 | Out-Null
$hr_lines = (Get-Content $traj_hr | Measure-Object -Line).Lines
Log "  -> $hr_lines records"

Get-Content $traj_hh, $traj_hr | Set-Content -Path $traj_all -Encoding utf8
$total_lines = (Get-Content $traj_all | Measure-Object -Line).Lines
Log "  combined: $total_lines records"

# -------- B. Filter --------
Log "=== B. Filter trajectories (outcome >= $min_outcome) ==="
$kept = 0
$dropped = 0
$out = New-Object System.Collections.ArrayList
Get-Content $traj_all | ForEach-Object {
    $r = $_ | ConvertFrom-Json
    if ($r.outcome_for_this_colony -ge $min_outcome) {
        [void]$out.Add($_)
        $kept++
    } else {
        $dropped++
    }
}
$out | Set-Content -Path $traj_filtered -Encoding utf8
Log "  kept $kept, dropped $dropped (kept ratio = $([math]::Round($kept / [math]::Max($total_lines,1) * 100, 1))%)"

# -------- C. JSONL -> aether corpus --------
Log "=== C. JSONL -> text corpus ==="
$corpus_lines = Get-Content $traj_filtered | ForEach-Object {
    $r = $_ | ConvertFrom-Json
    $s = $r.state; $d = $r.decision
    $ed = if ($s.enemy_distance_min -ne $null -and $s.enemy_distance_min -ne 'inf') { "{0:F1}" -f $s.enemy_distance_min } else { "inf" }
    $dia = if ($s.diapause_active) { 1 } else { 0 }
    $day = if ($s.is_daytime) { 1 } else { 0 }
    "state food={0:F1} inflow={1:F2} workers={2} soldiers={3} breeders={4} eggs={5} larvae={6} pupae={7} queens={8} losses={9} ed={10} ew={11} es={12} doy={13} t={14:F1} dia={15} day={16} action= w:{17:F2} s:{18:F2} b:{19:F2} f:{20:F2} d:{21:F2} n:{22:F2} r:none" -f `
        $s.food_stored, $s.food_inflow_recent, $s.worker_count, $s.soldier_count, $s.breeder_count,
        $s.brood_egg, $s.brood_larva, $s.brood_pupa, $s.queens_alive, $s.combat_losses_recent,
        $ed, $s.enemy_worker_count, $s.enemy_soldier_count, $s.day_of_year, $s.ambient_temp_c, $dia, $day,
        $d.caste_ratio_worker, $d.caste_ratio_soldier, $d.caste_ratio_breeder,
        $d.forage_weight, $d.dig_weight, $d.nurse_weight
}
$corpus_lines | Set-Content -Path $corpus_txt -Encoding utf8
Log "  $($corpus_lines.Count) corpus lines"

Copy-Item $corpus_txt $aether_corpus -Force
Set-Location $aether_root
Log "  copied to aether tree: $aether_corpus"

# -------- D. Tokenize once --------
Log "=== D. aether-prepare ==="
$prep_out = & .\target\release\aether-prepare.exe --in scratch\antcolony_train_corpus.txt --out $aether_prepared 2>&1
foreach ($line in $prep_out) { Log "  $line" }
if ($LASTEXITCODE -ne 0) { Log "ABORT: aether-prepare exit $LASTEXITCODE"; exit 1 }

# -------- E. Train multiple checkpoints --------
function Train-Ckpt {
    param($name, $config, $steps, $lr, $seed)
    Log "E. Training '$name' (config=$config, steps=$steps, lr=$lr, seed=$seed)"
    $ckpt = "checkpoints\antcolony_$name"
    $train_out = & .\target\release\aether-train.exe --config $config --steps $steps --batch 8 --seq 64 --lr $lr --seed $seed --data $aether_prepared --out $ckpt 2>&1
    foreach ($line in $train_out) { Log "  $line" }
    if ($LASTEXITCODE -ne 0) {
        Log "  WARNING: train failed for $name (exit $LASTEXITCODE)"
        return $null
    }
    return "J:\aether\$ckpt"
}

$ckpts = @{}
$ckpts["nano600"]  = Train-Ckpt "nano600"  "nano" $train_steps_short "3e-3" 42
$ckpts["nano1500"] = Train-Ckpt "nano1500" "nano" $train_steps_long  "3e-3" 42
$ckpts["nano1500_lr1e3"] = Train-Ckpt "nano1500_lr1e3" "nano" $train_steps_long "1e-3" 137
$ckpts["tiny600"]  = Train-Ckpt "tiny600"  "tiny" $train_steps_short "3e-3" 42

# -------- F. Evaluate each checkpoint --------
Set-Location J:\antcolony
Log "=== F. Evaluation ($eval_matches matches each, vs heuristic baseline) ==="
$results = @{}
foreach ($name in @("nano600","nano1500","nano1500_lr1e3","tiny600")) {
    $ckpt = $ckpts[$name]
    if ($null -eq $ckpt) { Log "  SKIP $name (training failed)"; continue }
    Log "F. Evaluating $name (ckpt=$ckpt)"
    $eval_dir = Join-Path $run_dir "eval_$name"
    $eval_out = & $bench_exe --left heuristic --right "aether:$ckpt" `
        --matches $eval_matches --max-ticks $max_ticks --out $eval_dir 2>&1
    foreach ($line in $eval_out) { Log "  $line" }
    $results[$name] = $eval_dir
}

# -------- G. Write report --------
Log "=== G. Writing REPORT.md ==="
$rpt = @()
$rpt += "# AI Training Run Report"
$rpt += ""
$rpt += "Generated: $(Get-Date -Format o)"
$rpt += ""
$rpt += "## Pipeline params"
$rpt += "- Self-play: $matches_hh heuristic-vs-heuristic + $matches_hr heuristic-vs-random"
$rpt += "- Tick budget per match: $max_ticks"
$rpt += "- Outcome filter: keep records with outcome >= $min_outcome"
$rpt += "- Eval: $eval_matches matches per trained checkpoint vs heuristic"
$rpt += ""
$rpt += "## Data"
$rpt += "- Total trajectories: $total_lines"
$rpt += "- Kept after filter:  $kept"
$rpt += "- Drop ratio:         $([math]::Round($dropped / [math]::Max($total_lines,1) * 100, 1))%"
$rpt += ""
$rpt += "## Trained checkpoints"
$rpt += ""
$rpt += "| Name | Config | Steps | LR | Seed | Path |"
$rpt += "|------|--------|------:|---:|-----:|------|"
$rpt += "| nano600         | nano | $train_steps_short | 3e-3 | 42  | $($ckpts['nano600']) |"
$rpt += "| nano1500        | nano | $train_steps_long  | 3e-3 | 42  | $($ckpts['nano1500']) |"
$rpt += "| nano1500_lr1e3  | nano | $train_steps_long  | 1e-3 | 137 | $($ckpts['nano1500_lr1e3']) |"
$rpt += "| tiny600         | tiny | $train_steps_short | 3e-3 | 42  | $($ckpts['tiny600']) |"
$rpt += ""
$rpt += "## Eval results"
$rpt += ""
foreach ($name in $results.Keys) {
    $sum = Join-Path $results[$name] "SUMMARY.md"
    if (Test-Path $sum) {
        $rpt += "### $name"
        $rpt += ""
        $rpt += (Get-Content $sum -Raw)
        $rpt += ""
    }
}
$rpt | Set-Content -Path $report -Encoding utf8
Log "Done. Report: $report"
