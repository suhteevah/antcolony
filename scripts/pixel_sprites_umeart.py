"""Pixel-art smoke: Lasius niger sprites via FLUX + UmeAiRT Modern_Pixel_art LoRA.

Drop-in variant of `lasius_niger_sprites.py`. Same FluxPipeline + int8 quanto +
cpu_offload path. Adds `pipe.load_lora_weights(...)` + `fuse_lora()` BEFORE the
quantize call so the LoRA gets baked into the quantized weights.

The LoRA is trained on FLUX.1-dev. Defaults here assume FLUX.1-schnell (your
existing base) with guidance_scale=0.0 and 4 steps — this is the cheapest
possible test. If output looks washed / un-stylized, re-run with `--dev` to
switch base to FLUX.1-dev, guidance 3.5, 24 steps.

Outputs to `assets/gen/lasius_niger/raw_pixel_umeart/`. Run `palette_lock.py`
on top if you want the fixed 8-color Lasius palette enforced.

Usage:
    python scripts/pixel_sprites_umeart.py                   # all 8, schnell, seed 42
    python scripts/pixel_sprites_umeart.py worker queen_alate
    python scripts/pixel_sprites_umeart.py --seed 137
    python scripts/pixel_sprites_umeart.py --dev --steps 24  # dev mode A/B
    python scripts/pixel_sprites_umeart.py --strength 0.9    # LoRA scale
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

SCHNELL_ID = "black-forest-labs/FLUX.1-schnell"
DEV_ID = "black-forest-labs/FLUX.1-dev"
LORA_ID = "UmeAiRT/FLUX.1-dev-LoRA-Modern_Pixel_art"
LORA_TRIGGER = "umempart"

DEFAULT_OUT = (
    Path(__file__).resolve().parents[1]
    / "assets" / "gen" / "lasius_niger" / "raw_pixel_umeart"
)

# Pixel-art style anchor. Trigger first (FLUX LoRAs respect front-loading;
# CLIP 77-tok truncation eats the tail). Keep it tight — the per-sprite prompt
# carries the morphology detail.
STYLE = (
    f"{LORA_TRIGGER}, pixel art, limited palette, crisp hard-edge pixels, "
    "no anti-aliasing, centered subject, solid flat black background, "
    "game sprite, Lasius niger Black Garden Ant, jet-black body "
    "with slight chocolate-brown highlights"
)

# Same morphology prompts as lasius_niger_sprites.py — morphology is base-model
# independent, LoRA only shifts style.
SPRITES: dict[str, dict] = {
    "worker": dict(
        size=512,
        prompt=(
            "worker ant, top-down orthogonal view from directly above, "
            "small monomorphic Lasius niger worker, six legs splayed outward, "
            "two mandibles pointing forward, two antennae, segmented body "
            "(head, thorax, petiole, gaster)"
        ),
    ),
    "queen_alate": dict(
        size=768,
        prompt=(
            "virgin queen ant, side profile view, wings perpendicular to body, "
            "Lasius niger alate, four translucent wings unfolded, large abdomen, "
            "pre-nuptial-flight pose, six legs, antennae forward"
        ),
    ),
    "queen_dealate": dict(
        size=768,
        prompt=(
            "mated queen ant, side profile view, "
            "Lasius niger dealate, wings removed (small wing-scar stubs on thorax), "
            "massively swollen egg-filled gaster, six legs, large head with mandibles"
        ),
    ),
    "drone": dict(
        size=512,
        prompt=(
            "male drone ant flying, top-down orthogonal view, "
            "Lasius niger male, slender amber-brown body, "
            "four long translucent wings, small head, long antennae, "
            "narrow abdomen, in-flight pose with legs tucked"
        ),
    ),
    "egg": dict(
        size=256,
        prompt=(
            "single ant egg, close-up side view, "
            "Lasius niger egg, tiny translucent-white oval, glossy surface"
        ),
    ),
    "larva": dict(
        size=384,
        prompt=(
            "ant larva, side view, "
            "Lasius niger larva, C-shaped curled grub, cream-colored segmented body, "
            "tiny dark mouthparts at head end, soft waxy skin, no legs"
        ),
    ),
    "pupa": dict(
        size=512,
        prompt=(
            "ant pupa in silk cocoon, side view, "
            "Lasius niger cocooned pupa, creamy-white oblong silk cocoon, "
            "adult body faintly visible inside, subtle silk texture"
        ),
    ),
    "corpse": dict(
        size=512,
        prompt=(
            "dead worker ant, top-down orthogonal view from above, "
            "Lasius niger worker lying on back, legs curled inward, "
            "dull flat black body color, faint dust"
        ),
    ),
}


def log(m: str) -> None:
    print(f"[umeart] {m}", flush=True)


def vram_gb() -> float:
    if not torch.cuda.is_available():
        return 0.0
    free, _ = torch.cuda.mem_get_info(0)
    return free / (1024 ** 3)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("names", nargs="*", help="Specific sprite names (default: all)")
    ap.add_argument("--dev", action="store_true",
                    help="Use FLUX.1-dev + guidance 3.5 + 24 steps (LoRA's native base). "
                         "Default is FLUX.1-schnell + guidance 0 + 4 steps.")
    ap.add_argument("--steps", type=int, default=None,
                    help="Override step count (schnell default 4, dev default 24)")
    ap.add_argument("--guidance", type=float, default=None,
                    help="Override guidance (schnell must be 0.0, dev default 3.5)")
    ap.add_argument("--strength", type=float, default=0.9,
                    help="LoRA scale (default 0.9)")
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--quant", choices=["int8", "none"], default="int8")
    ap.add_argument("--out-dir", type=str, default=None)
    args = ap.parse_args()

    targets = args.names if args.names else list(SPRITES)
    unknown = [n for n in targets if n not in SPRITES]
    if unknown:
        sys.exit(f"unknown sprite names: {unknown}. known: {list(SPRITES)}")

    model_id = DEV_ID if args.dev else SCHNELL_ID
    steps = args.steps if args.steps is not None else (24 if args.dev else 4)
    guidance = args.guidance if args.guidance is not None else (3.5 if args.dev else 0.0)

    out_root = Path(args.out_dir) if args.out_dir else DEFAULT_OUT
    mode_tag = "dev" if args.dev else "schnell"
    OUT = out_root / f"{mode_tag}_s{args.seed}"
    OUT.mkdir(parents=True, exist_ok=True)

    log(f"mode={mode_tag} base={model_id}")
    log(f"lora={LORA_ID} trigger='{LORA_TRIGGER}' strength={args.strength}")
    log(f"steps={steps} guidance={guidance} seed={args.seed} quant={args.quant}")
    log(f"out_dir={OUT}")
    log(f"VRAM free: {vram_gb():.2f} GB")

    log("loading FLUX pipeline (bf16)...")
    t0 = time.time()
    pipe = FluxPipeline.from_pretrained(model_id, torch_dtype=torch.bfloat16)
    log(f"loaded base in {time.time()-t0:.1f}s")

    # LoRA must be fused BEFORE quantize — quanto snapshots weights at freeze time.
    log("loading LoRA weights...")
    t0 = time.time()
    pipe.load_lora_weights(LORA_ID)
    pipe.fuse_lora(lora_scale=args.strength)
    pipe.unload_lora_weights()
    log(f"fused LoRA in {time.time()-t0:.1f}s")

    if args.quant == "int8":
        log("quantizing transformer to int8...")
        quantize(pipe.transformer, weights=qint8); freeze(pipe.transformer)
        log("quantizing text_encoder_2 (T5) to int8...")
        quantize(pipe.text_encoder_2, weights=qint8); freeze(pipe.text_encoder_2)
        gc.collect()
        if torch.cuda.is_available():
            torch.cuda.empty_cache()

    pipe.enable_model_cpu_offload()
    log(f"ready. VRAM free: {vram_gb():.2f} GB")

    for name in targets:
        spec = SPRITES[name]
        full_prompt = f"{STYLE}. {spec['prompt']}"
        size = spec["size"]
        gen = torch.Generator(
            device="cuda" if torch.cuda.is_available() else "cpu"
        ).manual_seed(args.seed)

        log(f"[{name}] {size}x{size} @ {steps} steps")
        t0 = time.time()
        with torch.inference_mode():
            image = pipe(
                prompt=full_prompt,
                width=size, height=size,
                num_inference_steps=steps,
                guidance_scale=guidance,
                generator=gen,
            ).images[0]
        out = OUT / f"{name}.png"
        image.save(out)
        log(f"[{name}] done in {time.time()-t0:.1f}s -> {out}")

    log("all done.")


if __name__ == "__main__":
    main()
