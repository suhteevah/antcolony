# Environment & World Sprite Prompts — claude.ai/design One-Shots

Companion to `PROMPTS.md` (which covers the 7 species × castes). This file covers everything else that's sprite-shaped: substrates, module backgrounds, brood piles, predators, food, HUD icons, decorations, environmental events, particles.

Total: **~95 prompts** across 3 tiers. Each is self-contained and copy-pasteable.

## Conventions baked into every prompt

- Pixel art, limited palette, hard-edge crisp pixels, no anti-aliasing
- Where listed as "tileable": the image must seamlessly tile when placed edge-to-edge in any direction (no diagonal seams)
- Centered subject on transparent or solid black background unless specified otherwise
- Save filename matches `assets/gen/<category>/<id>.png` (mkdir each subdir as you go)
- Resolution noted per prompt; pixel-art aesthetic locked

## Run order recommendation

The ants pack proved that morphology-grounded one-shots produce excellent results. Here, the equivalent grounding is **keeper-source authenticity** (real ant farms, real substrate, real keeper components from Tar Heel Ants / AntsCanada / Antstore catalogs) and **real biology** (cited in `docs/biology.md`).

Drive in priority order:
1. Tier 1 (~40 prompts) — sets the look
2. Tier 2 (~30 prompts) — polish + UI
3. Tier 3 (~25 prompts) — ambient / atmosphere

---

# TIER 1 — High ROI

## 1.1 Substrates (5 sprites, **TILEABLE**, 256×256)

These are the underground dig-substrate base textures. Each must seamlessly tile when laid edge-to-edge in any direction. Use full markdown structure in the prompt — design handles it cleanly.

### loam → `substrate/loam.png` · 256×256 tileable
```
# Loam soil substrate — pixel art tileable texture

**Format:** 256×256 PNG, pixel art, seamlessly tileable in all four directions.

**What this is:** the default underground nesting substrate for most temperate ants — a dark, organic, moisture-holding mix of decomposed leaf litter, fine clay, and coarser mineral grains. This is what a Lasius or Aphaenogaster colony would dig their tunnels through in a forest-floor or garden environment, and what a keeper would use in a "natural setup" hybrid nest.

**Visual reference:** think of the cross-section view in a sand-between-glass formicarium where the keeper has used a 70% sand + 20% potting soil + 10% coco coir mix. The texture should read as cohesive (will hold tunnel walls) but visibly heterogeneous at the pixel level.

**Color palette (8-color hard-limit):**
- Deep substrate base: `#2a1a0d`
- Mid-brown matrix: `#3d2415`
- Warm earth: `#5a3422`
- Highlight grain (pale tan): `#8a6740`
- Dark organic specks (decomposed leaf): `#1a1108`
- Tiny pale grain (mineral): `#6b4525`
- Ochre accent (oxidized iron): `#7a4a20`
- Optional faint cool tone (mineral): `#3a2a25`

**Composition rules:**
- No focal point. The texture must read uniformly when tiled — no diagonal seams, no center cluster, no edge bias.
- Grain density: ~50–60% of pixels are mid-tone matrix, ~25% are slight highlight grains, ~15% are dark organic specks, ~10% are accent colors.
- Tiny root-fragment hints OK: 1–2 dark hairline pixel runs anywhere in the image, but no full visible root.
- No anti-aliasing, no gradients, no smooth blends — all transitions are hard pixel edges.
- The four edges of the image must align with the opposite edge so the tile loops cleanly.

**Mood:** moist, rich, fertile, alive with microorganisms. This is good ant habitat.
```

### sand → `substrate/sand.png` · 256×256 tileable
```
# Sand substrate — pixel art tileable texture

**Format:** 256×256 PNG, pixel art, seamlessly tileable in all four directions.

**What this is:** the substrate variant for arid-adapted ants like Pogonomyrmex (Western Harvester) and Tapinoma in dry urban settings. Also the classic Uncle Milton "ant farm in a frame" sand. Visually paler and more uniform than loam; tunnels in pure sand collapse easily so this implies fragile architecture.

**Visual reference:** beach sand viewed under a magnifier — distinct individual grains, mostly warm tan with subtle variation in undertone (some grains warmer-pink, some cooler-gray), no organic content.

**Color palette (8-color hard-limit):**
- Sand base mid: `#c9a87c`
- Warm tan grain: `#d4b88c`
- Cool tan grain: `#b89c70`
- Pale highlight: `#e0c898`
- Shadow between grains: `#94774a`
- Warmest pink-tan accent: `#cea582`
- Cool gray accent (quartz): `#a89878`
- Deepest shadow: `#7a5e38`

**Composition rules:**
- The texture is grain-dominated. Each "grain" is a 2-3 pixel cluster of one color surrounded by neighboring grains in different shades.
- Dense grain coverage with no smooth areas — the entire image should read as packed sand with sub-pixel-scale variation.
- No focal point, no clumps of organic matter, no twigs, no moisture.
- All transitions are hard pixel edges. No anti-aliasing.
- Edges must tile seamlessly — verify the right edge mirrors the left and the top mirrors the bottom.

**Mood:** dry, hot, abrasive. This is desert-floor or roadside-shoulder substrate.
```

### ytong → `substrate/ytong.png` · 256×256 tileable
```
# Ytong (aerated autoclaved concrete / AAC) substrate — pixel art tileable texture

**Format:** 256×256 PNG, pixel art, seamlessly tileable in all four directions.

**What this is:** the keeper-favorite "permanent" formicarium nest material. Real-world Tar Heel Ants, AntsCanada, and most premium European ant-keeping suppliers use Ytong (the brand name) or its generic equivalent (AAC, autoclaved aerated concrete) for their pre-carved hybrid nests. It's pale, porous, holds humidity through wicking from a hydration port, and chambers carved into it are structurally permanent — ants cannot expand or collapse them. Visually iconic to anyone who's looked at a serious ant-keeping forum.

**Visual reference:** a cross-section of a cinder-block-like material with a uniform dust of small irregular air bubbles. The bubbles range from ~1mm to ~3mm and are scattered fairly randomly. The base material is a pale off-white that sometimes has a very faint warm or cool cast.

**Color palette (8-color hard-limit):**
- Off-white base: `#e8e8e2`
- Pale gray mid: `#d4d4cc`
- Cool gray (deeper bubble shadow): `#b0b0a8`
- Warm beige tint accent: `#dcd8cc`
- Bubble dark interior: `#888880`
- Bubble darkest core: `#6c6c66`
- Bright highlight on bubble rim: `#f0f0e8`
- Subtle warm-shadow accent: `#a09c90`

**Composition rules:**
- Dominant texture is the uniform pale concrete base (~70% of pixels).
- Scattered air bubbles: ~25–40 small irregular bubbles per 256×256 tile, varying from 3-pixel-wide tiny pores to 10-pixel-wide larger pores. Each bubble has a darker interior with a tiny brighter rim highlight on the upper edge (suggesting depth).
- No tool marks, no chamber outlines — just raw uncarved Ytong material.
- All transitions hard-edged. No anti-aliasing.
- Tile seamlessly — the bubble distribution near the edges must continue into the opposite edge without seam artifacts.

**Mood:** clean, sterile, manufactured. This is a keeper-bought permanent nest, not a natural environment.
```

### wood → `substrate/wood.png` · 256×256 tileable
```
# Wood substrate — pixel art tileable texture

**Format:** 256×256 PNG, pixel art, seamlessly tileable in all four directions.

**What this is:** the substrate for Camponotus carpenter ants. They don't eat wood — they excavate galleries through it, especially soft moisture-damaged wood like rotting fence posts, downed logs, or in keeper setups, soft balsa or poplar nest blocks. The texture should read unmistakably as wood end-grain so the player can identify Camponotus habitat at a glance.

**Visual reference:** a cross-section through a softwood log showing concentric growth rings, longitudinal grain striations, and faint darker streaks where heartwood transitions to sapwood. Imagine looking at the cut end of a 2-inch-thick poplar board.

**Color palette (8-color hard-limit):**
- Wood mid-tone (sapwood): `#b89060`
- Honey amber: `#c19060`
- Warm pine-brown: `#8a6038`
- Dark grain accent: `#5a3818`
- Deep shadow / heartwood: `#3d2818`
- Pale highlight ring: `#d4a878`
- Subtle olive tint: `#a08850`
- Aged-wood gray accent: `#6c5a3a`

**Composition rules:**
- Grain direction: predominantly horizontal striations across the image, with subtle vertical wave from natural wood movement.
- Faint concentric arcs visible: 2-3 darker curved lines suggesting growth rings, but partial — not a full bullseye.
- Subtle longitudinal streaks: thin darker hairlines running horizontally, breaking up the uniformity.
- Random small darker knots OK: 1-2 small dark spots per tile suggesting wood imperfections.
- Tile seamlessly — the horizontal grain must align edge-to-edge.
- No anti-aliasing, all hard-edge.

**Mood:** organic, warm, dry. Natural wood with the softness that makes it diggable — not a hardwood floor texture.
```

### gel → `substrate/gel.png` · 256×256 tileable
```
# Nutritive gel substrate (NASA-style) — pixel art tileable texture

**Format:** 256×256 PNG, pixel art, seamlessly tileable in all four directions.

**What this is:** the iconic semi-translucent blue gel from the NASA Ant Farm and the modern AntWorks line. Ants tunnel through it AND eat it as they go — biologically unsound for long-term keeping (no protein, eventually the colony fails) but visually striking and a piece of pop-cultural ant-farm iconography. Rendering this substrate signals "experimental setup" to any player who's seen one.

**Visual reference:** translucent blue agar-like gel viewed in cross-section. Faint internal swirls, suspended particles, glass-like surface highlights at the top edge of the cross-section. Imagine a slab of blue Jell-O lit from one side.

**Color palette (8-color hard-limit):**
- Gel base mid-cyan: `#5a8aa8`
- Warmer aqua: `#6ea0bc`
- Cool deep blue (shadow): `#3a6082`
- Pale highlight: `#a0c4d8`
- Surface glint: `#d8e8f0`
- Shadow within gel: `#284868`
- Suspended particle (dark): `#1c3850`
- Suspended particle (pale): `#8eb4cc`

**Composition rules:**
- Base is a uniform mid-cyan dominating ~60% of pixels.
- Soft swirls visible as faint diagonal bands of slightly different cyan tones — never high-contrast, always within the cool-blue family.
- 6-10 tiny suspended particles randomly placed per tile (each particle is a 1-2 pixel cluster).
- Surface-glint pixels: 1-2 small bright cluster highlights along the top edge, suggesting the gel surface catches light.
- Tile seamlessly. Don't put strong glints near the edges where they'd create seams when tiled.
- No anti-aliasing, all hard-edge.

**Mood:** sci-fi, clinical, otherworldly. This is not a natural environment — it's a lab.
```

## 1.2 Module Backgrounds (6 sprites, 512×512)

Full-panel background art for each formicarium module type. Replaces the current flat dark panel.

### test_tube_interior → `modules/test_tube_interior.png` · 512×256
```
Pixel art game background, side-view cross-section of a glass test tube founding chamber, 512x256 horizontal layout. Left third: white cotton plug. Middle third: water reservoir behind cotton, faint cyan-blue tint visible through glass. Right two-thirds: the dry chamber where the queen lives -- pale beige floor with a tiny brood pile area. Glass walls visible as thin highlights at top and bottom. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing, no environment outside the test tube (solid black background outside the glass).
```

### outworld_substrate → `modules/outworld_substrate.png` · 512×512 tileable
```
Pixel art tileable game background, 512x512 seamless tiling pattern, an outworld arena floor mixing fine sand and small pebbles, scattered tiny rocks and a few bits of organic debris (twigs, dried leaves), uniform overall density so it reads cleanly as foraging-area floor when tiled. Palette of warm tans #b8946a, browns #6b4525, soft grays #8a8a82, occasional darker leaf fragments. Hard-edge crisp pixels, no anti-aliasing, must seamlessly repeat at all four edges.
```

### ytong_module → `modules/ytong_module.png` · 512×512
```
Pixel art game background, top-down view of a Ytong (aerated concrete) formicarium module, 512x512. Solid pale gray-white concrete surface across the entire panel with the characteristic small air-bubble pore texture, four pre-carved chambers visible as smooth-edged rounded rectangular cavities (slightly darker gray inside the cavities), thin connecting tunnels carved between them. No ants, no decorations, just the empty nest interior. Limited 8-color palette of off-whites and pale grays, hard-edge crisp pixels, no anti-aliasing.
```

### acrylic_module → `modules/acrylic_module.png` · 512×512
```
Pixel art game background, top-down view of a clear acrylic formicarium module, 512x512. Visible chamber compartments separated by thin acrylic walls (rendered as light blue-gray pixel lines), each chamber floor is a subtle darker gray substrate, the whole module has a glassy clean look with faint highlights along the wall edges. No decorations or ants. Limited 8-color palette of pale grays, glass-blue tints, hard-edge crisp pixels, no anti-aliasing.
```

### feeding_dish → `modules/feeding_dish.png` · 384×384
```
Pixel art game background, top-down view of a small feeding dish, 384x384. Round shallow ceramic-white dish in the center, scattered food items: 3-4 grass seeds, 2-3 small honey water droplets, a couple of dead insect pieces. Clean light-tan substrate around the dish. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing.
```

### hibernation_chamber → `modules/hibernation_chamber.png` · 384×384
```
Pixel art game background, top-down view of a hibernation chamber module, 384x384. Cool blue-tinted substrate suggesting low temperature, faint frost crystal pixel-clusters on the chamber walls, a few sparse hidden ants in clustered diapause poses (huddled together, motionless), simple chamber structure. Palette of cool blues #6a90b0, pale frost-whites #d8e4ec, dark substrate accents. Hard-edge crisp pixels, no anti-aliasing.
```

## 1.3 Brood Piles in Chambers (8 sprites, 256×256)

These render *inside* chamber cells as a visible pile that scales with the colony's brood count. Naked vs cocooned per subfamily — same biology rule as the per-species pupa sprites.

### egg_pile_small → `brood/egg_pile_small.png` · 256×256
```
Pixel art game asset, top-down view, 256x256 on solid flat black background. A small cluster of 5-7 ant eggs piled together in a chamber, translucent-white tiny ovals slightly overlapping, soft shadow beneath the cluster, no environment. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing.
```

### egg_pile_large → `brood/egg_pile_large.png` · 384×384
```
Pixel art game asset, top-down view, 384x384 on solid flat black background. A large cluster of 30-40 ant eggs piled together in a chamber, translucent-white ovals stacked in a rough circular mass. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing.
```

### larva_pile → `brood/larva_pile.png` · 384×384
```
Pixel art game asset, top-down view, 384x384 on solid flat black background. A cluster of 12-15 ant larvae piled together, each a C-shaped cream-colored grub with tiny dark mouthparts at one end, larvae overlapping in a workers'-arranged pile. Soft segmented body texture on each. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing.
```

### cocoon_pile → `brood/cocoon_pile.png` · 384×384  *(Formicinae: Lasius / Camponotus / Formica)*
```
Pixel art game asset, top-down view, 384x384 on solid flat black background. A pile of 10-12 silken ant cocoons (Formicinae subfamily — these subfamilies cocoon their pupae), each a creamy-white oblong cocoon, slight silk-fiber texture on each, cocoons stacked side-by-side in a chamber arrangement. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing.
```

### naked_pupa_pile → `brood/naked_pupa_pile.png` · 384×384  *(Myrmicinae: Tetramorium / Pogonomyrmex / Aphaenogaster + Dolichoderinae: Tapinoma)*
```
Pixel art game asset, top-down view, 384x384 on solid flat black background. A pile of 10-12 NAKED ant pupae (Myrmicinae and Dolichoderinae subfamilies do not cocoon — pupae are exposed), each pupa shows the developing adult body shape (head, legs folded under body, gaster) in cream-white pre-emergence color, soft pre-eclosion skin. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing.
```

### seed_cache → `brood/seed_cache.png` · 384×384  *(granivores: Pogonomyrmex / Aphaenogaster)*
```
Pixel art game asset, top-down view, 384x384 on solid flat black background. A pile of stored grass and forb seeds in a granary chamber, mixed sizes and colors -- some small dark seeds, some larger tan grass seeds with husks, some round darker forb seeds, all heaped together in a rough circular cache. Limited 8-color palette of tans, browns, ochres. Hard-edge crisp pixels, no anti-aliasing.
```

### honeydew_droplets → `brood/honeydew_droplets.png` · 256×256  *(sugar-feeders: Lasius / Camponotus / Tapinoma)*
```
Pixel art game asset, top-down view, 256x256 on solid flat black background. A small chamber pool of honeydew -- 4-5 amber-gold sugar droplets arranged in a cluster, each droplet glossy with subtle highlight, slight viscous texture. Limited 8-color palette of amber, honey-gold, faint yellow highlights. Hard-edge crisp pixels, no anti-aliasing.
```

### midden_trash → `brood/midden_trash.png` · 384×384
```
Pixel art game asset, top-down view, 384x384 on solid flat black background. The chamber waste pile (midden): several small dark dead-ant fragments (curled legs, body parts), husk shells from eaten seeds, a few discarded eggs, various dried debris all heaped together. Dull dark palette dominated by browns and grays, faint dust haze around the edges. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing.
```

## 1.4 Predators (6 sprites, 512×512)

P6 hazards. Spider has 4 state variants (Patrol / Hunt / Eat / Dead corpse); antlion is stationary so only one pose.

### spider_patrol → `predators/spider_patrol.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view, centered on solid flat black background, no environment. Subject: a single jumping spider in patrol/idle pose, body 8mm equivalent in scale (smaller than ant queen). Anatomically accurate: cephalothorax with two large central forward-facing eyes plus six smaller eyes arranged around them, robust abdomen behind, EIGHT segmented legs splayed outward in a relaxed walking pose, body color dull medium brown #7a4a2a with faint pale chevron markings on the abdomen. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### spider_hunt → `predators/spider_hunt.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view, centered on solid flat black background, no environment. Subject: a single jumping spider in HUNT pose -- body lowered, eight legs in tense crouching stance, two large central forward-facing eyes especially prominent, body color BRIGHTER red-brown #a05030 (more aggressive coloration than patrol), faint motion-tension lines around the body. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### spider_eat → `predators/spider_eat.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view, centered on solid flat black background, no environment. Subject: a single jumping spider in EAT pose -- legs curled around a captured ant prey held under its body, fangs (chelicerae) visibly extended, body color brightest red-brown #b85838, partial ant body visible underneath the spider. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### spider_corpse → `predators/spider_corpse.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view, centered on solid flat black background, no environment. Subject: a single dead jumping spider on its back, eight legs curled inward in classic dead-spider pose, body color dull faded gray-brown #5a3a28 (translucent washed out), eyes dim. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### antlion_lair → `predators/antlion_lair.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view, centered on solid flat black background, no environment. Subject: the funnel pit of an antlion -- a circular conical sand pit roughly 12mm across at the rim, the antlion larva itself is buried at the bottom showing only its menacing mandible-pincers protruding from the sand at the pit center, sand grains around the pit visible at pixel scale. Larva body color dull dark-brown #3a2818 (only the mandibles visible). Limited 8-color palette of sand tans #b8946a and dark larva browns. Hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### parasitic_phorid_fly → `predators/parasitic_phorid_fly.png` · 256×256  *(bonus: real Lasius/Solenopsis enemy)*
```
Pixel art game sprite, top-down orthogonal view, centered on solid flat black background, no environment. Subject: a single phorid fly (genus Pseudacteon) in flight, 1.5mm body, hump-backed silhouette characteristic of phorids, two transparent wings visible, six tiny legs tucked, dark gray-brown body with red eyes. Phorids are real ant-decapitating parasitoids -- the fly should look small and menacing. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 256x256.
```

## 1.5 Food Types (5 sprites, 192×192)

The current single green dot becomes a species-appropriate set.

### grass_seed → `food/grass_seed.png` · 192×192
```
Pixel art game asset, side close-up, centered on solid flat black background, no environment. Subject: a single tan grass seed (foxtail or millet style), oblong shape with a small papery husk attached, faint longitudinal grooves on the seed surface. Limited 8-color palette of tans and pale browns. Hard-edge crisp pixels, no anti-aliasing. 192x192.
```

### honeydew_droplet → `food/honeydew_droplet.png` · 192×192
```
Pixel art game asset, side close-up, centered on solid flat black background, no environment. Subject: a single amber-gold honeydew droplet, glossy spherical with a bright highlight on top, slight pool flattening at the bottom from surface tension. Limited 8-color palette of ambers and honey golds. Hard-edge crisp pixels, no anti-aliasing. 192x192.
```

### dead_insect_cricket → `food/dead_insect_cricket.png` · 256×256
```
Pixel art game asset, top-down view, centered on solid flat black background, no environment. Subject: a single dead house cricket on its side, body 12mm equivalent, six legs curled up, two long antennae limp, wings folded over abdomen, body color medium brown #6a4528. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 256x256.
```

### dead_insect_fly → `food/dead_insect_fly.png` · 192×192
```
Pixel art game asset, top-down view, centered on solid flat black background, no environment. Subject: a single dead small fly on its back, two transparent wings spread, six tiny legs curled inward, large red eyes, dark gray-blue body. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 192x192.
```

### sugar_water_bead → `food/sugar_water_bead.png` · 192×192
```
Pixel art game asset, side close-up, centered on solid flat black background, no environment. Subject: a single clear-glassy sugar water droplet on a flat surface, bright white highlight at top, faint blue-tint refraction in the body of the droplet, sharp pixel-level edge. Limited 8-color palette of whites, pale blues, soft cyans. Hard-edge crisp pixels, no anti-aliasing. 192x192.
```

### fruit_chunk → `food/fruit_chunk.png` · 192×192
```
Pixel art game asset, top-down view, centered on solid flat black background, no environment. Subject: a small chunk of bright orange fruit (mango or papaya style), irregular cube with visible flesh texture, slight juicy highlights. Limited 8-color palette of oranges, yellows, and pale highlights. Hard-edge crisp pixels, no anti-aliasing. 192x192.
```

## 1.6 HUD Icons (10 sprites, 96×96)

Small clean iconography for the stat panel. Should read clearly at half-scale (48px) too.

### icon_worker → `icons/worker.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. Stylized small ant silhouette top-down, simple six-leg pose, jet-black body, no detailed features, just an unmistakable worker-ant silhouette readable at 48px. Limited 4-color palette (black, dark gray, light gray, transparent). Hard-edge crisp pixels, no anti-aliasing.
```

### icon_soldier → `icons/soldier.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. Stylized ant silhouette top-down with clearly oversized squared head and prominent mandibles, indicating major-caste soldier, jet-black body, simple six-leg pose. Readable at 48px. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### icon_breeder → `icons/breeder.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. Stylized ant silhouette top-down with two transparent wings extended (alate breeder/virgin queen), body slightly larger than worker, simple six-leg pose. Readable at 48px. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### icon_queen → `icons/queen.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. Stylized ant silhouette top-down with massively swollen egg-filled gaster, large head, no wings (dealate post-mating queen), simple crown shape above the head as a visual cue, jet-black body with a small gold/yellow accent on the crown. Readable at 48px. Limited 5-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### icon_egg → `icons/egg.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. Single translucent-white egg shape (oval), faint gloss highlight, simple iconic form. Readable at 48px. Limited 4-color palette of whites and pale yellows. Hard-edge crisp pixels, no anti-aliasing.
```

### icon_larva → `icons/larva.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. Single C-shaped cream-colored larva grub silhouette with tiny dark mouthparts at one end, soft segmented body. Readable at 48px. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### icon_pupa → `icons/pupa.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. Single oblong creamy-white silken cocoon shape with subtle silk fiber texture, very recognizable pupa silhouette. Readable at 48px. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### icon_food → `icons/food.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. Stylized food iconography: a small heap of mixed seeds and a honeydew droplet, or just a single iconic green food pellet, readable at 48px as "food storage." Limited 4-color palette of greens and warm yellows. Hard-edge crisp pixels, no anti-aliasing.
```

### icon_kill → `icons/kill.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. Crossed-mandibles motif (two ant mandibles arranged in an X) in dark brown/black, readable at 48px as a "combat kill" stat marker. Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### icon_death → `icons/death.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. Tiny stylized dead-ant-on-its-back silhouette with curled legs, dull gray-black, readable at 48px as a "casualty" stat marker. Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

## 1.7 Soil Pellet & Kickout Mound (3 sprites)

### soil_pellet → `mound/soil_pellet.png` · 64×64
```
Pixel art game asset, side close-up, centered on solid flat black background, no environment. Subject: a single ant-carried soil pellet, irregular roughly-spherical lump of cohesive dirt about the size of an ant's head, dark brown #4a2818 with subtle texture variation showing it's compacted soil grains and saliva-bound. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing. 64x64.
```

### kickout_mound_small → `mound/kickout_mound_small.png` · 192×96
```
Pixel art game asset, side view, centered on solid flat black background, no environment. Subject: a small kickout mound of excavated soil pellets piled around a nest entrance, low circular donut shape, perhaps 8-10 pellets visible, slight irregular outline showing the pellets are individually placed. Dark brown soil palette. Limited 5-color palette. Hard-edge crisp pixels, no anti-aliasing. 192x96.
```

### kickout_mound_large → `mound/kickout_mound_large.png` · 384×192
```
Pixel art game asset, side view, centered on solid flat black background, no environment. Subject: a large mature kickout mound around a nest entrance, broader donut shape, dozens of soil pellets accumulated into a clear ring with a darker depression at the center (the entrance hole), slight texture variation across the mound from old vs fresh pellets. Limited 6-color palette of soil browns. Hard-edge crisp pixels, no anti-aliasing. 384x192.
```

## 1.8 Tubes & Ports (3 sprites)

### tube_segment → `tubes/tube_segment.png` · 128×64
```
Pixel art game asset, side view, centered on solid flat black background, no environment. Subject: a horizontal section of clear silicone connecting tubing (the kind keepers use between formicarium modules), slightly curved profile, faint highlight along the top, faint shadow along the bottom, the inside of the tube visible as a mid-gray channel. Pale blue-gray tube color #b8c8d0 with darker accents. Limited 5-color palette. Hard-edge crisp pixels, no anti-aliasing. 128x64.
```

### port_marker_open → `tubes/port_open.png` · 64×64
```
Pixel art game asset, top-down view, centered on solid flat black background. Subject: a circular port hole in a module wall, dark interior (passageway visible), bright yellow rim ring around the hole as a clear visual marker that this is a connection point. Limited 4-color palette of yellows and dark interior. Hard-edge crisp pixels, no anti-aliasing. 64x64.
```

### port_marker_blocked → `tubes/port_blocked.png` · 64×64
```
Pixel art game asset, top-down view, centered on solid flat black background. Subject: a circular port hole sealed/blocked, the rim ring is dull red-brown (vs the bright yellow of an open port) with a subtle X or diagonal slash across the opening to indicate it's closed. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing. 64x64.
```

---

# TIER 2 — Polish

## 2.1 Decorative outworld scatter (10-15 sprites)

These spawn randomly in outworld substrate to break up the empty floor.

### pebble_small → `decoration/pebble_small.png` · 64×64
```
Pixel art game asset, top-down view, centered on solid flat black background, no environment. Subject: a single small rounded pebble, mottled gray surface with subtle variations, slight darker shadow on one side. Limited 5-color palette of grays. Hard-edge crisp pixels, no anti-aliasing. 64x64.
```

### pebble_medium → `decoration/pebble_medium.png` · 96×96
```
Pixel art game asset, top-down view, centered on solid flat black background, no environment. Subject: a single medium rounded pebble, more visible surface texture, palette of warm grays with brown tints. Limited 6-color palette. Hard-edge crisp pixels, no anti-aliasing. 96x96.
```

### twig_small → `decoration/twig_small.png` · 128×64
```
Pixel art game asset, top-down view, centered on solid flat black background, no environment. Subject: a single small dry twig laid horizontally, brown bark texture with visible small knots and grain, slightly bent shape. Limited 5-color palette of dry-wood browns. Hard-edge crisp pixels, no anti-aliasing. 128x64.
```

### dried_leaf → `decoration/dried_leaf.png` · 128×128
```
Pixel art game asset, top-down view, centered on solid flat black background, no environment. Subject: a single dried oak leaf, irregular lobed shape, autumn-orange-brown color with darker veins, slight curl giving it dimension. Limited 6-color palette of fall browns and tans. Hard-edge crisp pixels, no anti-aliasing. 128x128.
```

### pine_needle_pile → `decoration/pine_needle_pile.png` · 128×128  *(Formica rufa habitat)*
```
Pixel art game asset, top-down view, centered on solid flat black background, no environment. Subject: a small pile of dry brown pine needles, several individual needles overlapping at various angles, classic Formica-rufa-mound nesting material. Limited 5-color palette of pine browns. Hard-edge crisp pixels, no anti-aliasing. 128x128.
```

### moss_clump → `decoration/moss_clump.png` · 128×128
```
Pixel art game asset, top-down view, centered on solid flat black background, no environment. Subject: a small clump of green forest moss, fine textured surface, mixed darker and lighter green pixels for natural variation, slight irregular outline. Limited 5-color palette of forest greens. Hard-edge crisp pixels, no anti-aliasing. 128x128.
```

### tree_bark_chunk → `decoration/tree_bark_chunk.png` · 128×128  *(Camponotus habitat)*
```
Pixel art game asset, top-down view, centered on solid flat black background, no environment. Subject: a small chunk of textured tree bark, deep brown ridges and pale gray crevices, clearly readable as bark for Camponotus carpenter ant scenarios. Limited 5-color palette. Hard-edge crisp pixels, no anti-aliasing. 128x128.
```

### grass_blade → `decoration/grass_blade.png` · 96×128
```
Pixel art game asset, side view, centered on solid flat black background, no environment. Subject: a single tall grass blade rising vertically, slightly curved at the tip, mid-green color with darker line down the center vein. Limited 4-color palette of greens. Hard-edge crisp pixels, no anti-aliasing. 96x128.
```

### small_rock → `decoration/small_rock.png` · 96×96
```
Pixel art game asset, top-down view, centered on solid flat black background, no environment. Subject: a small irregular rock, granitic gray with darker mineral specks, faint shadow on one side. Limited 5-color palette of cool grays. Hard-edge crisp pixels, no anti-aliasing. 96x96.
```

### dandelion_seed → `decoration/dandelion_seed.png` · 96×128
```
Pixel art game asset, side view, centered on solid flat black background, no environment. Subject: a single dandelion seed (a Pogonomyrmex-style small forb seed) with its parachute pappus of fine white hairs spreading above the dark seed body. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing. 96x128.
```

### acorn → `decoration/acorn.png` · 96×96
```
Pixel art game asset, side view, centered on solid flat black background, no environment. Subject: a single acorn with its scaly cap intact, warm brown nut body with tan textured cap, slight glossy highlight. Limited 5-color palette of acorn browns. Hard-edge crisp pixels, no anti-aliasing. 96x96.
```

### aphid_herd → `decoration/aphid_herd.png` · 192×128  *(Lasius/Camponotus tend these)*
```
Pixel art game asset, top-down view, centered on solid flat black background, no environment. Subject: a small cluster of 5-7 plump green aphids on a stylized leaf surface, each aphid is a soft pear-shaped body with tiny six legs and antennae, classic ant-mutualism aphid herd. Limited 5-color palette of soft greens and yellow accents. Hard-edge crisp pixels, no anti-aliasing. 192x128.
```

## 2.2 Beacon & Cursor Icons (8 sprites)

### beacon_gather → `beacons/gather.png` · 128×128
```
Pixel art icon, 128x128 on solid flat black background. A pulsing pheromone beacon with a central "gather" symbol -- a stylized food droplet inside a circular field, surrounded by a soft yellow-green pheromone glow ring. Limited 5-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### beacon_attack → `beacons/attack.png` · 128×128
```
Pixel art icon, 128x128 on solid flat black background. A pulsing alarm beacon with a central "attack" symbol -- crossed mandibles inside a circular field, surrounded by a sharp red alarm-pheromone glow ring with jagged edges. Limited 5-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### beacon_dig → `beacons/dig.png` · 128×128
```
Pixel art icon, 128x128 on solid flat black background. A pulsing dig beacon with a central excavation symbol -- a stylized pickaxe-mandible motif or downward-arrow into soil inside a circular field, surrounded by a brown soil-pheromone glow ring. Limited 5-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### cursor_default → `cursors/default.png` · 64×64
```
Pixel art cursor, 64x64 on solid flat black background. A simple ant-mandible-shaped pointer cursor, clean outline, readable as a click pointer. Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### cursor_pan → `cursors/pan.png` · 64×64
```
Pixel art cursor, 64x64 on solid flat black background. A grab-hand-shaped cursor for panning the camera, four-way arrows around it suggesting drag motion. Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### cursor_beacon → `cursors/beacon.png` · 64×64
```
Pixel art cursor, 64x64 on solid flat black background. A targeting-reticle cursor with a small beacon symbol below it, indicating the user is in beacon-placement mode. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### cursor_inspect → `cursors/inspect.png` · 64×64
```
Pixel art cursor, 64x64 on solid flat black background. A magnifying-glass cursor with an ant silhouette inside, indicating ant-inspection mode. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### cursor_dig → `cursors/dig.png` · 64×64
```
Pixel art cursor, 64x64 on solid flat black background. A small pickaxe-mandible cursor for placing dig commands. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

## 2.3 Environmental Event Sprites (6 sprites)

### rain_droplet → `weather/rain_droplet.png` · 64×128
```
Pixel art game asset, side view, centered on solid flat black background, no environment. Subject: a single falling rain droplet, elongated teardrop shape, pale blue-white with a brighter highlight at the leading edge. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing. 64x128.
```

### rain_overlay → `weather/rain_overlay.png` · 256×256 tileable
```
Pixel art tileable rain texture, 256x256 seamless tiling pattern, multiple rain droplets at various stages of fall (some near top, some mid-air, some near bottom), pale blue-white droplets on transparent background, sparse density so the underlying scene shows through. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing, must seamlessly repeat at all four edges.
```

### snow_overlay → `weather/snow_overlay.png` · 256×256 tileable
```
Pixel art tileable snow texture, 256x256 seamless tiling pattern, scattered white snowflakes (simple 4-pointed star shapes) at various sizes against transparent background, sparse density. Limited 3-color palette of whites and pale blues. Hard-edge crisp pixels, no anti-aliasing, must seamlessly repeat at all four edges.
```

### heat_shimmer → `weather/heat_shimmer.png` · 256×256 tileable
```
Pixel art tileable heat-shimmer texture, 256x256 seamless tiling pattern, faint warm-orange diagonal wave bands suggesting hot air refraction, very low contrast against transparent background. Limited 3-color palette of pale oranges and ambers. Hard-edge crisp pixels, no anti-aliasing, must seamlessly repeat at all four edges.
```

### lawnmower_blade → `weather/lawnmower_blade.png` · 256×64
```
Pixel art game asset, top-down view, centered on solid flat black background. Subject: a horizontal lawnmower blade indicator, bright red-orange with a sharp leading edge and motion-blur trails, faint warning hash marks, scaled to span a tile-width. Limited 4-color palette of warning reds and oranges. Hard-edge crisp pixels, no anti-aliasing. 256x64.
```

### flood_water_overlay → `weather/flood_overlay.png` · 256×256 tileable
```
Pixel art tileable flood-water texture, 256x256 seamless tiling pattern, semi-transparent water surface with subtle ripple highlights and faint reflection patterns, pale blue-cyan tones. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing, must seamlessly repeat at all four edges.
```

## 2.4 Notification Icons (5 sprites, 96×96)

### notif_milestone → `notifications/milestone.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. A bright gold star with five points and a subtle inner sparkle highlight, stylized celebration marker. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### notif_danger → `notifications/danger.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. A red triangular warning sign with an exclamation mark inside, classic hazard motif. Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### notif_queen_died → `notifications/queen_died.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. A small queen ant skull (or a queen silhouette with an X across it) -- a clear "queen died" iconography, somber dark palette. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### notif_first_egg → `notifications/first_egg.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. A single egg with a soft light-yellow halo around it, indicating a celebratory first-egg-laid milestone. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

### notif_winter → `notifications/winter.png` · 96×96
```
Pixel art icon, 96x96 on solid flat black background. A snowflake with the colony entering hibernation -- a stylized 6-pointed snowflake in pale blue-white. Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing.
```

---

# TIER 3 — Atmosphere

## 3.1 Day-of-year time-of-day overlays (4 sprites)

These tint the whole scene with a soft color gradient.

### tod_dawn → `time_of_day/dawn.png` · 1024×1024 tileable
```
Pixel art tileable atmospheric overlay, 1024x1024 seamless tiling pattern, soft warm pink-orange dawn color wash with very low alpha (most of the image is mostly transparent, only faint warm tinting), no harsh seams, gentle cell-to-cell variation. Limited 4-color palette of pale pinks and warm oranges. Hard-edge crisp pixels, no anti-aliasing, must seamlessly repeat at all four edges.
```

### tod_noon → `time_of_day/noon.png` · 1024×1024 tileable
```
Pixel art tileable atmospheric overlay, 1024x1024 seamless tiling pattern, very faint pale yellow midday color wash with extremely low alpha (almost imperceptible), suggesting bright daylight. Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing, must seamlessly repeat at all four edges.
```

### tod_dusk → `time_of_day/dusk.png` · 1024×1024 tileable
```
Pixel art tileable atmospheric overlay, 1024x1024 seamless tiling pattern, soft warm orange-purple dusk color wash with low alpha, no harsh seams, gentle pixel-cell variation. Limited 4-color palette of warm oranges and purples. Hard-edge crisp pixels, no anti-aliasing, must seamlessly repeat at all four edges.
```

### tod_night → `time_of_day/night.png` · 1024×1024 tileable
```
Pixel art tileable atmospheric overlay, 1024x1024 seamless tiling pattern, deep blue-violet night color wash with moderate alpha (more visible than dawn/dusk to give a real "it's nighttime" feel), faint scattered tiny pale stars at random positions. Limited 5-color palette of deep blues and pale star whites. Hard-edge crisp pixels, no anti-aliasing, must seamlessly repeat at all four edges.
```

## 3.2 Weather variants (6 sprites)

### storm_cloud → `weather/storm_cloud.png` · 256×128
```
Pixel art game asset, side view, centered on solid flat black background, no environment. Subject: a dark gray storm cloud, billowing irregular shape with a few faint highlights along the top edge from sun, brief lightning bolt motif visible in the lower portion. Limited 5-color palette. Hard-edge crisp pixels, no anti-aliasing. 256x128.
```

### sunray → `weather/sunray.png` · 256×256
```
Pixel art game asset, top-down view, centered on solid flat black background. Subject: a soft yellow-white sunbeam rendered as a translucent diagonal cone of pale-yellow tint, faint dust particles visible within the beam. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing. 256x256.
```

### fog_overlay → `weather/fog_overlay.png` · 256×256 tileable
```
Pixel art tileable fog overlay, 256x256 seamless tiling pattern, soft white-gray drifting mist with very low alpha and gentle horizontal striations. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing, must seamlessly repeat at all four edges.
```

### dust_particles → `weather/dust.png` · 256×256 tileable
```
Pixel art tileable airborne dust texture, 256x256 seamless tiling pattern, scattered tiny bright pixels suggesting floating dust motes, transparent background. Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing, must seamlessly repeat at all four edges.
```

### lightning_flash → `weather/lightning.png` · 256×512
```
Pixel art game asset, side view, centered on solid flat black background. Subject: a single bright zigzag lightning bolt, pure white-blue with a faint glow halo, sharp angular path. Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing. 256x512.
```

### puddle_reflection → `weather/puddle.png` · 192×96
```
Pixel art game asset, top-down view, centered on solid flat black background. Subject: a small puddle of water on the ground, irregular oval outline, pale blue-cyan glassy surface with faint reflective highlights. Limited 4-color palette. Hard-edge crisp pixels, no anti-aliasing. 192x96.
```

## 3.3 Particle textures (6 sprites)

### particle_combat_spark → `particles/combat_spark.png` · 64×64
```
Pixel art game asset, centered on solid flat black background. Subject: a single yellow-white combat impact spark, 4-pointed cross/plus shape with a bright center, fading to red-orange at the tips. Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing. 64x64.
```

### particle_dust_puff → `particles/dust_puff.png` · 96×96
```
Pixel art game asset, centered on solid flat black background. Subject: a small puff of brown dust, irregular cloud shape, semi-transparent brown-tan, suggesting motion or impact. Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing. 96x96.
```

### particle_heart → `particles/heart.png` · 64×64
```
Pixel art game asset, centered on solid flat black background. Subject: a small pink-red heart shape, suggesting a positive interaction (trophallaxis, queen-tending). Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing. 64x64.
```

### particle_alarm_chevron → `particles/alarm_chevron.png` · 64×96
```
Pixel art game asset, centered on solid flat black background. Subject: a small red exclamation chevron, sharp angular shape, indicating an alarm signal emanating from an ant. Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing. 64x96.
```

### particle_zzz → `particles/zzz.png` · 64×64
```
Pixel art game asset, centered on solid flat black background. Subject: three small "Z" letters arranged in a stack, pale blue-white, indicating an ant in diapause/sleep. Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing. 64x64.
```

### particle_question → `particles/question.png` · 64×64
```
Pixel art game asset, centered on solid flat black background. Subject: a small white question mark, suggesting an idle/searching ant, faint outline glow. Limited 3-color palette. Hard-edge crisp pixels, no anti-aliasing. 64x64.
```

---

# Total counts and integration estimates

| Tier | Sprites | Driver-time (claude.ai) | Renderer integration |
|---|---|---|---|
| 1 | ~40 | 1 sitting | 4-6 commits, atlas categories: substrate, modules, brood, predators, food, icons, mound, tubes |
| 2 | ~30 | 1 sitting | 3-4 commits: decoration, beacons, weather, notifications |
| 3 | ~25 | half sitting | 2-3 commits: time_of_day, weather variants, particles |
| **Total** | **~95** | **~2.5 sittings** | **~10-13 commits** |

# Output directory layout (post-completion)

```
assets/gen/
├── lasius_niger/design/         # ants (existing)
├── camponotus_pennsylvanicus/design/
├── ...
├── substrate/
├── modules/
├── brood/
├── predators/
├── food/
├── icons/
├── mound/
├── tubes/
├── decoration/
├── beacons/
├── cursors/
├── weather/
├── notifications/
├── time_of_day/
└── particles/
```

# Cocoon-vs-naked pupa rule (reminder from `PROMPTS.md`)

| Subfamily | Genera in our pack | Pupa pile sprite |
|---|---|---|
| Formicinae | Lasius, Camponotus, Formica | `cocoon_pile.png` |
| Myrmicinae | Tetramorium, Pogonomyrmex, Aphaenogaster | `naked_pupa_pile.png` |
| Dolichoderinae | Tapinoma | `naked_pupa_pile.png` |

When wiring the brood-pile rendering into the renderer, branch on subfamily and pick the right sprite.
