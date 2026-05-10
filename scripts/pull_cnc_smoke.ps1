# Pulls cnc smoke results back to the local bench directory.
# Run after check_phase1_smoke.ps1 confirms all cnc procs are DONE.

$ErrorActionPreference = 'Stop'
$LocalOutDir = 'J:\antcolony\bench\smoke-phase1-2yr'
$RemoteOutRoot = '/opt/antcolony/runs/phase1-2yr'
$RemoteHost = 'cnc-server'

if (-not (Test-Path $LocalOutDir)) {
    Write-Host "No smoke run found at $LocalOutDir." -ForegroundColor Yellow
    exit 1
}

Write-Host "==> Pulling cnc smoke outputs to $LocalOutDir..."
# Tar+ssh-pipe avoids needing rsync on Windows.
$tmpTarball = "$env:TEMP\cnc_smoke_pull.tar"
ssh $RemoteHost "cd $RemoteOutRoot && tar -cf - --exclude='_logs/*' [a-z]*" | Set-Content -Path $tmpTarball -AsByteStream
if ($LASTEXITCODE -ne 0) { throw "Failed to tar cnc outputs" }

Push-Location $LocalOutDir
try {
    tar -xf $tmpTarball
    if ($LASTEXITCODE -ne 0) { throw "Failed to extract cnc outputs" }
} finally {
    Pop-Location
    Remove-Item $tmpTarball -ErrorAction SilentlyContinue
}

Write-Host "==> Pull complete. Summary of pulled species:"
Get-ChildItem -Directory $LocalOutDir -Exclude '_logs' | ForEach-Object {
    $sp = $_.Name
    $dailyCsv = "$($_.FullName)\daily.csv"
    if (Test-Path $dailyCsv) {
        $rows = (Get-Content $dailyCsv).Count
        Write-Host "  $sp : $rows rows in daily.csv"
    } else {
        Write-Host "  $sp : NO daily.csv" -ForegroundColor Yellow
    }
}
