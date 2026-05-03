# Smoke version of train_aether_brain — 3 matches, 30 train steps, ~30 sec total.
# Verifies the pipeline runs end-to-end. Run before the full version.

$ErrorActionPreference = 'Stop'
Set-Location J:\antcolony

$matches = 3
$max_ticks = 100
$train_steps = 30
$bench_dir = "J:\antcolony\bench\aether-smoke"
$traj_jsonl = Join-Path $bench_dir "trajectories.jsonl"
$corpus_txt = Join-Path $bench_dir "corpus.txt"
$prepared = Join-Path $bench_dir "prepared"
$ckpt_base = "J:\aether\checkpoints\antcolony_smoke"

New-Item -ItemType Directory -Path $bench_dir -Force | Out-Null
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"

Write-Output "[1/4] self-play"
& $bench_exe --left heuristic --right heuristic --matches $matches `
    --max-ticks $max_ticks --dump-trajectories $traj_jsonl
if ($LASTEXITCODE -ne 0) { throw "matchup_bench failed" }

Write-Output "[2/4] JSONL -> corpus"
$lines = Get-Content $traj_jsonl | ForEach-Object {
    $r = $_ | ConvertFrom-Json
    $s = $r.state; $d = $r.decision
    $ed = if ($s.enemy_distance_min -ne $null -and $s.enemy_distance_min -ne 'inf') { "{0:F1}" -f $s.enemy_distance_min } else { "inf" }
    $dia = if ($s.diapause_active) { 1 } else { 0 }
    $day = if ($s.is_daytime) { 1 } else { 0 }
    "state food={0:F1} inflow={1:F2} workers={2} soldiers={3} breeders={4} eggs={5} larvae={6} pupae={7} queens={8} losses={9} ed={10} ew={11} es={12} doy={13} t={14:F1} dia={15} day={16} action= w:{17:F2} s:{18:F2} b:{19:F2} f:{20:F2} d:{21:F2} n:{22:F2} r:none" -f `
        $s.food_stored, $s.food_inflow_recent, $s.worker_count, $s.soldier_count, $s.breeder_count,
        $s.brood_egg, $s.brood_larva, $s.brood_pupa, $s.queens_alive, $s.combat_losses_recent,
        $ed, $s.enemy_worker_count, $s.enemy_soldier_count, $s.day_of_year, $s.ambient_temp_c, $dia, $day,
        $d.caste_ratio_worker, $d.caste_ratio_soldier, $d.caste_ratio_breeder,
        $d.forage_weight, $d.dig_weight, $d.nurse_weight
}
$lines | Set-Content -Path $corpus_txt -Encoding utf8
Write-Output "  $($lines.Count) corpus lines"

Write-Output "[3/4] aether-prepare (corpus copied into aether tree to satisfy cwd-relative path constraint)"
$aether_root = "J:\aether"
$aether_corpus = Join-Path $aether_root "scratch\antcolony_smoke_corpus.txt"
$aether_prepared = "scratch\antcolony_smoke_prepared"
New-Item -ItemType Directory -Path (Join-Path $aether_root "scratch") -Force | Out-Null
Copy-Item $corpus_txt $aether_corpus -Force
Set-Location $aether_root
.\target\release\aether-prepare.exe --in scratch\antcolony_smoke_corpus.txt --out $aether_prepared
if ($LASTEXITCODE -ne 0) { throw "aether-prepare failed" }

Write-Output "[4/4] aether-train (config=nano, steps=$train_steps)"
$ckpt_rel = "checkpoints\antcolony_smoke"
.\target\release\aether-train.exe --config nano --steps $train_steps --batch 8 --seq 32 --lr 3e-3 --data $aether_prepared --out $ckpt_rel
if ($LASTEXITCODE -ne 0) { throw "aether-train failed" }

Set-Location J:\antcolony
Write-Output "Done. Checkpoint: $ckpt_base.weights"
