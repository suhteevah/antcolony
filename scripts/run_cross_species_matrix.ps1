# scripts/run_cross_species_matrix.ps1
# Cross-species win-matrix harness. Writes the matrix + intransitivity
# report to scratch/. Run from repo root.
#
# Usage:
#   .\scripts\run_cross_species_matrix.ps1
#   .\scripts\run_cross_species_matrix.ps1 -Mpe 10 -MaxTicks 4000
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
$out = "scratch/cross_species_matrix_$stamp.txt"

cargo build --release -p antcolony-trainer --bin cross_species_matrix
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

& ./target/release/cross_species_matrix --species-dir $SpeciesDir --mpe $Mpe --max-ticks $MaxTicks *>&1 |
    Tee-Object -FilePath $out

Write-Host "Win matrix written to $out"
