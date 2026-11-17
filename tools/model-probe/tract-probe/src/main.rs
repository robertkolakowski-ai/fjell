//! Måler `tract` mot modellene fra OpenCV Zoo: laster, kjører, tid per kjøring (median av 21).
//! Bruk: `cargo run --release -- <mappe med .onnx-filer>`. Kvantiserte modeller gir forventet feil (QLinearAdd/QLinearMul).
use std::time::Instant;
use tract_onnx::prelude::*;
fn run(
    path: &str,
    shape: &[usize],
    seed: u32,
) -> TractResult<(Vec<Vec<f32>>, Vec<Vec<usize>>, f64, f64)> {
    let t = Instant::now();
    let model = tract_onnx::onnx()
        .model_for_path(path)?
        .with_input_fact(0, f32::fact(shape).into())?
        .into_optimized()?
        .into_runnable()?;
    let load = t.elapsed().as_secs_f64() * 1000.0;
    let n: usize = shape.iter().product();
    let mut s = seed;
    let data: Vec<f32> = (0..n)
        .map(|_| {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            ((s >> 8) % 256) as f32
        })
        .collect();
    let input = tract_ndarray::Array::from_shape_vec(shape.to_vec(), data)?.into_tensor();
    let mut times = vec![];
    let mut out = None;
    for _ in 0..21 {
        let t = Instant::now();
        out = Some(model.run(tvec!(input.clone().into()))?);
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let out = out.unwrap();
    let shapes = out.iter().map(|t| t.shape().to_vec()).collect();
    let vals = out
        .iter()
        .map(|t| {
            t.to_plain_array_view::<f32>()
                .unwrap()
                .iter()
                .cloned()
                .collect::<Vec<f32>>()
        })
        .collect();
    Ok((vals, shapes, load, times[times.len() / 2]))
}
fn main() -> TractResult<()> {
    let d = std::env::args().nth(1).unwrap();
    let cases: [(&str, [usize; 4]); 7] = [
        ("face_detection_yunet_2023mar.onnx", [1, 3, 640, 640]),
        ("face_detection_yunet_2023mar_int8.onnx", [1, 3, 640, 640]),
        ("face_detection_yunet_2026may.onnx", [1, 3, 256, 256]),
        ("face_detection_yunet_2026may.onnx", [1, 3, 640, 640]),
        ("face_recognition_sface_2021dec.onnx", [1, 3, 112, 112]),
        ("face_recognition_sface_2021dec_int8.onnx", [1, 3, 112, 112]),
        ("face_detection_yunet_2023mar.onnx", [1, 3, 256, 256]),
    ];
    for (f, sh) in cases {
        if let Ok(b) = std::fs::read(format!("{d}/{f}")) {
            println!("{f}: {} B, blake3 {}", b.len(), blake3::hash(&b).to_hex());
        }
        match run(&format!("{d}/{f}"), &sh, 1) {
            Ok((_, shapes, load, med)) => println!(
                "{f} {sh:?}: last {load:.1} ms, median {med:.2} ms, ut {:?}",
                &shapes[..shapes.len().min(2)]
            ),
            Err(e) => println!(
                "{f} {sh:?}: FEIL {}",
                format!("{e:#}").lines().next().unwrap_or("")
            ),
        }
    }
    Ok(())
}
