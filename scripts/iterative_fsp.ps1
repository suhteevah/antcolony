# Iterative fictitious self-play (FSP). Each round:
#   1. Round-robin tournament across the current brain pool
#   2. Train a new MLP on filtered winning trajectories
#   3. Eval new MLP vs the original 7 archetypes
#   4. ADD the new MLP to the pool for the next round
#
# Hypothesis: each new MLP version brings novel strategic shape into
# the pool. The next round's training corpus contains strategies the
# previous MLP couldn't have learned from. Plateau is delayed until
# the strategy space saturates.
#
# Usage: scripts/iterative_fsp.ps1 [-Rounds 3] [-MatchesPerPair 1]

param(
    [int]$Rounds = 3,
    [int]$MatchesPerPair = 1
)

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$root_dir = "J:\antcolony\bench\iterative-fsp"
$pool_file = Join-Path $root_dir "brain_pool.tsv"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"
$max_ticks = 10000

if (-not (Test-Path $pool_file)) {
    Write-Host "FATAL: brain_pool.tsv not found. Run scripts/generate_full_brain_pool.py first."
    exit 1
}

# Load base 49-brain pool from TSV.
$base_pool = @()
foreach ($line in Get-Content $pool_file) {
    $parts = $line -split "`t"
    if ($parts.Count -eq 2) {
        $base_pool += @{ name = $parts[0]; spec = $parts[1] }
    }
}

$global_log = Join-Path $root_dir "fsp_master.log"
function GLog { param($m); $stamp = Get-Date -Format 'HH:mm:ss'; "[$stamp] $m" | Tee-Object -FilePath $global_log -Append }

GLog "=== Iterative FSP starting: $Rounds rounds, $MatchesPerPair m/p, $($base_pool.Count) base brains ==="

$pool = $base_pool.Clone()
$mlp_versions = @()  # tracked across rounds

for ($round = 1; $round -le $Rounds; $round++) {
    $round_dir = Join-Path $root_dir "round_$round"
    $traj      = Join-Path $round_dir "trajectories.jsonl"
    $filtered  = Join-Path $round_dir "trajectories_filtered.jsonl"
    $weights   = Join-Path $round_dir "mlp_weights_v$round.json"
    New-Item -ItemType Directory -Path $round_dir -Force | Out-Null
    if (Test-Path $traj) { Remove-Item $traj }

    GLog "=== Round ${round}/${Rounds}: $($pool.Count) brains in pool, $($pool.Count * ($pool.Count - 1) * $MatchesPerPair) matches ==="

    $pair_count = 0
    $total_pairs = $pool.Count * ($pool.Count - 1)
    foreach ($a in $pool) {
        foreach ($b in $pool) {
            if ($a.name -eq $b.name) { continue }
            $pair_count++
            $tmp = Join-Path $round_dir "tmp.jsonl"
            & $bench_exe --left $a.spec --right $b.spec --matches $MatchesPerPair `
                --max-ticks $max_ticks --dump-trajectories $tmp 2>&1 | Out-Null
            if (Test-Path $tmp) {
                Get-Content $tmp | Add-Content -Path $traj -Encoding utf8
                Remove-Item $tmp
            }
            if ($pair_count % 100 -eq 0) {
                GLog "  round ${round}: $pair_count/$total_pairs pairings done"
            }
        }
    }
    GLog "  round ${round}: $((Get-Content $traj | Measure-Object -Line).Lines) trajectory records"

    # Filter winners + train.
    $kept = New-Object System.Collections.ArrayList
    Get-Content $traj | ForEach-Object {
        try {
            $r = $_ | ConvertFrom-Json
            if ($r.outcome_for_this_colony -ge 0.55) { [void]$kept.Add($_) }
        } catch {}
    }
    $kept | Set-Content -Path $filtered -Encoding utf8
    GLog "  round ${round}: filtered $($kept.Count) winning records"

    GLog "=== Round ${round}: train MLP_v$round ==="
    $train = python "J:\antcolony\scripts\train_mlp_brain.py" `
        --trajectories $filtered --out $weights `
        --hidden 64 --epochs 100 --lr 1e-3 --device cuda 2>&1 | Select-Object -Last 6
    foreach ($l in $train) { GLog "  $l" }

    GLog "=== Round ${round}: eval MLP_v$round vs original 7 ==="
    $total_w = 0; $total_g = 0
    foreach ($opp in @("heuristic","defender","aggressor","economist","breeder","forager","conservative")) {
        $eval_dir = Join-Path $round_dir "eval_vs_$opp"
        & $bench_exe --left "mlp:$weights" --right $opp --matches 20 --max-ticks $max_ticks --out $eval_dir 2>&1 | Out-Null
        if (Test-Path (Join-Path $eval_dir "SUMMARY.md")) {
            $line = Get-Content (Join-Path $eval_dir "SUMMARY.md") | Where-Object { $_ -match "^\| Left" } | Select-Object -First 1
            if ($line -match "\|\s*(\d+)\s*\|") {
                $w = [int]$Matches[1]
                $total_w += $w
                $total_g += 20
                GLog "  vs $($opp.PadRight(15)): MLP $w/20"
            }
        }
    }
    $pct = [math]::Round(100.0 * $total_w / [math]::Max($total_g, 1), 1)
    GLog "*** Round $round MLP_v$round vs original 7: $total_w/$total_g  ($pct%)"

    # Add this MLP to the pool for next round.
    $mlp_versions += @{ name = "mlp_v$round"; spec = "mlp:$weights"; weights = $weights; pct = $pct }
    $pool += @{ name = "mlp_v$round"; spec = "mlp:$weights" }
}

GLog ""
GLog "=== FSP progression ==="
foreach ($v in $mlp_versions) {
    GLog "  $($v.name): $($v.pct)%"
}
GLog "=== Done ==="
