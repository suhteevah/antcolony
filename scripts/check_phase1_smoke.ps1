# Reports current status of the Phase 1 2yr smoke runs.
# Works for cnc-only runs (queue_smoke.sh) and legacy kokonoe+cnc runs.

$ErrorActionPreference = 'Stop'
$LocalOutDir   = 'J:\antcolony\bench\smoke-phase1-2yr'
$RemoteOutRoot = '/opt/antcolony/runs/phase1-2yr'
$RemoteHost    = 'cnc-server'

if (-not (Test-Path $LocalOutDir)) {
    Write-Host "No smoke run found at $LocalOutDir. Run scripts/run_phase1_smoke.ps1 first." -ForegroundColor Yellow
    exit 1
}

# ---- Kokonoe (only if a legacy run exists) ----
$KokonoePidsJson = "$LocalOutDir\_logs\kokonoe_pids.json"
if (Test-Path $KokonoePidsJson) {
    Write-Host "==> Kokonoe species" -ForegroundColor Cyan
    $KokonoePids = Get-Content $KokonoePidsJson | ConvertFrom-Json
    $KokonoeDirs = Get-ChildItem -Directory $LocalOutDir -Exclude '_logs' -ErrorAction SilentlyContinue
    foreach ($dir in $KokonoeDirs) {
        $sp = $dir.Name
        $dailyCsv = "$($dir.FullName)\daily.csv"
        $line = '(no daily.csv)'; $status = 'NO_DATA'
        if (Test-Path $dailyCsv) {
            $rows = Import-Csv $dailyCsv
            if ($rows.Count -gt 0) {
                $last = $rows[-1]
                $workers = [int]$last.workers
                $food = [float]$last.food
                $status = if ($workers -le 0) { 'EXTINCT' } else { 'ALIVE' }
                $line = "workers=$workers food=$($food.ToString('F1')) rows=$($rows.Count)"
            }
        }
        $procStatus = '?'
        if ($KokonoePids.PSObject.Properties.Name -contains $sp) {
            $procId = $KokonoePids.$sp
            $proc = Get-Process -Id $procId -ErrorAction SilentlyContinue
            $procStatus = if ($proc) { "PID $procId RUNNING" } else { "PID $procId DONE" }
        }
        $color = switch ($status) { 'EXTINCT' {'Red'} 'ALIVE' {'Green'} default {'Yellow'} }
        Write-Host ("  {0,-30} {1,-22} {2}" -f $sp, $procStatus, $line) -ForegroundColor $color
    }
    Write-Host ""
}

# ---- Cnc queue ----
Write-Host "==> cnc queue manager" -ForegroundColor Cyan
$QueueJson = "$LocalOutDir\_logs\cnc_queue.json"
$queuePid = $null
if (Test-Path $QueueJson) {
    $queuePid = (Get-Content $QueueJson | ConvertFrom-Json).queue_pid
}
$qAlive = ssh $RemoteHost "if [ -f $RemoteOutRoot/_logs/queue.pid ]; then qpid=`$(cat $RemoteOutRoot/_logs/queue.pid); if kill -0 `$qpid 2>/dev/null; then echo `"queue.pid=`$qpid RUNNING`"; else echo `"queue.pid=`$qpid DEAD`"; fi; else echo NO_QUEUE_PIDFILE; fi"
Write-Host "  $qAlive"
$done = ssh $RemoteHost "[ -f $RemoteOutRoot/_logs/queue.done ] && echo DONE || echo NOT_DONE"
Write-Host ("  queue.done : {0}" -f $done.Trim())

# Tail last few queue.log lines.
Write-Host ""
Write-Host "==> queue.log (last 8 lines)" -ForegroundColor Cyan
ssh $RemoteHost "tail -8 $RemoteOutRoot/_logs/queue.log 2>/dev/null || echo '(no queue.log yet)'"

# ---- Per-species status on cnc ----
Write-Host ""
Write-Host "==> cnc species progress" -ForegroundColor Cyan
$cncStatus = ssh $RemoteHost @"
if [ -f $RemoteOutRoot/_logs/queue_pids.json ]; then
  for d in $RemoteOutRoot/*/; do
    sp=`$(basename `$d)
    pid=`$(grep -E "\"`$sp\"" $RemoteOutRoot/_logs/queue_pids.json | sed -E 's/.*: ([0-9]+).*/\1/')
    if [ -n "`$pid" ] && kill -0 `$pid 2>/dev/null; then
      st=RUNNING
    elif [ -n "`$pid" ]; then
      st=DONE
    else
      st=PENDING
    fi
    if [ -f `$d/daily.csv ]; then
      rows=`$(wc -l < `$d/daily.csv)
      last=`$(tail -1 `$d/daily.csv)
      workers=`$(echo `$last | awk -F, '{print `$3}')
      food=`$(echo `$last | awk -F, '{print `$5}')
      day=`$(echo `$last | awk -F, '{print `$2}')
      printf "  %-30s pid=%-6s %-8s day=%-5s workers=%-6s food=%-8s rows=%s\n" "`$sp" "`${pid:--}" "`$st" "`$day" "`$workers" "`$food" "`$rows"
    else
      printf "  %-30s pid=%-6s %-8s (no daily.csv yet)\n" "`$sp" "`${pid:--}" "`$st"
    fi
  done
else
  echo "  (no queue_pids.json on cnc — queue may not have launched anything yet)"
fi
"@
Write-Host $cncStatus
