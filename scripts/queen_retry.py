"""Regenerate queen_alate with fixes for FLUX-schnell dual-gaster failure.

Fixes:
- Side profile instead of top-down (avoids top-down wing-abdomen duplication)
- 10 steps instead of 4 (resolves duplicated body parts)
- Shorter style prefix so CLIP keeps anatomy words in 77-token window
- Explicit single-gaster anatomy language
- Try 3 seeds, save all, pick best manually

Usage:
    python scripts/queen_retry.py              # default 3 seeds
    python scripts/queen_retry.py --seeds 42 7 1955 999  # custom seeds
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
OUT = Path(__file__).resolve().parents[1] / "assets" / "gen" / "lasius_niger" / "raw"

STYLE = "pixel art sprite, 8-color palette, crisp pixels, transparent black background"

QUEEN_ALATE_PROMPT = (
    f"{STYLE}, Lasius niger virgin queen ant side profile view, "
    "single large egg-swollen gaster connected to thorax by one narrow petiole waist, "
    "four translucent wings spread outward perpendicular to body, not folded back, "
    "wings extending sideways from thorax in pre-flight pose, "
    "six legs visible from side, two antennae forward, "
    "jet-black body with chocolate-brown highlights"
)


def log(m: str) -> None:
    print(f"[queen-retry] {m}", flush=True)


def vram_gb() -> float:
    if not torch.cuda.is_available():
        return 0.0
    free, _ = torch.cuda.mem_get_info(0)
    return free / (1024 ** 3)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seeds", type=int, nargs="+", default=[42, 137, 1955])
    ap.add_argument("--steps", type=int, default=10)
    ap.add_argument("--size", type=int, default=768)
    args = ap.parse_args()

    OUT.mkdir(parents=True, exist_ok=True)

    log(f"VRAM free: {vram_gb():.2f} GB")
    log(f"loading FLUX pipeline (bf16)...")
    dtype = torch.bfloat16 if torch.cuda.is_bf16_supported() else torch.float32
    pipe = FluxPipeline.from_pretrained(MODEL_ID, torch_dtype=dtype)

    log("quantizing transformer to int8...")
    quantize(pipe.transformer, weights=qint8)
    freeze(pipe.transformer)

    pipe.enable_model_cpu_offload()

    for seed in args.seeds:
        out_path = OUT / f"queen_alate_retry_s{seed}.png"
        log(f"[seed={seed}] {args.size}x{args.size} @ {args.steps} steps -> {out_path.name}")
        t0 = time.time()
        gen = torch.Generator("cpu").manual_seed(seed)
        image = pipe(
            prompt=QUEEN_ALATE_PROMPT,
            height=args.size,
            width=args.size,
            guidance_scale=0.0,
            num_inference_steps=args.steps,
            generator=gen,
        ).images[0]
        image.save(out_path)
        log(f"[seed={seed}] done in {time.time()-t0:.1f}s")
        gc.collect()
        torch.cuda.empty_cache()

    log("all seeds done. inspect outputs:")
    for seed in args.seeds:
        log(f"  {OUT / f'queen_alate_retry_s{seed}.png'}")


if __name__ == "__main__":
    main()
