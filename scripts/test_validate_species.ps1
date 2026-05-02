$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony
$out = "J:\antcolony\bench\test_out.log"
"START $(Get-Date -Format o)" | Out-File -FilePath $out -Encoding utf8

# Build first so timing reflects validation, not compile.
"BUILD START" | Add-Content -Path $out
cargo build --release -p antcolony-sim --bin validate-species 2>&1 | Tee-Object -FilePath $out -Append
"BUILD END" | Add-Content -Path $out

# Validate all 7 shipped species via shell glob (PowerShell expands the wildcard).
"VALIDATE START" | Add-Content -Path $out
$tomls = Get-ChildItem J:\antcolony\assets\species\*.toml | ForEach-Object { $_.FullName }
& "J:\antcolony\target\release\validate-species.exe" @tomls 2>&1 | Tee-Object -FilePath $out -Append
$validate_exit = $LASTEXITCODE
"VALIDATE END exit=$validate_exit" | Add-Content -Path $out

"EXIT $validate_exit $(Get-Date -Format o)" | Add-Content -Path $out
