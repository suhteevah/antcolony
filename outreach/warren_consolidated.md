# Email draft — Robert J. Warren II (Buffalo State)

**Status:** DRAFT — DO NOT SEND. Gated on (a) `predates_ants` schema + combat hookup and (b) the cold-foraging-vs-temperature curve being added to the sim, and (c) the two `repro/` writeups (`warren_chick_2013_cold_foraging.md` and `rodriguez_cabal_2012_displacement.md`) being completed.

**Subject line option A:** Reproducing Warren & Chick 2013 + Rodriguez-Cabal 2012 in a Rust ant simulation — sanity check?

**Subject line option B:** *Aphaenogaster rudis* cold foraging + *Brachyponera* displacement — published-figure comparison

---

Dear Dr. Warren,

I'm a software engineer (not an academic — context: solo developer, formerly worked on systems-level Rust). Over the last few months I've been building an ECS-architectured ant colony simulator with the goal of producing per-species behavior that's faithful enough to compare against published field figures rather than just cosmetically plausible.

I've been working from your *A. rudis* literature and would value a sanity check on two reproductions:

**1. Warren & Chick 2013, *Glob. Change Biol.* — cold-tolerance foraging.** I've added a per-species soft cold-foraging-vs-temperature curve (replacing a binary diapause cutoff) and run *A. rudis* across the same temperature range. The simulated forager-activity-vs-temperature curve and your Fig. 2/3 are at:

> [link to repro/warren_chick_2013_cold_foraging.md — placeholder]

The match is `<within ε / off by Δ>` — depending on what the harness ends up showing. I'd like to know whether the comparison is meaningfully analogous: is per-tick "ant in a non-Idle non-Returning state" a defensible analog to your field-observation foraging metric, or is there a confound (e.g., return-trip ants counted as foragers in field but not in sim)?

**2. Rodriguez-Cabal 2012 / Warren et al. 2018 — *B. chinensis* displacement of *A. rudis*.** Two-colony harness, both species established, 5-year horizon. Comparing relative *A. rudis* abundance and seed-removal rate against the ~96% reduction / ~70% reduction reported. Writeup at:

> [link to repro/rodriguez_cabal_2012_displacement.md — placeholder]

The blocker has been getting active ant predation (rather than symmetric combat) into the sim — *B. chinensis* needs to treat *A. rudis* foragers as prey. That's now in. I'd value a sanity check on whether the displacement gradient I'm seeing is at the right rate, or whether it's matching the published figure for the wrong reason.

The full sim is open-source (Rust, MIT) at github.com/suhteevah/antcolony. The methodology one-pager is at `docs/methodology.md`. Per-species sociometric biology is at `docs/species/aphaenogaster_rudis.md` and `docs/species/brachyponera_chinensis.md`, both citing AntWiki + your papers + Bednar & Silverman + Lubertazzi.

If neither figure-comparison is at a usable point, I'd rather know now than ship the wrong claim. No expectation of reply — appreciate your time either way.

Best,
Matt Gates
Ridge Cell Repair LLC
mmichels88@gmail.com
github.com/suhteevah

---

## Drafting notes (for later self-review)

- Don't pre-claim the comparison works. Frame it as "here are the numbers, are we doing this right." The researcher is more useful as a reviewer than as a confirmer.
- The sim has *F. fusca* shipped too but Warren doesn't work on Formica. Don't pad the email with extras.
- Subject A is more informative; Subject B is shorter. Pick A unless first line is also subject (some clients).
- Two papers in one email is the right call: same researcher, related findings, no point making him read two.
- Length budget: 300 words. Currently ~390. Can trim the *B. chinensis* paragraph to one sentence about predation if needed.
- DO NOT include the methodology doc as an attachment — link only. Inboxes don't like 12kB markdown.
