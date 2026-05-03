# Self-play -> training -> evaluation loop for the Aether brain.
#
# Pipeline:
#   1. Run matchup_bench HeuristicBrain self-play, dump trajectories
#      as JSONL to bench/aether-train/trajectories.jsonl.
#   2. Convert JSONL trajectories -> text corpus (one
#      "<prompt> <completion>" line per record).
#   3. aether-prepare --in <corpus> --out <prepared> tokenizes the
#      corpus into Aether's training format.
#   4. aether-train --data <prepared> --out checkpoints/antcolony_brain
#      trains a model on the corpus.
#   5. matchup_bench HeuristicBrain vs the trained model evaluates
#      whether the model learned anything useful.
#
# Edit the params at top to scale matches/steps. Final model lands at
# J:\aether\checkpoints\antcolony_brain.{weights,meta} and is loadable
# via `--right aether:J:/aether/checkpoints/antcolony_brain` in
# matchup_bench.

$ErrorActionPreference = 'Stop'
Set-Location J:\antcolony

# -------- Params --------
$matches      = 50      # self-play matches for trajectory generation
$max_ticks    = 2000    # tick budget per self-play match
$train_steps  = 600     # aether-train iterations
$train_config = "nano"  # nano (~85K params) or tiny (larger)

# -------- Paths --------
$bench_dir   = "J:\antcolony\bench\aether-train"
$traj_jsonl  = Join-Path $bench_dir "trajectories.jsonl"
$corpus_txt  = Join-Path $bench_dir "corpus.txt"
$prepared    = Join-Path $bench_dir "prepared"
$ckpt_base   = "J:\aether\checkpoints\antcolony_brain"

New-Item -ItemType Directory -Path $bench_dir -Force | Out-Null

# -------- Step 1: Self-play ----------
Write-Output ""
Write-Output "[1/5] Self-play HeuristicBrain x HeuristicBrain ($matches matches, max $max_ticks ticks each)"
$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"
if (-not (Test-Path $bench_exe)) {
    cargo build --release -p antcolony-sim --example matchup_bench
}
& $bench_exe --left heuristic --right heuristic --matches $matches `
    --max-ticks $max_ticks --dump-trajectories $traj_jsonl
if ($LASTEXITCODE -ne 0) { throw "matchup_bench failed (exit $LASTEXITCODE)" }
$traj_count = (Get-Content $traj_jsonl | Measure-Object -Line).Lines
Write-Output "  wrote $traj_count trajectory records to $traj_jsonl"

# -------- Step 2: JSONL -> text corpus ----------
Write-Output ""
Write-Output "[2/5] Convert JSONL trajectories -> text corpus"
$corpus_lines = Get-Content $traj_jsonl | ForEach-Object {
    $r = $_ | ConvertFrom-Json
    $s = $r.state
    $d = $r.decision
    $ed = if ($s.enemy_distance_min -ne $null -and $s.enemy_distance_min -ne 'inf') {
        "{0:F1}" -f $s.enemy_distance_min
    } else {
        "inf"
    }
    $diapause_int = if ($s.diapause_active) { 1 } else { 0 }
    $day_int      = if ($s.is_daytime)      { 1 } else { 0 }
    $prompt = "state food={0:F1} inflow={1:F2} workers={2} soldiers={3} breeders={4} eggs={5} larvae={6} pupae={7} queens={8} losses={9} ed={10} ew={11} es={12} doy={13} t={14:F1} dia={15} day={16} action=" -f `
        $s.food_stored, $s.food_inflow_recent,
        $s.worker_count, $s.soldier_count, $s.breeder_count,
        $s.brood_egg, $s.brood_larva, $s.brood_pupa, $s.queens_alive,
        $s.combat_losses_recent, $ed, $s.enemy_worker_count, $s.enemy_soldier_count,
        $s.day_of_year, $s.ambient_temp_c, $diapause_int, $day_int
    $completion = "w:{0:F2} s:{1:F2} b:{2:F2} f:{3:F2} d:{4:F2} n:{5:F2} r:none" -f `
        $d.caste_ratio_worker, $d.caste_ratio_soldier, $d.caste_ratio_breeder,
        $d.forage_weight, $d.dig_weight, $d.nurse_weight
    "$prompt $completion"
}
$corpus_lines | Set-Content -Path $corpus_txt -Encoding utf8
Write-Output "  wrote $($corpus_lines.Count) corpus lines to $corpus_txt"

# -------- Step 3: aether-prepare ----------
# aether-prepare + aether-train both reject paths that "escape cwd" —
# we copy the corpus into aether's tree and run with relative paths.
Write-Output ""
Write-Output "[3/5] aether-prepare tokenize corpus -> $prepared"
$aether_root = "J:\aether"
$aether_corpus = Join-Path $aether_root "scratch\antcolony_brain_corpus.txt"
$aether_prepared_rel = "scratch\antcolony_brain_prepared"
$ckpt_rel = "checkpoints\antcolony_brain"
New-Item -ItemType Directory -Path (Join-Path $aether_root "scratch") -Force | Out-Null
Copy-Item $corpus_txt $aether_corpus -Force
Set-Location $aether_root
.\target\release\aether-prepare.exe --in scratch\antcolony_brain_corpus.txt --out $aether_prepared_rel
if ($LASTEXITCODE -ne 0) { throw "aether-prepare failed (exit $LASTEXITCODE)" }

# -------- Step 4: aether-train ----------
Write-Output ""
Write-Output "[4/5] aether-train --config $train_config --steps $train_steps --data $aether_prepared_rel"
.\target\release\aether-train.exe --config $train_config --steps $train_steps `
    --batch 8 --seq 64 --lr 3e-3 --data $aether_prepared_rel --out $ckpt_rel
if ($LASTEXITCODE -ne 0) { throw "aether-train failed (exit $LASTEXITCODE)" }

# -------- Step 5: Evaluate ----------
Set-Location J:\antcolony
Write-Output ""
Write-Output "[5/5] Evaluate trained brain: HeuristicBrain vs aether:antcolony_brain (10 matches)"
$eval_out = "J:\antcolony\bench\aether-train\eval"
& $bench_exe --left heuristic --right "aether:$ckpt_base" `
    --matches 10 --max-ticks $max_ticks --out $eval_out
if ($LASTEXITCODE -ne 0) { Write-Output "  (eval matchup_bench non-zero exit $LASTEXITCODE)" }

Write-Output ""
Write-Output "Done. Trained checkpoint: $ckpt_base.weights"
Write-Output "Eval summary: $eval_out\SUMMARY.md"
