# Diagnose: does forager dominate ALL archetypes, or only reactive ones?
# 7x7 round-robin, 8 m/p. ~2 min. If forager wins 65%+ vs every other
# archetype, sim balance is the problem (not ML). If forager loses
# to certain commit-style specialists (economist, breeder), the issue
# is reactive-vs-proactive, which BC structurally can't fix.

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$out_dir = "J:\antcolony\bench\archetype-dominance"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"
$archetypes = @("heuristic","defender","aggressor","economist","breeder","forager","conservative")
$mpp = 8
$max_ticks = 10000

New-Item -ItemType Directory -Path $out_dir -Force | Out-Null

# Run all pairings. Print as a matrix.
$matrix = @{}
foreach ($a in $archetypes) {
    $matrix[$a] = @{}
    foreach ($b in $archetypes) { $matrix[$a][$b] = 0 }
}

foreach ($a in $archetypes) {
    foreach ($b in $archetypes) {
        if ($a -eq $b) { continue }
        $eval_dir = Join-Path $out_dir "${a}_vs_${b}"
        & $bench_exe --left $a --right $b --matches $mpp --max-ticks $max_ticks --out $eval_dir 2>&1 | Out-Null
        if (Test-Path (Join-Path $eval_dir "SUMMARY.md")) {
            $line = Get-Content (Join-Path $eval_dir "SUMMARY.md") | Where-Object { $_ -match "^\| Left" } | Select-Object -First 1
            if ($line -match "\|\s*(\d+)\s*\|") {
                $matrix[$a][$b] = [int]$Matches[1]
            }
        }
    }
}

# Print matrix.
Write-Host "`n=== Archetype dominance matrix (rows = LEFT brain, columns = RIGHT brain) ==="
Write-Host "    Cell value = LEFT wins out of $mpp matches"
Write-Host ""
$header = "  {0,-13}" -f ""
foreach ($b in $archetypes) { $header += " {0,-7}" -f ($b.Substring(0, [Math]::Min(7, $b.Length))) }
Write-Host $header
foreach ($a in $archetypes) {
    $row = "  {0,-13}" -f $a
    foreach ($b in $archetypes) {
        if ($a -eq $b) { $row += " {0,-7}" -f "  -" }
        else { $row += " {0,-7}" -f $matrix[$a][$b] }
    }
    Write-Host $row
}

# Compute per-archetype mean win rate vs others.
Write-Host "`n=== Mean win rate (each archetype vs the other 6) ==="
foreach ($a in $archetypes) {
    $wins = 0; $total = 0
    foreach ($b in $archetypes) {
        if ($a -eq $b) { continue }
        $wins += $matrix[$a][$b]
        $total += $mpp
    }
    $pct = if ($total -gt 0) { [math]::Round(100.0 * $wins / $total, 1) } else { 0 }
    Write-Host "  $($a.PadRight(13)) : $wins/$total ($pct%)"
}
