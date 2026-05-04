# Mixed-corpus retrain: combine FSP-r1 (general competence) with the
# adversarial-FSP-r1 "what beat me" corpus (specific counters), the
# latter weighted 4x by replication. This is the synthesis the
# adversarial result pointed to: pure adversarial loses general skill,
# but adversarial mixed in WITH general data should add counter-strategy
# without overwriting it.

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$run_dir       = "J:\antcolony\bench\mixed-corpus"
$general       = "J:\antcolony\bench\iterative-fsp\round_1\trajectories_filtered.jsonl"
$adversarial   = "J:\antcolony\bench\adversarial-fsp\round_1\trajectories_adv_filtered.jsonl"
$mixed         = Join-Path $run_dir "trajectories_mixed.jsonl"
$weights       = Join-Path $run_dir "mlp_weights_mixed.json"
$bench_exe     = "J:\antcolony\target\release\examples\matchup_bench.exe"
$adv_weight    = 4   # replicate adversarial records 4x

New-Item -ItemType Directory -Path $run_dir -Force | Out-Null
$logfile = Join-Path $run_dir "run.log"
function Log { param($m); $stamp = Get-Date -Format 'HH:mm:ss'; "[$stamp] $m" | Tee-Object -FilePath $logfile -Append }

if (-not (Test-Path $general)) { Log "FATAL: general corpus not found"; exit 1 }
if (-not (Test-Path $adversarial)) { Log "FATAL: adversarial corpus not found"; exit 1 }

$gen_count = (Get-Content $general | Measure-Object -Line).Lines
$adv_count = (Get-Content $adversarial | Measure-Object -Line).Lines
Log "=== Mixed-corpus retrain ==="
Log "  general corpus:     $gen_count records (FSP-r1 winning trajectories)"
Log "  adversarial corpus: $adv_count records ('what beat me')"
Log "  adversarial weight: ${adv_weight}x by replication"
Log "  mixed total:        $($gen_count + $adv_count * $adv_weight) records"

# Build mixed corpus: general 1x + adversarial Nx
if (Test-Path $mixed) { Remove-Item $mixed }
Get-Content $general | Set-Content -Path $mixed -Encoding utf8
for ($i = 0; $i -lt $adv_weight; $i++) {
    Get-Content $adversarial | Add-Content -Path $mixed -Encoding utf8
}

Log "=== Train MLP on mixed corpus ==="
$train = python "J:\antcolony\scripts\train_mlp_brain.py" `
    --trajectories $mixed --out $weights `
    --hidden 64 --epochs 100 --lr 1e-3 --device cuda 2>&1 | Select-Object -Last 6
foreach ($l in $train) { Log "  $l" }

Log "=== Eval mixed-trained MLP vs original 7 ==="
$total_w = 0; $total_g = 0
foreach ($opp in @("heuristic","defender","aggressor","economist","breeder","forager","conservative")) {
    $eval_dir = Join-Path $run_dir "eval_vs_$opp"
    & $bench_exe --left "mlp:$weights" --right $opp --matches 20 --max-ticks 10000 --out $eval_dir 2>&1 | Out-Null
    if (Test-Path (Join-Path $eval_dir "SUMMARY.md")) {
        $line = Get-Content (Join-Path $eval_dir "SUMMARY.md") | Where-Object { $_ -match "^\| Left" } | Select-Object -First 1
        if ($line -match "\|\s*(\d+)\s*\|") {
            $w = [int]$Matches[1]
            $total_w += $w; $total_g += 20
            Log "  vs $($opp.PadRight(15)): MLP $w/20"
        }
    }
}
$pct = [math]::Round(100.0 * $total_w / [math]::Max($total_g, 1), 1)
Log "*** Mixed-corpus MLP vs original 7: $total_w/$total_g  ($pct%)"
Log "    (vs prior SOTA MLP_v1 = 45.7%)"
Log "=== Done ==="
