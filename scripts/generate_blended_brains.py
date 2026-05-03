"""Generate species × archetype blended brain specs.

Each blended brain = lerp(species_baseline, archetype_overlay, blend).
Caste triple and behavior triple are renormalized after blending so they
each sum to 1.0 (sim invariant).

Result: a list of `tuned:<species>_<archetype>:9floats` specs ready for
matchup_bench. With 7 species × 7 archetypes = 49 distinct brains, each
combining biological identity (the species) with strategic posture (the
archetype).

Why this is biologically defensible: species sets the BIOLOGICAL FLOOR
(Camponotus has majors; Lasius does not). Archetype sets the STRATEGIC
POSTURE (push the fight vs accumulate food). The blend lets a Camponotus
play "economist" mode without losing its 10% major caste, and lets a
Lasius play "aggressor" mode without growing soldiers it can't have.
"""
import sys, tomllib
from pathlib import Path

sys.stdout.reconfigure(encoding='utf-8', errors='replace')

# Archetype overlay parameters — these match the corresponding hardcoded
# brains in crates/antcolony-sim/src/ai/brain.rs (HeuristicBrain et al.)
ARCHETYPES = {
    "heuristic":    dict(w=0.65, s=0.30, b=0.05, f=0.55, d=0.20, n=0.25, lr=1.0, fr=1.0, ft=20),
    "defender":     dict(w=0.50, s=0.45, b=0.05, f=0.20, d=0.10, n=0.70, lr=0.3, fr=0.5, ft=20),
    "aggressor":    dict(w=0.30, s=0.65, b=0.05, f=0.70, d=0.10, n=0.20, lr=1.5, fr=1.0, ft=30),
    "economist":    dict(w=0.85, s=0.05, b=0.10, f=0.85, d=0.05, n=0.10, lr=0.0, fr=0.3, ft=10),
    "breeder":      dict(w=0.55, s=0.05, b=0.40, f=0.50, d=0.20, n=0.30, lr=0.5, fr=0.7, ft=25),
    "forager":      dict(w=0.95, s=0.00, b=0.05, f=0.90, d=0.05, n=0.05, lr=0.0, fr=1.0, ft=30),
    "conservative": dict(w=0.70, s=0.20, b=0.10, f=0.30, d=0.30, n=0.40, lr=0.3, fr=0.4, ft=15),
}

DIG_HINT = {
    "lasius_niger": 0.20, "camponotus_pennsylvanicus": 0.30, "formica_rufa": 0.30,
    "pogonomyrmex_occidentalis": 0.25, "tetramorium_immigrans": 0.15,
    "tapinoma_sessile": 0.10, "aphaenogaster_rudis": 0.20,
}

def recruit_to_forage(rec):
    return {"mass": 0.70, "group": 0.55, "tandem_run": 0.45, "solitary": 0.40}.get(rec, 0.55)

def eggs_to_nurse(e):
    if e >= 40: return 0.45
    if e >= 25: return 0.30
    if e >= 15: return 0.22
    return 0.18

def species_baseline(toml_path: Path) -> dict:
    with open(toml_path, "rb") as f:
        d = tomllib.load(f)
    name = toml_path.stem
    aggro = float(d["combat"].get("aggression", 0.5))
    eggs = float(d["growth"]["queen_eggs_per_day"])
    rec = d["behavior"].get("recruitment", "mass")
    cr = d["default_caste_ratio"]
    forage = recruit_to_forage(rec)
    dig = DIG_HINT.get(name, 0.20)
    nurse = eggs_to_nurse(eggs)
    tot = forage + dig + nurse
    return dict(
        name=name,
        w=float(cr["worker"]), s=float(cr["soldier"]), b=float(cr["breeder"]),
        f=forage / tot, d=dig / tot, n=nurse / tot,
        lr=min(aggro * 2.0, 3.0),
        fr=max(0.1, 1.0 - aggro),
        ft=20.0,
    )

def normalize_triple(a, b, c):
    s = a + b + c
    if s <= 0: return (1/3, 1/3, 1/3)
    return (a / s, b / s, c / s)

def blend(species: dict, arch: dict, alpha: float) -> dict:
    """alpha=0 -> pure species, alpha=1 -> pure archetype, 0.5 = balanced blend."""
    keys = ["w", "s", "b", "f", "d", "n", "lr", "fr", "ft"]
    out = {k: (1 - alpha) * species[k] + alpha * arch[k] for k in keys}
    out["w"], out["s"], out["b"] = normalize_triple(out["w"], out["s"], out["b"])
    out["f"], out["d"], out["n"] = normalize_triple(out["f"], out["d"], out["n"])
    return out

def to_spec(name: str, p: dict) -> str:
    return (f"tuned:{name}:{p['w']:.3f},{p['s']:.3f},{p['b']:.3f},"
            f"{p['f']:.3f},{p['d']:.3f},{p['n']:.3f},"
            f"{p['lr']:.2f},{p['fr']:.2f},{p['ft']:.1f}")

if __name__ == "__main__":
    species_dir = Path("J:/antcolony/assets/species")
    out_dir = Path("J:/antcolony/bench/blended-tournament")
    out_dir.mkdir(parents=True, exist_ok=True)
    out_file = out_dir / "blended_brains.txt"

    alpha = 0.5
    specs = []
    for toml_path in sorted(species_dir.glob("*.toml")):
        sp = species_baseline(toml_path)
        for arch_name, arch in ARCHETYPES.items():
            blended = blend(sp, arch, alpha=alpha)
            label = f"{sp['name']}__{arch_name}"
            specs.append((label, to_spec(label, blended)))

    with open(out_file, "w", encoding="utf-8") as f:
        for label, spec in specs:
            f.write(f"{label}\t{spec}\n")
    print(f"Wrote {len(specs)} blended brain specs (alpha={alpha}) -> {out_file}")
    # Sample preview
    for label, spec in specs[:5]:
        print(f"  {spec}")
    print(f"  ... ({len(specs)-5} more)")
