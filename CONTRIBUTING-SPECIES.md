# Contributing a New Species to Antcolony

This guide walks you through adding a new ant species to the antcolony simulator. The bar is **PhD-grade biology fidelity** — every claim cited, every parameter justified, every gameplay departure from real biology explicitly tagged. CI + maintainer review enforce these rules.

If your favorite species takes more than a weekend to add, that means we are doing it right.

---

## Before you start

Read these in order:

1. **`docs/biology.md`** — the canonical natural-history mechanism log. Every general claim ("queens lay more when fed more protein") lives here.
2. **`docs/biology-roadmap.md`** — what species fidelity means for the sim, what hooks exist, what is parked. Your species' difficulty bucket maps directly to which hooks it needs.
3. **`docs/bench-audit.md`** — how the species bench harness scores your species against literature. Your PR must include a passing bench run.
4. **An existing reference species doc**, e.g. `docs/species/lasius_niger.md` — the format you will mirror.

If your species is closely related to one already shipped, read that species' doc end to end. Many ant traits are shared at the genus or subfamily level.

---

## Files you will create or modify

```
assets/species/<id>.toml                   NEW — sim parameters, fully cited
docs/species/<id>.md                       NEW — PhD-level natural-history doc
crates/antcolony-sim/src/bench/expected.rs MODIFY — add SpeciesExpectations + register in for_species_id
```

Three files. Nothing else, unless your species needs a brand-new sim mechanic that no shipped species uses (in which case, open an RFC issue first — that's Phase B/C territory and warrants a separate PR).

---

## Step 1 — Write the natural-history doc

Path: `docs/species/<id>.md`. Target length: 1,500–2,500 words.

Required sections (mirror `lasius_niger.md` exactly):

1. **Identity** — taxonomy, common names, range, habitat
2. **Morphology** — caste sizes, color, polymorphism, distinguishing features vs sister species
3. **Colony lifecycle** — nuptial flight timing, founding mode, queen count, mature population, colony lifespan
4. **Caste & development** — egg/larva/pupa durations, worker lifespan, queen lifespan, brood biology
5. **Foraging & diet** — what they eat, how they recruit, mutualisms
6. **Nest architecture** — substrate, depth, chamber organization, dig rate, mound construction
7. **Defense & combat** — weapon (mandible / sting / spray / chemical), aggression, predator response
8. **Climate & hibernation** — temperate vs tropical, hibernation requirement
9. **Sim implications** — how this biology should land in our parameters; bullet list, link back to `biology.md` mechanism sections
10. **Sources** — peer-reviewed papers (Author Year, Journal Vol(Issue):Pages), reference works, AntWiki/AntWeb, keeper sources clearly segregated

**Citation discipline.** Every quantitative claim needs a source. When literature disagrees, say so explicitly. When a number is unmeasured, say so. **Never invent or paraphrase a citation you have not personally verified exists** — fabricated citations are an instant PR rejection.

---

## Step 2 — Write the species TOML

Path: `assets/species/<id>.toml`. Mirror the structure of `lasius_niger.toml`.

**Every numeric value gets an inline `# comment` citing the source from your doc OR an explicit `# game-pacing — <reason>` tag.** No silent numbers. CI will reject TOMLs that fail this rule.

Example pattern:

```toml
queen_lifespan_years = 28.0  # Hermann Appel captive record 28y 8mo, Kutter & Stumper 1969; reviewed in Keller & Genoud 1997 Nature
worker_lifespan_months = 24.0  # game-pacing — natural ~1-12mo (Kramer et al. 2016), scaled up so adult cohort persists visibly across sim sessions
```

Required sections (in order):
- Top metadata (`id`, `common_name`, `genus`, `species_epithet`, `difficulty`, **`schema_version = 1`**)
- `[biology]`, `[growth]`, `[diet]`, `[combat]`, `[appearance]`, `[default_caste_ratio]`
- **Phase A extensions** (all optional but you should fill the relevant ones): `[behavior]`, `[colony_structure]`, `[substrate]`, `[combat_extended]`, `[diet_extended]`, `[ecological_role]`
- `[encyclopedia]` (player-facing text)

**Critical TOML gotcha — `schema_version` placement.** TOML parses any bare key written *after* a `[section]` header as a field of that section. So `schema_version = 1` placed after `[default_caste_ratio]` becomes `default_caste_ratio.schema_version`, not a top-level Species field. Always put `schema_version` in the top metadata block (right under `difficulty`). The `validate-species` CLI catches this with a `schema_version=0` error message.

### Picking a difficulty bucket

| Bucket | What qualifies |
|---|---|
| `beginner` | Standard claustral, monogyne, monomorphic, generic loam substrate, mass or no recruitment, no exotic mechanics. Lasius / Tetramorium pattern. |
| `intermediate` | Polydomy + relocation, supercolony toggle, OR myrmecochory food class — needs Phase B sim hooks but degrades gracefully when off. Tapinoma / Aphaenogaster pattern. |
| `advanced` | Defining mechanics not yet in sim (wood substrate, granivory, sand substrate, sting damage curve, tandem-only recruitment with no mass fallback). Camponotus / Pogonomyrmex pattern. |
| `expert` | Parasitic founding (host-species required), thatch construction, polygyne supercolony, OR multiple advanced traits combined. Formica rufa pattern. |

Be honest about the bucket. Marking `beginner` to "make it accessible" when your species is actually `expert` will produce a sim run that does not reflect its biology.

---

## Step 3 — Register the species expected ranges

Path: `crates/antcolony-sim/src/bench/expected.rs`.

Add a function that returns a `SpeciesExpectations` for your species, modeled after the existing entries (e.g. `lasius_niger()`). Then add an arm to the `for_species_id` match.

Required fields per `ExpectedRange`:
- `human_name` — plain English (no jargon)
- `human_why` — one sentence: why does an ecologist care about this number?
- `centroid` — the literature-typical value
- `tolerance` — `Strict` / `Loose` / `OrderOfMagnitude` / `Custom`
- `citation` — `PeerReviewed` / `ReferenceWork` / `TaxonomicDatabase` / `Extension` / `LiteratureRange` / `GamePacing` / `InternalDoc`

**Tolerance picking guidance:**
- `Strict` (±10%): well-measured values like egg-to-adult duration at standard temperature.
- `Loose` (±50%): high natural variance (mature colony population — two-orders-of-magnitude variation across populations is normal).
- `OrderOfMagnitude` (×0.1 to ×10): values that depend on sim map layout (food return rate).

If you mark something `GamePacing`, the bench will render its verdict as `[PACING — ...]` so an ecologist can tell biology checks from gameplay tuning.

---

## Step 4 — Validate

Three checks, in increasing depth.

```powershell
# (a) Fast: parse + Phase A schema correctness
cargo test --release -p antcolony-sim --test phase_a_toml_compat

# (b) Authoritative: full validator. Runs each TOML through the bench
#     harness and rejects CONFIG REJECTED, sim-init failures, and
#     [FAILED] composite scores. Use this to gate your PR locally.
cargo run --release --bin validate-species -- assets/species/<id>.toml

# (c) Deep: full bench output (CSV + Markdown, multi-year)
cargo run --release --example species_bench -- --species <id> --years 5 --out bench/contrib-<id>
```

Open `bench/contrib-<id>/<id>.md` and check:

1. Composite score is reported (not `n/a`).
2. No `CONFIG REJECTED` or `Sim-init failure` caveats.
3. Verdict is at least `[MARGINAL]` (60+/100) — `[CONCERNING]` or `[FAILED]` means either your TOML numbers are wrong or the sim cannot yet model this species. In the latter case, your PR may need to wait for a Phase B sim hook.
4. Spot-check the per-metric `Observed` column against your doc's claims.

Attach `<id>.md` and `<id>.csv` to your PR.

---

## Step 5 — Open the PR

PR template (the maintainer will copy these into the body if you forget):

```markdown
## Species: *<Genus species>* (<Common name>)

- Difficulty bucket: <beginner|intermediate|advanced|expert>
- Source-of-truth doc: `docs/species/<id>.md`
- TOML: `assets/species/<id>.toml`
- Bench expectations: `src/bench/expected.rs` arm `<id>()`

## Bench output
- Composite: `<XX.X>/100` — `[VERDICT]`
- Caveats: `<count>` (none / list)
- Per-species report attached: `bench/contrib-<id>/<id>.md`

## Citations spot-check
At least 3 of the citations in my TOML / doc / expected.rs that are most load-bearing for the run:
- `<author year, journal>` — used for `<field>`
- `<author year, journal>` — used for `<field>`
- `<author year, journal>` — used for `<field>`

## Sim hooks needed (if any)
- [ ] None — species plays correctly with current sim
- [ ] Wood substrate (Phase C) — currently falls back to loam
- [ ] Granivore food class (Phase C) — currently falls back to generic food
- [ ] Polydomy + relocation (Phase B) — currently single-nest
- [ ] Other: <describe>
```

---

## What the maintainer will check

1. **Citations are real.** The maintainer will spot-check ~3 of your numerical citations against their actual papers. Anything that does not match is grounds for PR rejection.
2. **Difficulty bucket is honest.** A `beginner`-tagged species that depends on Phase B hooks gets bumped.
3. **Bench passes ≥ MARGINAL.** Below that threshold needs an explanation.
4. **No silent numbers.** Every TOML field has either a citation comment or a `game-pacing` tag.
5. **Encyclopedia text is player-facing prose**, not citation-laden literature review. Save the rigor for the doc.

---

## When the literature is incomplete

You will run into species where some of the standard fields are not measured. That is normal — file biology has huge holes, especially for tropical species. Three valid responses:

1. **Use the closest sister species' value** with a comment: `# 1.5 — no measurement for this species; cited from sister sp. X (Author Year)`.
2. **Use a `GamePacing` tag** with a written rationale: `# game-pacing — no published lifespan for this species; placed at genus median to keep colony cycle visible`.
3. **Skip the field** if it's optional. Do not invent.

Never write a numeric value with no comment. Even `# unmeasured, value chosen by guess` is better than silence.

---

## Adding a new sim mechanic at the same time

If your species needs a mechanic that no shipped species uses (e.g. social parasitism for Polyergus, leaf-cutter fungus garden for Atta, raiding columns for Eciton), open a separate **Phase B/C RFC issue first**. Bundling new sim mechanics into a species PR makes the PR un-reviewable.

The species TOML can pre-declare the field your mechanic will need (e.g. `host_species_required = ["formica_fusca"]` already exists in the schema), but the mechanic implementation is a separate PR.

---

## Questions

Open an issue tagged `species-contrib` and describe what you are stuck on. Include the species name and which step from this doc you are on.
