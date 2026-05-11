# Launches the Phase 1 2yr 10-species smoke on cnc-server only.
# Strict 2-at-a-time queue managed by /opt/antcolony/scripts/queue_smoke.sh
# running under nohup on cnc — survives ssh disconnect.
#
# Kokonoe is intentionally not used by this launcher.

$ErrorActionPreference = 'Stop'
$LocalRoot     = 'J:\antcolony'
$LocalOutDir   = "$LocalRoot\bench\smoke-phase1-2yr"
$RemoteRoot    = '/opt/antcolony'
$RemoteOutRoot = "$RemoteRoot/runs/phase1-2yr"
$RemoteScripts = "$RemoteRoot/scripts"
$RemoteHost    = 'cnc-server'

Write-Host "==> Phase 1 2yr smoke launcher (cnc-only, 2-at-a-time queue)"

# ---- Safety: refuse to launch if cnc already has a queue or smoke running ----
$existing = ssh $RemoteHost "ps aux | grep -E 'smoke_10yr_ai|queue_smoke.sh' | grep -v grep || true"
if ($existing) {
    Write-Host "==> ABORT: cnc already has smoke/queue processes:" -ForegroundColor Red
    Write-Host $existing
    Write-Host "Kill them first (ssh cnc-server 'pkill -f smoke_10yr_ai; pkill -f queue_smoke.sh')."
    exit 1
}

# ---- Build (incremental; no-op if binary is current) ----
# -j 2 to leave fleet headroom on cnc (i5-4690K, 4 cores).
Write-Host ""
Write-Host "==> cnc: ensuring smoke binary is current (-j 2)..."
$buildCmd = 'cd ' + $RemoteRoot +
            ' ; RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER=' +
            ' cargo build --release -p antcolony-sim --example smoke_10yr_ai -j 2 2>&1 | tail -10'
ssh $RemoteHost $buildCmd
if ($LASTEXITCODE -ne 0) { throw "cnc build failed" }

# ---- Prep output root and ship queue script ----
Write-Host ""
Write-Host "==> cnc: preparing $RemoteOutRoot and shipping queue script..."
$prep = 'mkdir -p ' + $RemoteOutRoot + '/_logs ' + $RemoteScripts +
        ' ; rm -rf ' + $RemoteOutRoot + '/[a-z]*' +
        ' ; rm -f ' + $RemoteOutRoot + '/_logs/*.log* ' + $RemoteOutRoot + '/_logs/queue.*'
ssh $RemoteHost $prep
if ($LASTEXITCODE -ne 0) { throw "cnc prep failed" }

scp "$LocalRoot\scripts\queue_smoke.sh" "${RemoteHost}:$RemoteScripts/queue_smoke.sh"
if ($LASTEXITCODE -ne 0) { throw "scp queue_smoke.sh failed" }
ssh $RemoteHost "chmod +x $RemoteScripts/queue_smoke.sh ; dos2unix $RemoteScripts/queue_smoke.sh 2>/dev/null ; sed -i 's/\r$//' $RemoteScripts/queue_smoke.sh"

# ---- Local output dir (for pulled csvs later) ----
if (Test-Path $LocalOutDir) { Remove-Item -Recurse -Force $LocalOutDir }
New-Item -ItemType Directory -Force -Path "$LocalOutDir\_logs" | Out-Null

# ---- Launch queue under nohup (survives ssh drop) ----
Write-Host ""
Write-Host "==> cnc: launching queue_smoke.sh under nohup..."
$launchCmd = 'cd ' + $RemoteRoot +
             ' ; nohup ' + $RemoteScripts + '/queue_smoke.sh' +
             ' > ' + $RemoteOutRoot + '/_logs/queue.stdout' +
             ' 2> ' + $RemoteOutRoot + '/_logs/queue.stderr' +
             ' < /dev/null & echo $!'
$queuePid = (ssh $RemoteHost $launchCmd) -as [int]
if (-not $queuePid) { throw "Failed to launch queue_smoke.sh" }
@{ queue_pid = $queuePid; host = $RemoteHost; launched_at = (Get-Date).ToString('o') } |
    ConvertTo-Json | Set-Content -Path "$LocalOutDir\_logs\cnc_queue.json" -Encoding utf8

Start-Sleep -Seconds 3
$alive = ssh $RemoteHost "kill -0 $queuePid 2>/dev/null && echo ALIVE || echo DEAD"
Write-Host ("    queue PID {0}: {1}" -f $queuePid, $alive.Trim())

Write-Host ""
Write-Host "==> Queue launched on cnc ($queuePid)."
Write-Host "    10 species, 2 concurrent at a time, ~3-4 days wall-clock total."
Write-Host "    Outputs:   ${RemoteHost}:$RemoteOutRoot/<species>/daily.csv"
Write-Host "    Logs:      ${RemoteHost}:$RemoteOutRoot/_logs/{queue.log,<species>.log.*}"
Write-Host ""
Write-Host "    Check status:  .\scripts\check_phase1_smoke.ps1"
Write-Host "    Pull results:  .\scripts\pull_cnc_smoke.ps1   (after queue.done appears)"
Write-Host "    Tail queue:    ssh cnc-server 'tail -f $RemoteOutRoot/_logs/queue.log'"
