# Personvern og teknikk

- **On-device først:** analyse, ansiktsgjenkjenning og embeddings lokalt (Core ML / NNAPI / ONNX). Ingen bilder forlater enheten i standardmodus.
- **Valgfri sky-analyse** for tyngre modeller, med eksplisitt samtykke og ende-til-ende-kryptering. Bilder lagres ikke.
- Ingen annonser, ingen salg av data.
- **Indeks:** SQLite med vektorindeks (f.eks. HNSW). Inkrementell bakgrunnsindeksering (lading + Wi-Fi) så 50 000 bilder ikke tømmer batteriet.
- **Plattformer:** iOS (PhotoKit), Android (MediaStore), desktop (Electron/Tauri) for filbibliotek, NAS og eksterne disker.
- **Format:** HEIC, RAW, Live Photos og video.

## Ytelsesmål
- Første indeksering < 10 min per 10 000 bilder på moderne telefon
- Handlinger < 100 ms (p95)
- Kald oppstart < 1,5 s

## Åpne tekniske valg
- Delt kjerne: Rust, Kotlin Multiplatform eller separate implementasjoner?
- Desktop: Electron eller Tauri?
- Valg av embedding- og estetikkmodeller (lisens, størrelse, ONNX-støtte)
