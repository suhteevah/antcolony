# scripts/run_cross_species_nest_matrix.ps1
# Cross-species win matrix in the UNDERGROUND NEST arena (5-module topology:
# 3 surface + 2 underground).  Compares against the flat chokepoint arena to
# test the intransitivity hypothesis: does defense-in-depth break the strict
# dominance hierarchy?
#
# Usage:
#   .\scripts\run_cross_species_nest_matrix.ps1
#   .\scripts\run_cross_species_nest_matrix.ps1 -Mpe 10 -MaxTicks 4000
#
param(
    [int]$Mpe = 50,
    [int]$MaxTicks = 8000,
    [string]$SpeciesDir = "assets/species"
)

$ErrorActionPreference = "Stop"
$env:RUST_LOG = if ($env:RUST_LOG) { $env:RUST_LOG } else { "info" }
New-Item -ItemType Directory -Force -Path scratch | Out-Null
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$out = "scratch/cross_species_nest_matrix_$stamp.txt"

cargo build --release -p antcolony-trainer --bin cross_species_matrix
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

& ./target/release/cross_species_matrix --species-dir $SpeciesDir --mpe $Mpe --max-ticks $MaxTicks --nest *>&1 |
    Tee-Object -FilePath $out

Write-Host "Nest win matrix written to $out"
