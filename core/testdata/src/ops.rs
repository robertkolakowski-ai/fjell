//! Grensesnitt: `Img` (RGB8), `resize_area`, `gaussian`, `motion_h`, `crop_margin`,
//! `luma`, `encode_png`, `encode_jpeg`, `decode`. Ren heltalls-/f32-kode, ingen
//! tråder, slik at resultatet er bitvis likt mellom kjøringer.

use image::{codecs::jpeg::JpegEncoder, ExtendedColorType, ImageEncoder};

#[derive(Clone)]
pub struct Img {
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<u8>,
}

impl Img {
    pub fn new(w: usize, h: usize) -> Self {
        Img {
            w,
            h,
            rgb: vec![0; w * h * 3],
        }
    }
    pub fn solid(w: usize, h: usize, c: [u8; 3]) -> Self {
        let mut i = Img::new(w, h);
        for p in i.rgb.chunks_mut(3) {
            p.copy_from_slice(&c);
        }
        i
    }
    pub fn set(&mut self, x: usize, y: usize, c: [u8; 3]) {
        let o = (y * self.w + x) * 3;
        self.rgb[o..o + 3].copy_from_slice(&c);
    }
}

/// Fraksjonell arealmiddelverdi (referansenedskalering), separabel: først horisontalt, så vertikalt.
pub fn resize_area(img: &Img, nw: usize, nh: usize) -> Img {
    let (nw, nh) = (nw.max(1), nh.max(1));
    let src: Vec<f32> = img.rgb.iter().map(|&v| v as f32).collect();
    let mut tmp = vec![0.0f32; nw * img.h * 3];
    let sx = img.w as f32 / nw as f32;
    for y in 0..img.h {
        for o in 0..nw {
            let (a, b) = (o as f32 * sx, (o + 1) as f32 * sx);
            let (i0, i1) = (a.floor() as usize, (b.ceil() as usize).min(img.w));
            for c in 0..3 {
                let (mut acc, mut ws) = (0.0f32, 0.0f32);
                for i in i0..i1 {
                    let w = (b.min(i as f32 + 1.0) - a.max(i as f32)).max(0.0);
                    acc += w * src[(y * img.w + i) * 3 + c];
                    ws += w;
                }
                tmp[(y * nw + o) * 3 + c] = acc / ws.max(1e-6);
            }
        }
    }
    let mut out = vec![0.0f32; nw * nh * 3];
    let scale = img.h as f32 / nh as f32;
    for o in 0..nh {
        let (a, b) = (o as f32 * scale, (o + 1) as f32 * scale);
        let (i0, i1) = (a.floor() as usize, (b.ceil() as usize).min(img.h));
        for x in 0..nw * 3 {
            let (mut acc, mut ws) = (0.0f32, 0.0f32);
            for i in i0..i1 {
                let w = (b.min(i as f32 + 1.0) - a.max(i as f32)).max(0.0);
                acc += w * tmp[i * nw * 3 + x];
                ws += w;
            }
            out[o * nw * 3 + x] = acc / ws.max(1e-6);
        }
    }
    Img {
        w: nw,
        h: nh,
        rgb: out
            .iter()
            .map(|v| v.round().clamp(0.0, 255.0) as u8)
            .collect(),
    }
}

fn conv_h(img: &Img, k: &[f32]) -> Vec<f32> {
    let r = (k.len() / 2) as isize;
    let mut out = vec![0.0f32; img.rgb.len()];
    for y in 0..img.h {
        for x in 0..img.w {
            for c in 0..3 {
                let mut acc = 0.0;
                for (j, kv) in k.iter().enumerate() {
                    let xx = (x as isize + j as isize - r).clamp(0, img.w as isize - 1) as usize;
                    acc += kv * img.rgb[(y * img.w + xx) * 3 + c] as f32;
                }
                out[(y * img.w + x) * 3 + c] = acc;
            }
        }
    }
    out
}

fn conv_v(w: usize, h: usize, src: &[f32], k: &[f32]) -> Img {
    let r = (k.len() / 2) as isize;
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let mut acc = 0.0;
                for (j, kv) in k.iter().enumerate() {
                    let yy = (y as isize + j as isize - r).clamp(0, h as isize - 1) as usize;
                    acc += kv * src[(yy * w + x) * 3 + c];
                }
                out.rgb[(y * w + x) * 3 + c] = acc.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Gauss-uskarphet med standardavvik `sigma` (piksler).
pub fn gaussian(img: &Img, sigma: f32) -> Img {
    let r = (sigma * 3.0).ceil().max(1.0) as isize;
    let mut k: Vec<f32> = (-r..=r)
        .map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp())
        .collect();
    let s: f32 = k.iter().sum();
    k.iter_mut().for_each(|v| *v /= s);
    let tmp = conv_h(img, &k);
    conv_v(img.w, img.h, &tmp, &k)
}

/// Horisontal bevegelsesuskarphet (boksfilter med lengde `len` piksler).
pub fn motion_h(img: &Img, len: usize) -> Img {
    let n = len.max(1) | 1;
    let k = vec![1.0 / n as f32; n];
    let tmp = conv_h(img, &k);
    conv_v(img.w, img.h, &tmp, &[1.0])
}

/// Jevn beskjæring: `pct` prosent fjernes fra hver kant.
pub fn crop_margin(img: &Img, pct: f32) -> Img {
    let mx = ((img.w as f32 * pct / 100.0).round() as usize).max(1);
    let my = ((img.h as f32 * pct / 100.0).round() as usize).max(1);
    let (nw, nh) = (img.w - 2 * mx, img.h - 2 * my);
    let mut out = Img::new(nw, nh);
    for y in 0..nh {
        let s = ((y + my) * img.w + mx) * 3;
        out.rgb[y * nw * 3..(y + 1) * nw * 3].copy_from_slice(&img.rgb[s..s + nw * 3]);
    }
    out
}

/// Luma (Rec. 601, heltall) som Gray8.
pub fn luma(img: &Img) -> Vec<u8> {
    img.rgb
        .chunks(3)
        .map(|p| ((299 * p[0] as u32 + 587 * p[1] as u32 + 114 * p[2] as u32) / 1000) as u8)
        .collect()
}

pub fn encode_png(img: &Img) -> Vec<u8> {
    let mut v = Vec::new();
    image::codecs::png::PngEncoder::new(&mut v)
        .write_image(
            &img.rgb,
            img.w as u32,
            img.h as u32,
            ExtendedColorType::Rgb8,
        )
        .expect("png");
    v
}

pub fn encode_gray_png(w: usize, h: usize, g: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    image::codecs::png::PngEncoder::new(&mut v)
        .write_image(g, w as u32, h as u32, ExtendedColorType::L8)
        .expect("png");
    v
}

pub fn encode_jpeg(img: &Img, quality: u8) -> Vec<u8> {
    let mut v = Vec::new();
    JpegEncoder::new_with_quality(&mut v, quality)
        .write_image(
            &img.rgb,
            img.w as u32,
            img.h as u32,
            ExtendedColorType::Rgb8,
        )
        .expect("jpeg");
    v
}

pub fn decode(bytes: &[u8]) -> Img {
    let d = image::load_from_memory(bytes).expect("decode").to_rgb8();
    Img {
        w: d.width() as usize,
        h: d.height() as usize,
        rgb: d.into_raw(),
    }
}

/// Lineær bevegelsesuskarphet i vilkårlig retning: `len` piksler langs vinkel `deg` (grader, 0 = horisontal).
/// Kjernen er et linjestykke samplet med bilineær interpolasjon (realistisk for kamerabevegelse).
pub fn motion_dir(img: &Img, len: f32, deg: f32) -> Img {
    let n = (len.ceil() as usize).max(2) * 2; // taps langs linja
    let (s, c) = deg.to_radians().sin_cos();
    let taps: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let t = (i as f32 + 0.5) / n as f32 - 0.5;
            (t * len * c, t * len * s)
        })
        .collect();
    let at = |x: f32, y: f32, ch: usize| -> f32 {
        let (x, y) = (
            x.clamp(0.0, (img.w - 1) as f32),
            y.clamp(0.0, (img.h - 1) as f32),
        );
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(img.w - 1), (y0 + 1).min(img.h - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let p = |xx: usize, yy: usize| img.rgb[(yy * img.w + xx) * 3 + ch] as f32;
        (p(x0, y0) * (1.0 - fx) + p(x1, y0) * fx) * (1.0 - fy)
            + (p(x0, y1) * (1.0 - fx) + p(x1, y1) * fx) * fy
    };
    let mut out = Img::new(img.w, img.h);
    for y in 0..img.h {
        for x in 0..img.w {
            for ch in 0..3 {
                let acc: f32 = taps
                    .iter()
                    .map(|&(dx, dy)| at(x as f32 + dx, y as f32 + dy, ch))
                    .sum();
                out.rgb[(y * img.w + x) * 3 + ch] =
                    (acc / n as f32).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Fokusfeil: uskarphet med skiveformet kjerne (pillbox, radius `r` piksler). Hver kjernerad er en
/// korde som summeres eksakt fra radprefikser (fraksjonelle endepunkter gir kantutjevning), så
/// kostnaden er O(r) per piksel. Standardavviket til kjernen er `r / 2`.
pub fn defocus(img: &Img, r: f32) -> Img {
    let ri = r.floor() as isize;
    let (wi, hi) = (img.w as isize, img.h as isize);
    let pad = ri as usize + 2;
    let pw = img.w + 2 * pad;
    // Radprefikser per kanal, kantreplikert; prefix[(y*3+c)*(pw+1) + i] = sum av de i første.
    let mut prefix = vec![0i64; img.h * 3 * (pw + 1)];
    for y in 0..img.h {
        for c in 0..3 {
            let base = (y * 3 + c) * (pw + 1);
            for i in 0..pw {
                let x = (i as isize - pad as isize).clamp(0, wi - 1) as usize;
                prefix[base + i + 1] = prefix[base + i] + img.rgb[(y * img.w + x) * 3 + c] as i64;
            }
        }
    }
    // Kontinuerlig prefiks: piksel i dekker [i - 0.5, i + 0.5] (i i utvidet koordinat).
    let f = |base: usize, t: f32| -> f32 {
        let t = t + 0.5 + pad as f32;
        let i = t.floor();
        let fr = t - i;
        let i = (i as usize).min(pw - 1);
        let v = (prefix[base + i + 1] - prefix[base + i]) as f32;
        prefix[base + i] as f32 + fr * v
    };
    let chords: Vec<(isize, f32)> = (-ri..=ri)
        .map(|dy| (dy, (r * r - (dy * dy) as f32).max(0.0).sqrt() + 0.5))
        .collect();
    let wsum: f32 = chords.iter().map(|&(_, hw)| 2.0 * hw).sum();
    let mut out = Img::new(img.w, img.h);
    for y in 0..hi {
        for x in 0..wi {
            for c in 0..3 {
                let mut acc = 0.0f32;
                for &(dy, hw) in &chords {
                    let yy = (y + dy).clamp(0, hi - 1) as usize;
                    let base = (yy * 3 + c) * (pw + 1);
                    acc += f(base, x as f32 + hw) - f(base, x as f32 - hw);
                }
                out.rgb[(y * wi + x) as usize * 3 + c] =
                    (acc / wsum).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Jevnt fordelt sensorstøy (uavhengig per kanal), amplitude ±`amp` nivåer.
pub fn add_noise(img: &Img, seed: u64, amp: i32) -> Img {
    let mut r = crate::rng::Rng::new(seed);
    let mut o = img.clone();
    for v in o.rgb.iter_mut() {
        let n = r.below(2 * amp as u64 + 1) as i32 - amp;
        *v = (*v as i32 + n).clamp(0, 255) as u8;
    }
    o
}
