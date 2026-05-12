# Validates a 2yr smoke result against per-species literature criteria
# from docs/superpowers/plans/2026-05-12-proper-food-spawn-calibration.md §App C.

$ErrorActionPreference = 'Stop'
$RunDir = if ($args.Count -gt 0) { $args[0] } else { 'J:\antcolony\bench\smoke-phase1-2yr-attempt3' }

Write-Host "Verifying smoke run: $RunDir" -ForegroundColor Cyan
Write-Host ""

# Per-species criteria (year_2_worker_min, year_2_worker_max, food_per_worker_max,
# year_over_year_growth_min_pct, cliff_drop_max_pct, hard_stop)
$Criteria = @{
    lasius_niger              = @{ wMin=500;  wMax=3000; fwMax=3.0;  yoyMin=100; cliffMax=20; hard=$true  }
    pogonomyrmex_occidentalis = @{ wMin=200;  wMax=1500; fwMax=30.0; yoyMin=80;  cliffMax=15; hard=$false }
    formica_rufa              = @{ wMin=50;   wMax=500;  fwMax=2.0;  yoyMin=200; cliffMax=25; hard=$false }
    camponotus_pennsylvanicus = @{ wMin=40;   wMax=200;  fwMax=5.0;  yoyMin=200; cliffMax=15; hard=$true  }
    tapinoma_sessile          = @{ wMin=300;  wMax=2000; fwMax=4.0;  yoyMin=150; cliffMax=25; hard=$false }
    aphaenogaster_rudis       = @{ wMin=80;   wMax=350;  fwMax=8.0;  yoyMin=100; cliffMax=15; hard=$false }
    formica_fusca             = @{ wMin=150;  wMax=800;  fwMax=3.0;  yoyMin=150; cliffMax=20; hard=$false }
    tetramorium_immigrans     = @{ wMin=400;  wMax=2500; fwMax=4.0;  yoyMin=150; cliffMax=20; hard=$false }
    brachyponera_chinensis    = @{ wMin=80;   wMax=500;  fwMax=2.0;  yoyMin=100; cliffMax=15; hard=$false }
    temnothorax_curvinodis    = @{ wMin=40;   wMax=150;  fwMax=2.0;  yoyMin=50;  cliffMax=15; hard=$false }
}

$Pass = 0
$Fail = 0
$HardFail = 0
$Results = @()

foreach ($sp in $Criteria.Keys) {
    $crit = $Criteria[$sp]
    $csv = Join-Path $RunDir "$sp\daily.csv"
    if (-not (Test-Path $csv)) {
        Write-Host "  [SKIP] $sp : no daily.csv at $csv" -ForegroundColor Yellow
        $Results += [pscustomobject]@{ Species=$sp; Status='SKIP'; Reason='no daily.csv' }
        continue
    }
    $rows = Import-Csv $csv
    if ($rows.Count -lt 700) {
        Write-Host "  [SKIP] $sp : incomplete run ($($rows.Count) rows < 700)" -ForegroundColor Yellow
        $Results += [pscustomobject]@{ Species=$sp; Status='SKIP'; Reason="incomplete run ($($rows.Count) rows)" }
        continue
    }
    $yr1End = $rows | Where-Object { [int]$_.year -eq 1 -and [int]$_.doy -ge 360 } | Select-Object -First 1
    $yr2End = $rows[-1]
    $w1 = if ($yr1End) { [int]$yr1End.workers } else { 0 }
    $w2 = [int]$yr2End.workers
    $f2 = [float]$yr2End.food
    $fw = if ($w2 -gt 0) { $f2 / $w2 } else { 0 }
    $yoy = if ($w1 -gt 0) { 100.0 * $w2 / $w1 } else { 0 }
    $maxCliff = 0.0
    for ($i = 1; $i -lt $rows.Count; $i++) {
        $wA = [int]$rows[$i-1].workers
        $wB = [int]$rows[$i].workers
        if ($wA -gt 50) {
            $drop = 100.0 * ($wA - $wB) / $wA
            if ($drop -gt $maxCliff) { $maxCliff = $drop }
        }
    }
    $reasons = @()
    if ($w2 -lt $crit.wMin) { $reasons += "workers=$w2 < $($crit.wMin)" }
    if ($w2 -gt $crit.wMax) { $reasons += "workers=$w2 > $($crit.wMax)" }
    if ($fw -gt $crit.fwMax) { $reasons += "food/worker=$([math]::Round($fw,1)) > $($crit.fwMax)" }
    if ($yoy -lt $crit.yoyMin) { $reasons += "yoy=$([math]::Round($yoy,0))% < $($crit.yoyMin)%" }
    if ($maxCliff -gt $crit.cliffMax) { $reasons += "max_cliff_drop=$([math]::Round($maxCliff,1))% > $($crit.cliffMax)%" }
    $status = if ($reasons.Count -eq 0) { 'PASS' } else { 'FAIL' }
    if ($status -eq 'PASS') { $Pass++ } else {
        $Fail++
        if ($crit.hard) { $HardFail++ }
    }
    $Results += [pscustomobject]@{
        Species   = $sp
        Status    = $status
        Workers   = $w2
        FoodPerW  = [math]::Round($fw, 2)
        YoY       = "$([math]::Round($yoy, 0))%"
        MaxCliff  = "$([math]::Round($maxCliff, 1))%"
        Reasons   = $reasons -join '; '
    }
    $color = if ($status -eq 'PASS') { 'Green' } else { if ($crit.hard) {'Red'} else {'Yellow'} }
    Write-Host ("  [{0}] {1,-28} {2}" -f $status, $sp, ($reasons -join '; ')) -ForegroundColor $color
}

Write-Host ""
Write-Host ("Summary: {0} PASS / {1} FAIL ({2} hard-stop)" -f $Pass, $Fail, $HardFail)
$Green = ($Pass -ge 8 -and $HardFail -eq 0)
if ($Green) {
    Write-Host "==> GREEN LIGHT for outreach (8/10 + no hard-stop violations)" -ForegroundColor Green
    exit 0
} else {
    Write-Host "==> NOT READY for outreach" -ForegroundColor Red
    exit 1
}
