# Researcher Outreach — Plan and Status

> Drafts for emails to working myrmecologists whose published findings the simulation aims to reproduce. **Nothing in this directory has been sent.** All emails are drafts pending the reproductions being built and validated.

---

## Status (2026-05-09)

🟡 **Drafts only — DO NOT SEND.**

Per HANDOFF.md, the gating sequence before any of these go out:

1. Confirm the 2yr heuristic smoke shows all 8 species surviving year-2 hibernation. If any species dies, that's a balance bug — fix before claiming species-faithfulness to a researcher.
2. Implement the three sim hot-path additions (`predates_ants` field+combat hookup, per-ant activity-fraction tracking, soft cold-foraging-vs-temperature curve).
3. Build the four reproduction harnesses (`*_bench.rs` examples + per-paper `repro/<paper>.md` writeups with the figure, our number, the published number, deviation).
4. Post results in `repro/`. Self-review against [`docs/methodology.md`](../docs/methodology.md).
5. **Then, and only then, send the emails below.**

Premature outreach is the failure mode to avoid: Matt's name attached to a half-built reproduction destroys the credibility we'd need for any later, better-sourced project.

## The four papers and three researchers

| # | Researcher | Paper | Sim status | Email draft |
|---|---|---|---|---|
| 1 | Robert J. Warren II (Buffalo State) | Warren & Chick 2013, *Glob. Change Biol.* — *A. rudis* cold-tolerance foraging | Soft cold-curve schema not yet added; binary diapause cutoff only | [`warren_consolidated.md`](warren_consolidated.md) |
| 2 | Robert J. Warren II (Buffalo State) | Rodriguez-Cabal 2012 / Warren et al. 2018 *Ecosphere* — *B. chinensis* displacement of *A. rudis* | `predates_ants` not yet wired; two-colony scenario possible but no published-figure match | [`warren_consolidated.md`](warren_consolidated.md) (combined) |
| 3 | Diane Wiernasz / Blaine Cole (U Houston) | Cole & Wiernasz, *Insectes Sociaux* — *P. occidentalis* 7yr growth curve to 6,000-12,000 workers | No species-specific blocker. Need to run the 7yr harness and compare against the published colony-size curve | [`wiernasz_cole.md`](wiernasz_cole.md) |
| 4 | Anna Dornhaus (U Arizona) | Charbonneau, Sasaki & Dornhaus 2017 *PLoS ONE* — *Temnothorax* "lazy worker" bimodality + reserve-labor mobilization | Per-ant activity-fraction tracking not yet implemented; this is the largest sim gap of the four | [`dornhaus_charbonneau.md`](dornhaus_charbonneau.md) |

Paper 5 (Pratt 2005 quorum-sensing emigration) is **deferred** per HANDOFF.md — relocation mechanics in the present sim are coarse and don't yet reproduce decision-quorum dynamics.

## Tone calibration

These researchers are working scientists with limited time. The drafts aim for:

- **Specific.** Reference the paper by figure number, not a hand-wave at "your work."
- **Provisional.** Frame as "we tried to reproduce this; here's our number; here's what we may have wrong" — not "we have validated your finding."
- **Short.** Under 300 words per email body. Links to repository and the relevant `repro/<paper>.md` writeup carry the detail.
- **Asking for one specific thing.** Either "is this comparison sensible?" or "did we get the field setup right?" — never "would you advise this project?"
- **No fanboy.** Matt is a Rust engineer with an ant simulation, not a graduate student. Approach is collegial, not deferential.

## What the recipient gets

When a draft does eventually go out, the recipient will receive:

1. The email body itself.
2. A link to the public repository (`github.com/suhteevah/antcolony`).
3. A link to the per-paper writeup at `repro/<paper-slug>.md` containing:
   - The published figure (cited).
   - Our number from the sim.
   - The deviation, with a discussion of which sim abstractions are load-bearing.
   - The specific ant-biology decisions from the species TOML and per-species doc that drive the result.
   - A reproduction recipe (cargo command, seed, expected output).

If the writeup isn't compelling on its own, the email won't make it compelling — so the writeup must land first.

## Where to send

| Researcher | Affiliation | Public contact |
|---|---|---|
| Robert J. Warren II | Buffalo State College, Biology | TBD — verify on department page before sending |
| Diane Wiernasz / Blaine Cole | U Houston, Biology and Biochemistry | TBD — verify on department page before sending |
| Anna Dornhaus | U Arizona, Ecology and Evolutionary Biology | TBD — verify on department page before sending |

**Verify each address from the researcher's current institutional page within 7 days of sending.** Department pages are the source of truth; ResearchGate / Google Scholar may have stale or personal addresses.
