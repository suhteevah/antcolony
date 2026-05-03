# Manual model inspection — feed each trained checkpoint a sample
# game-state prompt, see what it emits, check if it parses to a valid
# AiDecision. Surfaces "model is emitting garbage vs model is in-format
# but wrong values" debugging info.
#
# Usage: powershell -File scripts\inspect_aether_brain.ps1

$ErrorActionPreference = 'Continue'

$ckpts = @(
    "antcolony_nano600",
    "antcolony_nano1500",
    "antcolony_nano1500_lr1e3",
    "antcolony_tiny600"
)

# Three diverse sample prompts representing different game states.
$prompts = @(
    # Healthy mid-game.
    "state food=80.0 inflow=0.50 workers=30 soldiers=5 breeders=1 eggs=10 larvae=5 pupae=5 queens=1 losses=0 ed=inf ew=0 es=0 doy=150 t=22.0 dia=0 day=1 action=",
    # Under attack — high losses, enemies close.
    "state food=40.0 inflow=0.20 workers=15 soldiers=3 breeders=0 eggs=5 larvae=3 pupae=2 queens=1 losses=8 ed=2.0 ew=12 es=4 doy=180 t=25.0 dia=0 day=1 action=",
    # Starving — low food, no enemies.
    "state food=5.0 inflow=0.05 workers=20 soldiers=4 breeders=1 eggs=8 larvae=4 pupae=3 queens=1 losses=0 ed=inf ew=0 es=0 doy=200 t=18.0 dia=0 day=0 action="
)

Set-Location J:\aether
foreach ($ckpt in $ckpts) {
    Write-Output ""
    Write-Output "=========================="
    Write-Output "CHECKPOINT: $ckpt"
    Write-Output "=========================="
    $weights = "checkpoints\$ckpt.weights"
    if (-not (Test-Path $weights)) {
        Write-Output "  (skipped — weights file missing)"
        continue
    }
    foreach ($p in $prompts) {
        $label = if ($p.Contains("losses=8")) { "[under attack]" }
                 elseif ($p.Contains("food=5.0")) { "[starving]" }
                 else { "[healthy mid-game]" }
        Write-Output ""
        Write-Output "--- prompt $label ---"
        Write-Output "  $p"
        $completion = & .\target\release\aether-infer.exe --ckpt "checkpoints\$ckpt" --prompt $p --max-new 40 2>&1 | Select-Object -Last 1
        # Strip the prompt prefix if echoed.
        if ($completion.StartsWith($p)) { $completion = $completion.Substring($p.Length) }
        Write-Output "  -> '$completion'"
        # Naive parse: look for "w:0.X" etc.
        $hasW = $completion -match 'w:(\d+\.?\d*)'
        $hasS = $completion -match 's:(\d+\.?\d*)'
        $hasF = $completion -match 'f:(\d+\.?\d*)'
        $hasD = $completion -match 'd:(\d+\.?\d*)'
        $valid = $hasW -and $hasS -and $hasF -and $hasD
        Write-Output "  parse: w=$hasW s=$hasS f=$hasF d=$hasD  -> $(if ($valid) { 'VALID' } else { 'UNPARSEABLE' })"
    }
}

Write-Output ""
Write-Output "Done. Compare to baseline output for the same prompts:"
Write-Output "  HeuristicBrain mid-game default: w:0.65 s:0.30 b:0.05 f:0.55 d:0.20 n:0.25"
Write-Output "  HeuristicBrain under attack:     soldier ratio escalates"
Write-Output "  HeuristicBrain starving:         forage weight escalates"
