# Sync local source + species TOMLs to cnc, rebuild smoke_10yr_ai, and launch
# attempt3 of the Phase 1 2yr 10-species smoke to runs/phase1-2yr-attempt3/
# (preserving attempt2 at runs/phase1-2yr/).

$ErrorActionPreference = 'Stop'
$LocalRoot     = 'J:\antcolony'
$LocalOutDir   = "$LocalRoot\bench\smoke-phase1-2yr-attempt3"
$RemoteRoot    = '/opt/antcolony'
$RemoteOutRoot = "$RemoteRoot/runs/phase1-2yr-attempt3"
$RemoteScripts = "$RemoteRoot/scripts"
$RemoteHost    = 'cnc-server'

Write-Host "==> Phase 1 attempt3 launcher" -ForegroundColor Cyan

# ---- Safety: refuse if smoke/queue is already running ----
$existing = ssh $RemoteHost "ps aux | grep -E 'smoke_10yr_ai|queue_smoke.sh' | grep -v grep || true"
if ($existing) {
    Write-Host "==> ABORT: cnc already has smoke/queue processes:" -ForegroundColor Red
    Write-Host $existing
    exit 1
}

# ---- Sync source + TOMLs via tar over ssh ----
Write-Host ""
Write-Host "==> Tar local source + TOMLs and ship to cnc..."
$tmpTar = "$env:TEMP\antcolony_sync_attempt3.tar"
Push-Location $LocalRoot
try {
    # Sim is self-contained (no bevy/candle), so only ship its src + examples
    # + the cnc-stripped workspace Cargo.toml. game/render/trainer/net stay
    # untouched on cnc but are excluded from the workspace members.
    tar -cf $tmpTar `
        crates/antcolony-sim/src `
        crates/antcolony-sim/examples `
        crates/antcolony-sim/Cargo.toml `
        assets/species `
        scripts/queue_smoke.sh
    if ($LASTEXITCODE -ne 0) { throw "tar create failed" }
    # Ship Cargo.cnc.toml separately so it lands as /opt/antcolony/Cargo.toml.
    scp "$LocalRoot\scripts\Cargo.cnc.toml" "${RemoteHost}:/tmp/Cargo.cnc.toml"
    if ($LASTEXITCODE -ne 0) { throw "scp Cargo.cnc.toml failed" }
    if ($LASTEXITCODE -ne 0) { throw "tar create failed" }
} finally {
    Pop-Location
}

scp $tmpTar "${RemoteHost}:/tmp/antcolony_sync_attempt3.tar"
if ($LASTEXITCODE -ne 0) { throw "scp tarball failed" }
Remove-Item $tmpTar -ErrorAction SilentlyContinue

ssh $RemoteHost @"
set -e
cd $RemoteRoot
tar -xf /tmp/antcolony_sync_attempt3.tar
rm /tmp/antcolony_sync_attempt3.tar
cp /tmp/Cargo.cnc.toml Cargo.toml
rm /tmp/Cargo.cnc.toml
rm -f Cargo.lock
chmod +x scripts/queue_smoke.sh
sed -i 's/\r$//' scripts/queue_smoke.sh
echo '==> sync extracted; new src files:'
ls -la Cargo.toml crates/antcolony-sim/src/colony.rs crates/antcolony-sim/src/species.rs scripts/queue_smoke.sh assets/species/lasius_niger.toml
"@
if ($LASTEXITCODE -ne 0) { throw "cnc sync extract failed" }

# ---- Cargo clean + rebuild (with proper exit-code propagation) ----
Write-Host ""
Write-Host "==> cnc: cargo clean -p antcolony-sim + rebuild smoke_10yr_ai (-j 2)..."
$buildCmd = @"
set -o pipefail
cd $RemoteRoot
RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= cargo clean -p antcolony-sim
RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= cargo build --release -p antcolony-sim --example smoke_10yr_ai -j 2 2>&1 | tail -25
rc=`${PIPESTATUS[0]}
echo "==> cargo build exit=`$rc"
exit `$rc
"@
ssh $RemoteHost $buildCmd
if ($LASTEXITCODE -ne 0) { throw "cnc cargo build failed (exit=$LASTEXITCODE)" }

# ---- Verify binary timestamp is fresh (within 5 minutes) ----
$binStat = ssh $RemoteHost "stat -c '%Y %n' $RemoteRoot/target/release/examples/smoke_10yr_ai"
Write-Host "    binary stat: $binStat"
$binEpoch = ($binStat -split ' ')[0] -as [long]
$nowEpoch = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
if (-not $binEpoch -or ($nowEpoch - $binEpoch) -gt 300) {
    throw "smoke_10yr_ai binary is stale (epoch=$binEpoch, now=$nowEpoch). Aborting launch."
}
Write-Host "    binary is fresh ($([math]::Round(($nowEpoch - $binEpoch) / 60.0, 1)) min old)"

# ---- Prep output root for attempt3 ----
Write-Host ""
Write-Host "==> cnc: preparing $RemoteOutRoot..."
ssh $RemoteHost "mkdir -p $RemoteOutRoot/_logs ; rm -rf $RemoteOutRoot/[a-z]* ; rm -f $RemoteOutRoot/_logs/*"
if ($LASTEXITCODE -ne 0) { throw "cnc prep failed" }

# ---- Local output dir for later pulls ----
if (Test-Path $LocalOutDir) { Remove-Item -Recurse -Force $LocalOutDir }
New-Item -ItemType Directory -Force -Path "$LocalOutDir\_logs" | Out-Null

# ---- Launch queue under nohup with OUTROOT override ----
Write-Host ""
Write-Host "==> cnc: launching queue_smoke.sh -> $RemoteOutRoot under nohup..."
$launchCmd = "cd $RemoteRoot ; OUTROOT=$RemoteOutRoot nohup $RemoteScripts/queue_smoke.sh > $RemoteOutRoot/_logs/queue.stdout 2> $RemoteOutRoot/_logs/queue.stderr < /dev/null & echo `$!"
$queuePid = (ssh $RemoteHost $launchCmd) -as [int]
if (-not $queuePid) { throw "Failed to launch queue_smoke.sh" }

@{
    queue_pid   = $queuePid
    host        = $RemoteHost
    out_root    = $RemoteOutRoot
    launched_at = (Get-Date).ToString('o')
} | ConvertTo-Json | Set-Content -Path "$LocalOutDir\_logs\cnc_queue.json" -Encoding utf8

Start-Sleep -Seconds 3
$alive = ssh $RemoteHost "kill -0 $queuePid 2>/dev/null && echo ALIVE || echo DEAD"
Write-Host ("    queue PID {0}: {1}" -f $queuePid, $alive.Trim())

Write-Host ""
Write-Host "==> attempt3 queue launched (PID $queuePid)."
Write-Host "    Tail:  ssh cnc-server 'tail -f $RemoteOutRoot/_logs/queue.log'"
Write-Host "    Done:  ssh cnc-server 'ls $RemoteOutRoot/_logs/queue.done'"
