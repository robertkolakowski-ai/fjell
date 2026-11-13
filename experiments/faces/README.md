# Ansiktsspike (M19)

Frittstående prototype for `docs/mvp/ML-ADR.md`. Ikke i core-workspacet, ikke bygget i CI, ikke koblet til appen.

```
./fetch-model.sh                                   # henter YuNet (MIT) + testbilde, verifiserer sha256
CARGO_TARGET_DIR=/home/user/tgt-core cargo build   # tract-onnx 0.23.10, ca. 130 crates
/home/user/tgt-core/debug/fjell-faces-spike models/face_detection_yunet_2023mar.onnx testdata/selfie.jpg 30
```

Prototypen gjør bare deteksjon + 5 landemerker. Den lagrer ingen embedding, mal eller identitet, og skriver
ingenting til disk. Øyne-åpne og smil er IKKE implementert (se ADR). `models/` og `testdata/` er ignorert av git.
