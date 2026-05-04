# Adversarial FSP — iterative training that ACTUALLY iterates.
#
# Each round:
#   1. Eval current MLP vs original 7 archetypes
#   2. Pick the 2 weakest matchups (highest opponent win rate)
#   3. Generate 3 variants of each weak archetype (params perturbed +/-15%)
#   4. Have current MLP play vs the full pool (49 species + 6 new variants)
#   5. ADVERSARIAL FILTER: keep ONLY trajectories where the OPPONENT won
#      (i.e., the decisions of brains that beat the current MLP)
#   6. Train MLP_v(n+1) on this "what beat me" corpus
#   7. Eval, repeat
#
# Hypothesis: vanilla FSP plateaus because BC produces an AVERAGE of
# its teachers. Adversarial filter forces each round to learn responses
# specifically to its own weaknesses. Targeted variants explore strategy
# space the model can't yet handle.
#
# Usage: scripts/adversarial_fsp.ps1 [-StartWeights <path>] [-Rounds 3]

param(
    [string]$StartWeights = "J:\antcolony\bench\iterative-fsp\round_1\mlp_weights_v1.json",
    [int]$Rounds = 3,
    [int]$MatchesPerOpponent = 16
)

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$root_dir = "J:\antcolony\bench\adversarial-fsp"
$pool_file = "J:\antcolony\bench\iterative-fsp\brain_pool.tsv"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"
$max_ticks = 10000
$archetypes = @("heuristic","defender","aggressor","economist","breeder","forager","conservative")

if (-not (Test-Path $StartWeights)) { Write-Host "FATAL: StartWeights not found: $StartWeights"; exit 1 }
if (-not (Test-Path $pool_file)) { Write-Host "FATAL: brain_pool.tsv not found"; exit 1 }

# Load 49-brain species pool.
$species_pool = @()
foreach ($line in Get-Content $pool_file) {
    $parts = $line -split "`t"
    if ($parts.Count -eq 2) {
        $species_pool += @{ name = $parts[0]; spec = $parts[1] }
    }
}

New-Item -ItemType Directory -Path $root_dir -Force | Out-Null
$global_log = Join-Path $root_dir "adv_fsp.log"
function GLog { param($m); $stamp = Get-Date -Format 'HH:mm:ss'; "[$stamp] $m" | Tee-Object -FilePath $global_log -Append }

GLog "=== Adversarial FSP starting: $Rounds rounds, start=$(Split-Path -Leaf $StartWeights) ==="

$current_weights = $StartWeights
$mlp_history = @()

for ($round = 1; $round -le $Rounds; $round++) {
    $round_dir = Join-Path $root_dir "round_$round"
    New-Item -ItemType Directory -Path $round_dir -Force | Out-Null

    # === Step 1: Eval current MLP to find weak matchups ===
    GLog "=== Round ${round}/${Rounds} step 1: scoreboard current MLP ==="
    $scoreboard = @{}
    foreach ($opp in $archetypes) {
        $eval_dir = Join-Path $round_dir "scoreboard_vs_$opp"
        & $bench_exe --left "mlp:$current_weights" --right $opp --matches 8 --max-ticks $max_ticks --out $eval_dir 2>&1 | Out-Null
        if (Test-Path (Join-Path $eval_dir "SUMMARY.md")) {
            $line = Get-Content (Join-Path $eval_dir "SUMMARY.md") | Where-Object { $_ -match "^\| Right" } | Select-Object -First 1
            if ($line -match "\|\s*(\d+)\s*\|") { $scoreboard[$opp] = [int]$Matches[1] }
        }
    }
    # Sort opponents by THEIR win count (highest = weakest matchup for MLP)
    $sorted = $scoreboard.GetEnumerator() | Sort-Object Value -Descending
    $weakest = @($sorted | Select-Object -First 2 | ForEach-Object { $_.Key })
    GLog "  weakest matchups: $($weakest -join ', ')"

    # === Step 2: Generate variants of weak archetypes ===
    GLog "=== Round ${round} step 2: generate variants ==="
    $variants = @()
    $vseed = $round * 100
    foreach ($arch in $weakest) {
        $vlines = python "J:\antcolony\scripts\generate_archetype_variants.py" `
            --archetype $arch --n 3 --magnitude 0.15 --seed $vseed 2>&1
        foreach ($vl in $vlines) {
            $parts = $vl -split "`t"
            if ($parts.Count -eq 2) {
                # Prefix variant name with round to avoid collisions across rounds
                $variants += @{ name = "r${round}_$($parts[0])"; spec = $parts[1] }
            }
        }
        $vseed += 1
    }
    GLog "  generated $($variants.Count) variants"

    # Combined pool for this round = species + variants. Current MLP plays vs each.
    $opp_pool = $species_pool + $variants
    GLog "=== Round ${round} step 3: MLP vs $($opp_pool.Count) opponents, $MatchesPerOpponent matches each ==="

    $traj = Join-Path $round_dir "trajectories.jsonl"
    $filtered = Join-Path $round_dir "trajectories_adv_filtered.jsonl"
    if (Test-Path $traj) { Remove-Item $traj }

    $opp_idx = 0
    foreach ($opp in $opp_pool) {
        $opp_idx++
        $tmp = Join-Path $round_dir "tmp.jsonl"
        & $bench_exe --left "mlp:$current_weights" --right $opp.spec --matches $MatchesPerOpponent `
            --max-ticks $max_ticks --dump-trajectories $tmp 2>&1 | Out-Null
        if (Test-Path $tmp) {
            Get-Content $tmp | Add-Content -Path $traj -Encoding utf8
            Remove-Item $tmp
        }
        if ($opp_idx % 20 -eq 0) {
            GLog "  $opp_idx/$($opp_pool.Count) opponents played"
        }
    }
    GLog "  total trajectory records: $((Get-Content $traj | Measure-Object -Line).Lines)"

    # === Step 4: ADVERSARIAL FILTER ===
    # Keep records where colony=1 (right = the opponent) AND outcome=1.0 (opponent won).
    # These are the decisions of brains that beat the current MLP.
    GLog "=== Round ${round} step 4: adversarial filter (opponent-wins only) ==="
    $kept = New-Object System.Collections.ArrayList
    Get-Content $traj | ForEach-Object {
        try {
            $r = $_ | ConvertFrom-Json
            # Adversarial: opponent (colony 1) won this match
            if ($r.colony -eq 1 -and $r.outcome_for_this_colony -ge 0.55) {
                [void]$kept.Add($_)
            }
        } catch {}
    }
    $kept | Set-Content -Path $filtered -Encoding utf8
    GLog "  adversarial-filtered (opponent wins): $($kept.Count) records"

    if ($kept.Count -lt 1000) {
        GLog "  WARNING: fewer than 1000 adversarial records; iteration may overfit. Continuing anyway."
    }

    # === Step 5: Train new MLP ===
    $new_weights = Join-Path $round_dir "mlp_weights_adv_v$round.json"
    GLog "=== Round ${round} step 5: train MLP_adv_v$round ==="
    $train = python "J:\antcolony\scripts\train_mlp_brain.py" `
        --trajectories $filtered --out $new_weights `
        --hidden 64 --epochs 100 --lr 1e-3 --device cuda 2>&1 | Select-Object -Last 6
    foreach ($l in $train) { GLog "  $l" }

    # === Step 6: Eval new MLP ===
    GLog "=== Round ${round} step 6: eval MLP_adv_v$round vs original 7 ==="
    $total_w = 0; $total_g = 0
    foreach ($opp in $archetypes) {
        $eval_dir = Join-Path $round_dir "eval_vs_$opp"
        & $bench_exe --left "mlp:$new_weights" --right $opp --matches 20 --max-ticks $max_ticks --out $eval_dir 2>&1 | Out-Null
        if (Test-Path (Join-Path $eval_dir "SUMMARY.md")) {
            $line = Get-Content (Join-Path $eval_dir "SUMMARY.md") | Where-Object { $_ -match "^\| Left" } | Select-Object -First 1
            if ($line -match "\|\s*(\d+)\s*\|") {
                $w = [int]$Matches[1]
                $total_w += $w; $total_g += 20
                GLog "  vs $($opp.PadRight(15)): MLP $w/20"
            }
        }
    }
    $pct = [math]::Round(100.0 * $total_w / [math]::Max($total_g, 1), 1)
    GLog "*** Round $round MLP_adv_v$round vs original 7: $total_w/$total_g  ($pct%)"

    $mlp_history += @{ round = $round; weights = $new_weights; pct = $pct }
    $current_weights = $new_weights  # next round trains on the descendant
}

GLog ""
GLog "=== Adversarial FSP progression ==="
GLog "  start (MLP_v1):     45.7%"
foreach ($h in $mlp_history) {
    GLog "  adv_v$($h.round):     $($h.pct)%"
}
GLog "=== Done ==="
