//! Delte testhjelpere for `fjell-fs`: temp-mapper, syntetiske JPEG-er (med EXIF-orientering) og sjekksummer.

#![allow(dead_code)]

use image::{codecs::jpeg::JpegEncoder, ExtendedColorType};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn tempdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("fjell-fs-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

/// JPEG `w`x`h`: venstre halvdel rød, høyre blå. `orientation` 1..=8 legges inn som EXIF.
pub fn jpeg(w: u32, h: u32, orientation: Option<u8>) -> Vec<u8> {
    let mut px = Vec::new();
    for _y in 0..h {
        for x in 0..w {
            px.extend_from_slice(if x < w / 2 {
                &[255, 0, 0]
            } else {
                &[0, 0, 255]
            });
        }
    }
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 95)
        .encode(&px, w, h, ExtendedColorType::Rgb8)
        .unwrap();
    let Some(o) = orientation else { return out };
    // APP1/Exif rett etter SOI: TIFF big-endian, én IFD0-oppføring (0x0112, SHORT).
    let mut app = b"Exif\0\0MM\0*\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01".to_vec();
    app.extend_from_slice(&[0, o, 0, 0, 0, 0, 0, 0]);
    let mut seg = vec![0xFF, 0xE1];
    seg.extend_from_slice(&((app.len() + 2) as u16).to_be_bytes());
    seg.extend_from_slice(&app);
    let mut res = out[..2].to_vec();
    res.extend_from_slice(&seg);
    res.extend_from_slice(&out[2..]);
    res
}

pub fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

/// rel-sti (med `/`) -> BLAKE3, for alle filer under `root`, unntatt karantenemappen.
pub fn snapshot(root: &Path) -> BTreeMap<String, String> {
    fn go(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                if p.file_name().unwrap() != ".fjell-karantene" {
                    go(root, &p, out);
                }
            } else {
                let rel = p
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(
                    rel,
                    blake3::hash(&fs::read(&p).unwrap()).to_hex().to_string(),
                );
            }
        }
    }
    let mut m = BTreeMap::new();
    go(root, root, &mut m);
    m
}
