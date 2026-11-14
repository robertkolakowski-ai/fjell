//! Spike M19: YuNet (ONNX) via tract på ett bilde. Kun deteksjon + 5 landemerker.
//! Ingen embedding/mal lagres, ingen identifikasjon. Resultatet skrives til stdout og kastes.
//! Bruk: fjell-faces-spike <modell.onnx> <bilde.jpg|png> [antall_kjøringer]
use std::time::Instant;
use tract_onnx::prelude::*;

#[derive(Debug, Clone)]
struct Face {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    score: f32,
    lm: [(f32, f32); 5], // høyre øye, venstre øye, nese, høyre/venstre munnvik
}

fn iou(a: &Face, b: &Face) -> f32 {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.w).min(b.x + b.w);
    let y2 = (a.y + a.h).min(b.y + b.h);
    let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    inter / (a.w * a.h + b.w * b.h - inter).max(1e-6)
}

fn main() -> TractResult<()> {
    let mut args = std::env::args().skip(1);
    let model_path = args.next().expect("modell.onnx");
    let img_path = args.next().expect("bilde");
    let runs: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(20);

    // Modellens deklarerte inngangsform (YuNet 2023mar er fast; 2026may er dynamisk).
    let proto = tract_onnx::onnx().model_for_path(&model_path)?;
    let declared = proto.input_fact(0)?.shape.as_concrete_finite().ok().flatten().map(|s| s.to_vec());
    let (h, w) = match declared {
        Some(s) if s.len() == 4 => (s[2], s[3]),
        _ => (480usize, 640usize),
    };
    let t_load = Instant::now();
    let model = proto
        .with_input_fact(0, f32::fact([1, 3, h, w]).into())?
        .into_optimized()?
        .into_runnable()?;
    eprintln!("modell lastet/optimert på {:?}, inngang {}x{}", t_load.elapsed(), w, h);

    let t_dec = Instant::now();
    let img = image::open(&img_path).expect("bilde").to_rgb8();
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let small = image::imageops::resize(&img, w as u32, h as u32, image::imageops::FilterType::Triangle);
    eprintln!("bilde {}x{} dekodet+skalert på {:?}", iw, ih, t_dec.elapsed());

    // YuNet forventer BGR, 0..255, NCHW.
    let input: Tensor = tract_ndarray::Array4::from_shape_fn((1, 3, h, w), |(_, c, y, x)| {
        let p = small.get_pixel(x as u32, y as u32);
        p.0[2 - c] as f32
    })
    .into();

    let mut out = None;
    let mut times = Vec::new();
    for _ in 0..runs.max(1) {
        let t = Instant::now();
        out = Some(model.run(tvec!(input.clone().into()))?);
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    eprintln!(
        "inferens: median {:.1} ms, p95 {:.1} ms over {} kjøringer (1 tråd per kall)",
        times[times.len() / 2],
        times[(times.len() as f64 * 0.95) as usize % times.len()],
        times.len()
    );
    let out = out.unwrap();

    // Identifiser utganger via form: [1,N,C], C=1 (cls/obj), 4 (bbox), 10 (kps).
    // Rekkefølge i 2023mar: cls_8,16,32, obj_8,16,32, bbox_8,16,32, kps_8,16,32.
    let mut cls = vec![];
    let mut obj = vec![];
    let mut bbox = vec![];
    let mut kps = vec![];
    for (i, t) in out.iter().enumerate() {
        let shape = t.shape().to_vec();
        let c = *shape.last().unwrap();
        eprintln!("utgang {i}: {:?}", shape);
        let v = t.to_plain_array_view::<f32>()?.iter().copied().collect::<Vec<f32>>();
        match (c, i / 3) {
            (1, 0) => cls.push(v),
            (1, 1) => obj.push(v),
            (4, 2) => bbox.push(v),
            (10, 3) => kps.push(v),
            _ => panic!("uventet utgangsform {:?} på plass {i}", shape),
        }
    }

    let mut faces: Vec<Face> = vec![];
    for (si, stride) in [8usize, 16, 32].into_iter().enumerate() {
        let (cols, rows) = (w / stride, h / stride);
        for r in 0..rows {
            for c in 0..cols {
                let idx = r * cols + c;
                let s = (cls[si][idx].clamp(0.0, 1.0) * obj[si][idx].clamp(0.0, 1.0)).sqrt();
                if s < 0.6 {
                    continue;
                }
                let b = &bbox[si][idx * 4..idx * 4 + 4];
                let cx = (c as f32 + b[0]) * stride as f32;
                let cy = (r as f32 + b[1]) * stride as f32;
                let bw = b[2].exp() * stride as f32;
                let bh = b[3].exp() * stride as f32;
                let k = &kps[si][idx * 10..idx * 10 + 10];
                let mut lm = [(0.0, 0.0); 5];
                for j in 0..5 {
                    lm[j] = (
                        (k[2 * j] + c as f32) * stride as f32 * iw / w as f32,
                        (k[2 * j + 1] + r as f32) * stride as f32 * ih / h as f32,
                    );
                }
                faces.push(Face {
                    x: (cx - bw / 2.0) * iw / w as f32,
                    y: (cy - bh / 2.0) * ih / h as f32,
                    w: bw * iw / w as f32,
                    h: bh * ih / h as f32,
                    score: s,
                    lm,
                });
            }
        }
    }
    faces.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    let mut keep: Vec<Face> = vec![];
    for f in faces {
        if keep.iter().all(|k| iou(k, &f) < 0.3) {
            keep.push(f);
        }
    }
    println!("{} ansikt(er) funnet", keep.len());
    for f in &keep {
        println!(
            "  boks=({:.0},{:.0},{:.0},{:.0}) score={:.2} øyne=({:.0},{:.0})/({:.0},{:.0})",
            f.x, f.y, f.w, f.h, f.score, f.lm[0].0, f.lm[0].1, f.lm[1].0, f.lm[1].1
        );
    }
    Ok(())
}
