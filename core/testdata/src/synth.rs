//! Grensesnitt: `scene(seed, w, h)`, `lowcontrast(img, f)`, `ui_screen(seed, w, h)`,
//! `noise_texture(seed, w, h)`. Prosedyrebilder bare fra seed; ingen ekstern kilde.

use crate::ops::Img;
use crate::rng::Rng;

fn color(r: &mut Rng) -> [u8; 3] {
    [r.below(256) as u8, r.below(256) as u8, r.below(256) as u8]
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let m = |i: usize| (a[i] as f32 * (1.0 - t) + b[i] as f32 * t) as u8;
    [m(0), m(1), m(2)]
}

/// Gradient + former + støytekstur. Nok høyfrekvent innhold til at uskarphet er målbar.
pub fn scene(seed: u64, w: usize, h: usize) -> Img {
    let mut r = Rng::new(seed);
    let (c0, c1) = (color(&mut r), color(&mut r));
    let (gx, gy) = (r.range(-1.0, 1.0), r.range(-1.0, 1.0));
    let mut img = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let t =
                ((x as f32 / w as f32 * gx + y as f32 / h as f32 * gy) * 0.5 + 0.5).clamp(0.0, 1.0);
            img.set(x, y, mix(c0, c1, t));
        }
    }
    for _ in 0..(10 + r.below(8)) {
        let col = color(&mut r);
        let (cx, cy) = (r.range(0.0, w as f32), r.range(0.0, h as f32));
        let (rx, ry) = (
            r.range(0.04, 0.25) * w as f32,
            r.range(0.04, 0.25) * h as f32,
        );
        let round = r.below(2) == 0;
        for y in (cy - ry).max(0.0) as usize..((cy + ry) as usize).min(h) {
            for x in (cx - rx).max(0.0) as usize..((cx + rx) as usize).min(w) {
                let (dx, dy) = ((x as f32 - cx) / rx, (y as f32 - cy) / ry);
                if !round || dx * dx + dy * dy <= 1.0 {
                    img.set(x, y, col);
                }
            }
        }
    }
    // Kantskarp rutenettstripe og lett støy gir detaljer på flere skalaer.
    let step = 6 + r.below(10) as usize;
    for y in 0..h {
        for x in 0..w {
            let n = r.below(25) as i32 - 12;
            let stripe = if (x / step + y / step) % 2 == 0 {
                0
            } else {
                14
            };
            let o = (y * w + x) * 3;
            for c in 0..3 {
                img.rgb[o + c] = (img.rgb[o + c] as i32 + n + stripe - 7).clamp(0, 255) as u8;
            }
        }
    }
    img
}

/// Skalerer kontrasten rundt midtgrått (f < 1 gir lav kontrast).
pub fn lowcontrast(img: &Img, f: f32) -> Img {
    let mut o = img.clone();
    for v in o.rgb.iter_mut() {
        *v = (128.0 + (*v as f32 - 128.0) * f).clamp(0.0, 255.0) as u8;
    }
    o
}

/// Flate felt og tekstlignende blokker, som et skjermbilde av en app.
pub fn ui_screen(seed: u64, w: usize, h: usize) -> Img {
    let mut r = Rng::new(seed);
    let dark = r.below(2) == 0;
    let (bg, fg) = if dark {
        ([18, 18, 22], [235, 235, 240])
    } else {
        ([245, 245, 248], [30, 30, 35])
    };
    let accent = color(&mut r);
    let mut img = Img::solid(w, h, bg);
    let rect = |img: &mut Img, x0: usize, y0: usize, rw: usize, rh: usize, c: [u8; 3]| {
        for y in y0..(y0 + rh).min(h) {
            for x in x0..(x0 + rw).min(w) {
                img.set(x, y, c);
            }
        }
    };
    rect(&mut img, 0, 0, w, h / 14, accent);
    let mut y = h / 10;
    while y + h / 12 < h {
        let line_h = (h / 120).max(2);
        for l in 0..(2 + r.below(3) as usize) {
            let lw = (w as f32 * r.range(0.3, 0.85)) as usize;
            rect(&mut img, w / 16, y + l * line_h * 2, lw, line_h, fg);
        }
        rect(
            &mut img,
            w / 16,
            y + line_h * 8,
            w * 7 / 8,
            1.max(h / 600),
            mix(bg, fg, 0.3),
        );
        y += h / 12;
    }
    rect(&mut img, w / 4, h * 15 / 16, w / 2, h / 40, accent);
    img
}

/// Ren støy med litt struktur (typisk «nedlastet» bilde i vilkårlig størrelse).
pub fn noise_texture(seed: u64, w: usize, h: usize) -> Img {
    let mut r = Rng::new(seed);
    let base = color(&mut r);
    let mut img = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let n = r.below(120) as i32 - 60;
            let o = (y * w + x) * 3;
            for c in 0..3 {
                img.rgb[o + c] = (base[c] as i32 + n).clamp(0, 255) as u8;
            }
        }
    }
    img
}
