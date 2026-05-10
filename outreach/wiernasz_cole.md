# Email draft — Diane Wiernasz / Blaine Cole (U Houston)

**Status:** DRAFT — DO NOT SEND. Gated on `repro/cole_wiernasz_growth_curve.md` being completed: 7-year *P. occidentalis* harness producing colony-size points across years 1-7 and a side-by-side comparison with the published growth curve.

**Send-to decision:** Either Wiernasz or Cole. They co-author and are both at U Houston Biology. **Default: Diane Wiernasz**, with Blaine Cole CC'd. Reasoning: Wiernasz is generally the corresponding author on the demography papers; Cole's methodological work is more relevant to the harness setup but the question being asked is about colony-size demography.

**Subject:** Reproducing your *Pogonomyrmex occidentalis* 7-year colony-size curve in a simulation — sanity check on year-1 founding bottleneck

---

Dear Dr. Wiernasz (cc Dr. Cole),

I'm a software engineer running a Rust-based ant-colony simulation as a solo, open-source project. Over the last few weeks I've been working from your demographic papers on *Pogonomyrmex occidentalis* and would value a sanity check on a reproduction.

**The harness.** I've been running 7-year horizons against your published colony-size curve — single-colony solitaire, Wyoming-region climate (mean annual + diapause window), seed and protein diet, no parasitism. Each species' biology is from a TOML grounded in your papers, AntWiki, and the broader *Pogonomyrmex* literature; the relevant docs are:

> github.com/suhteevah/antcolony — `docs/species/pogonomyrmex_occidentalis.md`, `assets/species/pogonomyrmex_occidentalis.toml`

Year-7 mature colony size in the sim is `<N>` workers, against the 6,000-12,000 range you and Cole report. Per-year curve at:

> [link to repro/cole_wiernasz_growth_curve.md — placeholder]

**The specific question.** The sim's year-1 numbers are sensitive to founding-period parameters that aren't well-pinned for *P. occidentalis*, specifically:

1. The fraction of nanitic-cohort survival before the colony reaches stable-foraging equilibrium.
2. Per-queen egg-laying rate during the first overwintering, which I'm currently abstracting as "she lays from food reserves until first thaw."

Both are likely critical to whether the sim's year-1 colony-size point lands in your range. I haven't found published per-day egg-laying figures for the species — most sources (Hölldobler & Wilson 1990, the broader Pogonomyrmex literature) give relative rather than absolute rates. If you have a working figure or a source we should be using, I'd appreciate the pointer. Conversely if the right answer is "this isn't well-pinned in the field either, just match year-7 and don't worry about year-1," that's also useful to know.

Methodology one-pager: `docs/methodology.md` in the same repository. The sim is deterministic (verified bit-identical across processes and thread counts) so the figures are reproducible from a seed.

Appreciate your time. No expectation of reply.

Best,
Matt Gates
Ridge Cell Repair LLC
mmichels88@gmail.com
github.com/suhteevah

---

## Drafting notes (for later self-review)

- The "no expectation of reply" line is honest but also opens the door without obligating. Keep.
- The specific question is the highlight — a researcher will gloss everything else but will read the bullet list because it's a concrete ask in their domain.
- Don't claim the year-7 point matches before the harness has actually been run. Replace `<N>` with the real number, then re-evaluate whether the email is honest.
- Consider including the seed/cargo command for the harness inline so they can run it. Decision: **no** — links only. Asking a tenured PI to run someone's Rust binary is too much.
- Alternative subject if year-1 isn't actually the issue: "Reproducing your *P. occidentalis* growth curve — looking for sanity check on per-day queen egg rate."
- Length budget: 300 words. Currently ~325. Tight.
