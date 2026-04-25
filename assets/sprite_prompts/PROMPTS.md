# Sprite Prompt Pack — claude.ai/design One-Shots

Copy a single block into a fresh claude.ai/design chat. Each prompt is **self-contained** — style anchor + species biology + caste-specific anatomy.

## Conventions baked into every prompt

- Pixel art, limited palette, hard-edge crisp pixels, no anti-aliasing
- Centered subject on solid flat black background — no environment, no ground texture, no soil, no chamber floor (FLUX/diffusion lesson: environment language makes the model paint texture instead of isolating the subject; same applies here)
- View locked per caste:
  - **worker / corpse / drone**: top-down orthogonal (matches in-game render perspective)
  - **queen_alate / queen_dealate**: SIDE PROFILE (top-down with wings causes double-gaster artifacts in every diffusion model — burned-in lesson from FLUX queen retries)
  - **egg / larva / pupa**: side close-up, isolated
- Anatomy: head + thorax + petiole + gaster, six legs in correct insect arrangement, two antennae, two mandibles
- Save filenames match `crates/antcolony-sim/src/species.rs` caste IDs

## Output directory convention

```
assets/gen/<species_id>/design/<caste>.png
```

Where `<species_id>` matches the TOML id (`lasius_niger`, `camponotus_pennsylvanicus`, etc.).

---

# Lasius niger — Black Garden Ant

Tagline: small, jet-black, monomorphic. Default starter. 4mm worker, 9mm queen_alate, 10mm queen_dealate.

### worker → `lasius_niger/design/worker.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment, no ground, no soil. Subject: a single Lasius niger Black Garden Ant worker, 4mm, jet-black body color #1a1a1a with subtle chocolate-brown highlights on the gaster, monomorphic small ant. Anatomically accurate: distinct head with two forward-pointing mandibles and two elbowed antennae, narrow thorax, single-segment petiole node, oval gaster. Six legs splayed outward in walking pose, three on each side, each leg showing coxa-femur-tibia-tarsus joints. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing, no shading gradients. 512x512.
```

### queen_alate → `lasius_niger/design/queen_alate.png` · 768×768
```
Pixel art game sprite, side profile view, wings perpendicular to body extending upward and back, centered on solid flat black background, no environment. Subject: a single Lasius niger virgin queen alate, 9mm, jet-black body color #1a1a1a, pre-nuptial-flight pose. Anatomically accurate: large head with mandibles, robust thorax (mesosoma) bearing four translucent membranous wings (forewing larger than hindwing), single-segment petiole node, large pre-mating gaster. Six legs visible in side profile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### queen_dealate → `lasius_niger/design/queen_dealate.png` · 768×768
```
Pixel art game sprite, side profile view, centered on solid flat black background, no environment, no ground. Subject: a single mated Lasius niger dealate queen, 10mm, jet-black body color #1a1a1a. Anatomically accurate: large head with mandibles, thorax with small wing-scar stubs only (wings shed after mating), single-segment petiole, massively swollen egg-filled gaster two to three times worker proportion. Six legs visible in side profile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### drone → `lasius_niger/design/drone.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view, centered on solid flat black background, no environment. Subject: a single Lasius niger male drone, 4mm, slender amber-brown body slightly darker than worker, in-flight pose with legs tucked under body. Anatomically accurate: small head dominated by large dark compound eyes, long delicate antennae, narrow thorax bearing four long translucent wings extended outward, single-segment petiole, narrow tubular abdomen. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### egg → `lasius_niger/design/egg.png` · 256×256
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment, no soil. Subject: a single Lasius niger ant egg, 1mm, tiny translucent-white oval, glossy soft surface, faint shadow beneath. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 256x256.
```

### larva → `lasius_niger/design/larva.png` · 384×384
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Lasius niger ant larva, 3mm, C-shaped curled grub, cream-colored segmented body with tiny dark mouthparts at the head end, soft waxy skin, no legs. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 384x384.
```

### pupa → `lasius_niger/design/pupa.png` · 512×512
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Lasius niger ant pupa inside a silk cocoon, 4mm, creamy-white oblong silk cocoon with subtle silk fiber texture, faint outline of the developing adult body visible through the cocoon. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### corpse → `lasius_niger/design/corpse.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single dead Lasius niger worker lying on its back, 4mm, dull flat black body color #0a0a0a with no highlights, six legs curled inward toward the body in limp posture, antennae limp. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

---

# Camponotus pennsylvanicus — Eastern Black Carpenter Ant

Tagline: huge, polymorphic (minors 6-9mm, majors 12-14mm), matte black with golden gaster pubescence. NEEDS BOTH MINOR AND MAJOR WORKERS.

### worker_minor → `camponotus_pennsylvanicus/design/worker_minor.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single Camponotus pennsylvanicus minor worker, 7mm, matte black body color #0a0a0a, warm golden-yellow pubescent hairs visible on the gaster catching faint light. Anatomically accurate: rounded head with two forward-pointing mandibles and two elbowed antennae, robust thorax, single-segment petiole, large oval gaster with the distinctive golden pile. Six legs splayed outward in walking pose. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### worker_major → `camponotus_pennsylvanicus/design/worker_major.png` · 768×768
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single Camponotus pennsylvanicus major worker (soldier-caste), 13mm, matte black body color #0a0a0a, golden-yellow pubescent hairs on the gaster. DISTINGUISHING TRAIT: dramatically oversized squared-off head as wide as the thorax, with massive forward-pointing mandibles disproportionately larger than the minor worker. Robust thorax, single-segment petiole, large gaster with golden pile. Six legs splayed outward. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### queen_alate → `camponotus_pennsylvanicus/design/queen_alate.png` · 768×768
```
Pixel art game sprite, side profile view, wings perpendicular extending upward and back, centered on solid flat black background, no environment. Subject: a single Camponotus pennsylvanicus virgin queen alate, 18mm, matte black body color #0a0a0a, golden-yellow pubescence on gaster, pre-nuptial-flight pose. Anatomically accurate: very large head with mandibles, massive thorax bearing four translucent membranous wings, single-segment petiole, large pre-mating gaster. Six legs visible in side profile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### queen_dealate → `camponotus_pennsylvanicus/design/queen_dealate.png` · 768×768
```
Pixel art game sprite, side profile view, centered on solid flat black background, no environment. Subject: a single mated Camponotus pennsylvanicus dealate queen, 18mm, matte black body color #0a0a0a, golden gaster pubescence. Anatomically accurate: very large head with mandibles, massive thorax with small wing-scar stubs only (wings shed after mating), single-segment petiole, massively swollen egg-filled gaster. Six legs in side profile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### drone → `camponotus_pennsylvanicus/design/drone.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view, centered on solid flat black background, no environment. Subject: a single Camponotus pennsylvanicus male drone, 9mm, slender dark-brown body, in-flight pose with legs tucked. Anatomically accurate: small head dominated by huge dark compound eyes, long delicate antennae, narrow thorax bearing four long translucent wings, narrow tubular abdomen with faint golden pile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### egg → `camponotus_pennsylvanicus/design/egg.png` · 256×256
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Camponotus pennsylvanicus ant egg, 1.5mm, translucent-white oval, glossy soft surface. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 256x256.
```

### larva → `camponotus_pennsylvanicus/design/larva.png` · 384×384
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Camponotus pennsylvanicus ant larva, 6mm, large C-shaped curled grub (carpenter ants have proportionally large larvae), cream-colored segmented body with tiny dark mouthparts at head end, soft waxy skin, no legs. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 384x384.
```

### pupa → `camponotus_pennsylvanicus/design/pupa.png` · 512×512
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Camponotus pennsylvanicus ant pupa in silk cocoon, 8mm, creamy-white oblong silk cocoon, faint outline of large developing adult body inside. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### corpse → `camponotus_pennsylvanicus/design/corpse.png` · 768×768
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single dead Camponotus pennsylvanicus worker on its back, 13mm, dull flat black body, golden gaster pubescence faded to dusty grey-yellow, six legs curled inward in limp posture, mandibles slack. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

---

# Tetramorium immigrans — Pavement Ant

Tagline: tiny, dark brown, sculptured/punctate head, propodeal spines on thorax. 2.8mm.

### worker → `tetramorium_immigrans/design/worker.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single Tetramorium immigrans Pavement Ant worker, 2.8mm, dark brown body color #3a2818 with slightly lighter brown legs and antennae. DISTINGUISHING TRAITS: heavily sculptured / punctate head with parallel longitudinal striations giving an "armored" texture, two backward-pointing propodeal spines on the rear of the thorax, two-segment petiole (petiole + postpetiole nodes both visible). Six legs splayed in walking pose, two forward-pointing mandibles, two elbowed antennae. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### queen_alate → `tetramorium_immigrans/design/queen_alate.png` · 768×768
```
Pixel art game sprite, side profile view, wings perpendicular extending upward and back, centered on solid flat black background, no environment. Subject: a single Tetramorium immigrans virgin queen alate, 6mm, dark brown body color #3a2818, pre-nuptial-flight pose. Anatomically accurate: large striated head, robust thorax with two backward propodeal spines and four translucent wings, two-segment petiole (petiole + postpetiole), large pre-mating gaster. Six legs in side profile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### queen_dealate → `tetramorium_immigrans/design/queen_dealate.png` · 768×768
```
Pixel art game sprite, side profile view, centered on solid flat black background, no environment. Subject: a single mated Tetramorium immigrans dealate queen, 6mm, dark brown body color #3a2818. Anatomically accurate: large striated head with mandibles, thorax with two propodeal spines and small wing-scar stubs only, two-segment petiole, swollen egg-filled gaster. Six legs in side profile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### drone → `tetramorium_immigrans/design/drone.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view, centered on solid flat black background, no environment. Subject: a single Tetramorium immigrans male drone, 4mm, slender dark brown body, in-flight pose with legs tucked. Anatomically accurate: small head with prominent dark compound eyes, long thin antennae, narrow thorax bearing four translucent wings, two-segment petiole, narrow abdomen. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### egg → `tetramorium_immigrans/design/egg.png` · 256×256
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Tetramorium immigrans ant egg, 0.7mm, very small translucent-white oval, glossy soft surface. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 256x256.
```

### larva → `tetramorium_immigrans/design/larva.png` · 384×384
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Tetramorium immigrans ant larva, 2mm, small C-shaped curled grub, cream-colored segmented body with tiny dark mouthparts, soft waxy skin, no legs. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 384x384.
```

### pupa → `tetramorium_immigrans/design/pupa.png` · 384×384
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Tetramorium immigrans ant pupa, 3mm, NAKED PUPA (Myrmicinae do NOT spin cocoons — the pupa is exposed), cream-white body with adult ant features faintly visible (head, legs folded against body, gaster), soft pre-emergence skin. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 384x384.
```

### corpse → `tetramorium_immigrans/design/corpse.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single dead Tetramorium immigrans worker on its back, 2.8mm, dull dark brown body, six legs curled inward in limp posture, mandibles slack, propodeal spines still visible on thorax. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

---

# Formica rufa — Red Wood Ant

Tagline: bicolored (red-brown thorax, black gaster), 8mm, formic-acid sprayer, aggressive forest predator.

### worker → `formica_rufa/design/worker.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single Formica rufa Red Wood Ant worker, 8mm, BICOLORED: head and thorax bright red-brown #8b3a1a, gaster jet black #1a1a1a sharply contrasting with the red thorax. Anatomically accurate: large head with two forward-pointing mandibles and two elbowed antennae, robust red-brown thorax, single-segment petiole, oval black gaster. Long legs splayed outward in walking pose. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### worker_spray → `formica_rufa/design/worker_spray.png` · 512×512 *(bonus pose: signature formic-acid defensive)*
```
Pixel art game sprite, side profile view, centered on solid flat black background, no environment. Subject: a single Formica rufa worker in formic-acid spray defensive pose: gaster curled forward UNDERNEATH the body, abdomen tip pointed FORWARD between the front legs, mandibles open. Bicolored red-brown thorax #8b3a1a and jet-black gaster. A faint cyan-white droplet or fine mist emerging from the gaster tip indicating formic acid spray. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### queen_alate → `formica_rufa/design/queen_alate.png` · 768×768
```
Pixel art game sprite, side profile view, wings perpendicular extending upward and back, centered on solid flat black background, no environment. Subject: a single Formica rufa virgin queen alate, 11mm, bicolored: red-brown thorax #8b3a1a and dark brown to black gaster, pre-nuptial-flight pose. Anatomically accurate: large head with mandibles, massive red-brown thorax bearing four translucent wings, single-segment petiole, large dark gaster. Long legs in side profile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### queen_dealate → `formica_rufa/design/queen_dealate.png` · 768×768
```
Pixel art game sprite, side profile view, centered on solid flat black background, no environment. Subject: a single mated Formica rufa dealate queen, 11mm, bicolored red-brown thorax and dark gaster, post-nuptial-flight. Anatomically accurate: large head, red-brown thorax with small wing-scar stubs only, single-segment petiole, swollen egg-filled black gaster. Long legs in side profile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### drone → `formica_rufa/design/drone.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view, centered on solid flat black background, no environment. Subject: a single Formica rufa male drone, 9mm, slender dark-brown body almost entirely black (males are darker than workers), in-flight pose with legs tucked. Small head with huge dark compound eyes, long delicate antennae, narrow thorax with four long translucent wings, narrow tubular abdomen. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### egg → `formica_rufa/design/egg.png` · 256×256
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Formica rufa ant egg, 1mm, translucent-white oval, glossy soft surface. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 256x256.
```

### larva → `formica_rufa/design/larva.png` · 384×384
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Formica rufa ant larva, 5mm, C-shaped curled grub, cream-colored segmented body with tiny dark mouthparts, soft waxy skin, no legs. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 384x384.
```

### pupa → `formica_rufa/design/pupa.png` · 512×512
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Formica rufa ant pupa in silk cocoon, 7mm, creamy-tan oblong silk cocoon (Formicinae spin cocoons), faint outline of developing adult visible inside, subtle silk fiber texture. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### corpse → `formica_rufa/design/corpse.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single dead Formica rufa worker on its back, 8mm, faded bicolor body (dull red-brown thorax, dull black gaster), six long legs curled inward in limp posture, mandibles slack. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

---

# Pogonomyrmex occidentalis — Western Harvester Ant

Tagline: bright red-brown #a83418, 6mm, robust head, **psammophore** (beard of stiff golden hairs under chin for carrying sand and seeds), heavy stinger, two-segment petiole.

### worker → `pogonomyrmex_occidentalis/design/worker.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single Pogonomyrmex occidentalis Western Harvester Ant worker, 6mm, bright rust-red body color #a83418 throughout. Anatomically accurate: robust square head with two large forward-pointing mandibles and two elbowed antennae, DISTINGUISHING TRAIT: a fringe of stiff golden hairs (psammophore beard) underneath the head visible from above as a faint golden halo around the chin, heavy thorax, TWO-segment petiole (petiole + postpetiole nodes both visible), oval gaster with a visible stinger tip. Six legs splayed outward in walking pose. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### worker_seed → `pogonomyrmex_occidentalis/design/worker_seed.png` · 512×512 *(bonus pose: carrying a seed)*
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single Pogonomyrmex occidentalis worker carrying a tan grass seed in its mandibles, the seed roughly half the size of the ant's head. Bright rust-red body #a83418, psammophore golden chin-beard visible, two-segment petiole, six legs splayed in walking pose. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### queen_alate → `pogonomyrmex_occidentalis/design/queen_alate.png` · 768×768
```
Pixel art game sprite, side profile view, wings perpendicular extending upward and back, centered on solid flat black background, no environment. Subject: a single Pogonomyrmex occidentalis virgin queen alate, 10mm, bright rust-red body color #a83418, pre-nuptial-flight pose. Anatomically accurate: massive head with mandibles, robust thorax bearing four translucent wings, two-segment petiole (petiole + postpetiole), large pre-mating gaster with stinger. Six legs in side profile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### queen_dealate → `pogonomyrmex_occidentalis/design/queen_dealate.png` · 768×768
```
Pixel art game sprite, side profile view, centered on solid flat black background, no environment. Subject: a single mated Pogonomyrmex occidentalis dealate queen, 10mm, bright rust-red body color #a83418. Anatomically accurate: massive head with mandibles, thorax with small wing-scar stubs only, two-segment petiole, swollen egg-filled gaster with visible stinger. Six legs in side profile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### drone → `pogonomyrmex_occidentalis/design/drone.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view, centered on solid flat black background, no environment. Subject: a single Pogonomyrmex occidentalis male drone, 7mm, slender dark-red-brown body (darker than worker), in-flight pose with legs tucked. Small head with huge dark compound eyes, long delicate antennae, narrow thorax bearing four translucent wings, two-segment petiole, narrow abdomen. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### egg → `pogonomyrmex_occidentalis/design/egg.png` · 256×256
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Pogonomyrmex occidentalis ant egg, 1mm, translucent-white oval, glossy soft surface. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 256x256.
```

### larva → `pogonomyrmex_occidentalis/design/larva.png` · 384×384
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Pogonomyrmex occidentalis ant larva, 4mm, C-shaped curled grub, cream-colored segmented body with tiny dark mouthparts, soft waxy skin, no legs. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 384x384.
```

### pupa → `pogonomyrmex_occidentalis/design/pupa.png` · 384×384
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Pogonomyrmex occidentalis ant pupa, 5mm, NAKED PUPA (Myrmicinae do NOT spin cocoons — pupa exposed), cream-white body with adult ant features faintly visible (large head, legs folded against body, gaster with stinger outline). Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 384x384.
```

### corpse → `pogonomyrmex_occidentalis/design/corpse.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single dead Pogonomyrmex occidentalis worker on its back, 6mm, faded dull rust-red body, six legs curled inward in limp posture, mandibles slack, stinger visible at gaster tip. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

---

# Tapinoma sessile — Odorous House Ant

Tagline: tiny, dark brown #2a1e1a, smooth shiny cuticle, 2.7mm. Distinguishing trait: hidden petiole node (concealed under gaster) — looks "smooth" between thorax and gaster.

### worker → `tapinoma_sessile/design/worker.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single Tapinoma sessile Odorous House Ant worker, 2.7mm, very dark brown body color #2a1e1a with a slight glossy sheen. DISTINGUISHING TRAIT: petiole node is HIDDEN/CONCEALED under the front of the gaster (not visible from above) — gives the ant a smooth uninterrupted line from thorax directly to gaster, unlike most ants. Anatomically accurate: small triangular head with two forward-pointing mandibles and two elbowed antennae, narrow thorax, smooth oval gaster. Six legs splayed in walking pose. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### queen_alate → `tapinoma_sessile/design/queen_alate.png` · 768×768
```
Pixel art game sprite, side profile view, wings perpendicular extending upward and back, centered on solid flat black background, no environment. Subject: a single Tapinoma sessile virgin queen alate, 4mm, very dark brown body #2a1e1a with glossy sheen, pre-nuptial-flight pose. Anatomically accurate: small head with mandibles, narrow thorax bearing four translucent wings, hidden petiole node, large pre-mating gaster. Six legs in side profile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### queen_dealate → `tapinoma_sessile/design/queen_dealate.png` · 768×768
```
Pixel art game sprite, side profile view, centered on solid flat black background, no environment. Subject: a single mated Tapinoma sessile dealate queen, 4mm, very dark brown body #2a1e1a glossy sheen. Anatomically accurate: small head with mandibles, narrow thorax with small wing-scar stubs only, concealed petiole node, swollen egg-filled gaster. Six legs in side profile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### drone → `tapinoma_sessile/design/drone.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view, centered on solid flat black background, no environment. Subject: a single Tapinoma sessile male drone, 3mm, slender very dark brown body, in-flight pose with legs tucked. Small head with prominent dark compound eyes, long thin antennae, narrow thorax bearing four translucent wings, narrow abdomen. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### egg → `tapinoma_sessile/design/egg.png` · 256×256
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Tapinoma sessile ant egg, 0.6mm, very small translucent-white oval, glossy soft surface. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 256x256.
```

### larva → `tapinoma_sessile/design/larva.png` · 384×384
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Tapinoma sessile ant larva, 2mm, small C-shaped curled grub, cream-colored segmented body with tiny dark mouthparts, soft waxy skin, no legs. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 384x384.
```

### pupa → `tapinoma_sessile/design/pupa.png` · 384×384
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Tapinoma sessile ant pupa, 2.5mm, NAKED PUPA (Dolichoderinae do NOT spin cocoons — pupa exposed), cream-white body with adult ant features faintly visible (small head, legs folded, gaster). Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 384x384.
```

### corpse → `tapinoma_sessile/design/corpse.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single dead Tapinoma sessile worker on its back, 2.7mm, dull dark brown body with glossy sheen faded, six legs curled inward in limp posture, mandibles slack. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

---

# Aphaenogaster rudis — Winnow Ant

Tagline: red-brown #6b3a22, 4.5mm, slender / "graceful" body — long legs and antennae proportionally longer than typical worker. Two-segment petiole.

### worker → `aphaenogaster_rudis/design/worker.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single Aphaenogaster rudis Winnow Ant worker, 4.5mm, red-brown body color #6b3a22. DISTINGUISHING TRAITS: SLENDER and GRACEFUL build — proportionally longer thin legs and longer antennae than a typical worker, narrow head, slim thorax. Anatomically accurate: head with two forward-pointing mandibles, two long elbowed antennae, slim thorax, TWO-segment petiole (petiole + postpetiole), narrow oval gaster. Six long thin legs splayed in walking pose. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### worker_seed → `aphaenogaster_rudis/design/worker_seed.png` · 512×512 *(bonus: classic seed-disperser pose)*
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single Aphaenogaster rudis worker carrying a wildflower seed (with a small pale yellow elaiosome attachment) in its mandibles, the seed roughly the size of the ant's head. Slender red-brown body #6b3a22, long legs splayed in walking pose, two-segment petiole visible. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### queen_alate → `aphaenogaster_rudis/design/queen_alate.png` · 768×768
```
Pixel art game sprite, side profile view, wings perpendicular extending upward and back, centered on solid flat black background, no environment. Subject: a single Aphaenogaster rudis virgin queen alate, 7mm, red-brown body color #6b3a22, slender graceful build, pre-nuptial-flight pose. Anatomically accurate: head with mandibles, slim thorax bearing four translucent wings, two-segment petiole, large pre-mating gaster. Six long thin legs in side profile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### queen_dealate → `aphaenogaster_rudis/design/queen_dealate.png` · 768×768
```
Pixel art game sprite, side profile view, centered on solid flat black background, no environment. Subject: a single mated Aphaenogaster rudis dealate queen, 7mm, red-brown body #6b3a22, slender build. Anatomically accurate: head with mandibles, slim thorax with small wing-scar stubs only, two-segment petiole, swollen egg-filled gaster. Six long thin legs in side profile. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 768x768.
```

### drone → `aphaenogaster_rudis/design/drone.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view, centered on solid flat black background, no environment. Subject: a single Aphaenogaster rudis male drone, 5mm, very slender dark-brown body, in-flight pose with legs tucked. Small head dominated by huge dark compound eyes, long thin antennae, narrow thorax bearing four long translucent wings, two-segment petiole, narrow abdomen. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

### egg → `aphaenogaster_rudis/design/egg.png` · 256×256
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Aphaenogaster rudis ant egg, 0.9mm, translucent-white oval, glossy soft surface. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 256x256.
```

### larva → `aphaenogaster_rudis/design/larva.png` · 384×384
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Aphaenogaster rudis ant larva, 3mm, C-shaped curled grub, cream-colored segmented body with tiny dark mouthparts, soft waxy skin, no legs. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 384x384.
```

### pupa → `aphaenogaster_rudis/design/pupa.png` · 384×384
```
Pixel art game sprite, side close-up, centered on solid flat black background, no environment. Subject: a single Aphaenogaster rudis ant pupa, 4mm, NAKED PUPA (Myrmicinae do NOT spin cocoons — pupa exposed), cream-white body with adult ant features faintly visible (slender head, long legs folded against body, narrow gaster). Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 384x384.
```

### corpse → `aphaenogaster_rudis/design/corpse.png` · 512×512
```
Pixel art game sprite, top-down orthogonal view from directly above, centered on solid flat black background, no environment. Subject: a single dead Aphaenogaster rudis worker on its back, 4.5mm, faded dull red-brown body, six long thin legs curled inward in limp posture, mandibles slack. Limited 8-color palette, hard-edge crisp pixels, no anti-aliasing. 512x512.
```

---

# Cocoon-vs-naked-pupa cheat sheet

This trips up every diffusion model. The biology:

| Subfamily | Genera in this pack | Pupa form |
|-----------|---------------------|-----------|
| Formicinae | Lasius, Camponotus, Formica | **Cocooned** — silk wrapping |
| Myrmicinae | Tetramorium, Pogonomyrmex, Aphaenogaster | **Naked** — exposed pupa |
| Dolichoderinae | Tapinoma | **Naked** — exposed pupa |

Each pupa prompt above is already correct per subfamily. Don't blend them — a "cocoon" Tapinoma is a biological error and would land in `docs/biology.md` as a violation.

---

# Total sprite count

- 7 species × 8 standard castes = 56
- + Camponotus polymorphic split (worker_minor + worker_major) = +1
- + 2 bonus poses (Formica spray, Pogonomyrmex/Aphaenogaster carrying seeds) = +3 (Formica spray + 2 seed-carriers)

**~60 prompts total.** Each is one-shot copyable into claude.ai/design.

# Suggested run order

Start with these 5 to verify the pattern works across morphologies before grinding all 60:

1. Lasius niger worker (your baseline — claude.ai/design already nailed this)
2. Camponotus pennsylvanicus worker_major (validates polymorphic distinguishing features)
3. Formica rufa worker (validates bicolor handling)
4. Pogonomyrmex worker (validates psammophore + 2-node petiole)
5. Tapinoma sessile worker (validates the hidden petiole / "smooth" silhouette)

If these five look right, the queen/drone/brood prompts will follow the same pattern.

# Save → atlas pipeline (next session)

Once a species is fully generated under `assets/gen/<id>/design/`, I'll wire:
1. A `palette_lock.py` pass for the fixed Lasius palette set (already exists, just needs per-species palette files)
2. A `species_atlas.toml` per species mapping caste → sprite path
3. Render-side: load atlas at picker time, swap procedural 6-leg sprites for atlas sprites where available, fall back to procedural for missing castes
