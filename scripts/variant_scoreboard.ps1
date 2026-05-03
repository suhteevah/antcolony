# Quick scoreboard: each variant plays 4 matches vs heuristic.
# Shows which variants are strategically viable vs which are dead weight
# in the tournament corpus.

$ErrorActionPreference = 'Continue'
Set-Location J:\antcolony

$bench_exe = "J:\antcolony\target\release\examples\matchup_bench.exe"
$out_dir = "J:\antcolony\bench\variant-tournament\scoreboard"
New-Item -ItemType Directory -Path $out_dir -Force | Out-Null

$variants = @(
    @{ name = "balanced_a";   spec = "tuned:balanced_a:0.50,0.40,0.10,0.40,0.20,0.40,0.7,0.7,20" }
    @{ name = "balanced_b";   spec = "tuned:balanced_b:0.60,0.30,0.10,0.60,0.10,0.30,1.0,0.5,15" }
    @{ name = "glass_cannon"; spec = "tuned:glass_cannon:0.25,0.70,0.05,0.80,0.05,0.15,2.0,1.0,35" }
    @{ name = "swarm";        spec = "tuned:swarm:0.40,0.55,0.05,0.85,0.05,0.10,1.2,1.2,25" }
    @{ name = "turtle";       spec = "tuned:turtle:0.45,0.40,0.15,0.10,0.25,0.65,0.2,0.3,15" }
    @{ name = "excavator";    spec = "tuned:excavator:0.60,0.15,0.25,0.20,0.50,0.30,0.4,0.5,20" }
    @{ name = "queen_focus";  spec = "tuned:queen_focus:0.40,0.05,0.55,0.40,0.20,0.40,0.3,0.6,30" }
    @{ name = "alate_swarm";  spec = "tuned:alate_swarm:0.35,0.10,0.55,0.65,0.10,0.25,0.4,0.8,25" }
    @{ name = "pure_econ";    spec = "tuned:pure_econ:0.90,0.02,0.08,0.95,0.02,0.03,0.0,0.5,15" }
    @{ name = "worker_swarm"; spec = "tuned:worker_swarm:0.92,0.05,0.03,0.80,0.05,0.15,0.0,0.8,25" }
    @{ name = "nurse_heavy";  spec = "tuned:nurse_heavy:0.60,0.10,0.30,0.30,0.10,0.60,0.4,0.5,25" }
    @{ name = "panic_fort";   spec = "tuned:panic_fort:0.45,0.45,0.10,0.20,0.15,0.65,3.0,0.3,10" }
    @{ name = "expansionist"; spec = "tuned:expansionist:0.55,0.20,0.25,0.55,0.30,0.15,0.6,0.7,25" }
    @{ name = "berserker";    spec = "tuned:berserker:0.20,0.75,0.05,0.75,0.05,0.20,3.0,1.5,40" }
)

Write-Host "=== Variant scoreboard vs heuristic (4 matches each) ==="
foreach ($v in $variants) {
    $eval_dir = Join-Path $out_dir $v.name
    & $bench_exe --left $v.spec --right "heuristic" --matches 4 --max-ticks 10000 --out $eval_dir 2>&1 | Out-Null
    if (Test-Path (Join-Path $eval_dir "SUMMARY.md")) {
        $line = Get-Content (Join-Path $eval_dir "SUMMARY.md") | Where-Object { $_ -match "^\| Left" } | Select-Object -First 1
        Write-Host "  $($v.name.PadRight(15)): $line"
    }
}
Write-Host "=== Done ==="
