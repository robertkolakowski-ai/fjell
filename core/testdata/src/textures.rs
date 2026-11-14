//! Grensesnitt: `Family` (seks teksturfamilier) og `texture(family, seed, w, h) -> Img`.
//! Prosedyretekstur med realistisk detaljfordeling (fbm-skyer, løv, tekstside, vev, treverk med
//! sterk retning, kantfattig himmel/horisont). Brukes av `blurset`. Ren funksjon av `seed`.

use crate::ops::Img;
use crate::rng::Rng;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    Clouds,
    Foliage,
    Text,
    Weave,
    Wood,
    Sparse,
    /// Samme motivtype som golden-korpuset (`synth::scene`), men egne seeds.
    Scene,
}

pub const FAMILIES: [Family; 7] = [
    Family::Clouds,
    Family::Foliage,
    Family::Text,
    Family::Weave,
    Family::Wood,
    Family::Sparse,
    Family::Scene,
];

impl Family {
    pub fn name(self) -> &'static str {
        match self {
            Family::Clouds => "skyer",
            Family::Foliage => "lov",
            Family::Text => "tekst",
            Family::Weave => "vev",
            Family::Wood => "tre",
            Family::Sparse => "kantfattig",
            Family::Scene => "scene",
        }
    }
}

fn lattice(seed: u64, ix: i64, iy: i64) -> f32 {
    let mut z = seed
        ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (iy as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 40) as f32 / (1u64 << 24) as f32
}

fn vnoise(seed: u64, x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (ix, iy) = (x0 as i64, y0 as i64);
    let a = lattice(seed, ix, iy) * (1.0 - sx) + lattice(seed, ix + 1, iy) * sx;
    let b = lattice(seed, ix, iy + 1) * (1.0 - sx) + lattice(seed, ix + 1, iy + 1) * sx;
    a * (1.0 - sy) + b * sy
}

/// Fraktal støy i [0, 1), `base` = celleavstand i piksler for grunnoktaven.
fn fbm(seed: u64, x: f32, y: f32, base: f32, octaves: u32, gain: f32) -> f32 {
    let (mut amp, mut f, mut acc, mut norm) = (1.0f32, 1.0 / base, 0.0f32, 0.0f32);
    for o in 0..octaves {
        acc += amp * vnoise(seed.wrapping_add(o as u64 * 7919), x * f, y * f);
        norm += amp;
        amp *= gain;
        f *= 2.0;
    }
    acc / norm
}

fn rgb(r: &mut Rng, lo: u32, hi: u32) -> [f32; 3] {
    let m = |r: &mut Rng| lo as f32 + (hi - lo) as f32 * r.unit();
    [m(r), m(r), m(r)]
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn put(img: &mut Img, x: usize, y: usize, c: [f32; 3]) {
    img.set(
        x,
        y,
        [
            c[0].clamp(0.0, 255.0) as u8,
            c[1].clamp(0.0, 255.0) as u8,
            c[2].clamp(0.0, 255.0) as u8,
        ],
    );
}

fn stroke(img: &mut Img, x0: f32, y0: f32, x1: f32, y1: f32, c: [u8; 3]) {
    let n = ((x1 - x0).abs().max((y1 - y0).abs()) * 2.0) as usize + 1;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
        if x >= 0.0 && y >= 0.0 && (x as usize) < img.w && (y as usize) < img.h {
            img.set(x as usize, y as usize, c);
        }
    }
}

pub fn texture(f: Family, seed: u64, w: usize, h: usize) -> Img {
    if f == Family::Scene {
        return crate::synth::scene(seed, w, h);
    }
    let mut r = Rng::new(seed);
    let mut img = Img::new(w, h);
    match f {
        Family::Scene => unreachable!(),
        Family::Clouds => {
            let (c0, c1) = (rgb(&mut r, 20, 120), rgb(&mut r, 130, 250));
            let base = r.range(40.0, 110.0);
            let gain = r.range(0.7, 0.9);
            for y in 0..h {
                for x in 0..w {
                    let n = fbm(seed, x as f32, y as f32, base, 7, gain);
                    let t = ((n - 0.5) * 2.4 + 0.5).clamp(0.0, 1.0);
                    put(&mut img, x, y, lerp3(c0, c1, t));
                }
            }
        }
        Family::Foliage => {
            let bg = rgb(&mut r, 10, 50);
            for y in 0..h {
                for x in 0..w {
                    let n = fbm(seed ^ 5, x as f32, y as f32, 60.0, 3, 0.5);
                    put(
                        &mut img,
                        x,
                        y,
                        [bg[0] + n * 30.0, bg[1] + n * 50.0, bg[2] + n * 20.0],
                    );
                }
            }
            for _ in 0..(2200 + r.below(900)) {
                let (x, y) = (r.range(0.0, w as f32), r.range(0.0, h as f32));
                let (len, a) = (r.range(5.0, 16.0), r.range(0.0, std::f32::consts::TAU));
                let g = r.range(60.0, 230.0);
                let c = [
                    (g * r.range(0.3, 0.8)) as u8,
                    g as u8,
                    (g * r.range(0.1, 0.5)) as u8,
                ];
                stroke(&mut img, x, y, x + len * a.cos(), y + len * a.sin(), c);
            }
        }
        Family::Text => {
            let dark = r.below(5) == 0;
            let (paper, ink) = if dark {
                ([25u8, 28, 34], [220u8, 225, 230])
            } else {
                ([238, 232, 220], [28, 26, 30])
            };
            img = Img::solid(w, h, paper);
            // Svak papirstruktur.
            for y in 0..h {
                for x in 0..w {
                    let n = (fbm(seed ^ 9, x as f32, y as f32, 3.0, 2, 0.5) - 0.5) * 10.0;
                    let o = (y * w + x) * 3;
                    for c in 0..3 {
                        img.rgb[o + c] = (img.rgb[o + c] as f32 + n).clamp(0.0, 255.0) as u8;
                    }
                }
            }
            let gh = 9 + r.below(4) as usize; // glyfhøyde
            let pitch = gh + 6 + r.below(4) as usize;
            let mut y = 12usize;
            while y + gh + 4 < h {
                let mut x = 14.0f32;
                while x < w as f32 - 20.0 {
                    let wl = 2 + r.below(8);
                    for _ in 0..wl {
                        let gw = gh as f32 * r.range(0.4, 0.7);
                        for _ in 0..(2 + r.below(3)) {
                            let (xa, xb) = (x + r.range(0.0, gw), x + r.range(0.0, gw));
                            let (ya, yb) = (
                                y as f32 + r.range(0.0, gh as f32),
                                y as f32 + r.range(0.0, gh as f32),
                            );
                            if r.below(2) == 0 {
                                stroke(&mut img, xa, ya, xa, yb, ink);
                            } else {
                                stroke(&mut img, xa, ya, xb, ya, ink);
                            }
                        }
                        x += gw + 2.0;
                    }
                    x += gh as f32 * 0.6;
                }
                y += pitch;
            }
        }
        Family::Weave => {
            let (c0, c1) = (rgb(&mut r, 40, 200), rgb(&mut r, 40, 200));
            let (bw, bh) = (6 + r.below(9) as usize, 4 + r.below(6) as usize);
            for y in 0..h {
                for x in 0..w {
                    let row = y / bh;
                    let xo = x + if row % 2 == 0 { 0 } else { bw / 2 };
                    let edge = (xo % bw == 0) || (y % bh == 0);
                    let n = fbm(seed ^ 3, x as f32, y as f32, 9.0, 3, 0.6);
                    let t = lattice(seed, (xo / bw) as i64, row as i64);
                    let base = lerp3(c0, c1, t * 0.7 + n * 0.5);
                    put(
                        &mut img,
                        x,
                        y,
                        if edge {
                            [base[0] * 0.35, base[1] * 0.35, base[2] * 0.35]
                        } else {
                            base
                        },
                    );
                }
            }
        }
        Family::Wood => {
            let (c0, c1) = (rgb(&mut r, 50, 110), rgb(&mut r, 130, 220));
            let freq = r.range(0.25, 0.6);
            let vertical = r.below(2) == 0;
            let warp = r.range(8.0, 30.0);
            for y in 0..h {
                for x in 0..w {
                    let (u, v) = if vertical {
                        (y as f32, x as f32)
                    } else {
                        (x as f32, y as f32)
                    };
                    let wv = fbm(seed ^ 11, u, v, 90.0, 4, 0.5) * warp;
                    let s = ((v + wv) * freq).sin() * 0.5 + 0.5;
                    let grain = fbm(seed ^ 13, u * 0.12, v, 1.5, 3, 0.8);
                    put(
                        &mut img,
                        x,
                        y,
                        lerp3(c0, c1, (s * 0.45 + grain * 0.55).clamp(0.0, 1.0)),
                    );
                }
            }
        }
        Family::Sparse => {
            let (top, bot) = (rgb(&mut r, 90, 250), rgb(&mut r, 120, 255));
            let hor = (h as f32 * r.range(0.4, 0.7)) as usize;
            let gnd = rgb(&mut r, 30, 140);
            for y in 0..h {
                for x in 0..w {
                    let n = fbm(seed ^ 17, x as f32, y as f32, 120.0, 3, 0.5) * 0.25;
                    let c = if y < hor {
                        lerp3(top, bot, y as f32 / hor as f32 + n)
                    } else {
                        lerp3(
                            gnd,
                            [gnd[0] * 0.6, gnd[1] * 0.6, gnd[2] * 0.6],
                            (y - hor) as f32 / (h - hor) as f32 + n,
                        )
                    };
                    // Grus/gress under horisonten gir finkornet tekstur; himmelen er jevn.
                    let tex = if y < hor {
                        0.0
                    } else {
                        (fbm(seed ^ 19, x as f32, y as f32, 2.0, 3, 0.8) - 0.5) * 70.0
                    };
                    put(&mut img, x, y, [c[0] + tex, c[1] + tex, c[2] + tex]);
                }
            }
            // Noen få skarpe siluetter (bygninger/trær) langs horisonten.
            let mut x = 0usize;
            let sil = [r.below(60) as u8, r.below(60) as u8, r.below(70) as u8];
            while x < w {
                let bw = 12 + r.below(50) as usize;
                let bh = 10 + r.below((h / 3) as u64) as usize;
                for xx in x..(x + bw).min(w) {
                    for yy in hor.saturating_sub(bh)..hor {
                        img.set(xx, yy, sil);
                    }
                }
                x += bw + r.below(40) as usize;
            }
        }
    }
    img
}
