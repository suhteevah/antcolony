# Canonical species-blend tournament: each species paired with the
# archetype that matches its real-world ecological strategy. This keeps
# BOTH biological grounding AND strategic diversity:
#
#   Formica rufa            x Aggressor   - mass raids, formic-acid spray
#   Aphaenogaster rudis     x Forager     - timid scavenger
#   Pogonomyrmex occidentalis x Economist - seed-harvester accumulator
#   Camponotus pennsylvanicus x Defender  - slow-growth carpenter, fortified
#   Tapinoma sessile        x Breeder     - supercolony alate-budding
#
# 12 brains total (7 originals + 5 canonical species blends), 4 m/p.

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$run_dir   = "J:\antcolony\bench\species-canon-tournament"
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
    @{ name = "formica_a";    spec = "tuned:formica_aggressor:0.635,0.325,0.040,0.591,0.153,0.255,1.65,0.55,25.0" }
    @{ name = "aphaeno_f";    spec = "tuned:aphaeno_forager:0.960,0.000,0.040,0.721,0.145,0.133,0.25,0.88,25.0" }
    @{ name = "pogono_e";     spec = "tuned:pogono_economist:0.910,0.025,0.065,0.695,0.148,0.158,0.70,0.30,15.0" }
    @{ name = "campo_d";      spec = "tuned:campo_defender:0.685,0.275,0.040,0.342,0.211,0.447,0.55,0.55,20.0" }
    @{ name = "tapinoma_b";   spec = "tuned:tapinoma_breeder:0.760,0.025,0.215,0.568,0.145,0.286,0.55,0.70,22.5" }
)

Log "=== Species-canon tournament: $($brains.Count) brains, $matches_per_pair m/p ==="
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

Log "=== Train MLP ==="
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
