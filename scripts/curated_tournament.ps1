# Curated tournament: 7 originals + 5 strong variants (50% vs heuristic
# in the scoreboard pass). Drops glass_cannon, berserker, and other weak
# teachers that polluted the variant corpus.
#
# Hypothesis: variant tournament regressed (28.6%) not because of
# diversity itself, but because losing strategies generated bad teaching
# signal. A curated pool of viable strategies should match or beat
# DAgger v1's 40.7%.

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$run_dir   = "J:\antcolony\bench\curated-tournament"
$traj      = Join-Path $run_dir "trajectories.jsonl"
$filtered  = Join-Path $run_dir "trajectories_filtered.jsonl"
$weights   = Join-Path $run_dir "mlp_weights_curated.json"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"
$matches_per_pair = 4   # 12*11*4 = 528 matches
$max_ticks = 10000

New-Item -ItemType Directory -Path $run_dir -Force | Out-Null
$logfile = Join-Path $run_dir "run.log"
function Log { param($m); $stamp = Get-Date -Format 'HH:mm:ss'; "[$stamp] $m" | Tee-Object -FilePath $logfile -Append }

# Curated pool: 7 originals + 5 variants that scored 50% vs heuristic.
$brains = @(
    @{ name = "heuristic";    spec = "heuristic" }
    @{ name = "defender";     spec = "defender" }
    @{ name = "aggressor";    spec = "aggressor" }
    @{ name = "economist";    spec = "economist" }
    @{ name = "breeder";      spec = "breeder" }
    @{ name = "forager";      spec = "forager" }
    @{ name = "conservative"; spec = "conservative" }
    @{ name = "queen_focus";  spec = "tuned:queen_focus:0.40,0.05,0.55,0.40,0.20,0.40,0.3,0.6,30" }
    @{ name = "alate_swarm";  spec = "tuned:alate_swarm:0.35,0.10,0.55,0.65,0.10,0.25,0.4,0.8,25" }
    @{ name = "pure_econ";    spec = "tuned:pure_econ:0.90,0.02,0.08,0.95,0.02,0.03,0.0,0.5,15" }
    @{ name = "worker_swarm"; spec = "tuned:worker_swarm:0.92,0.05,0.03,0.80,0.05,0.15,0.0,0.8,25" }
    @{ name = "nurse_heavy";  spec = "tuned:nurse_heavy:0.60,0.10,0.30,0.30,0.10,0.60,0.4,0.5,25" }
)

Log "=== Curated tournament: $($brains.Count) brains, $matches_per_pair matches/pair ==="
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

$total_records = (Get-Content $traj | Measure-Object -Line).Lines
Log "  Total trajectory records: $total_records"

$kept = New-Object System.Collections.ArrayList
Get-Content $traj | ForEach-Object {
    try {
        $r = $_ | ConvertFrom-Json
        if ($r.outcome_for_this_colony -ge 0.55) { [void]$kept.Add($_) }
    } catch {}
}
$kept | Set-Content -Path $filtered -Encoding utf8
Log "  Filtered (winners): $($kept.Count) records"

Log "=== Train MLP on curated corpus ==="
$train = python "J:\antcolony\scripts\train_mlp_brain.py" `
    --trajectories $filtered --out $weights `
    --hidden 64 --epochs 100 --lr 1e-3 --device cuda 2>&1 | Select-Object -Last 6
foreach ($l in $train) { Log "  $l" }

Log "=== Eval curated-trained MLP vs original 7 ==="
foreach ($opp in @("heuristic","defender","aggressor","economist","breeder","forager","conservative")) {
    $eval_dir = Join-Path $run_dir "eval_vs_$opp"
    & $bench_exe --left "mlp:$weights" --right $opp --matches 20 --max-ticks $max_ticks --out $eval_dir 2>&1 | Out-Null
}
Log "=== Done ==="
