$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony
$out = "J:\antcolony\bench\test_out.log"
"START $(Get-Date -Format o)" | Out-File -FilePath $out -Encoding utf8
cargo test -p antcolony-sim --lib bench:: 2>&1 | Tee-Object -FilePath $out -Append
cargo test -p antcolony-sim --lib species_extended:: 2>&1 | Tee-Object -FilePath $out -Append
"EXIT $LASTEXITCODE $(Get-Date -Format o)" | Add-Content -Path $out
