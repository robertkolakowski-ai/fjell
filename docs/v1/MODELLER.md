# V1: modeller (øyne og ansikter)

Status: beslutningsunderlag og beskrivelse av rammen i `core/crates/fjell-models`. **Ingen modell er valgt, lastet ned eller sjekket inn.** Alle fakta om eksterne modeller under er hentet fra hukommelsen uten nettilgang og er ikke verifisert; det som må sjekkes er merket «må verifiseres av eier».

## 1. Krav (fra CLAUDE.md og V1-PLAN)

- On-device: ingen bilder eller embeddings forlater enheten. Ingen nettverkskode i appen eller i `fjell-models`.
- Modeller følger med installasjonsprogrammet. Appen laster aldri ned noe, heller ikke oppdateringer av modeller.
- Appen foreslår, den dømmer ikke: usikker analyse er «ikke analysert» (`AnalyzeError::Unavailable`), aldri «ingen lukkede øyne».
- Permissiv lisens (Apache-2.0, MIT, BSD-2/3-Clause, 0BSD, CC0-1.0), både for vektene og, så langt det lar seg avklare, treningsdataene. Lisensen sjekkes maskinelt i lasteporten, men selve lisensvurderingen er eiers.
- Størrelse ≤ 30 MB per modellfil (håndhevet). Ansiktsdeteksjon + embedding + øyeanalyse til sammen bør holde seg godt under ca. 60 MB i installasjonsprogrammet.
- Handlinger < 100 ms p95 og kald oppstart < 1,5 s: modellen lastes lazy første gang den trengs, aldri ved oppstart, og kjøres bare på 256 px-miniatyrer, i bakgrunnen (ikke i en brukerhandling).
- Ansiktsanalyse kjører bare når brukeren har slått på `face_index`.

## 2. Rammen (implementert)

| Del | Hva |
|---|---|
| `ModelManifest` | Lokal JSON: navn, formål `eye`/`face`, versjon, BLAKE3 (64 hex), byte-størrelse, SPDX-lisens, `source_url` (kun metadata, aldri hentet), `file`, `input_shape` `[1,C,H,W]`, `output_shape`, `embedding_dim`, `input_mean`/`input_std`, kalibrerte `thresholds`. Ukjente felt avvises. |
| Lasteport (`load_verified`) | Rekkefølge: parse, formål, lisens på tillatt-liste, deklarert størrelse ≤ 30 MB, faktisk filstørrelse (metadata, så lesing med tak), størrelse = manifest, BLAKE3 = manifest. Alt feiler lukket med typet `ModelError`. Først etter dette finnes en `VerifiedModel`, og bare den kan åpnes av en `SessionFactory`. |
| `OnnxSession`/`SessionFactory` | Trait for selve kjøringen. `MockSession` i tester. `NoRuntime` svarer `RuntimeUnavailable`. |
| `ModelEyeAnalyzer`, `ModelFaceAnalyzer` | Implementerer `EyeAnalyzer`/`FaceAnalyzer`. Manglende fil, feil formål, hash-/størrelses-/lisensfeil og manglende runtime gir `Unavailable` (årsak i `unavailable_reason()`). Feil utdataform/-verdier og kjørefeil gir `Failed`. |
| Feature `onnx` | Av som standard. Slått på gir den bare en `OnnxFactory` som feiler lukket (se «Hva mangler»). |

### Utdatakontrakt

Modellen må levere dette; en ekte modell pakkes (grafetterbehandling i ONNX eller en tynn per-modell-omformer i en senere fase) slik at den følger kontrakten:

- Øye: `[1,3]` = (antall ansikter, antall med lukket øye, sikkerhet 0..1). Under `eye_min_confidence` (når det finnes ansikter) svarer adapteren `Unavailable`.
- Ansikt: `[1,N,5+D]`, rad = (x, y, w, h normalisert 0..1, kvalitet 0..1, embedding\[D]). Ansikter under `face_min_quality` utelates. NaN/uendelig eller kvalitet utenfor 0..1 gir `Failed`.
- Inndata: miniatyr skaleres (nærmeste nabo) til `H×W`, 1 eller 3 kanaler, `(v/255 − mean)/std`.

### Eksempelmanifest (utenfor repoet; hashen er en plassholder)

```json
{
  "name": "face-embed-example", "purpose": "face", "version": "1.0",
  "blake3": "<64 hex>", "bytes": 9000000, "license": "Apache-2.0",
  "source_url": "<kun metadata>", "file": "face.onnx",
  "input_shape": [1, 3, 112, 112], "output_shape": [1, 0, 133],
  "embedding_dim": 128, "input_mean": 0.5, "input_std": 0.5,
  "thresholds": { "face_min_quality": 0.6 }
}
```

### Hva mangler (ikke gjort)

- Ingen ONNX-runtime (`ort` eller `tract`) er lagt inn: den finnes ikke i `Cargo.lock`, og uten nett kan ikke `--locked` og `cargo deny` bekreftes. `ort` bruker dessuten en innebygd native bibliotek-binær (ONNX Runtime), som må vurderes mot offline-krav, lisens og installasjonsstørrelse; `tract` er ren Rust og antakelig enklere å revidere, men ikke alle operatorer støttes. Må avgjøres og verifiseres av eier/integratør.
- Ingen ekte modell, ingen kalibrerte terskler, ingen måling av ytelse. Tersklene i testene er illustrasjoner.
- Per-modell-etterbehandling (ankere, NMS, ansiktsjustering før embedding) er ikke skrevet: avhenger av valgt modell.
- Ingen integrasjon: `fjell-core`, `fjell-ffi` og skallet injiserer ennå bare `NullEyeAnalyzer`/`NullFaceAnalyzer`. Å koble inn `Model*Analyzer` hører til integrasjonsfasen. Her trengs også en installasjonsløype for modellmappen.
- `cargo deny check` er ikke kjørt (ikke installert her).

## 3. Kandidater

Alle tall og lisenser: **må verifiseres av eier** (hentet fra hukommelsen, ikke bekreftet mot kilde). Lisens for vekter og for treningsdata er to ulike spørsmål.

### Øyenåpenhet

| Kandidat | Omtrentlig størrelse | Lisens (påstått) | Risiko |
|---|---|---|---|
| Landemerkebasert (ansiktslandemerker + øyeåpningsmål, EAR) fra f.eks. MediaPipe Face Landmarker | få MB | Apache-2.0 for MediaPipe-kode; modellkortets vilkår må verifiseres av eier | Krever landemerkemodell; brillebærere, sideprofil, smil/myke øyne og lav oppløsning (256 px miniatyr) gir mange falske «lukket». EAR-terskel må kalibreres. |
| Liten CNN for åpent/lukket øye (egentrent eller fra åpent prosjekt) | < 1 MB | Ukjent per prosjekt; må verifiseres av eier | Åpne datasett for øyestatus (f.eks. CEW) har uavklarte vilkår; modeller trent på dem arver kanskje ikke en permissiv lisens. |
| dlib 68-punkts landemerker + EAR | ca. 95 MB uten kvantisering (for stor) | Kode Boost-lisens; vektene er trent på 300-W-data med begrensende vilkår (må verifiseres) | For stor og lisensrisiko. Anbefales ikke. |

Kalibreringsrisiko generelt: «lukket øye» er et mykt begrep (blunk midt i, myke øyne, solbriller). Terskel skal kalibreres på eget testsett (se `docs/v1/KALIBRERING-V1.md`) med fokus på å **unngå falske «lukket»** (kan føre til at et godt bilde foreslås bort), og lav sikkerhet skal gi «ikke analysert».

### Ansiktsdeteksjon

| Kandidat | Omtrentlig størrelse | Lisens (påstått) | Risiko |
|---|---|---|---|
| YuNet (OpenCV Zoo) | under 1 MB | MIT (må verifiseres av eier) | Svakere på små og sterkt vinklede ansikter; god nok for 256 px? Må måles. |
| BlazeFace (MediaPipe) | ca. 1 MB | Apache-2.0 (må verifiseres) | Kortdistansevariant dekker selfie-nære ansikter dårlig for gruppebilder. |
| UltraFace (lett RFB/slim) | 1–2 MB | MIT (må verifiseres) | Lavere nøyaktighet; treningsdata (WIDER FACE) har egne vilkår, må avklares. |
| SCRFD / RetinaFace (InsightFace) | 2–30 MB | Kode MIT, men ferdigtrente vekter har ofte kun ikke-kommersiell bruk | Lisensrisiko på vektene. Utelukk med mindre eier verifiserer det motsatte. |

### Ansiktsembedding

| Kandidat | Omtrentlig størrelse | Lisens (påstått) | Risiko |
|---|---|---|---|
| SFace (OpenCV Zoo) | ca. 37 MB fp32, ca. 10 MB int8 | Apache-2.0 (må verifiseres av eier) | fp32 er over 30 MB-taket, så int8 er det eneste som passer. Kvantisering kan svekke nøyaktigheten. Treningsdata (MS1M/Glint e.l.) er omstridt, må vurderes av eier. |
| MobileFaceNet (ulike open source-vekter) | 4–5 MB | Varierer per vekt; må verifiseres av eier | Samme treningsdatarisiko. Demografisk skjevhet i likhet gir feilgruppering av personer. |
| ArcFace/buffalo (InsightFace) | 5–170 MB | Ferdigtrente modeller: ikke-kommersiell (må verifiseres) | Lisens. Utelukk med mindre eier verifiserer det motsatte. |

Kalibreringsrisiko: likhetsterskel for «samme person» varierer med alder, lys, vinkel og demografi. Feil gruppering av personer er en integritets- og tillitsrisiko, så terskler må settes konservativt, og appen må bare **foreslå** personer (bruker bekrefter).

## 4. Installasjons- og oppdateringsflyt

1. Modellfiler og `manifest.json` legges i en skrivebeskyttet mappe ved siden av programmet av installasjonsprogrammet (f.eks. `models/eye/` og `models/face/`). Ingenting lastes ned av appen.
2. Ved første bruk (ikke ved oppstart) laster appen manifestet via `load_verified`: hash, størrelse og lisens sjekkes før en økt åpnes.
3. Feiler noe, vises funksjonen som ikke tilgjengelig (ikke som «ingenting funnet»), og resten av appen fungerer som i MVP.
4. Modelloppdatering følger programoppdatering (nytt installasjonsprogram). Nye vekter gir ny `version` og ny hash; algoritmeversjonen (`ALGO_VER`/fingeravtrykk) må økes når embedding-rommet endres, slik at gamle embeddings ikke blandes med nye. Det er integrasjonsfasens sak.
5. Brukerens `face_index`-data (embeddings) slettes via den vanlige papirkurven/angre-flyten; modellfiler endres aldri av appen.
6. THIRD_PARTY.md oppdateres med modellens navn, versjon og lisenstekst ved innlemmelse (lisenstekster må hentes og kontrolleres av eier).

## 5. Hva Robert må velge

1. **Øyemodell**: landemerke + EAR (krever landemerkemodell) eller egen liten klassifikator? Hva er akseptabel feilrate for falske «lukket»?
2. **Ansiktsdetektor** (f.eks. YuNet eller BlazeFace) og **embeddingmodell** (f.eks. SFace int8). Verifiser lisens for vekter og treningsdata mot kilden, og godkjenn evt. restrisiko.
3. **Kjøremotor**: `ort` (ONNX Runtime, innebygd binær) eller `tract` (ren Rust). Verifiser `cargo deny`, `--locked`, installasjonsstørrelse og Windows-ytelse.
4. **Kalibreringssett**: hvem lager og eier testbilder for terskler (bilder forlater ikke enheten, jf. KALIBRERING-V1).
5. **Totalbudsjett** for modellstørrelse i installasjonsprogrammet (forslag over: ca. 60 MB).
6. **Standardtilstand**: skal personfunksjonen være av til brukeren aktivt slår den på (anbefalt, i tråd med `face_index`)?

## 6. Ikke verifisert

- Alle eksterne modellers størrelse, lisens og treningsdatavilkår (ingen nett).
- Ytelse (p95-mål), Windows-oppførsel og ekte ONNX-kjøring (ingen runtime, ingen modell).
- `cargo deny check` (ikke installert i dette miljøet).
