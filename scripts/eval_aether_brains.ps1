# Evaluate trained Aether checkpoints vs HeuristicBrain.
# Use this when training is done and you want quick win-rate numbers.

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$ckpts = @("antcolony_nano600", "antcolony_nano1500", "antcolony_nano1500_lr1e3")
$matches = 30
$max_ticks = 500
$run_dir = "J:\antcolony\bench\ai-train-run"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"

New-Item -ItemType Directory -Path $run_dir -Force | Out-Null
$logfile = Join-Path $run_dir "eval.log"
function Log { param($m); $stamp = Get-Date -Format 'HH:mm:ss'; "[$stamp] $m" | Tee-Object -FilePath $logfile -Append }

Log "=== Evaluating $($ckpts.Count) checkpoints, $matches matches each, max-ticks=$max_ticks ==="

# Sanity baseline first: heuristic vs random (we expect heuristic to win clearly).
Log ""
Log "Baseline: heuristic vs random ($matches matches)"
$baseline_dir = Join-Path $run_dir "eval_baseline_hr"
$out = & $bench_exe --left heuristic --right random --right-seed 42 `
    --matches $matches --max-ticks $max_ticks --out $baseline_dir 2>&1
foreach ($l in $out) { Log "  $l" }

foreach ($ckpt in $ckpts) {
    Log ""
    Log "Evaluating $ckpt vs heuristic ($matches matches)"
    $ckpt_path = "J:\aether\checkpoints\$ckpt"
    $eval_dir = Join-Path $run_dir "eval_$ckpt"
    $out = & $bench_exe --left heuristic --right "aether:$ckpt_path" `
        --matches $matches --max-ticks $max_ticks --out $eval_dir 2>&1
    foreach ($l in $out) { Log "  $l" }
}

Log ""
Log "Done. Eval outputs in $run_dir/eval_*"
