"""Generate the full 49-brain species×archetype pool as bench specs.

Output: a tab-separated `name<TAB>spec` file readable by PowerShell.
Each spec uses the new Rust-native `species:` form so the SpeciesBrain
does the math at load time (no Python derivation drift).
"""
import sys
from pathlib import Path

sys.stdout.reconfigure(encoding='utf-8', errors='replace')

ARCHETYPES = ["heuristic", "defender", "aggressor", "economist", "breeder", "forager", "conservative"]
BLEND = 0.5
species_dir = Path("J:/antcolony/assets/species")
out_dir = Path("J:/antcolony/bench/iterative-fsp")
out_dir.mkdir(parents=True, exist_ok=True)
out_file = out_dir / "brain_pool.tsv"

with open(out_file, "w", encoding="utf-8") as f:
    for toml_path in sorted(species_dir.glob("*.toml")):
        species_id = toml_path.stem
        # Use forward slashes for cross-shell path handling
        rel_path = f"assets/species/{toml_path.name}"
        for arch in ARCHETYPES:
            label = f"{species_id}__{arch}"
            spec = f"species:{rel_path}:{arch}:{BLEND}"
            f.write(f"{label}\t{spec}\n")

count = sum(1 for _ in open(out_file, encoding="utf-8"))
print(f"Wrote {count} species×archetype brain specs -> {out_file}")
