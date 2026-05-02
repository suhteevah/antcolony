# Overnight species bench — 5-year Seasonal run for all 7 species.
#
# Estimated wall time: ~6-12 hrs depending on per-tick cost.
# Output: bench/overnight-<date>/{*.csv,*.md,SUMMARY.md,run.log}
#
# Run from any shell:
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\overnight_bench.ps1

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$stamp = Get-Date -Format 'yyyy-MM-dd'
$out = "bench\overnight-$stamp"
New-Item -ItemType Directory -Path $out -Force | Out-Null

$log = Join-Path $out 'run.log'
"START $(Get-Date -Format o)" | Out-File -FilePath $log -Encoding utf8
"git: $((git rev-parse HEAD).Trim())" | Add-Content -Path $log
"host: $env:COMPUTERNAME" | Add-Content -Path $log

# Build release first so the timer below excludes compile time.
"BUILD START $(Get-Date -Format o)" | Add-Content -Path $log
cargo build --release -p antcolony-sim --example species_bench 2>&1 | Tee-Object -FilePath $log -Append
"BUILD END $(Get-Date -Format o)" | Add-Content -Path $log

"BENCH START $(Get-Date -Format o)" | Add-Content -Path $log
cargo run --release --example species_bench -- `
  --all --years 5 --scale seasonal --seed 42 --sample-every-days 7 --out $out `
  2>&1 | Tee-Object -FilePath $log -Append
"BENCH END $(Get-Date -Format o)" | Add-Content -Path $log
"EXIT $LASTEXITCODE" | Add-Content -Path $log
