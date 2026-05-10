# Reports current status of the Phase 1 2yr smoke runs across kokonoe + cnc.
# Safe to run any time after run_phase1_smoke.ps1 has been launched.

$ErrorActionPreference = 'Stop'
$LocalOutDir = 'J:\antcolony\bench\smoke-phase1-2yr'
$RemoteOutRoot = '/opt/antcolony/runs/phase1-2yr'
$RemoteHost = 'cnc-server'

if (-not (Test-Path $LocalOutDir)) {
    Write-Host "No smoke run found at $LocalOutDir. Run scripts/run_phase1_smoke.ps1 first." -ForegroundColor Yellow
    exit 1
}

function Get-SpeciesProgress {
    param([string]$DailyCsv, [string]$Label)
    if (-not (Test-Path $DailyCsv)) {
        return @{ status = 'NO_DATA'; line = '(no daily.csv)'; }
    }
    $rows = Import-Csv $DailyCsv
    if ($rows.Count -eq 0) {
        return @{ status = 'EMPTY'; line = '(empty daily.csv)'; }
    }
    $lastRow = $rows[-1]
    $workers = [int]$lastRow.workers
    $food = [float]$lastRow.food
    $tick = if ($lastRow.PSObject.Properties.Name -contains 'tick') { [long]$lastRow.tick } else { -1 }
    $day = if ($lastRow.PSObject.Properties.Name -contains 'day') { [int]$lastRow.day } else { -1 }
    $status = if ($workers -le 0) { 'EXTINCT' } else { 'ALIVE' }
    return @{
        status = $status
        line   = "tick=$tick day=$day workers=$workers food=$($food.ToString('F1')) (rows=$($rows.Count))"
    }
}

Write-Host "==> Kokonoe species" -ForegroundColor Cyan
$KokonoePids = if (Test-Path "$LocalOutDir\_logs\kokonoe_pids.json") {
    Get-Content "$LocalOutDir\_logs\kokonoe_pids.json" | ConvertFrom-Json
} else { $null }

$KokonoeDirs = Get-ChildItem -Directory $LocalOutDir -Exclude '_logs' -ErrorAction SilentlyContinue
foreach ($dir in $KokonoeDirs) {
    $sp = $dir.Name
    $progress = Get-SpeciesProgress -DailyCsv "$($dir.FullName)\daily.csv" -Label $sp

    $procStatus = '?'
    if ($KokonoePids -and ($KokonoePids.PSObject.Properties.Name -contains $sp)) {
        $procId = $KokonoePids.$sp
        $proc = Get-Process -Id $procId -ErrorAction SilentlyContinue
        $procStatus = if ($proc) { "PID $procId RUNNING" } else { "PID $procId DONE" }
    }
    $color = switch ($progress.status) {
        'EXTINCT' { 'Red' }
        'ALIVE'   { 'Green' }
        default   { 'Yellow' }
    }
    Write-Host ("  {0,-30} {1,-20} {2}" -f $sp, $procStatus, $progress.line) -ForegroundColor $color
}

Write-Host ""
Write-Host "==> cnc species" -ForegroundColor Cyan
$CncStatus = ssh $RemoteHost "for d in $RemoteOutRoot/*/; do
  sp=\$(basename \$d)
  if [ -f \$d/daily.csv ]; then
    rows=\$(wc -l < \$d/daily.csv)
    last=\$(tail -1 \$d/daily.csv)
    echo \"\$sp ROWS=\$rows LAST=\$last\"
  else
    echo \"\$sp NO_DATA\"
  fi
done"

# Process status on cnc (best-effort; works if PIDs persisted).
$CncPidsJson = "$LocalOutDir\_logs\cnc_pids.json"
$CncPids = if (Test-Path $CncPidsJson) { Get-Content $CncPidsJson | ConvertFrom-Json } else { $null }
if ($CncPids) {
    Write-Host ""
    Write-Host "==> cnc process status" -ForegroundColor Cyan
    foreach ($sp in $CncPids.PSObject.Properties.Name) {
        $procId = $CncPids.$sp
        $alive = ssh $RemoteHost "if kill -0 $procId 2>/dev/null; then echo RUNNING; else echo DONE; fi"
        Write-Host ("  {0,-30} PID {1} {2}" -f $sp, $procId, $alive.Trim())
    }
}

Write-Host ""
Write-Host $CncStatus
