# Launches the Phase 1 2yr 10-species smoke split across kokonoe (7) + cnc (3).
# Returns immediately after dispatch — does NOT block on completion.
# Use scripts/check_phase1_smoke.ps1 to poll status.
#
# Prereqs:
# - Phase 1 sim fixes committed (Tasks 1-6 of plan)
# - cnc has been provisioned via scripts/cnc_provision.ps1
# - kokonoe smoke binary built (this script builds it if missing)

$ErrorActionPreference = 'Stop'
$LocalRoot = 'J:\antcolony'
$LocalOutDir = "$LocalRoot\bench\smoke-phase1-2yr"
$RemoteRoot = '/opt/antcolony'
$RemoteOutRoot = "$RemoteRoot/runs/phase1-2yr"
$RemoteHost = 'cnc-server'

# Species split: kokonoe takes 7 (heavier-cohort + cliff casualties),
# cnc takes 3 (newer species + lightweight). Keep the 4 cliff casualties
# on kokonoe for fastest iteration if a re-fix is needed.
$KokonoeSpecies = @(
    'lasius_niger',                # autumn cliff casualty
    'pogonomyrmex_occidentalis',   # autumn cliff casualty
    'formica_rufa',                # spring cliff casualty
    'camponotus_pennsylvanicus',   # spring cliff casualty
    'tapinoma_sessile',            # spring cliff casualty
    'aphaenogaster_rudis',         # food-overaccumulation survivor
    'formica_fusca'                # food-overaccumulation survivor
)
$CncSpecies = @(
    'tetramorium_immigrans',       # spring cliff casualty
    'brachyponera_chinensis',      # new, never smoke-tested
    'temnothorax_curvinodis'       # new, never smoke-tested
)

$Seed = 42
$Years = 2

Write-Host "==> Phase 1 2yr smoke launcher"
Write-Host "    Kokonoe species ($($KokonoeSpecies.Count)): $($KokonoeSpecies -join ', ')"
Write-Host "    Cnc species ($($CncSpecies.Count)): $($CncSpecies -join ', ')"
Write-Host "    Seed: $Seed; Years: $Years; Brain: HeuristicBrain (--no-mlp)"

# ---- KOKONOE SIDE ----
Write-Host ""
Write-Host "==> Kokonoe: building release smoke binary (cargo is incremental — recompiles only if changed)..."
$KokonoeBinary = "$LocalRoot\target\release\examples\smoke_10yr_ai.exe"
Push-Location $LocalRoot
try {
    cargo build --release -p antcolony-sim --example smoke_10yr_ai 2>&1 | Select-Object -Last 5
    if ($LASTEXITCODE -ne 0) { throw "Kokonoe build failed" }
    if (-not (Test-Path $KokonoeBinary)) { throw "Binary missing after build at $KokonoeBinary" }
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "==> Kokonoe: ensuring output dir + clearing any prior run..."
if (Test-Path $LocalOutDir) {
    Write-Host "    Removing prior $LocalOutDir"
    Remove-Item -Recurse -Force $LocalOutDir
}
New-Item -ItemType Directory -Force -Path $LocalOutDir | Out-Null
New-Item -ItemType Directory -Force -Path "$LocalOutDir\_logs" | Out-Null

Write-Host ""
Write-Host "==> Kokonoe: launching $($KokonoeSpecies.Count) detached smoke processes..."
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
    Write-Host "    $sp -> PID $($proc.Id)"
}

# Persist PIDs so check script can find them.
$KokonoePids | ConvertTo-Json | Set-Content -Path "$LocalOutDir\_logs\kokonoe_pids.json" -Encoding utf8

# ---- CNC SIDE ----
Write-Host ""
Write-Host "==> cnc: setting up remote output directory..."
ssh $RemoteHost "mkdir -p $RemoteOutRoot/_logs && rm -rf $RemoteOutRoot/[a-z]*"
if ($LASTEXITCODE -ne 0) { throw "cnc remote dir setup failed" }

Write-Host ""
Write-Host "==> cnc: launching $($CncSpecies.Count) detached smoke processes..."
$CncPids = @{}
foreach ($sp in $CncSpecies) {
    $speciesOutDir = "$RemoteOutRoot/$sp"
    $logOut = "$RemoteOutRoot/_logs/$sp.log.out"
    $logErr = "$RemoteOutRoot/_logs/$sp.log.err"
    # nohup + & so the process detaches from the ssh session.
    # Echo the PID so we can capture it on the local side.
    $cmd = "cd $RemoteRoot && mkdir -p $speciesOutDir && nohup ./target/release/examples/smoke_10yr_ai --years $Years --no-mlp --species $sp --seed $Seed --out $speciesOutDir > $logOut 2> $logErr & echo \`$!"
    $pid_remote = (ssh $RemoteHost $cmd) -as [int]
    $CncPids[$sp] = $pid_remote
    Write-Host "    $sp -> remote PID $pid_remote"
}

# Persist remote PIDs locally.
$CncPids | ConvertTo-Json | Set-Content -Path "$LocalOutDir\_logs\cnc_pids.json" -Encoding utf8

Write-Host ""
Write-Host "==> All 10 smokes launched." -ForegroundColor Green
Write-Host "    Kokonoe outputs: $LocalOutDir"
Write-Host "    Cnc outputs: ${RemoteHost}:$RemoteOutRoot"
Write-Host ""
Write-Host "    Estimated wall-clock: ~18-24 hours."
Write-Host "    Check status: .\scripts\check_phase1_smoke.ps1"
Write-Host "    Pull cnc results when done: .\scripts\pull_cnc_smoke.ps1"
