# Personvern og teknikk

- **On-device først:** analyse, ansiktsgjenkjenning og embeddings lokalt (Core ML / NNAPI / ONNX). Ingen bilder forlater enheten i standardmodus.
- **Valgfri sky-analyse** for tyngre modeller, med eksplisitt samtykke og ende-til-ende-kryptering. Bilder lagres ikke.
- Ingen annonser, ingen salg av data.
- **Indeks:** SQLite med vektorindeks (f.eks. HNSW). Inkrementell bakgrunnsindeksering med lav prioritet (på mobil: lading + Wi-Fi) så 50 000 bilder ikke går ut over maskinen eller batteriet.
- **Plattformer:** MVP: Windows 10/11 desktop (Tauri 2) for filbibliotek, eksterne disker og nettverksstasjoner. Etter MVP: iPhone (PhotoKit), Android (MediaStore). Linux/macOS-bygg er bonus.
- **Format:** HEIC, RAW, Live Photos og video.

## Ytelsesmål
- Første indeksering < 10 min per 10 000 bilder (MVP: referansemaskin med Windows og lokal SSD; nettverksstasjoner måles separat)
- Handlinger < 100 ms (p95)
- Kald oppstart < 1,5 s

## Åpne tekniske valg
- ~~Delt kjerne~~ avgjort: Rust. ~~Desktop~~ avgjort: Tauri 2 (se `docs/mvp/ARKITEKTUR.md`).
- Valg av embedding- og estetikkmodeller (lisens, størrelse, ONNX-støtte)
