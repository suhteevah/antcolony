# Species Bench Harness — Audit Document

**Audience.** Ecologists and biologists who want to verify that the antcolony simulation reproduces real species behavior. Programmers may also use this doc, but it does NOT assume the reader can read Rust.

**Source code.** The harness lives in `crates/antcolony-sim/src/bench/`. Every threshold, metric, and expected range is in source. This doc explains *what* the harness does, *why* each metric exists, and *how* to read its output.

---

## What the harness does

For each ant species the simulation supports, the harness:

1. Loads the species definition from its TOML (`assets/species/{id}.toml`).
2. Builds a standard starter formicarium (one nest module + one outworld + an underground layer + three food clusters).
3. Runs a deterministic headless simulation for N in-game years.
4. Samples colony state on a regular cadence (default: once per in-game day).
5. Scores the run against literature-cited expected ranges.
6. Writes:
   - `<species_id>.csv` — full per-tick telemetry (raw audit data).
   - `<species_id>.md` — human-readable scored report.
   - `SUMMARY.md` (top-level) — composite scores across all species in the run.

---

## Determinism

Same seed + same config = byte-identical telemetry. This is enforced by `crates/antcolony-sim/tests/bench_determinism.rs`. The default seed is `42`. If two runs of the same species at the same seed disagree, that is a bug in the simulation (not the harness) — please report it.

---

## What the harness measures

Each metric carries four pieces of metadata in source (`src/bench/metrics.rs`):

- `human_name` — plain-English label (no jargon)
- `human_definition` — one-paragraph explanation of exactly what is computed
- `units` — the measurement unit
- `interpretation` — how to read the value

The six built-in metrics are:

| Metric | Unit | Interpretation |
|---|---|---|
| Colony survival | boolean | 1 = colony alive at end of run; 0 = extinct |
| Queen survival | boolean | 1 = ≥1 queen alive at end of run; 0 = irrecoverable |
| Brood pipeline health | fraction | Fraction of late-run samples with all 3 brood stages non-zero |
| Adult population stability | CV (dimensionless) | <0.3 = stable; >0.5 = unstable oscillation |
| Food economy ratio | dimensionless | ≥1.0 = sustainable; <0.5 = colony collapsing |
| Hibernation compliance | fraction of years | 1.0 = every year had ≥30 consecutive cold-days for diapause |

Composite score is a weighted average (weights in source: colony_survival=30, queen_survival=20, brood_pipeline=15, stability=10, food_economy=15, hibernation=10). Score 0–100, rendered as ✅/⚠/❌/🛑 verdict bands.

---

## What the harness compares against

For each shipped species there is a **`SpeciesExpectations`** entry in `src/bench/expected.rs`. Each entry contains:

- A pointer to the canonical PhD-level doc (`docs/species/{id}.md`)
- A `key_sources` list — the citations that justify the expected ranges
- One `ExpectedRange` per checked observable (year-5 worker count, brood presence, queen alive, etc.)

Each `ExpectedRange` has:

- `human_name` — plain-English description of what the value represents
- `human_why` — why this matters biologically
- `centroid` — the literature-typical value
- `tolerance` — how strict the comparison is
- `citation` — where the expected centroid came from

### Citation types (see `src/bench/expected.rs::Citation`)

| Type | Used for |
|---|---|
| `PeerReviewed` | A specific journal paper. Format: "Author Year, Journal Vol(Issue):Pages" |
| `ReferenceWork` | A canonical book (Hölldobler & Wilson 1990, Hansen & Klotz 2005) |
| `TaxonomicDatabase` | AntWiki / AntWeb |
| `Extension` | Government/extension publication (UF/IFAS, USFS) |
| `LiteratureRange` | When papers disagree, both endpoints are cited |
| `GamePacing` | Explicitly NOT a biological measurement; sim-pacing choice with written rationale |
| `InternalDoc` | Cross-reference to our own `docs/species/{id}.md` (which carries the primary citation) |

### Tolerance bands (see `src/bench/expected.rs::Tolerance`)

| Band | Multiplier range | When to use |
|---|---|---|
| `Strict` | ±10% | Well-measured values (egg-to-adult duration at standard temperature) |
| `Loose` | ±50% | High natural variance (mature colony population) |
| `OrderOfMagnitude` | 0.1× to 10× | Ballpark only (food return, depends on map layout) |
| `Custom` | asymmetric | Defined per-case |

---

## Known caveats (surfaced in every report)

The harness does not paper over known sim issues. The following are emitted into the report's "Caveats" section automatically when triggered.

### Time scale calibration

The simulation is currently calibrated only for `TimeScale::Seasonal` (60× real-time). Higher scales (`Timelapse`, 1440×) suffer the **long-run colony collapse bug** documented in `HANDOFF.md` ("Open Bug — Long-run colony collapse at non-Seasonal time scales"). Per-tick consumption auto-scales with time scale, but trail-throughput does not, so foragers cannot keep up at higher scales. The harness emits a loud caveat if a non-default scale is used.

The architecturally-correct fix (substep architecture) is parked. Until then, all bench runs default to Seasonal.

### Phase B sim hooks not yet wired

Several species require sim mechanics that are documented in `docs/biology-roadmap.md` but not yet implemented:

- **Camponotus pennsylvanicus** — currently plays as monomorphic mass-recruiting Lasius variant. Wood substrate, polymorphism size buckets, and tandem recruitment are Phase B work.
- **Pogonomyrmex occidentalis** — currently plays as generic Lasius. Sand substrate, granivore food class, and sting damage curve are Phase B+C work.
- **Formica rufa** — currently falls back to claustral founding. Parasitic founding (host = Formica fusca) and thatch mound construction are Phase C work.
- **Tapinoma sessile** — currently plays as small Lasius. Polydomy / nest relocation and supercolony toggle are Phase B work.
- **Aphaenogaster rudis** — currently plays as small Lasius. Myrmecochory (seed-as-food with elaiosome reward) is Phase B+C work.

The harness does NOT score these species against the gameplay-mode-they-ought-to-be-in. Until those hooks land, scores reflect "Lasius-flavored" sim behavior, not species-authentic behavior.

### Hibernation compliance is a proxy

The harness computes hibernation compliance from ambient-temperature samples (counting consecutive in-game days below the diapause threshold). It does not directly inspect the sim's diapause flag because that flag is not exposed in the public API. This proxy correlates with actual diapause behavior but is not a direct measurement.

---

## How to run

```bash
# All seven species, 5-year Seasonal run (default)
cargo run --release --example species_bench -- --all --years 5 --out bench/$(date +%Y-%m-%d)

# Single species, faster smoke
cargo run --release --example species_bench -- --species lasius_niger --years 1 --out bench/lasius

# Custom tolerance: 1-week sample interval, longer run
cargo run --release --example species_bench -- --all --years 25 --sample-every-days 7 --out bench/long
```

A 5-year all-species Seasonal run produces ~7 CSV files (each ~1MB) and ~7 Markdown files plus a SUMMARY.md, total ~10MB.

---

## How to add a species to the harness

1. Write the PhD-level doc at `docs/species/{id}.md` with full citations.
2. Author the TOML at `assets/species/{id}.toml` with inline citation comments.
3. Add a `SpeciesExpectations` entry in `src/bench/expected.rs` and register it in `for_species_id`.
4. Run the bench against the new species and verify the report renders cleanly.
5. Open a PR. CI will run `cargo test --lib bench` to verify the entry has all required fields.

See `CONTRIBUTING-SPECIES.md` (Phase D deliverable, in progress) for the full template.

---

## How to challenge a result

If the bench reports a species score that you (an ecologist) believe is wrong:

1. Open the per-species `.md` report and check the "Literature Expectations" section. Each row tells you (a) what was checked, (b) what the expected centroid was, (c) where the centroid came from, (d) what the sim produced, (e) the verdict.
2. If the **expected centroid is wrong**: open `src/bench/expected.rs`, fix the value, update the citation, file a PR. Include the corrected citation in your PR description.
3. If the **observed value is wrong**: open the corresponding `.csv` to see the full timeline. The bug is either in the simulation (likely) or in how the harness computes the metric (rare). File an issue with the CSV attached.
4. If a **threshold is too strict / too loose**: edit the `Tolerance` for the relevant `ExpectedRange` and explain why in the PR description.

The harness is meant to be **adversarially editable** — your discipline as a reviewer is the actual quality control here, not the code.

---

## Reproducibility checklist (for citations)

Before citing a bench result in a publication or proposal:

- [ ] Did you record the git commit hash of the antcolony source?
- [ ] Did you record the full command line (including `--seed`)?
- [ ] Did you save the `.csv` along with the `.md` report?
- [ ] Did you note the time scale used?
- [ ] Did you check the report's "Caveats" section for any active warnings?

Without these, the bench output is not reproducible to the level an ecology paper requires.
