$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony
$out = "J:\antcolony\bench\determinism_out.log"
"START $(Get-Date -Format o)" | Out-File -FilePath $out -Encoding utf8
cargo test --release -p antcolony-sim --test bench_determinism 2>&1 | Tee-Object -FilePath $out -Append
"EXIT $LASTEXITCODE $(Get-Date -Format o)" | Add-Content -Path $out
