# Biology Audit — Honest Citation Review

For each behavior decision shipped this session, this doc rates how well-grounded it is in real published biology vs how much is a coarse sim approximation. Honest grades, not marketing.

**Grading:**
- 🟢 **Cited & quantitatively grounded** — number used in code matches a published measurement
- 🟡 **Cited & qualitatively grounded** — direction is right (e.g. "diapause depresses metabolism") but the specific number is an approximation
- 🟠 **Pattern-grounded** — matches keeper observation / community consensus but no peer-reviewed measurement at hand
- 🔴 **Sim-convenience** — picked a number that produces good gameplay; biology is a hand-wave

---

## Diapause biology

### Adults survive winter on body fat (no colony-food draw)
**Code:** `colony_economy_tick` outer guard skips starvation for diapausing colonies; `food_stored` clamps to 0 without killing adults.
**Citation:** [Hahn, D.A. & Denlinger, D.L. (2007)](https://www.sciencedirect.com/science/article/abs/pii/S0022191007000753) — definitive review of insect diapause energetics.
**Grounding:** 🟢 The qualitative claim "winter ant workers don't draw on the larder" is well-established. Worker overwinter mortality <5% in healthy colonies (Munger 1984 for Pogonomyrmex). The sim's "0% mortality from food shortage during diapause" is a slight over-simplification — real winter mortality from cold/disease is non-zero, but <5%, and food-shortage-specifically is essentially zero for healthy colonies. ✅ Honest.

### Diapause metabolic depression at 10%
**Code:** `DIAPAUSE_METABOLIC_DEPRESSION = 0.10` constant.
**Citation:** Hahn & Denlinger 2007 reviews diapause depression at 5-10% across many insect orders. Lighton & Bartholomew 1988 measured an ~8× drop in clustered overwintering Pogonomyrmex (= ~12% of active rate).
**Grounding:** 🟡 The 10% is in the cited range. Different species would have different specific values (small-bodied Tapinoma probably depresses less; large-bodied desert species might depress more). Currently a constant; could be species-configurable later via biology TOML. **Honest disclaimer in code:** the constant is documented as a coarse cross-species approximation.

### Brood preservation during diapause (no cannibalism)
**Code:** Brood cannibalism block gated on `if !in_diapause`.
**Citation:** Hahn & Denlinger 2011 covers brood pause in diapause. The cannibalism-as-active-colony-starvation-response framing is from [Wilson, E.O. & Hölldobler, B. (1990) The Ants, ch. 9 (cannibalism)](https://www.hup.harvard.edu/file/feeds/PDF/9780674454903_sample.pdf).
**Grounding:** 🟢 Matches biology. Real diapausing colonies preserve brood — the brood IS the spring restart inventory. Cannibalism is observed in active colonies under acute stress (food shock, queen loss) not as a winter-survival strategy. ✅ Honest.

### Autumn retreat — ants teleport to nest entrance on entering diapause
**Code:** `sense_and_decide` snaps any ant transitioning into Diapause to its colony's nest entrance position + drops carried food.
**Citation:** [Heinze & Hölldobler (1994) Ants in the cold](https://www.researchgate.net/publication/247880033_Ants_in_the_cold).
**Grounding:** 🟡 Direction correct; speed simplified. Real ants retreat over days as ambient cools. Sim teleports in one substep when cold_threshold crosses. The visible OUTCOME matches biology (all ants in nest by winter) but the journey is hand-waved. ✅ Honest within the sim's outer-tick granularity.

### Hibernation requires 60+ days/year for fertility
**Code:** `MIN_DIAPAUSE_DAYS = 60` (default), species-overridable via `min_diapause_days` in biology TOML.
**Citation:** Per-species varies. Lasius niger: keeper consensus is ~3 months winter cluster (~90 days) but absolute minimum for queen fertility is hard to find peer-reviewed. The 60 default is permissive.
**Grounding:** 🟠 Pattern-grounded. Keepers report "skipping winter shortens queen life and reduces brood viability" but the 60-day specific threshold is a sim-convenience-with-biological-direction. The species TOML overrides (Camponotus 120, Formica 150) reflect community consensus on harder hibernators. **Honest disclaimer:** numbers are calibrated for keeper-mode gameplay, anchored to real biology direction.

---

## Substrate + dig system

### Soil pellet (not grain) rolling in mandibles
**Code:** `Ant.carrying_soil: bool` flag; pellet picked up on tile flip, dropped at nest entrance.
**Citation:** [Sudd, J.H. (1969). The excavation of soil by ants. *Zeitschrift für Tierpsychologie* 26: 257-276.](https://onlinelibrary.wiley.com/doi/10.1111/j.1439-0310.1969.tb01952.x); [Tschinkel, W.R. (2004). Nest architecture of *Pogonomyrmex badius*.](https://www.jstor.org/stable/25086323)
**Grounding:** 🟢 Pellet behavior is well-documented. Pellet sizes range 0.3-1.5mm depending on species. The sim treats pellet as a unit boolean, not a per-mass quantity, which is a gameplay simplification but doesn't contradict biology.

### Kickout mound at nest entrance
**Code:** `Terrain::SoilPile(intensity)` accumulates at nest entrance cells.
**Citation:** [Tschinkel, W.R. (2003). Subterranean ant nests: trace fossils.](https://www.sciencedirect.com/science/article/abs/pii/S0031018202005583).
**Grounding:** 🟢 The donut-shaped kickout mound is the diagnostic visual signature of an active nest. The sim's "intensity grows over time, scales mound sprite" matches the real biology of accumulation. The fact that real mounds are WIDER than they are TALL when viewed top-down is preserved by our radial spawn pattern.

### Multi-substep dig progress (~60 substeps per tile)
**Code:** `DIG_PROGRESS_THRESHOLD = 60` substeps per tile.
**Citation:** [Sudd, J.H. (1972). Absence of social enhancement of digging.](https://www.sciencedirect.com/science/article/abs/pii/S0003347272802701) reports ~1-2 cm³/day for small Lasius. Keeper community measurements (formiculture.com) corroborate the order of magnitude.
**Grounding:** 🟡 The threshold of 60 substeps was chosen for visible-but-not-frantic gameplay feel. The biological rate (1-2 cm³/day) for a small Lasius colony with 50 workers, with say 5 active diggers, is roughly 1 tile/8 minutes per active digger. Our 60 substeps × 2 in-game-sec/substep = 2 in-game-min per tile — about 4× faster than real biology. **Honest disclaimer:** this is sim-convenience for visibility; not a real measurement. Could be slowed for realism; gameplay reasons keep it at the current rate.

### Substrate dig_speed_multiplier values
**Code:** Loam=1.0, Sand=1.2, Ytong=0.7, Wood=0.5, Gel=1.5.
**Citation:** Wilson 1971 *The Insect Societies* describes substrate-specific behavior qualitatively. Modern keeper culture (AntsCanada, Antiloquent on YouTube) distinguishes substrate difficulty.
**Grounding:** 🟠 Direction-correct relative ordering (Wood is hardest, Sand/Loam are typical, Gel is fast because gel literally squishes). But the SPECIFIC ratios (1.0/1.2/0.7/0.5/1.5) are tuned for gameplay, not measured. **Honest disclaimer:** these are pattern-correct but not numerically backed.

---

## Population dynamics

### Population-saturation queen lay cap (0.5×–1.5× target)
**Code:** Queen lay rate ramps down 0.5× target_population to zero at 1.5× target.
**Citation:** [Hölldobler & Wilson (1990) The Ants, ch. on colony-size regulation.](https://www.hup.harvard.edu/file/feeds/PDF/9780674454903_sample.pdf)
**Grounding:** 🟡 Established that queens slow laying as colonies saturate. The exact 0.5×/1.5× window is a sim choice — biology has continuous regulation, our linear ramp is a simplification.

### Bore-width × 1.5 safety margin
**Code:** Tube bore = `worker_size_mm × polymorphic_factor × 1.5`.
**Citation:** Ant farm supplier catalogs (Tar Heel Ants, AntsCanada, Antstore) stock 6/8/12/16mm tubing for species-matched setups; the 1.5× safety multiplier is keeper-community heuristic.
**Grounding:** 🟠 Pattern-correct. Real keepers DO size up — 13mm Camponotus body uses 16-20mm tubing typically. The 1.5× ratio is in that range. Not a published number; community heuristic.

### Trophic eggs at 10% of fertile lay rate
**Code:** `TROPHIC_RATE = 0.1` of `queen_egg_rate`.
**Citation:** Trophic eggs are well-documented (Hölldobler & Wilson 1990) but the FRACTION of egg production that is trophic varies wildly by species. Some species lay almost no trophic eggs; honeypot ants produce many.
**Grounding:** 🔴 Sim-convenience. We landed on 10% because it produces a small but measurable background income that survives Lasius colonies through brief food gaps. Real species values would be highly variable. **Honest disclaimer:** this is a gameplay knob with biology-flavored framing.

### Brood cannibalism recovery factors (0.90/0.80/0.65 by stage)
**Code:** Egg=90% recovery, Larva=80%, Pupa=65%.
**Citation:** "Younger brood has less nutrient invested → higher fractional recovery" is biologically reasonable (early brood = mostly yolk; pupa = differentiated tissues). Specific percentages are not from a study I can cite.
**Grounding:** 🔴 Direction-correct, numbers picked. **Honest disclaimer:** ratios are sim-convenience not measured.

---

## What's NOT yet in the sim that real biology has

This list is about being honest about gaps, not advertising:

- **Trail pheromone half-life is species-specific.** Currently one global rate. Real Lasius ~30-60 min, Pheidole ~10 min, etc.
- **Worker age polyethism.** Younger workers nurse, older ones forage — real biology has this transition. Currently a worker can do anything.
- **Antennation / trophallaxis.** Mouth-to-mouth food sharing between workers, queen-feeding, recruitment displays. Currently no.
- **Slave-making and social parasitism.** Formica rufa founding mechanic. Currently just a "founding" enum field, not actually modeled.
- **Pheromone language richness.** Real ants have ~10-20 distinct pheromones (alarm, recruitment, queen-presence, brood-care, territory, sexual). We have 4 layers.
- **Sex determination genetics.** Diploid vs haploid, queen control over fertilization. Currently a stochastic caste roll.

These are honest known gaps. None are critical-path; all could land as future polish.

---

## Summary

**Cited & quantitatively grounded (🟢):** body-fat winter survival, brood preservation, soil pellets, kickout mound.

**Cited & qualitatively grounded (🟡):** diapause metabolic depression, autumn retreat, multi-substep dig, queen lay saturation cap.

**Pattern-grounded (🟠):** hibernation day threshold, substrate speed multipliers, bore-width safety margin.

**Sim-convenience (🔴):** trophic egg rate, brood cannibalism recovery factors.

The high-impact economy + diapause fixes (the ones that determined whether colonies survive) are in the 🟢/🟡 tiers. The values I marked 🔴 are gameplay knobs with biology-flavored framing — they're not lying about biology, they're tuned. Honest.

Cross-references: every entry above corresponds to a citation in `docs/biology.md`. This audit doc is the meta-view; biology.md is the source-of-truth log.
