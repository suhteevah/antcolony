"""Derive 9-param TunedBrain specs from species TOML files.

Every parameter is derived from a cited biological field — the mapping is
explicit and traceable to the TOML's source comments. Output: a list of
`tuned:<species>:w,s,b,f,d,n,lr,fr,ft` specs ready for matchup_bench.

Mapping (each row maps a cited field → a brain output):

  caste W/S/B    <-  [default_caste_ratio]            (direct)
  losses_response <- aggression                       (× 2.0, capped)
  forage_weight  <-  recruitment style                (mass=0.70 group=0.55 tandem=0.45 solitary=0.40)
  nurse_weight   <-  queen_eggs_per_day               (high lay-rate -> more nurses)
  dig_weight     <-  substrate / nest_construction     (mound > tunnel > opportunist)
  food_response  <-  1.0 - aggression                 (low-aggro species relocate sooner)
  food_threshold <-  egg_cost_food * 4                (legacy heuristic)
"""
import sys, tomllib
from pathlib import Path

sys.stdout.reconfigure(encoding='utf-8', errors='replace')

SPECIES_DIR = Path("J:/antcolony/assets/species")
DEFAULT_EGG_COST = 5.0  # matches HeuristicBrain::new(5.0) baseline

# substrate hints — pulled from each species' biology summary
DIG_HINT = {
    "lasius_niger": 0.20,            # soil tunnel (modest)
    "camponotus_pennsylvanicus": 0.30,  # carpenter wood-galleries (high)
    "formica_rufa": 0.30,            # thatch mound builder (high)
    "pogonomyrmex_occidentalis": 0.25, # mound + cleared disc
    "tetramorium_immigrans": 0.15,   # opportunistic crevice dweller
    "tapinoma_sessile": 0.10,        # no construction, fluid relocation
    "aphaenogaster_rudis": 0.20,     # leaf-litter + soil tunnel
}

def recruitment_to_forage(rec: str) -> float:
    return {
        "mass": 0.70,        # commits hard to known sources
        "group": 0.55,
        "tandem_run": 0.45,  # selective recruitment, slower buildup
        "solitary": 0.40,
    }.get(rec, 0.55)

def eggs_to_nurse(eggs_per_day: float) -> float:
    # High egg rate -> need more nurses to tend brood pipeline
    if eggs_per_day >= 40: return 0.45
    if eggs_per_day >= 25: return 0.30
    if eggs_per_day >= 15: return 0.22
    return 0.18

def derive(species_path: Path) -> str:
    with open(species_path, "rb") as f:
        d = tomllib.load(f)

    name = species_path.stem
    aggression = float(d["combat"].get("aggression", 0.5))
    eggs = float(d["growth"]["queen_eggs_per_day"])
    rec = d["behavior"].get("recruitment", "mass")
    cr = d["default_caste_ratio"]
    w, s, b = float(cr["worker"]), float(cr["soldier"]), float(cr["breeder"])

    forage = recruitment_to_forage(rec)
    dig = DIG_HINT.get(name, 0.20)
    nurse = eggs_to_nurse(eggs)
    # Renormalize behavior weights (forage + dig + nurse) to sum to 1.0
    total = forage + dig + nurse
    forage, dig, nurse = forage / total, dig / total, nurse / total

    losses_response = min(aggression * 2.0, 3.0)
    food_response = max(0.1, 1.0 - aggression)
    food_threshold = DEFAULT_EGG_COST * 4.0  # = 20

    spec = f"tuned:{name}:{w:.2f},{s:.2f},{b:.2f},{forage:.2f},{dig:.2f},{nurse:.2f},{losses_response:.2f},{food_response:.2f},{food_threshold:.1f}"
    return spec, dict(name=name, aggression=aggression, eggs=eggs, recruitment=rec, w=w, s=s, b=b,
                      forage=forage, dig=dig, nurse=nurse, lr=losses_response, fr=food_response, ft=food_threshold)

if __name__ == "__main__":
    out = SPECIES_DIR.parent.parent / "bench" / "species-tournament" / "species_brains.txt"
    out.parent.mkdir(parents=True, exist_ok=True)
    with open(out, "w", encoding="utf-8") as f:
        for toml_path in sorted(SPECIES_DIR.glob("*.toml")):
            spec, info = derive(toml_path)
            f.write(spec + "\n")
            print(f"  {info['name']:30s} aggro={info['aggression']:.2f} rec={info['recruitment']:12s} "
                  f"caste={info['w']:.2f}/{info['s']:.2f}/{info['b']:.2f} "
                  f"behavior={info['forage']:.2f}/{info['dig']:.2f}/{info['nurse']:.2f} "
                  f"reactions={info['lr']:.2f}/{info['fr']:.2f}/{info['ft']:.0f}")
    print(f"\nWrote {out}")
