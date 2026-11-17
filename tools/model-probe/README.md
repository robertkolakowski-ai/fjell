# Modellmåling (utvikler-verktøy)

Reproduserer målingene i `docs/v1/MODELLER.md`, avsnitt 7. **Ikke en del av appen, kjernen eller CI**: ingenting her bygges eller kjøres av `just`, og ingen modellfiler sjekkes inn. Alt kjøres lokalt av den som vil etterprøve tallene.

## 1. Hent modellene (utenfor repoet)

Modellene ligger i OpenCV Zoo (`opencv/opencv_zoo`, git LFS). Last dem ned til en tom mappe utenfor repoet og kontroller sha256 mot LFS-pekeren (`oid sha256:` i `raw.githubusercontent.com/.../<fil>`):

```bash
M=https://media.githubusercontent.com/media/opencv/opencv_zoo/main/models
mkdir -p /tmp/zoo/bin && cd /tmp/zoo/bin
for f in face_detection_yunet/face_detection_yunet_2023mar.onnx \
         face_detection_yunet/face_detection_yunet_2023mar_int8.onnx \
         face_detection_yunet/face_detection_yunet_2026may.onnx \
         face_recognition_sface/face_recognition_sface_2021dec.onnx \
         face_recognition_sface/face_recognition_sface_2021dec_int8.onnx; do curl -sSLO "$M/$f"; done
sha256sum *.onnx
```

## 2. ONNX Runtime (Python) og kvalitetssjekk

```bash
python3 -m venv /tmp/venv && /tmp/venv/bin/pip install onnxruntime numpy onnx pillow
/tmp/venv/bin/python bench.py /tmp/zoo/bin          # latens, operatorer, fp32 mot int8
/tmp/venv/bin/python faces.py /tmp/zoo/bin <bildemappe>   # antall ansikter per oppløsning, fp32 mot int8 på utsnitt
```

`faces.py` bruker enkel dekoding og NMS og enkel bbox-utsnitt uten 5-punktsjustering. Tallene er veiledende, ikke et kalibreringssett. Bruk egne testbilder (bilder skal ikke forlate enheten, jf. `docs/v1/KALIBRERING-V1.md`).

## 3. tract (ren Rust)

```bash
cd tract-probe && cargo run --release -- /tmp/zoo/bin
```

Skriver størrelse og BLAKE3 per modell, og median kjøretid (21 kjøringer). Kvantiserte modeller gir forventet feil (`QLinearAdd`/`QLinearMul` støttes ikke).
