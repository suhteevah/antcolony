param(
    [string]$Weights = "bench/ppo-rust-r7/current.json",
    [string]$OutDir = "bench/ppo-rust-r7/eval",
    [int]$NumMatches = 50,
    [int]$MaxTicks = 5000,
    # Inference-time exploration noise. 0 = deterministic MLP. 0.05/0.1
    # turn the policy stochastic at deployment, which the diagnosis notes
    # is one of the three paths to actually clear the 47% Nash plateau.
    [float[]]$StochasticStds = @(0.0, 0.05, 0.1)
)

# Eval matrix:
#   rows = stochastic stds (incl. 0 = deterministic, the legacy comparison)
#   cols = (a) original 7 archetypes, (b) mix opponents
# Per-cell metric = aggregate win rate across NumMatches matches.

$ErrorActionPreference = "Stop"
$bench = "J:\antcolony\target\release\examples\matchup_bench.exe"

$archetypes = @("heuristic","defender","aggressor","economist","breeder","forager","conservative")

# Mix opponents -- per-tick weighted random pick over named archetypes.
# No single Nash best-response policy exists against these.
$mix_opps = @(
    @{ name = "mix_da";       spec = "mix:defender,aggressor" },
    @{ name = "mix_aef";      spec = "mix:aggressor,economist,forager" },
    @{ name = "mix_de2";      spec = "mix:defender=2,economist=1" },
    @{ name = "mix_eco_heavy"; spec = "mix:economist=2,forager=1,heuristic=1" },
    @{ name = "mix_full7";    spec = "mix:heuristic,defender,aggressor,economist,breeder,forager,conservative" }
)

if (-not (Test-Path $Weights)) {
    Write-Error "weights not found: $Weights"
    exit 1
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$report = New-Object System.Collections.Generic.List[string]
$report.Add("# PPO r7 Eval - wider bench plus stochastic-inference matrix")
$report.Add("")
$report.Add("Weights: $Weights  /  $NumMatches matches per cell  /  max-ticks $MaxTicks")
$report.Add("")

function Run-Cell {
    param([string]$leftSpec, [string]$rightSpec, [string]$cellDir)
    & $bench --left $leftSpec --right $rightSpec --matches $NumMatches --max-ticks $MaxTicks --out $cellDir 2>&1 | Out-Null
    $sumPath = Join-Path $cellDir "SUMMARY.md"
    if (-not (Test-Path $sumPath)) { return $null }
    $line = Get-Content $sumPath | Where-Object { $_ -match "^\| Left" } | Select-Object -First 1
    if ($line -match '\|\s*(\d+)\s*\|\s*([\d\.]+)') {
        return @{ wins = [int]$Matches[1]; pct = [float]$Matches[2] }
    }
    return $null
}

foreach ($std in $StochasticStds) {
    if ($std -eq 0.0) {
        $leftSpec = "mlp:$Weights"
        $stdLabel = "deterministic"
        $rowLabel = "det"
    } else {
        $leftSpec = "noisy_mlp:${Weights}:${std}"
        $stdLabel = "noisy std=$std"
        $rowLabel = "n$std"
    }
    Write-Host ""
    Write-Host "=== Left = $stdLabel ($leftSpec) ==="

    # ---- Original 7 archetypes ----
    $arch_w = 0; $arch_g = 0
    foreach ($opp in $archetypes) {
        $cell = Join-Path $OutDir ("$rowLabel" + "_vs_" + $opp)
        $r = Run-Cell $leftSpec $opp $cell
        if ($null -ne $r) {
            $arch_w += $r.wins; $arch_g += $NumMatches
            Write-Host ("  {0,-15} {1}/{2}  ({3:N1} pct)" -f $opp, $r.wins, $NumMatches, $r.pct)
        }
    }
    $arch_pct = if ($arch_g -gt 0) { [math]::Round(100.0 * $arch_w / $arch_g, 1) } else { 0 }

    # ---- Mix opponents ----
    $mix_w = 0; $mix_g = 0
    foreach ($mx in $mix_opps) {
        $cell = Join-Path $OutDir ("$rowLabel" + "_vs_" + $mx.name)
        $r = Run-Cell $leftSpec $mx.spec $cell
        if ($null -ne $r) {
            $mix_w += $r.wins; $mix_g += $NumMatches
            Write-Host ("  {0,-15} {1}/{2}  ({3:N1} pct)" -f $mx.name, $r.wins, $NumMatches, $r.pct)
        }
    }
    $mix_pct = if ($mix_g -gt 0) { [math]::Round(100.0 * $mix_w / $mix_g, 1) } else { 0 }

    Write-Host ("  --- aggregate vs 7 archetypes : {0}/{1} ({2} pct)" -f $arch_w, $arch_g, $arch_pct)
    Write-Host ("  --- aggregate vs {0} mix opps  : {1}/{2} ({3} pct)" -f $mix_opps.Count, $mix_w, $mix_g, $mix_pct)

    $report.Add("## $stdLabel")
    $report.Add("")
    $report.Add("| Bench | Wins | Total | Win pct |")
    $report.Add("|-------|-----:|------:|--------:|")
    $report.Add("| 7-archetype (legacy) | $arch_w | $arch_g | $arch_pct |")
    $report.Add("| $($mix_opps.Count)-mix (wider)        | $mix_w | $mix_g | $mix_pct |")
    $report.Add("")
}

$report | Out-File -FilePath (Join-Path $OutDir "OVERALL.md") -Encoding utf8
Write-Host ""
Write-Host "Wrote $(Join-Path $OutDir 'OVERALL.md')"
