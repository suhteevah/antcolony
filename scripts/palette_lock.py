"""Quantize raw FLUX sprites to the fixed Lasius niger palette and
downscale to the canonical sprite-grid size, producing true pixel art.

Pipeline per input image:
  1. Nearest-neighbor downscale to target pixel-grid size (e.g. 64 from 512)
  2. Alpha cut: everything lighter than a threshold becomes fully transparent
  3. Palette quantize against the fixed PALETTE (8 colors)
  4. Re-upscale 8× with nearest-neighbor for preview ("render-ready" PNGs)

Inputs:  assets/gen/lasius_niger/raw/<name>.png
Outputs:
  assets/gen/lasius_niger/sprites/<name>.png       (final pixel size)
  assets/gen/lasius_niger/preview/<name>_x8.png    (8x preview)
"""
from __future__ import annotations

import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1] / "assets" / "gen" / "lasius_niger"
RAW = ROOT / "raw"
OUT_SPRITES = ROOT / "sprites"
OUT_PREVIEW = ROOT / "preview"

# Canonical sprite-grid sizes (in true pixels) per sprite name.
# Must match lasius_niger_sprites.py output-intent.
GRID = {
    "worker":        64,
    "queen_alate":   96,
    "queen_dealate": 96,
    "drone":         64,
    "egg":           16,
    "larva":         32,
    "pupa":          48,
    "corpse":        64,
}

# Fixed 8-color palette (RGB) anchored on the species TOML's #1a1a1a body color.
PALETTE_RGB = [
    (0x00, 0x00, 0x00),  # outline / deep shadow
    (0x1a, 0x1a, 0x1a),  # body base
    (0x2a, 0x24, 0x22),  # body highlight
    (0x0a, 0x05, 0x06),  # leg
    (0xb8, 0xa6, 0x84),  # larva cream
    (0xe8, 0xd9, 0xb5),  # larva highlight
    (0xf4, 0xee, 0xe0),  # egg / pupa white
    (0xc2, 0xb8, 0x9f),  # egg / pupa shadow
]

# Anything brighter than this in the luminance channel is treated as background
# and cut to fully transparent alpha before palette quantization.
BG_LUMA_CUTOFF = 0.92

PREVIEW_SCALE = 8


def build_palette_image() -> Image.Image:
    """PIL palette image usable with Image.quantize(palette=...)."""
    flat: list[int] = []
    for r, g, b in PALETTE_RGB:
        flat.extend([r, g, b])
    flat.extend([0, 0, 0] * (256 - len(PALETTE_RGB)))
    pal = Image.new("P", (1, 1))
    pal.putpalette(flat)
    return pal


def cut_background(img: Image.Image) -> Image.Image:
    """Make near-white background transparent."""
    img = img.convert("RGBA")
    w, h = img.size
    px = img.load()
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            # perceptual luma ~= 0.2126 R + 0.7152 G + 0.0722 B (on 0..255 scale)
            luma = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0
            if luma >= BG_LUMA_CUTOFF:
                px[x, y] = (0, 0, 0, 0)
    return img


def palette_lock_one(src: Path, grid: int) -> tuple[Image.Image, Image.Image]:
    """Returns (final_sprite, preview_x8) for one image."""
    img = Image.open(src).convert("RGBA")
    # 1. Nearest-neighbor downscale to sprite grid
    img = img.resize((grid, grid), Image.Resampling.NEAREST)
    # 2. Alpha cut on bright background
    img = cut_background(img)
    # 3. Separate alpha so we can palette-quantize only the opaque pixels
    rgb = img.convert("RGB")
    alpha = img.split()[-1]
    pal_img = build_palette_image()
    quantized = rgb.quantize(palette=pal_img, dither=Image.Dither.NONE)
    # Re-merge alpha; use quantized palette index only where alpha > 0
    final = quantized.convert("RGBA")
    r, g, b, _ = final.split()
    final = Image.merge("RGBA", (r, g, b, alpha))
    # 4. 8x preview
    preview = final.resize((grid * PREVIEW_SCALE, grid * PREVIEW_SCALE), Image.Resampling.NEAREST)
    return final, preview


def main() -> None:
    OUT_SPRITES.mkdir(parents=True, exist_ok=True)
    OUT_PREVIEW.mkdir(parents=True, exist_ok=True)

    names = sys.argv[1:] if len(sys.argv) > 1 else list(GRID.keys())
    for name in names:
        src = RAW / f"{name}.png"
        if not src.exists():
            print(f"[skip] {name}: no raw input at {src}")
            continue
        grid = GRID[name]
        final, preview = palette_lock_one(src, grid)
        final.save(OUT_SPRITES / f"{name}.png")
        preview.save(OUT_PREVIEW / f"{name}_x8.png")
        print(f"[ok] {name}: {grid}x{grid} sprite + {PREVIEW_SCALE}x preview")


if __name__ == "__main__":
    main()
