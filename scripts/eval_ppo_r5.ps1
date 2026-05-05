param(
    [string]$Weights = "bench/ppo-rust-r5/current.json",
    [string]$OutDir = "bench/ppo-rust-r5/eval",
    [int]$NumMatches = 20,
    [int]$MaxTicks = 5000
)

$ErrorActionPreference = "Stop"
$bench = "J:\antcolony\target\release\examples\matchup_bench.exe"
$archetypes = @("heuristic","defender","aggressor","economist","breeder","forager","conservative")

if (-not (Test-Path $Weights)) {
    Write-Error "weights not found: $Weights"
    exit 1
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$total_w = 0
$total_g = 0
$lines = @()

foreach ($opp in $archetypes) {
    $dir = Join-Path $OutDir "vs_$opp"
    & $bench --left "mlp:$Weights" --right $opp --matches $NumMatches --max-ticks $MaxTicks --out $dir 2>&1 | Out-Null
    $sumPath = Join-Path $dir "SUMMARY.md"
    if (Test-Path $sumPath) {
        $line = Get-Content $sumPath | Where-Object { $_ -match "^\| Left" } | Select-Object -First 1
        if ($line -match "\|\s*(\d+)\s*\|") {
            $w = [int]$Matches[1]
            $total_w += $w
            $total_g += $NumMatches
            $rec = "  vs $($opp.PadRight(15)): MLP $w/$NumMatches"
            $lines += $rec
            Write-Host $rec
        }
    }
}

$pct = [math]::Round(100.0 * $total_w / [math]::Max($total_g, 1), 1)
$summary = "*** PPO r5 vs original 7: $total_w/$total_g  ($pct%)"
Write-Host ""
Write-Host $summary
$lines += ""
$lines += $summary
$lines | Out-File -FilePath (Join-Path $OutDir "OVERALL.md") -Encoding utf8
