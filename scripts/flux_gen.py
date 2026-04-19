"""FLUX.1-schnell local generation for antcolony pixel art.

Strategy on an 8GB GPU:
  1. Load pipeline in bfloat16
  2. Quantize transformer + T5 text encoder to int8 via optimum-quanto
     (no bitsandbytes — we learned bnb TDRs on this box)
  3. enable_model_cpu_offload() — keeps modules on CPU until needed

Usage:
    python scripts/flux_gen.py "a pixel art ant worker top-down view 32x32 sprite transparent"
    python scripts/flux_gen.py --out assets/out/ant.png --steps 4 --size 512 <prompt>
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
from optimum.quanto import freeze, qfloat8, qint8, quantize

MODEL_ID = os.environ.get("FLUX_MODEL", "black-forest-labs/FLUX.1-schnell")
OUT_DIR_DEFAULT = Path(__file__).resolve().parents[1] / "assets" / "gen"


def log(msg: str) -> None:
    print(f"[flux] {msg}", flush=True)


def vram_free_gb() -> float:
    if not torch.cuda.is_available():
        return 0.0
    free, _ = torch.cuda.mem_get_info(0)
    return free / (1024 ** 3)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("prompt", nargs="+", help="Text prompt")
    ap.add_argument("--out", type=Path, default=None, help="Output PNG path")
    ap.add_argument("--steps", type=int, default=4, help="Inference steps (schnell=4)")
    ap.add_argument("--size", type=int, default=512, help="Image WxH (square)")
    ap.add_argument("--seed", type=int, default=None)
    ap.add_argument("--quant", choices=["int8", "fp8", "none"], default="int8")
    args = ap.parse_args()

    prompt = " ".join(args.prompt)

    if args.out is None:
        OUT_DIR_DEFAULT.mkdir(parents=True, exist_ok=True)
        slug = "".join(c if c.isalnum() else "_" for c in prompt[:40]).strip("_")
        args.out = OUT_DIR_DEFAULT / f"{int(time.time())}_{slug}.png"
    args.out.parent.mkdir(parents=True, exist_ok=True)

    log(f"VRAM free: {vram_free_gb():.2f} GB")
    log(f"Model:  {MODEL_ID}")
    log(f"Prompt: {prompt}")
    log(f"Out:    {args.out}")

    log("loading pipeline (bf16)…")
    t0 = time.time()
    pipe = FluxPipeline.from_pretrained(MODEL_ID, torch_dtype=torch.bfloat16)

    if args.quant != "none":
        qtype = qfloat8 if args.quant == "fp8" else qint8
        log(f"quantizing transformer to {args.quant}…")
        quantize(pipe.transformer, weights=qtype)
        freeze(pipe.transformer)
        log(f"quantizing text_encoder_2 (T5) to {args.quant}…")
        quantize(pipe.text_encoder_2, weights=qtype)
        freeze(pipe.text_encoder_2)
        gc.collect()
        if torch.cuda.is_available():
            torch.cuda.empty_cache()

    log("enabling model CPU offload…")
    pipe.enable_model_cpu_offload()

    log(f"loaded in {time.time()-t0:.1f}s. VRAM free now: {vram_free_gb():.2f} GB")

    gen = None
    if args.seed is not None:
        gen = torch.Generator(device="cuda" if torch.cuda.is_available() else "cpu").manual_seed(args.seed)

    log(f"generating {args.size}x{args.size} @ {args.steps} steps…")
    t0 = time.time()
    with torch.inference_mode():
        image = pipe(
            prompt=prompt,
            width=args.size,
            height=args.size,
            num_inference_steps=args.steps,
            guidance_scale=0.0,  # schnell is distilled — guidance MUST be 0
            generator=gen,
        ).images[0]
    dt = time.time() - t0

    image.save(args.out)
    log(f"done in {dt:.1f}s — {args.out}")


if __name__ == "__main__":
    main()
