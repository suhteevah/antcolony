"""Regenerate egg/larva/pupa/corpse with environment-stripped prompts.

Problem: v1+v2 batches baked soil/ground textures across the entire frame
because per-sprite prompts said "on soil" / "chamber floor" / "bare ground".
Combined with CLIP 77-token truncation eating the "transparent background"
part of the STYLE prefix, the game-asset-clean isolation was lost.

Fix:
- Drop ALL environment grounding from per-sprite prompts
- Put the transparent-background directive FIRST so it survives truncation
- Explicit "solid flat black background, no environment, no texture" negatives
- 2 seeds per sprite for A/B
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
OUT = Path(__file__).resolve().parents[1] / "assets" / "gen" / "lasius_niger" / "raw_clean"

# Background-first style prefix to survive CLIP 77-token truncation.
STYLE = (
    "solid flat black background, no environment, no ground texture, "
    "pixel art sprite, 8-color palette, crisp pixels"
)

SPRITES = {
    "egg": dict(
        size=256,
        prompt=(
            f"{STYLE}, single isolated Lasius niger ant egg on empty black background, "
            "tiny translucent-white oval 1mm, soft glossy surface, "
            "no soil, no environment, centered subject"
        ),
    ),
    "larva": dict(
        size=384,
        prompt=(
            f"{STYLE}, single isolated Lasius niger ant larva on empty black background, "
            "C-shaped curled grub, cream-colored segmented body 3mm, "
            "tiny dark mouthparts at head end, soft waxy skin, no legs, "
            "no soil, no environment, centered subject"
        ),
    ),
    "pupa": dict(
        size=512,
        prompt=(
            f"{STYLE}, single isolated Lasius niger ant pupa cocoon on empty black background, "
            "creamy-white oblong silk cocoon 4mm, "
            "adult body faintly visible inside, subtle silk texture, "
            "no soil, no environment, centered subject"
        ),
    ),
    "corpse": dict(
        size=512,
        prompt=(
            f"{STYLE}, single isolated dead Lasius niger worker ant on empty black background, "
            "top-down orthogonal view from above, lying on back, "
            "legs curled inward toward body, limp posture, "
            "dull flat black body color, faint dust on body, "
            "no ground, no environment, centered subject"
        ),
    ),
}


def log(m: str) -> None:
    print(f"[brood-retry] {m}", flush=True)


def vram_gb() -> float:
    if not torch.cuda.is_available():
        return 0.0
    free, _ = torch.cuda.mem_get_info(0)
    return free / (1024 ** 3)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("names", nargs="*", help="Specific sprite names (default: all brood)")
    ap.add_argument("--seeds", type=int, nargs="+", default=[42, 137])
    ap.add_argument("--steps", type=int, default=6)
    args = ap.parse_args()

    targets = args.names if args.names else list(SPRITES)
    unknown = [n for n in targets if n not in SPRITES]
    if unknown:
        sys.exit(f"unknown: {unknown}. known: {list(SPRITES)}")

    OUT.mkdir(parents=True, exist_ok=True)

    log(f"VRAM free: {vram_gb():.2f} GB")
    log("loading FLUX pipeline (bf16)...")
    pipe = FluxPipeline.from_pretrained(MODEL_ID, torch_dtype=torch.bfloat16)

    log("quantizing transformer + T5 to int8...")
    quantize(pipe.transformer, weights=qint8); freeze(pipe.transformer)
    quantize(pipe.text_encoder_2, weights=qint8); freeze(pipe.text_encoder_2)
    gc.collect()
    if torch.cuda.is_available():
        torch.cuda.empty_cache()
    pipe.enable_model_cpu_offload()

    for name in targets:
        spec = SPRITES[name]
        for seed in args.seeds:
            out = OUT / f"{name}_s{seed}.png"
            log(f"[{name}/s{seed}] {spec['size']}x{spec['size']} @ {args.steps} steps")
            t0 = time.time()
            gen = torch.Generator("cpu").manual_seed(seed)
            with torch.inference_mode():
                image = pipe(
                    prompt=spec["prompt"],
                    width=spec["size"], height=spec["size"],
                    num_inference_steps=args.steps,
                    guidance_scale=0.0,
                    generator=gen,
                ).images[0]
            image.save(out)
            log(f"[{name}/s{seed}] done in {time.time()-t0:.1f}s -> {out.name}")

    log("all done.")


if __name__ == "__main__":
    main()
