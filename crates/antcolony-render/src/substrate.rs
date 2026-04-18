//! Procedural module-substrate textures.
//!
//! Each `ModuleKind` gets a colored fBM noise texture with an edge
//! vignette so the formicarium stops looking like flat grey panels.
//! Cheap, deterministic, and 1-time work at formicarium build.

use antcolony_sim::ModuleKind;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

/// Texels per tile. Four gives a soft grain without blowing up memory.
const TEXELS_PER_TILE: u32 = 4;

/// Pack sRGB color into the byte layout Bevy expects for
/// `Rgba8UnormSrgb`.
#[inline]
fn encode_srgb(r: f32, g: f32, b: f32, a: f32) -> [u8; 4] {
    let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
    [c(r), c(g), c(b), c(a)]
}

#[inline]
fn hash2(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = seed.wrapping_mul(0x9E3779B1);
    h = h.wrapping_add((x as u32).wrapping_mul(0x85EBCA77));
    h ^= (y as u32).wrapping_mul(0xC2B2AE3D);
    h ^= h >> 16;
    h = h.wrapping_mul(0x27D4EB2F);
    h ^ (h >> 15)
}

#[inline]
fn rand01(x: i32, y: i32, seed: u32) -> f32 {
    hash2(x, y, seed) as f32 / u32::MAX as f32
}

fn value_noise(px: f32, py: f32, seed: u32) -> f32 {
    let x0 = px.floor() as i32;
    let y0 = py.floor() as i32;
    let fx = px - x0 as f32;
    let fy = py - y0 as f32;
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let v00 = rand01(x0, y0, seed);
    let v10 = rand01(x0 + 1, y0, seed);
    let v01 = rand01(x0, y0 + 1, seed);
    let v11 = rand01(x0 + 1, y0 + 1, seed);
    let a = v00 + sx * (v10 - v00);
    let b = v01 + sx * (v11 - v01);
    a + sy * (b - a)
}

fn fbm(px: f32, py: f32, seed: u32, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for i in 0..octaves {
        sum += amp * value_noise(px * freq, py * freq, seed.wrapping_add(i * 13));
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm.max(0.001)
}

/// Per-kind palette: (base, accent, grain_frequency, accent_weight).
/// `accent_weight` is how strongly the noise pushes toward the accent color.
fn palette(kind: ModuleKind) -> ([f32; 3], [f32; 3], f32, f32) {
    match kind {
        ModuleKind::TestTubeNest => ([0.88, 0.84, 0.74], [0.62, 0.60, 0.52], 0.12, 0.55),
        ModuleKind::Outworld => ([0.46, 0.36, 0.22], [0.28, 0.20, 0.10], 0.09, 0.65),
        ModuleKind::YTongNest => ([0.70, 0.70, 0.66], [0.46, 0.46, 0.44], 0.15, 0.70),
        ModuleKind::AcrylicNest => ([0.30, 0.22, 0.16], [0.14, 0.10, 0.08], 0.11, 0.60),
        ModuleKind::Hydration => ([0.22, 0.40, 0.58], [0.10, 0.22, 0.40], 0.07, 0.80),
        ModuleKind::HeatChamber => ([0.55, 0.28, 0.18], [0.82, 0.42, 0.18], 0.10, 0.60),
        ModuleKind::HibernationChamber => ([0.42, 0.52, 0.62], [0.22, 0.30, 0.42], 0.08, 0.65),
        ModuleKind::FeedingDish => ([0.82, 0.80, 0.76], [0.64, 0.62, 0.56], 0.14, 0.40),
        ModuleKind::Graveyard => ([0.24, 0.18, 0.14], [0.10, 0.08, 0.06], 0.10, 0.70),
    }
}

/// Second pass: kind-specific texture features on top of the base noise.
/// Mutates `(r, g, b)` in place for the current pixel.
fn accent_pass(
    kind: ModuleKind,
    x: i32,
    y: i32,
    fx: f32,
    fy: f32,
    seed: u32,
    r: &mut f32,
    g: &mut f32,
    b: &mut f32,
) {
    match kind {
        ModuleKind::Outworld => {
            // Pebbles: occasional dark spots scattered through the sand.
            let spot = value_noise(x as f32 * 0.35, y as f32 * 0.35, seed ^ 0x1111);
            if spot > 0.82 {
                let t = ((spot - 0.82) / 0.18).clamp(0.0, 1.0) * 0.55;
                *r *= 1.0 - t * 0.6;
                *g *= 1.0 - t * 0.6;
                *b *= 1.0 - t * 0.6;
            }
            // Occasional lighter sand grains.
            let grain = value_noise(x as f32 * 0.55, y as f32 * 0.55, seed ^ 0x1717);
            if grain > 0.88 {
                *r = (*r + 0.08).min(1.0);
                *g = (*g + 0.07).min(1.0);
                *b = (*b + 0.04).min(1.0);
            }
        }
        ModuleKind::TestTubeNest => {
            // Cotton-fibre streaks: stretched-y noise, lightens pixels.
            let streak = value_noise(x as f32 * 0.20, y as f32 * 0.05, seed ^ 0x2222);
            if streak > 0.72 {
                let t = ((streak - 0.72) / 0.28).clamp(0.0, 1.0) * 0.30;
                *r = (*r + t * 0.35).min(1.0);
                *g = (*g + t * 0.32).min(1.0);
                *b = (*b + t * 0.25).min(1.0);
            }
            // Cool water hint along the left third (the reservoir side).
            if fx < -0.18 {
                let w = ((-0.18 - fx) / 0.25).clamp(0.0, 1.0) * 0.15;
                *b += w * 0.25;
                *g += w * 0.08;
            }
        }
        ModuleKind::YTongNest => {
            // Thin crack-like ridges where two noise fields cross threshold.
            let n1 = value_noise(x as f32 * 0.08, y as f32 * 0.08, seed ^ 0x3333);
            if (n1 - 0.5).abs() < 0.02 {
                *r *= 0.55;
                *g *= 0.55;
                *b *= 0.55;
            }
        }
        ModuleKind::AcrylicNest => {
            // Wood-grain bands along the long axis with slight phase jitter.
            let phase = value_noise(x as f32 * 0.04, y as f32 * 0.18, seed ^ 0x4444) * 5.0;
            let band = (y as f32 * 0.28 + phase).sin();
            let t = band * 0.12;
            *r = (*r + t).clamp(0.0, 1.0);
            *g = (*g + t * 0.85).clamp(0.0, 1.0);
            *b = (*b + t * 0.7).clamp(0.0, 1.0);
        }
        ModuleKind::Hydration => {
            // Water beads: small bright dots.
            let spot = value_noise(x as f32 * 0.32, y as f32 * 0.32, seed ^ 0x5555);
            if spot > 0.84 {
                let t = ((spot - 0.84) / 0.16).clamp(0.0, 1.0);
                *r = (*r + 0.25 * t).min(1.0);
                *g = (*g + 0.35 * t).min(1.0);
                *b = (*b + 0.40 * t).min(1.0);
            }
            // Subtle caustic streaks.
            let c = (x as f32 * 0.12 + y as f32 * 0.08).sin() * 0.04;
            *g = (*g + c).clamp(0.0, 1.0);
            *b = (*b + c).clamp(0.0, 1.0);
        }
        ModuleKind::HeatChamber => {
            // Inverted vignette — warm glow at centre.
            let d2 = fx * fx + fy * fy;
            let glow = (1.0 - d2 * 5.5).clamp(0.0, 1.0);
            *r = (*r + glow * 0.28).min(1.0);
            *g = (*g + glow * 0.12).min(1.0);
        }
        ModuleKind::HibernationChamber => {
            // Frost specks: rare near-white pixels.
            let spot = rand01(x, y, seed ^ 0x6666);
            if spot > 0.985 {
                *r = 0.90;
                *g = 0.93;
                *b = 0.98;
            }
        }
        ModuleKind::FeedingDish => {
            // Subtle concentric rings so it reads as a plate.
            let d = (fx * fx + fy * fy).sqrt();
            let ring = (d * 24.0).sin() * 0.04;
            *r = (*r + ring).clamp(0.0, 1.0);
            *g = (*g + ring).clamp(0.0, 1.0);
            *b = (*b + ring * 0.8).clamp(0.0, 1.0);
        }
        ModuleKind::Graveyard => {
            // Darker debris specks.
            let spot = value_noise(x as f32 * 0.45, y as f32 * 0.45, seed ^ 0x7777);
            if spot > 0.80 {
                *r *= 0.55;
                *g *= 0.55;
                *b *= 0.55;
            }
        }
    }
}

/// Build a substrate texture sized to the module. Deterministic from
/// `(kind, seed)`.
pub fn make_substrate(kind: ModuleKind, grid_w: u32, grid_h: u32, seed: u32) -> Image {
    let w = (grid_w * TEXELS_PER_TILE).max(8);
    let h = (grid_h * TEXELS_PER_TILE).max(8);
    let (base, accent, freq, weight) = palette(kind);

    let mut data: Vec<u8> = Vec::with_capacity((w * h * 4) as usize);
    let inv_w = 1.0 / w as f32;
    let inv_h = 1.0 / h as f32;

    for y in 0..h {
        for x in 0..w {
            // fBM in tile-space so texture scale reads the same across modules.
            let nx = x as f32 * freq;
            let ny = y as f32 * freq;
            let n = fbm(nx, ny, seed, 4); // 0..1
            // Blend base ↔ accent by noise (centered on 0.5).
            let t = ((n - 0.5) * weight + 0.5).clamp(0.0, 1.0);
            let mut r = base[0] + (accent[0] - base[0]) * t;
            let mut g = base[1] + (accent[1] - base[1]) * t;
            let mut b = base[2] + (accent[2] - base[2]) * t;

            // High-frequency speckle: tiny per-pixel jitter.
            let speckle = (rand01(x as i32, y as i32, seed ^ 0xA5A5) - 0.5) * 0.06;
            r = (r + speckle).clamp(0.0, 1.0);
            g = (g + speckle).clamp(0.0, 1.0);
            b = (b + speckle).clamp(0.0, 1.0);

            // Radial vignette: center bright, edges darker (~0.55×).
            let fx = x as f32 * inv_w - 0.5;
            let fy = y as f32 * inv_h - 0.5;
            let d = (fx * fx + fy * fy).sqrt(); // 0 (center) .. 0.707 (corner)
            let vig = (1.0 - (d / 0.65).clamp(0.0, 1.0) * 0.45).clamp(0.55, 1.0);
            r *= vig;
            g *= vig;
            b *= vig;

            // Kind-specific decorative pass stacked on top.
            accent_pass(kind, x as i32, y as i32, fx, fy, seed, &mut r, &mut g, &mut b);

            let px = encode_srgb(r, g, b, 1.0);
            data.extend_from_slice(&px);
        }
    }

    let mut img = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD
            | bevy::render::render_asset::RenderAssetUsages::MAIN_WORLD,
    );
    img.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    img
}
