"""Batch-generate the Lasius niger sprite pack with FLUX.1-schnell.

Loads FLUX once, iterates over the 8 sprite prompts, writes PNGs to
`assets/gen/lasius_niger/raw/`. A separate pass in `palette_lock.py`
quantizes them to the fixed Lasius niger palette.

Usage:
    python scripts/lasius_niger_sprites.py            # all 8 sprites
    python scripts/lasius_niger_sprites.py worker     # just one
    python scripts/lasius_niger_sprites.py --seed 42  # reproducible
"""
from __future__ import annotations

import argparse
import gc
import os
import sys
import time
from pathlib import Path

if sys.platform == "win32":
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

import torch
from diffusers import FluxPipeline
from optimum.quanto import freeze, qint8, quantize

MODEL_ID = os.environ.get("FLUX_MODEL", "black-forest-labs/FLUX.1-schnell")
DEFAULT_OUT = Path(__file__).resolve().parents[1] / "assets" / "gen" / "lasius_niger" / "raw"

# Shared style anchor: Lasius niger palette + pixel-art constraints
STYLE = (
    "pixel art sprite, limited 8-color palette, crisp sharp pixels, "
    "no anti-aliasing, no motion blur, centered on transparent black background, "
    "game asset, Lasius niger Black Garden Ant, jet-black body color #1a1a1a "
    "with slight chocolate-brown highlights, cream-tipped legs"
)

SPRITES: dict[str, dict] = {
    "worker": dict(
        size=512,
        prompt=(
            "worker ant, top-down orthogonal view from directly above, "
            "small monomorphic Lasius niger worker 4mm, six legs splayed outward, "
            "two mandibles pointing forward, two antennae, segmented body "
            "(head, thorax, petiole, gaster), moving forward pose"
        ),
    ),
    "queen_alate": dict(
        size=768,
        prompt=(
            "virgin queen ant with four translucent wings unfolded, top-down orthogonal view, "
            "Lasius niger alate 9mm, large abdomen visible between wings, "
            "pre-nuptial-flight pose, six legs splayed, antennae forward"
        ),
    ),
    "queen_dealate": dict(
        size=768,
        prompt=(
            "mated queen ant in royal chamber, side profile view, "
            "Lasius niger dealate 10mm, wings removed (only small wing-scar stubs on thorax), "
            "massively swollen egg-filled gaster abdomen, resting on soil floor, "
            "six legs visible in side view, large head with mandibles"
        ),
    ),
    "drone": dict(
        size=512,
        prompt=(
            "male drone ant flying, top-down orthogonal view, "
            "Lasius niger male 4mm, slender amber-brown body (darker than worker), "
            "four long translucent wings, small head, long antennae, "
            "narrow abdomen, in-flight pose with legs tucked"
        ),
    ),
    "egg": dict(
        size=256,
        prompt=(
            "single ant egg, close-up side view on moist dark soil chamber floor, "
            "Lasius niger egg, tiny translucent-white oval 1mm, "
            "soft glossy surface, faint shadow beneath"
        ),
    ),
    "larva": dict(
        size=384,
        prompt=(
            "ant larva resting in brood chamber, side view, "
            "Lasius niger larva, C-shaped curled grub, cream-colored segmented body 3mm, "
            "tiny dark mouthparts at head end, soft waxy skin, no legs, "
            "on dark soil"
        ),
    ),
    "pupa": dict(
        size=512,
        prompt=(
            "ant pupa in silk cocoon, side view in brood chamber, "
            "Lasius niger cocooned pupa 4mm, creamy-white oblong silk cocoon, "
            "adult body faintly visible inside, lying on soil floor, "
            "subtle silk texture"
        ),
    ),
    "corpse": dict(
        size=512,
        prompt=(
            "dead worker ant, top-down orthogonal view from above, "
            "Lasius niger worker lying on back, legs curled inward toward body, "
            "limp posture, dull flat black body color #0a0a0a (no highlights), "
            "faint dust on body, on bare ground"
        ),
    ),
}


def log(m: str) -> None:
    print(f"[sprites] {m}", flush=True)


def vram_gb() -> float:
    if not torch.cuda.is_available():
        return 0.0
    free, _ = torch.cuda.mem_get_info(0)
    return free / (1024 ** 3)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("names", nargs="*", help="Specific sprite names to generate (default: all)")
    ap.add_argument("--steps", type=int, default=4)
    ap.add_argument("--seed", type=int, default=None)
    ap.add_argument("--quant", choices=["int8", "none"], default="int8")
    ap.add_argument("--out-dir", type=str, default=None, help="Override output dir")
    args = ap.parse_args()

    targets = args.names if args.names else list(SPRITES)
    unknown = [n for n in targets if n not in SPRITES]
    if unknown:
        sys.exit(f"unknown sprite names: {unknown}. known: {list(SPRITES)}")

    OUT = Path(args.out_dir) if args.out_dir else DEFAULT_OUT
    OUT.mkdir(parents=True, exist_ok=True)

    log(f"VRAM free: {vram_gb():.2f} GB")
    log(f"loading FLUX pipeline (bf16)…")
    t0 = time.time()
    pipe = FluxPipeline.from_pretrained(MODEL_ID, torch_dtype=torch.bfloat16)

    if args.quant == "int8":
        log("quantizing transformer to int8…")
        quantize(pipe.transformer, weights=qint8); freeze(pipe.transformer)
        log("quantizing text_encoder_2 (T5) to int8…")
        quantize(pipe.text_encoder_2, weights=qint8); freeze(pipe.text_encoder_2)
        gc.collect()
        if torch.cuda.is_available():
            torch.cuda.empty_cache()

    pipe.enable_model_cpu_offload()
    log(f"loaded in {time.time()-t0:.1f}s. VRAM free: {vram_gb():.2f} GB")

    gen = None
    if args.seed is not None:
        gen = torch.Generator(device="cuda" if torch.cuda.is_available() else "cpu").manual_seed(args.seed)

    for name in targets:
        spec = SPRITES[name]
        full_prompt = f"{spec['prompt']}. {STYLE}"
        size = spec["size"]
        log(f"[{name}] {size}x{size} @ {args.steps} steps")
        t0 = time.time()
        with torch.inference_mode():
            image = pipe(
                prompt=full_prompt,
                width=size, height=size,
                num_inference_steps=args.steps,
                guidance_scale=0.0,  # schnell must use 0
                generator=gen,
            ).images[0]
        out = OUT / f"{name}.png"
        image.save(out)
        log(f"[{name}] done in {time.time()-t0:.1f}s → {out}")

    log("all done.")


if __name__ == "__main__":
    main()
