# Launches the Phase 1 2yr 10-species smoke split across kokonoe (7) + cnc (3).
# Returns immediately after dispatch.
# Use scripts/check_phase1_smoke.ps1 to poll status.

$ErrorActionPreference = 'Stop'
$LocalRoot = 'J:\antcolony'
$LocalOutDir = "$LocalRoot\bench\smoke-phase1-2yr"
$RemoteRoot = '/opt/antcolony'
$RemoteOutRoot = "$RemoteRoot/runs/phase1-2yr"
$RemoteHost = 'cnc-server'

$KokonoeSpecies = @(
    'lasius_niger',
    'pogonomyrmex_occidentalis',
    'formica_rufa',
    'camponotus_pennsylvanicus',
    'tapinoma_sessile',
    'aphaenogaster_rudis',
    'formica_fusca'
)
$CncSpecies = @(
    'tetramorium_immigrans',
    'brachyponera_chinensis',
    'temnothorax_curvinodis'
)

$Seed = 42
$Years = 2

Write-Host "==> Phase 1 2yr smoke launcher"
Write-Host ("    Kokonoe species ({0}): {1}" -f $KokonoeSpecies.Count, ($KokonoeSpecies -join ', '))
Write-Host ("    Cnc species ({0}): {1}" -f $CncSpecies.Count, ($CncSpecies -join ', '))
Write-Host ("    Seed: {0}; Years: {1}; Brain: HeuristicBrain (--no-mlp)" -f $Seed, $Years)

# ---- KOKONOE SIDE ----
Write-Host ""
Write-Host "==> Kokonoe: building release smoke binary (cargo incremental)..."
$KokonoeBinary = "$LocalRoot\target\release\examples\smoke_10yr_ai.exe"
Push-Location $LocalRoot
try {
    # Save+restore ErrorActionPreference because cargo writes progress
    # to stderr and Stop would treat that as a script error.
    $prevEAP = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $buildLog = "$env:TEMP\smoke_build.log"
    cmd /c "cargo build --release -p antcolony-sim --example smoke_10yr_ai > `"$buildLog`" 2>&1"
    $buildExit = $LASTEXITCODE
    $ErrorActionPreference = $prevEAP
    Get-Content $buildLog -Tail 5
    if ($buildExit -ne 0) { throw "Kokonoe build failed (see $buildLog)" }
    if (-not (Test-Path $KokonoeBinary)) { throw "Binary missing after build" }
    Remove-Item $buildLog -ErrorAction SilentlyContinue
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "==> Kokonoe: ensuring output dir + clearing prior run..."
if (Test-Path $LocalOutDir) {
    Remove-Item -Recurse -Force $LocalOutDir
}
New-Item -ItemType Directory -Force -Path $LocalOutDir | Out-Null
New-Item -ItemType Directory -Force -Path "$LocalOutDir\_logs" | Out-Null

Write-Host ""
Write-Host ("==> Kokonoe: launching {0} detached smoke processes..." -f $KokonoeSpecies.Count)
$KokonoePids = @{}
foreach ($sp in $KokonoeSpecies) {
    $speciesOutDir = "$LocalOutDir\$sp"
    $logOut = "$LocalOutDir\_logs\$sp.log.out"
    $logErr = "$LocalOutDir\_logs\$sp.log.err"
    $cmdArgs = @(
        '--years', $Years,
        '--no-mlp',
        '--species', $sp,
        '--seed', $Seed,
        '--out', $speciesOutDir
    )
    $proc = Start-Process -FilePath $KokonoeBinary -ArgumentList $cmdArgs `
        -RedirectStandardOutput $logOut -RedirectStandardError $logErr `
        -WindowStyle Hidden -PassThru
    $KokonoePids[$sp] = $proc.Id
    Write-Host ("    {0} -> PID {1}" -f $sp, $proc.Id)
}
$KokonoePids | ConvertTo-Json | Set-Content -Path "$LocalOutDir\_logs\kokonoe_pids.json" -Encoding utf8

# ---- CNC SIDE ----
# All ssh-target shell strings are SINGLE-QUOTED in PowerShell so that
# embedded shell `&` and `&&` are not eaten by the PS parser. We
# interpolate variables before passing to ssh by string concatenation.

Write-Host ""
Write-Host "==> cnc: setting up remote output directory..."
$cncSetup = 'mkdir -p ' + $RemoteOutRoot + '/_logs ; rm -rf ' + $RemoteOutRoot + '/[a-z]*'
ssh $RemoteHost $cncSetup
if ($LASTEXITCODE -ne 0) { throw "cnc remote dir setup failed" }

Write-Host ""
Write-Host ("==> cnc: launching {0} detached smoke processes..." -f $CncSpecies.Count)
$CncPids = @{}
foreach ($sp in $CncSpecies) {
    $speciesOutDir = $RemoteOutRoot + '/' + $sp
    $logOut = $RemoteOutRoot + '/_logs/' + $sp + '.log.out'
    $logErr = $RemoteOutRoot + '/_logs/' + $sp + '.log.err'
    # Build the bash command via concat to avoid PS string-parsing issues.
    $bashCmd = 'cd ' + $RemoteRoot + ' ; mkdir -p ' + $speciesOutDir +
               ' ; nohup ./target/release/examples/smoke_10yr_ai' +
               ' --years ' + $Years + ' --no-mlp --species ' + $sp +
               ' --seed ' + $Seed + ' --out ' + $speciesOutDir +
               ' > ' + $logOut + ' 2> ' + $logErr +
               ' < /dev/null >/dev/null 2>&1 & echo $!'
    $remotePid = (ssh $RemoteHost $bashCmd) -as [int]
    $CncPids[$sp] = $remotePid
    Write-Host ("    {0} -> remote PID {1}" -f $sp, $remotePid)
}
$CncPids | ConvertTo-Json | Set-Content -Path "$LocalOutDir\_logs\cnc_pids.json" -Encoding utf8

Write-Host ""
Write-Host "==> All 10 smokes launched."
Write-Host ("    Kokonoe outputs: {0}" -f $LocalOutDir)
Write-Host ("    Cnc outputs: {0}:{1}" -f $RemoteHost, $RemoteOutRoot)
Write-Host ""
Write-Host "    Estimated wall-clock: ~18-24 hours."
Write-Host "    Check status: .\scripts\check_phase1_smoke.ps1"
Write-Host "    Pull cnc results: .\scripts\pull_cnc_smoke.ps1"
