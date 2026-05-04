# Same dominance matrix as scripts/archetype_dominance_check.ps1 but on
# a 64x64 arena with 50 starting ants per colony. If combat-archetypes
# (defender, aggressor) climb out of the 16-17% basement and timeouts
# drop below 50%, the original "70% timeouts + economy dominates"
# pattern was a bench fixture problem, not a sim balance problem.

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony
$out_dir = "J:\antcolony\bench\dominance-64-50"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"
$archetypes = @("heuristic","defender","aggressor","economist","breeder","forager","conservative")
$mpp = 4   # smaller per-pair count since each match takes longer on bigger arena
$max_ticks = 15000

New-Item -ItemType Directory -Path $out_dir -Force | Out-Null

$matrix = @{}
foreach ($a in $archetypes) {
    $matrix[$a] = @{}
    foreach ($b in $archetypes) { $matrix[$a][$b] = 0 }
}

foreach ($a in $archetypes) {
    foreach ($b in $archetypes) {
        if ($a -eq $b) { continue }
        $eval_dir = Join-Path $out_dir "${a}_vs_${b}"
        & $bench_exe --left $a --right $b --matches $mpp --max-ticks $max_ticks `
            --arena-size 64 --initial-ants 50 --out $eval_dir 2>&1 | Out-Null
        if (Test-Path (Join-Path $eval_dir "SUMMARY.md")) {
            $line = Get-Content (Join-Path $eval_dir "SUMMARY.md") | Where-Object { $_ -match "^\| Left" } | Select-Object -First 1
            if ($line -match "\|\s*(\d+)\s*\|") {
                $matrix[$a][$b] = [int]$Matches[1]
            }
        }
    }
}

Write-Host "`n=== Big-arena dominance (64x64, 50 ants, 15k ticks, $mpp m/p) ==="
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
