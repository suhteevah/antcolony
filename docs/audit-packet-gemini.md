# External Audit Packet — Species Bench Harness

**To the auditor (Gemini, Claude API, or other LLM):** This packet is self-contained. You do not need access to the antcolony codebase to evaluate it. Read everything below, then produce the audit response described in **§7**.

**Audit scope.** Verify that the *species bench harness* shipped in `crates/antcolony-sim/src/bench/` correctly:

1. Encodes published species biology in the `expected` tables (no fabricated citations, no silent numbers).
2. Computes its metrics in a way an ecologist would accept.
3. Surfaces known sim limitations as caveats rather than masking them.
4. Produces audit-grade output (CSV + Markdown) suitable for an ecology paper's supplementary materials.

You are NOT auditing the simulation itself, only the harness that observes and scores it.

---

## §1. What the project is

`antcolony` is a Rust/Bevy ant colony simulator. The differentiating goal is *PhD-grade species fidelity*: every modeled species is grounded in published natural-history literature, with citations carried inline in the simulation parameters. See `J:/antcolony/CLAUDE.md` and `J:/antcolony/docs/biology.md` for the design philosophy.

Currently 7 species ship: *Lasius niger*, *Camponotus pennsylvanicus*, *Formica rufa*, *Pogonomyrmex occidentalis*, *Tetramorium immigrans*, *Tapinoma sessile*, *Aphaenogaster rudis*. Each has:

- A PhD-level natural-history doc at `docs/species/{id}.md` (~1,500-2,400 words, every claim cited).
- A simulation parameter TOML at `assets/species/{id}.toml` with inline citation comments next to every numeric.

---

## §2. What the bench harness is for

The harness exists to **independently verify** that the simulation reproduces the cited biology. Without a verifier, the citation discipline is theater. The harness:

1. Loads each species TOML.
2. Runs a deterministic headless sim of N in-game years with a fixed seed.
3. Samples colony state on a regular cadence.
4. Scores against literature-cited expected ranges.
5. Emits CSV (raw) + Markdown (review).

The Markdown output is meant for ecologists, not programmers — it's self-contained, every metric defined inline, every threshold cited.

---

## §3. The harness source code (full text)

There are five source files. Read them in this order — each builds on the previous.

### 3.1 `expected.rs` — species expected-range table (the biology authority)

```rust
{{INSERT crates/antcolony-sim/src/bench/expected.rs}}
```

### 3.2 `metrics.rs` — what is measured + scoring math

```rust
{{INSERT crates/antcolony-sim/src/bench/metrics.rs}}
```

### 3.3 `run.rs` — the runner (samples + score)

```rust
{{INSERT crates/antcolony-sim/src/bench/run.rs}}
```

### 3.4 `report.rs` — CSV + Markdown output

```rust
{{INSERT crates/antcolony-sim/src/bench/report.rs}}
```

### 3.5 `mod.rs` + CLI

```rust
{{INSERT crates/antcolony-sim/src/bench/mod.rs}}
{{INSERT crates/antcolony-sim/examples/species_bench.rs}}
```

---

## §4. The audit doc (target audience)

This is the doc that an ecologist will read alongside the harness output:

```markdown
{{INSERT docs/bench-audit.md}}
```

---

## §5. Sample species inputs (for context)

One species TOML, fully cited:

```toml
{{INSERT assets/species/lasius_niger.toml}}
```

The corresponding PhD-level biology doc:

```markdown
{{INSERT docs/species/lasius_niger.md}}
```

---

## §6. Known caveats already documented

The harness already emits caveats for these. Audit them as "is this caveat the right one to emit, or is it papering something over?"

- **Time scale calibration.** `Seasonal` (60×) is the only calibrated scale. Higher scales suffer the long-run-collapse bug (per-tick consumption auto-scales but trail throughput does not). Documented in `HANDOFF.md`. Harness emits a loud caveat for non-default scales.
- **Phase B sim hooks not yet wired.** 5/7 species currently play as small-Lasius variants because their defining biology (substrate, granivory, polydomy, parasitic founding, myrmecochory) is not yet implemented. Harness mentions this per-species in the absent-mechanics section.
- **Hibernation compliance is a proxy.** Computed from ambient temp samples (consecutive cold-day stretches), not from the sim's actual diapause flag, because that flag is not exposed in the public API.

---

## §7. Audit response format

Reply in this structure. Be specific: cite line numbers / file names where applicable.

### A. Citation integrity (most important)

For each `Citation::PeerReviewed` / `Citation::ReferenceWork` / `Citation::Extension` entry in `expected.rs`:

- Does the citation **plausibly exist**? (Fake citations would be the worst possible failure mode here.)
- Does the citation **plausibly support the claimed centroid value**?
- Is the citation specific enough that an ecologist could find it?

Flag any that fail any of those.

### B. Metric integrity

For each metric in `metrics.rs`:

- Is the `human_definition` actually correct relative to the code in `compute_*` and `score_run`?
- Is the unit correct?
- Does the interpretation match how the score is computed?
- Is the weighting in `composite_0_to_100` defensible?

### C. Edge case handling

- Are there silent failures? (Division by zero, empty collection, NaN propagation, integer overflow.)
- Does every metric correctly return `None` rather than a fabricated number when not computable?
- Does the report layer render `None` honestly?

### D. Audit-trail completeness

Imagine an ecologist reading only the Markdown report (not the source). Could they:

- Reproduce the run? (seed, scale, years, command line in metadata)
- Identify what was measured vs what was expected vs what was observed?
- Trace each expected value back to a publication?
- Identify caveats that affect interpretation?

If any answer is "no", point to which section needs more.

### E. Specific concerns

Are there any obvious **biology errors** in the expected ranges? (Not "the centroid is too high/low" — actual citation-vs-claim mismatches you can identify from your training corpus.)

### F. Suggestions (ranked)

Top 5 specific improvements you would prioritize. Each should be actionable (file + line + what to change), not vague.

---

## §8. Out of scope

- Simulation correctness (the harness audits the harness, not the sim).
- TOML schema design (covered in `docs/biology-roadmap.md` Phase A).
- Future Phase B/C/D sim hooks (will be audited in their own packets when shipped).
- Game design / playability / UI.

Focus on §3–§6 only.
