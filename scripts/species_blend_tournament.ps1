# Species-blend tournament: same 12-brain structure as curated (7 originals
# + 5 extras), but the extras are species-baseline × heuristic-archetype
# blends instead of made-up parameter perturbations. Apples-to-apples
# vs curated 41.9%.
#
# The 5 species chosen for max biological diversity:
#   formica_rufa (aggro=0.9, mass)        - extreme high-aggression mass-recruiter
#   aphaenogaster_rudis (aggro=0.25, tandem) - low-aggression solitary-tandem
#   pogonomyrmex_occidentalis (aggro=0.7, group) - defensive granivore
#   camponotus_pennsylvanicus (aggro=0.4, tandem, has 10% soldier) - polymorphic carpenter
#   tapinoma_sessile (aggro=0.3, mass)    - fast subordinate competitor

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$run_dir   = "J:\antcolony\bench\species-blend-tournament"
$traj      = Join-Path $run_dir "trajectories.jsonl"
$filtered  = Join-Path $run_dir "trajectories_filtered.jsonl"
$weights   = Join-Path $run_dir "mlp_weights.json"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"
$matches_per_pair = 4
$max_ticks = 10000

New-Item -ItemType Directory -Path $run_dir -Force | Out-Null
$logfile = Join-Path $run_dir "run.log"
function Log { param($m); $stamp = Get-Date -Format 'HH:mm:ss'; "[$stamp] $m" | Tee-Object -FilePath $logfile -Append }

$brains = @(
    @{ name = "heuristic";    spec = "heuristic" }
    @{ name = "defender";     spec = "defender" }
    @{ name = "aggressor";    spec = "aggressor" }
    @{ name = "economist";    spec = "economist" }
    @{ name = "breeder";      spec = "breeder" }
    @{ name = "forager";      spec = "forager" }
    @{ name = "conservative"; spec = "conservative" }
    @{ name = "formica_h";    spec = "tuned:formica_rufa__heuristic:0.810,0.150,0.040,0.516,0.203,0.280,1.40,0.55,20.0" }
    @{ name = "aphaeno_h";    spec = "tuned:aphaenogaster_rudis__heuristic:0.810,0.150,0.040,0.546,0.220,0.233,0.75,0.88,20.0" }
    @{ name = "pogono_h";     spec = "tuned:pogonomyrmex_occidentalis__heuristic:0.810,0.150,0.040,0.545,0.223,0.233,1.20,0.65,20.0" }
    @{ name = "campo_h";      spec = "tuned:camponotus_pennsylvanicus__heuristic:0.760,0.200,0.040,0.517,0.261,0.222,0.90,0.80,20.0" }
    @{ name = "tapinoma_h";   spec = "tuned:tapinoma_sessile__heuristic:0.810,0.150,0.040,0.593,0.145,0.261,0.80,0.85,20.0" }
)

Log "=== Species-blend tournament: $($brains.Count) brains, $matches_per_pair m/p ==="
Log "    Total matches: $($brains.Count * ($brains.Count - 1) * $matches_per_pair)"

if (Test-Path $traj) { Remove-Item $traj }

$pair_count = 0
$total_pairs = $brains.Count * ($brains.Count - 1)
foreach ($a in $brains) {
    foreach ($b in $brains) {
        if ($a.name -eq $b.name) { continue }
        $pair_count++
        $tmp = Join-Path $run_dir "tmp_$($a.name)_vs_$($b.name).jsonl"
        & $bench_exe --left $a.spec --right $b.spec --matches $matches_per_pair `
            --max-ticks $max_ticks --dump-trajectories $tmp 2>&1 | Out-Null
        if (Test-Path $tmp) {
            Get-Content $tmp | Add-Content -Path $traj -Encoding utf8
            Remove-Item $tmp
        }
        if ($pair_count % 25 -eq 0) {
            Log "  $pair_count/$total_pairs pairings done"
        }
    }
}
Log "  Total trajectory records: $((Get-Content $traj | Measure-Object -Line).Lines)"

$kept = New-Object System.Collections.ArrayList
Get-Content $traj | ForEach-Object {
    try {
        $r = $_ | ConvertFrom-Json
        if ($r.outcome_for_this_colony -ge 0.55) { [void]$kept.Add($_) }
    } catch {}
}
$kept | Set-Content -Path $filtered -Encoding utf8
Log "  Filtered: $($kept.Count) records"

Log "=== Train MLP on species-blend corpus ==="
$train = python "J:\antcolony\scripts\train_mlp_brain.py" `
    --trajectories $filtered --out $weights `
    --hidden 64 --epochs 100 --lr 1e-3 --device cuda 2>&1 | Select-Object -Last 6
foreach ($l in $train) { Log "  $l" }

Log "=== Eval vs original 7 ==="
foreach ($opp in @("heuristic","defender","aggressor","economist","breeder","forager","conservative")) {
    $eval_dir = Join-Path $run_dir "eval_vs_$opp"
    & $bench_exe --left "mlp:$weights" --right $opp --matches 20 --max-ticks $max_ticks --out $eval_dir 2>&1 | Out-Null
}
Log "=== Done ==="
