//! Grensesnitt: `generate(out_dir, seed) -> io::Result<Manifest>` skriver bilder under
//! `images/<gruppe>/`, 256 px-miniatyrer (Gray8-PNG, area-average) under `thumbs/` og
//! `manifest.json`. Gruppene og fasiten følger PLAN-KJERNE del 2 (ca. 330 bilder).
//! Alt er en ren funksjon av `seed`; ingen klokke, ingen tråder.

use crate::manifest::{Entry, Manifest};
use crate::ops::*;
use crate::rng::{fnv64, Rng};
use crate::synth::*;
use std::{fs, io, path::Path};

pub const DEFAULT_SEED: u64 = 0xF1E1_1C0D_E5EE_D001;
const T0: i64 = 1_700_000_000;

#[derive(Default, Clone)]
struct Meta {
    exact: Option<u32>,
    visual: Option<u32>,
    blurry: Option<bool>,
    screenshot: Option<bool>,
    hint: bool,
    camera: Option<bool>,
}

struct Builder<'a> {
    out: &'a Path,
    entries: Vec<Entry>,
}

impl Builder<'_> {
    fn add(&mut self, group: &str, name: &str, ext: &str, bytes: &[u8], m: Meta) -> io::Result<()> {
        let rel = format!("images/{group}/{name}.{ext}");
        let thumb = format!("thumbs/{group}_{name}.png");
        for (p, b) in [(&rel, bytes.to_vec()), (&thumb, self.thumb_png(bytes))] {
            let path = self.out.join(p);
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, b)?;
        }
        let d = decode(bytes);
        let n = self.entries.len() as i64;
        self.entries.push(Entry {
            file: rel,
            thumb,
            group: group.into(),
            mime: if ext == "png" {
                "image/png"
            } else {
                "image/jpeg"
            }
            .into(),
            width: d.w as u32,
            height: d.h as u32,
            bytes: bytes.len() as u64,
            fnv64: format!("{:016x}", fnv64(bytes)),
            created_at: T0 + n * 3600,
            exact_group: m.exact,
            visual_group: m.visual,
            blurry: m.blurry,
            screenshot: m.screenshot,
            platform_screenshot_hint: m.hint,
            has_camera_exif: m.camera,
        });
        Ok(())
    }

    /// Referansenedskalering: lengste side 256, bildeforhold bevart, aldri oppskalert.
    fn thumb_png(&self, bytes: &[u8]) -> Vec<u8> {
        let d = decode(bytes);
        let s = (256.0 / d.w.max(d.h) as f32).min(1.0);
        let t = resize_area(
            &d,
            (d.w as f32 * s).round() as usize,
            (d.h as f32 * s).round() as usize,
        );
        encode_gray_png(t.w, t.h, &luma(&t))
    }
}

fn dims(i: usize) -> (usize, usize) {
    if i % 2 == 0 {
        (512, 384)
    } else {
        (384, 512)
    }
}

pub fn generate(out: &Path, seed: u64) -> io::Result<Manifest> {
    let mut b = Builder {
        out,
        entries: Vec::new(),
    };
    let sd = |tag: u64, i: usize| seed ^ (tag << 32) ^ (i as u64).wrapping_mul(0x9E37_79B1);
    let originals: Vec<Img> = (0..60)
        .map(|i| {
            let (w, h) = dims(i);
            scene(sd(1, i), w, h)
        })
        .collect();
    let png: Vec<Vec<u8>> = originals.iter().map(encode_png).collect();
    let has_derived = |i: usize| i < 50;
    let vis = |i: usize| if has_derived(i) { Some(i as u32) } else { None };

    for i in 0..60 {
        let exact = (i < 20).then_some(i as u32);
        b.add(
            "original",
            &format!("o{i:02}"),
            "png",
            &png[i],
            Meta {
                exact,
                visual: vis(i),
                ..Default::default()
            },
        )?;
    }
    for i in 0..20 {
        b.add(
            "exact",
            &format!("kopi{i:02}"),
            "png",
            &png[i],
            Meta {
                exact: Some(i as u32),
                visual: vis(i),
                ..Default::default()
            },
        )?;
    }
    for k in 0..30 {
        let i = 20 + k;
        let f = if k % 2 == 0 { 2 } else { 4 };
        let s = resize_area(&originals[i], originals[i].w / f, originals[i].h / f);
        b.add(
            "downscaled",
            &format!("n{k:02}_{}", 100 / f),
            "png",
            &encode_png(&s),
            Meta {
                visual: vis(i),
                ..Default::default()
            },
        )?;
    }
    for k in 0..30 {
        let i = 10 + k;
        let (q, name) = match k % 4 {
            0 => (30, "q30"),
            1 => (60, "q60"),
            2 => (85, "q85"),
            _ => (95, "png2jpg"),
        };
        b.add(
            "recoded",
            &format!("r{k:02}_{name}"),
            "jpg",
            &encode_jpeg(&originals[i], q),
            Meta {
                visual: vis(i),
                ..Default::default()
            },
        )?;
    }
    for k in 0..20 {
        let i = 30 + k;
        let c = crop_margin(&originals[i], 5.0);
        b.add(
            "cropped",
            &format!("c{k:02}"),
            "png",
            &encode_png(&c),
            Meta {
                visual: vis(i),
                ..Default::default()
            },
        )?;
    }

    // Motiver for uskarphetstesten. Varianter deler visuell gruppe 100+i.
    let motifs: Vec<Img> = (0..40).map(|i| scene(sd(2, i), 512, 384)).collect();
    for (i, m) in motifs.iter().enumerate() {
        // Sigma som prosent av bredden; bevegelsesuskarphet teller som lengde i prosent.
        let (name, img, blurry) = match i % 4 {
            0 => ("g05", gaussian(m, 0.005 * 512.0), false),
            1 => ("g10", gaussian(m, 0.01 * 512.0), true),
            2 => ("g20", gaussian(m, 0.02 * 512.0), true),
            _ => ("mot", motion_h(m, (0.04 * 512.0) as usize), true),
        };
        b.add(
            "blurry",
            &format!("u{i:02}_{name}"),
            "png",
            &encode_png(&img),
            Meta {
                visual: Some(100 + i as u32),
                blurry: Some(blurry),
                ..Default::default()
            },
        )?;
    }
    for (i, m) in motifs.iter().enumerate() {
        // Siste ti er lav kontrast (hvit vegg, himmel); skal fortsatt være skarpe.
        let img = if i >= 30 {
            lowcontrast(m, 0.15 + 0.03 * (i - 30) as f32)
        } else {
            m.clone()
        };
        let vg = if i >= 30 { None } else { Some(100 + i as u32) };
        b.add(
            "sharp",
            &format!("s{i:02}"),
            "png",
            &encode_png(&img),
            Meta {
                visual: vg,
                blurry: Some(false),
                ..Default::default()
            },
        )?;
    }

    // Skjermbilder i kjente enhetsoppløsninger.
    const RES: [(usize, usize); 5] = [
        (1170, 2532),
        (1284, 2778),
        (1125, 2436),
        (750, 1334),
        (1179, 2556),
    ];
    for i in 0..30 {
        let (w, h) = RES[i % 5];
        let img = ui_screen(sd(3, i), w, h);
        let (ext, bytes) = if i % 2 == 0 {
            ("png", encode_png(&img))
        } else {
            ("jpg", encode_jpeg(&img, 90))
        };
        let hint = i % 3 == 0; // resten: uten plattformflagg
        b.add(
            "screenshot",
            &format!("sk{i:02}"),
            ext,
            &bytes,
            Meta {
                screenshot: Some(true),
                hint,
                camera: Some(false),
                ..Default::default()
            },
        )?;
    }
    let mut r = Rng::new(sd(4, 0));
    for i in 0..20 {
        if i < 10 {
            let (w, h) = (200 + r.below(700) as usize, 200 + r.below(700) as usize);
            let img = noise_texture(sd(5, i), w, h);
            b.add(
                "screenshot_neg",
                &format!("ned{i:02}"),
                "png",
                &encode_png(&img),
                Meta {
                    screenshot: Some(false),
                    camera: Some(false),
                    ..Default::default()
                },
            )?;
        } else {
            let (w, h) = RES[i % 5];
            let img = scene(sd(6, i), w / 2, h / 2);
            b.add(
                "screenshot_neg",
                &format!("kam{i:02}"),
                "jpg",
                &encode_jpeg(&img, 88),
                Meta {
                    screenshot: Some(false),
                    camera: Some(true),
                    ..Default::default()
                },
            )?;
        }
    }

    // Negative par: ingen gruppe (hverken eksakt eller visuell).
    for p in 0..10 {
        // Samme farger (samme palett-seed), men annen komposisjon.
        let pal = sd(7, p);
        let (a, mut c) = (scene(pal, 384, 288), scene(pal, 384, 288));
        let mut rr = Rng::new(pal ^ 0xABCD);
        for _ in 0..40 {
            let (x, y) = (rr.below(380) as usize, rr.below(284) as usize);
            c.set(x, y, [255, 255, 255]);
        }
        let mut shifted = Img::new(384, 288);
        for y in 0..288 {
            for x in 0..384 {
                let o = (y * 384 + (x + 190) % 384) * 3;
                shifted.set(x, y, [c.rgb[o], c.rgb[o + 1], c.rgb[o + 2]]);
            }
        }
        c = shifted;
        b.add(
            "negpair",
            &format!("p{p:02}a"),
            "png",
            &encode_png(&a),
            Meta::default(),
        )?;
        b.add(
            "negpair",
            &format!("p{p:02}b"),
            "png",
            &encode_png(&c),
            Meta::default(),
        )?;
    }
    for p in 0..5 {
        // Lik scene, ulik innramming (30 % beskjæring i én retning).
        let s = scene(sd(8, p), 512, 384);
        let mut part = Img::new(300, 384);
        for y in 0..384 {
            for x in 0..300 {
                let o = (y * 512 + x + 212 * (p % 2)) * 3;
                part.set(x, y, [s.rgb[o], s.rgb[o + 1], s.rgb[o + 2]]);
            }
        }
        b.add(
            "negpair",
            &format!("f{p:02}a"),
            "png",
            &encode_png(&s),
            Meta::default(),
        )?;
        b.add(
            "negpair",
            &format!("f{p:02}b"),
            "png",
            &encode_png(&part),
            Meta::default(),
        )?;
    }

    // Degenererte bilder.
    let degen: Vec<(&str, Img)> = vec![
        ("svart", Img::solid(256, 192, [0, 0, 0])),
        ("hvit", Img::solid(256, 192, [255, 255, 255])),
        ("enfarget1", Img::solid(320, 240, [200, 30, 60])),
        ("enfarget2", Img::solid(200, 300, [20, 90, 200])),
        ("grå", Img::solid(256, 256, [128, 128, 128])),
        ("en1", Img::solid(1, 1, [10, 200, 90])),
        ("en2", Img::solid(1, 1, [255, 255, 255])),
        ("smal1", noise_texture(sd(9, 0), 1, 400)),
        ("smal2", noise_texture(sd(9, 1), 400, 2)),
        ("smal3", noise_texture(sd(9, 2), 3, 600)),
    ];
    for (n, img) in &degen {
        b.add(
            "degenerate",
            n,
            "png",
            &encode_png(img),
            Meta {
                blurry: Some(false),
                ..Default::default()
            },
        )?;
    }

    let m = Manifest {
        seed,
        thumb_note: "Gray8 PNG, lengste side 256, area-average, aldri oppskalert",
        entries: b.entries,
    };
    fs::write(out.join("manifest.json"), m.to_json())?;
    Ok(m)
}
