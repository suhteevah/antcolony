"""Generate perturbed variants of a named archetype.

Used by adversarial-FSP: each round, identify the archetypes that beat
the current MLP hardest, generate 3 variants each by perturbing the
9-tuple params ±15%, and add them to the next round's training pool.

Why mutate instead of inventing: the named archetypes are tuned by hand
to be coherent strategies. Random perturbations stay close enough to the
seed to remain coherent while exploring nearby strategy space.

Args:
  --archetype <name>     Name of base archetype (heuristic, defender, ...)
  --n <int>              Number of variants to generate (default 3)
  --magnitude <float>    Perturbation magnitude in [0,1] (default 0.15)
  --seed <int>           RNG seed for reproducibility
"""
import argparse, random, sys
from pathlib import Path

sys.stdout.reconfigure(encoding='utf-8', errors='replace')

# Archetype 9-tuples: (W, S, B, F, D, N, lr, fr, ft)
# Match BrainArchetype::params() in crates/antcolony-sim/src/ai/brain.rs
ARCHETYPE_PARAMS = {
    "heuristic":    (0.65, 0.30, 0.05, 0.55, 0.20, 0.25, 1.0, 1.0, 20.0),
    "defender":     (0.50, 0.45, 0.05, 0.20, 0.10, 0.70, 0.3, 0.5, 20.0),
    "aggressor":    (0.30, 0.65, 0.05, 0.70, 0.10, 0.20, 1.5, 1.0, 30.0),
    "economist":    (0.85, 0.05, 0.10, 0.85, 0.05, 0.10, 0.0, 0.3, 10.0),
    "breeder":      (0.55, 0.05, 0.40, 0.50, 0.20, 0.30, 0.5, 0.7, 25.0),
    "forager":      (0.95, 0.00, 0.05, 0.90, 0.05, 0.05, 0.0, 1.0, 30.0),
    "conservative": (0.70, 0.20, 0.10, 0.30, 0.30, 0.40, 0.3, 0.4, 15.0),
}

def renormalize(a, b, c):
    s = a + b + c
    return (a/s, b/s, c/s) if s > 0 else (1/3, 1/3, 1/3)

def perturb(seed_params, rng, magnitude):
    """Perturb 9-tuple by uniform +/- magnitude * seed_value, then renormalize triples."""
    out = list(seed_params)
    for i in range(len(out)):
        delta = rng.uniform(-magnitude, magnitude) * abs(out[i] if out[i] != 0 else 0.1)
        out[i] = max(0.0, out[i] + delta)
    # Renormalize caste triple (0,1,2) and behavior triple (3,4,5)
    out[0], out[1], out[2] = renormalize(out[0], out[1], out[2])
    out[3], out[4], out[5] = renormalize(out[3], out[4], out[5])
    # Clamp reaction params to plausible ranges
    out[6] = max(0.0, min(out[6], 3.5))    # losses_response
    out[7] = max(0.05, min(out[7], 1.5))   # food_response
    out[8] = max(5.0, min(out[8], 50.0))   # food_threshold
    return tuple(out)

def to_spec(label, p):
    return f"tuned:{label}:{p[0]:.3f},{p[1]:.3f},{p[2]:.3f},{p[3]:.3f},{p[4]:.3f},{p[5]:.3f},{p[6]:.2f},{p[7]:.2f},{p[8]:.1f}"

if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--archetype", required=True)
    ap.add_argument("--n", type=int, default=3)
    ap.add_argument("--magnitude", type=float, default=0.15)
    ap.add_argument("--seed", type=int, default=42)
    args = ap.parse_args()

    if args.archetype not in ARCHETYPE_PARAMS:
        print(f"Unknown archetype: {args.archetype}", file=sys.stderr)
        sys.exit(1)

    rng = random.Random(args.seed)
    seed_params = ARCHETYPE_PARAMS[args.archetype]
    for i in range(args.n):
        p = perturb(seed_params, rng, args.magnitude)
        label = f"{args.archetype}_var{i+1}"
        print(f"{label}\t{to_spec(label, p)}")
