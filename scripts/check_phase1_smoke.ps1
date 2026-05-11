# Reports current status of the Phase 1 2yr smoke runs.
# Works for cnc-only runs (queue_smoke.sh) and legacy kokonoe+cnc runs.

$ErrorActionPreference = 'Stop'
$LocalOutDir   = 'J:\antcolony\bench\smoke-phase1-2yr'
$RemoteOutRoot = '/opt/antcolony/runs/phase1-2yr'
$RemoteHost    = 'cnc-server'

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
$qStatus = ssh $RemoteHost "if [ -f $RemoteOutRoot/_logs/queue.pid ]; then qpid=`$(cat $RemoteOutRoot/_logs/queue.pid); if kill -0 `$qpid 2>/dev/null; then echo `"queue.pid=`$qpid RUNNING`"; else echo `"queue.pid=`$qpid DEAD`"; fi; else echo NO_QUEUE_PIDFILE; fi"
Write-Host "  $qStatus"
$done = ssh $RemoteHost "[ -f $RemoteOutRoot/_logs/queue.done ] && echo DONE || echo NOT_DONE"
Write-Host ("  queue.done : {0}" -f $done.Trim())

Write-Host ""
Write-Host "==> queue.log (last 10 lines)" -ForegroundColor Cyan
ssh $RemoteHost "tail -10 $RemoteOutRoot/_logs/queue.log 2>/dev/null || echo '(no queue.log yet)'"

# ---- Per-species status on cnc ----
Write-Host ""
Write-Host "==> cnc species progress" -ForegroundColor Cyan
$cncStatus = ssh $RemoteHost @"
if [ -f $RemoteOutRoot/_logs/queue_pids.txt ]; then
  while read pid sp; do
    if [ -z "`$sp" ]; then continue; fi
    if kill -0 `$pid 2>/dev/null; then st=RUNNING; else st=DONE; fi
    csv=$RemoteOutRoot/`$sp/daily.csv
    if [ -f `$csv ]; then
      rows=`$(wc -l < `$csv)
      last=`$(tail -1 `$csv)
      day=`$(echo `$last | awk -F, '{print `$2}')
      yr=`$(echo `$last | awk -F, '{print `$3}')
      doy=`$(echo `$last | awk -F, '{print `$4}')
      workers=`$(echo `$last | awk -F, '{print `$6}')
      food=`$(echo `$last | awk -F, '{print \$13}')
      printf "  %-30s pid=%-6s %-8s yr=%s doy=%-4s day=%-5s workers=%-6s food=%-8s rows=%s\n" "`$sp" "`$pid" "`$st" "`$yr" "`$doy" "`$day" "`$workers" "`$food" "`$rows"
    else
      printf "  %-30s pid=%-6s %-8s (no daily.csv yet)\n" "`$sp" "`$pid" "`$st"
    fi
  done < $RemoteOutRoot/_logs/queue_pids.txt
else
  echo "  (no queue_pids.txt yet — queue may not have dispatched anything)"
fi
"@
Write-Host $cncStatus
