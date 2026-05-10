# Email draft — Anna Dornhaus (U Arizona)

**Status:** DRAFT — DO NOT SEND. Gated on (a) per-ant activity-fraction tracking being implemented in the sim, (b) the *Temnothorax curvinodis* lazy-worker bench (`crates/antcolony-sim/examples/lazy_worker_bimodality_bench.rs`) being built, and (c) `repro/charbonneau_2017_lazy_workers.md` being completed.

This is the **largest sim gap** of the four outreach targets — per-ant activity tracking does not exist today. Email accordingly cannot be sent until that ships.

**Subject:** Reproducing Charbonneau-Sasaki-Dornhaus 2017 inactive-worker bimodality in a Rust ant simulator

---

Dear Dr. Dornhaus,

I'm a software engineer running a Rust ant-colony simulator as a solo open-source project. The sim is deterministic and built around emergent trail-pheromone dynamics with per-species biology grounded in TOML configurations and citation-tagged docs. I've been working through your group's *Temnothorax* literature with the goal of reproducing the inactive-worker bimodality finding from your 2017 *PLoS ONE* paper.

**The reproduction attempt.** I've added per-ant activity-fraction tracking — every ant now carries a counter of ticks-spent-in-each-FSM-state across its lifetime. Running a *T. curvinodis* colony to maturity (target ~200 workers per the published demography) and plotting per-ant activity-fraction histograms produces:

> [link to repro/charbonneau_2017_lazy_workers.md — placeholder]

Two questions where I'd value your judgment:

**1. Is the bimodality I'm seeing real?** The sim produces a population at high activity-fraction (foragers + nurses doing visible work) and a population at near-zero activity-fraction (workers in `Idle` essentially permanently). Whether the gap is *bimodal* in the strong sense or just bottom-heavy is sensitive to the histogram bin width and the colony horizon. I'd appreciate a sanity check on whether the histogram I'm producing is showing the same shape as your Fig. 2-3.

**2. Reserve-labor mobilization on worker removal.** The follow-on test is to remove a fraction of active workers and observe whether previously-inactive workers transition to active states. I can implement that ablation deterministically (the sim is bit-reproducible from a seed). The critical methodological question: in your study, the inactive workers that mobilized — were they previously demonstrably inactive (low activity-fraction across many days), or were they sampled before they had become "permanent" inactives? The sim's assumption matters for whether I can claim the reproduction.

A specific concern with reproducing this finding in a sim: my decision rules don't currently model the underlying source of inactivity (whatever it actually is — physiological, behavioral, age-related). The sim produces inactive workers as a consequence of ECS state-machine dynamics rather than from a modeled trait. So even if the histogram matches, the underlying mechanism doesn't. I'm comfortable describing the reproduction as "the sim produces the same population-level statistic via a different mechanism" but want to be sure that framing is honest to the finding.

Code, methodology, and per-species biology: github.com/suhteevah/antcolony, with `docs/methodology.md` and `docs/species/temnothorax_curvinodis.md` as the relevant entries.

No expectation of reply. Appreciate your time.

Best,
Matt Gates
Ridge Cell Repair LLC
mmichels88@gmail.com
github.com/suhteevah

---

## Drafting notes (for later self-review)

- This is the email most likely to elicit a substantive reply, because the reservations stated are genuine and Dornhaus's group is on record taking these methodological points seriously. Don't soften them.
- The "different mechanism" caveat is the load-bearing piece. If the sim's bimodality is just a numerical artifact of the FSM state distribution, that's important to know — and Dornhaus is exactly the right person to call it out.
- Pratt 2005 quorum-sensing emigration is **deferred** per HANDOFF.md. Don't bundle it into this email — it'd dilute the focused ask.
- The reservation about decision-rule mechanism may dissuade her from replying — that's fine. Better to be honest and unanswered than to claim a reproduction that doesn't hold up under examination.
- Length: 380 words. Slightly over budget but the methodological caveat is the point of the email.
- DO NOT name-drop other reproductions in progress. This email is about one paper.
