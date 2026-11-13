#!/usr/bin/env bash
# Henter YuNet (MIT) og verifiserer sjekksum. Modellfiler ligger IKKE i git (models/ er ignorert).
set -euo pipefail
cd "$(dirname "$0")"; mkdir -p models testdata
M=models/face_detection_yunet_2023mar.onnx
SHA=8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4
# github.com/.../raw gir LFS-peker/403 bak enkelte proxyer; media.githubusercontent.com leverer selve filen.
[ -f "$M" ] || curl -fsSL -o "$M" "https://media.githubusercontent.com/media/opencv/opencv_zoo/main/models/face_detection_yunet/face_detection_yunet_2023mar.onnx"
echo "$SHA  $M" | sha256sum -c -
# Testbilde (kun lokal test, ikke i git). Lisensen til bildet er ikke avklart: bruk egne testbilder ved evaluering.
I=testdata/selfie.jpg
ISHA=ab8413ad9bb4f53068f4fb63c6747e5989991dd02241c923d5595b614ecf2bf6
[ -f "$I" ] || curl -fsSL -o "$I" "https://media.githubusercontent.com/media/opencv/opencv_zoo/main/models/face_detection_yunet/example_outputs/largest_selfie.jpg"
echo "$ISHA  $I" | sha256sum -c -
