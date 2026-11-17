# V1: modeller (øyne og ansikter)

Status: beslutningsunderlag og beskrivelse av rammen i `core/crates/fjell-models`. **Ingen modell er valgt eller sjekket inn.** Avsnitt 7 er verifisert mot primærkilder 2026-11-17 (lisensfiler, LFS-hasher, egne målinger i `tools/model-probe/`). Resten av dokumentet (avsnitt 1–6) er skrevet før det og er merket der det er foreldet. Det som fortsatt ikke er verifisert står i avsnitt 8.

## 1. Krav (fra CLAUDE.md og V1-PLAN)

- On-device: ingen bilder eller embeddings forlater enheten. Ingen nettverkskode i appen eller i `fjell-models`.
- Modeller følger med installasjonsprogrammet. Appen laster aldri ned noe, heller ikke oppdateringer av modeller.
- Appen foreslår, den dømmer ikke: usikker analyse er «ikke analysert» (`AnalyzeError::Unavailable`), aldri «ingen lukkede øyne».
- Permissiv lisens (Apache-2.0, MIT, BSD-2/3-Clause, 0BSD, CC0-1.0), både for vektene og, så langt det lar seg avklare, treningsdataene. Lisensen sjekkes maskinelt i lasteporten, men selve lisensvurderingen er eiers.
- Størrelse ≤ 40 MB per modellfil (håndhevet). Ansiktsdeteksjon + embedding + øyeanalyse til sammen bør holde seg godt under ca. 60 MB i installasjonsprogrammet.
- Handlinger < 100 ms p95 og kald oppstart < 1,5 s: modellen lastes lazy første gang den trengs, aldri ved oppstart, og kjøres bare på 256 px-miniatyrer, i bakgrunnen (ikke i en brukerhandling).
- Ansiktsanalyse kjører bare når brukeren har slått på `face_index`.

## 2. Rammen (implementert)

| Del | Hva |
|---|---|
| `ModelManifest` | Lokal JSON: navn, formål `eye`/`face`, versjon, BLAKE3 (64 hex), byte-størrelse, SPDX-lisens, `source_url` (kun metadata, aldri hentet), `file`, `input_shape` `[1,C,H,W]`, `output_shape`, `embedding_dim`, `input_mean`/`input_std`, kalibrerte `thresholds`. Ukjente felt avvises. |
| Lasteport (`load_verified`) | Rekkefølge: parse, formål, lisens på tillatt-liste, deklarert størrelse ≤ 40 MB, faktisk filstørrelse (metadata, så lesing med tak), størrelse = manifest, BLAKE3 = manifest. Alt feiler lukket med typet `ModelError`. Først etter dette finnes en `VerifiedModel`, og bare den kan åpnes av en `SessionFactory`. |
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

- Ingen ONNX-runtime (`ort` eller `tract`) er lagt inn i arbeidsområdet (kjøremotorene er vurdert i avsnitt 7.3; valget er eiers). `ort` bruker dessuten en innebygd native bibliotek-binær (ONNX Runtime), som må vurderes mot offline-krav, lisens og installasjonsstørrelse; `tract` er ren Rust og antakelig enklere å revidere, men ikke alle operatorer støttes. Må avgjøres og verifiseres av eier/integratør.
- Ingen ekte modell, ingen kalibrerte terskler, ingen måling av ytelse. Tersklene i testene er illustrasjoner.
- Per-modell-etterbehandling (ankere, NMS, ansiktsjustering før embedding) er ikke skrevet: avhenger av valgt modell.
- Ingen integrasjon: `fjell-core`, `fjell-ffi` og skallet injiserer ennå bare `NullEyeAnalyzer`/`NullFaceAnalyzer`. Å koble inn `Model*Analyzer` hører til integrasjonsfasen. Her trengs også en installasjonsløype for modellmappen.
- `cargo deny check` er ikke kjørt (ikke installert her).

## 3. Kandidater

Tallene i tabellene under er fra hukommelsen. **YuNet og SFace er siden verifisert, se avsnitt 7** (der gjelder tallene i avsnitt 7, ikke tabellene). De andre kandidatene er fortsatt uverifisert. Lisens for vekter og for treningsdata er to ulike spørsmål.

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
| SFace (OpenCV Zoo) | ca. 37 MB fp32, ca. 10 MB int8 | Apache-2.0 (må verifiseres av eier) | fp32 var over det opprinnelige 30 MB-taket; taket heves til 40 MB (avsnitt 9), så fp32 passer nå. Kvantisering kan svekke nøyaktigheten. Treningsdata (MS1M/Glint e.l.) er omstridt, må vurderes av eier. |
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

## 5. Hva Robert må velge (se avsnitt 7.5 for oppdatert anbefaling)

1. **Øyemodell**: landemerke + EAR (krever landemerkemodell) eller egen liten klassifikator? Hva er akseptabel feilrate for falske «lukket»?
2. **Ansiktsdetektor** (f.eks. YuNet eller BlazeFace) og **embeddingmodell** (f.eks. SFace int8). Verifiser lisens for vekter og treningsdata mot kilden, og godkjenn evt. restrisiko.
3. **Kjøremotor**: `ort` (ONNX Runtime, innebygd binær) eller `tract` (ren Rust). Verifiser `cargo deny`, `--locked`, installasjonsstørrelse og Windows-ytelse.
4. **Kalibreringssett**: hvem lager og eier testbilder for terskler (bilder forlater ikke enheten, jf. KALIBRERING-V1).
5. **Totalbudsjett** for modellstørrelse i installasjonsprogrammet (forslag over: ca. 60 MB).
6. **Standardtilstand**: skal personfunksjonen være av til brukeren aktivt slår den på (anbefalt, i tråd med `face_index`)?

## 6. Ikke verifisert (foreldet, se avsnitt 8)

- Alle eksterne modellers størrelse, lisens og treningsdatavilkår (ingen nett).
- Ytelse (p95-mål), Windows-oppførsel og ekte ONNX-kjøring (ingen runtime, ingen modell).
- `cargo deny check` (ikke installert i dette miljøet).

## 7. Verifisert underlag (2026-11-17)

Kilder: `opencv/opencv_zoo` (filene i `models/face_detection_yunet/` og `models/face_recognition_sface/`, hentet via `raw.githubusercontent.com` og `media.githubusercontent.com`), PyPI-hjulet `onnxruntime` 1.32.0 og crates.io. Målingene er kjørt av `tools/model-probe/` på en delt Linux-maskin (4 vCPU, Intel Xeon 2,1 GHz, en prosess om gangen). **Alle tider er Linux, ikke Windows.**

### 7.1 Modellfilene (størrelse og hash verifisert)

Filene er lastet ned og SHA-256 er sammenlignet med `oid sha256:` i Zoo-ens LFS-pekere: alle stemmer. BLAKE3 er beregnet med samme `blake3`-versjon som lasteporten bruker.

| Fil (OpenCV Zoo) | Bytes | SHA-256 | BLAKE3 | Kjører i |
|---|---|---|---|---|
| `face_detection_yunet_2026may.onnx` (dynamisk H×W) | 229 738 | `ebafce4e…f0f0` | `226e749f6dba5390a1b1415d8177c54d2d74328824b111cdc4a136416c6de365` | tract og ONNX Runtime |
| `face_detection_yunet_2023mar.onnx` (fast 640×640) | 232 589 | `8f2383e4…2fa4` | `3d5938c4cd5a02dc416f1cd1f7fc1f662a22adc370477112c871954587e63431` | tract og ONNX Runtime |
| `face_detection_yunet_2023mar_int8.onnx` | 100 416 | `321aa5a6…9294` | `3cdde626e6cd9a4654bf6f9071c6c45285e9d2347739ebf727127b4551c619a2` | bare ONNX Runtime |
| `face_recognition_sface_2021dec.onnx` (fp32) | 38 696 353 | `0ba9fbfa…4e79` | `584d0f5792e9e776ab5e5bf3f559f2310ff8fdd3b39f94dd364751046592b512` | tract og ONNX Runtime |
| `face_recognition_sface_2021dec_int8.onnx` | 9 896 933 | `2b0e941e…d78a` | `ddd6db2a770ae616276f767cae0bf93500a4f7d32e4db74c5cccadc5a49df2f5` | bare ONNX Runtime |

Rettelser til tidligere påstander: YuNet fp32 er 0,23 MB (ikke «under 1 MB» som tabellen sa, men riktig størrelsesorden). SFace fp32 er **38,7 MB** (ikke ca. 37 MB) og dermed **over 30 MB-taket**; int8 er 9,9 MB. Zoo-en har også `*_int8bq.onnx` (blokkvis kvantisert, 122 KB og 10,7 MB); den er ikke testet her.

### 7.2 Lisens (verifisert mot filene i repoet; treningsdata ikke avklart)

- YuNet: `models/face_detection_yunet/LICENSE` er MIT (Copyright 2020 Shiqi Yu). Modellkilden er `ShiqiYu/libfacedetection.train`. README oppgir WIDER Face for evaluering; **treningsdatavilkårene er ikke verifisert.**
- SFace: `models/face_recognition_sface/LICENSE` er Apache-2.0, og README sier «All files in this directory are licensed under Apache 2.0». Modellen er MobileFaceNet trent med SFace-tapsfunksjonen. **README oppgir ikke treningsdata.** Artikkelen bruker, så vidt jeg vet, et stort ansiktsdatasett med omstridt opphav (MS-Celeb-1M-slekten); det er ikke bekreftet mot kilde her og må vurderes av eier/jurist.
- ONNX Runtime (Python-hjulet 1.32.0): `onnxruntime/LICENSE` er MIT (Copyright Microsoft). Hjulet har egen `ThirdPartyNotices.txt` (341 KB) som må inn i `THIRD_PARTY.md` hvis biblioteket leveres.

Begge modell-lisensene ligger på lasteportens tillatt-liste (MIT, Apache-2.0). Å legge dem i `models/` krever uansett at lisensteksten følger med og at `THIRD_PARTY.md` oppdateres (avsnitt 4, punkt 6).

### 7.3 Kjøremotor: `tract` mot ONNX Runtime

| | `tract` 0.23.10 (ren Rust) | ONNX Runtime via `ort` 2.0.0-rc.15 |
|---|---|---|
| Lisens | MIT OR Apache-2.0 (123 pakker i treet; alle på `deny.toml`-listen bortsett fra at `same-file`/`walkdir` har «Unlicense/MIT», som er et valg og tillater MIT) | `ort` MIT OR Apache-2.0; ONNX Runtime MIT |
| Native binær | nei | ja: `onnxruntime.dll` x64 CPU **22,7 MB** ukomprimert (1.32.0), pluss `onnxruntime_providers_shared.dll` 22 KB |
| Bygg | ren cargo, ingen nedlasting; ingen `tokio`/async i treet (`deny.toml` forbyr `tokio`) | standardfunksjonene `download-binaries` og `tls-native` henter binæren fra nett ved bygging: **bryter offline- og reproduserbarhetskravet**. Må bygges med `load-dynamic` og en egen, hash-sjekket DLL. Versjonen er en forhåndsutgave (`rc`). |
| Kvantiserte modeller (int8) | **støttes ikke:** `QLinearAdd` og `QLinearMul` gir «Unimplemented» (YuNet-int8 og SFace-int8) | støttes |
| fp32-modeller | kjører alle tre fp32-modellene | kjører alle |
| Dynamiske dimensjoner | fungerer med 2026may-modellen; 2023mar er låst til 640×640 og kan ikke endres til 256 | som tract |

Målt kjøretid (median av 21–31 kjøringer, Linux, ett inndatabilde; «1 tråd» og «4 tråder» er ONNX Runtimes intra-op-tråder; tract kjørte med sin standard):

| Modell | Inndata | tract | ONNX Runtime 1 tråd | ONNX Runtime 4 tråder |
|---|---|---|---|---|
| YuNet 2026may fp32 | 256×256 | 5–6 ms | 1,0 ms | – |
| YuNet 2026may fp32 | 640×640 | 41–44 ms | 4,0 ms | – |
| YuNet 2023mar fp32 | 640×640 | 35–40 ms | 8,3 ms | 4,0 ms |
| YuNet 2023mar int8 | 640×640 | feiler | 51 ms | 17 ms |
| SFace fp32 | 112×112 | 39 ms | 37 ms | 13 ms |
| SFace int8 | 112×112 | feiler | 84 ms | 33 ms |

Lesning: ONNX Runtime er 5–10 ganger raskere enn tract på YuNet; på SFace er de like. **int8 er langsommere enn fp32 på denne CPU-en** (ingen glede av heltallsinstruksjoner her; Windows-maskiner kan se annerledes ut). Første lasting tok 35–165 ms i tract (lazy, ikke ved oppstart). Alle kjøringer ligger langt under 100 ms-budsjettet, men ansiktsanalyse kjører uansett i bakgrunnen og ikke i en brukerhandling.

### 7.4 Kvalitetsindikasjoner (to offentlige demobilder, enkel dekoding)

Veiledende, ikke et kalibreringssett: to bilder fra Zoo-en (en gruppeselfie 2048×1150 og et bilde med flere personer 1024×512), enkel dekoding og NMS (IoU 0,3), score ≥ 0,6.

- **Oppløsning på inndata betyr mye.** YuNet 2026may fant på gruppeselfien 8 ansikter ved 256 px lang side, 30 ved 512 px og 53 ved 640 px; på det andre bildet 2, 7 og 6. De faste 2023mar-modellene (fp32/int8, strukket til 640×640) fant 27/40 og 7/7. Følgen er at **256 px-miniatyrer (avsnitt 1) er for lite for gruppebilder**; ansiktsanalyse må dekode til ca. 512–640 px, eller vi må akseptere at bare store ansikter fanges.
- **int8 mot fp32 for embedding** (SFace, seks ansiktsutsnitt, enkelt bbox-utsnitt uten justering): cosinus mellom fp32- og int8-vektor for samme utsnitt 0,978–0,988; likhet mellom par av ulike utsnitt var nesten identisk (forskjell ≤ 0,03). Kvantiseringen svekker altså ikke likheten målbart på dette lille utvalget. Tilfeldig støy som inndata ga bare 0,95; støy er ikke ansikter, så tallet sier lite.
- Ikke målt: nøyaktighet på norske bilder, barn, ulik hudtone, briller og solbriller, bevegelsesuskarphet; likhetsterskler (se `docs/v1/KALIBRERING-V1.md`).

### 7.5 Anbefaling (valget er Roberts)

1. **Detektor: YuNet `2026may` (dynamisk).** MIT, 0,23 MB, kjører i begge motorer, og den dynamiske formen lar oss velge oppløsning etter bildet. Dekod til ca. 512–640 px for ansiktsdeteksjon, ikke 256 px.
2. **Embedding: SFace.** To veier:
   - **A (anbefalt): `tract` + SFace fp32 (38,7 MB).** Ren Rust, ingen DLL, færrest bevegelige deler (CLAUDE.md), og ingen nedlasting ved bygging. Pris: modellfilen er 8,7 MB over dagens tak, så `MAX_MODEL_BYTES` må heves (f.eks. til 40 MB; det er en konstant i `fjell-models`, men en beslutning). Samlet modellstørrelse ca. 39 MB, og installasjonsprogrammet komprimerer.
   - **B: ONNX Runtime (`ort`) + SFace int8 (9,9 MB).** Raskest, og modellen er liten, men prisen er en 22,7 MB DLL, en `rc`-avhengighet og et bygg som må tvinges bort fra nedlasting. Samlet ca. 33 MB. Flere bevegelige deler og mer å revidere.
3. **Øyemodell: ikke verifisert i denne runden.** Ingen kandidat er sjekket mot kilde. Landemerkebasert øyeåpning (EAR) er fortsatt mest realistisk, men krever en landemerkemodell som ikke er vurdert her. Se avsnitt 5, punkt 1.
4. **Kalibrering og terskler:** uendret (avsnitt 5, punkt 4 og `docs/v1/KALIBRERING-V1.md`).

### 7.6 Hva som må gjøres for å ta modellene i bruk (ikke gjort)

- Velge vei A eller B; deretter legge motoren inn i `fjell-models` bak feature `onnx` (i dag en `OnnxFactory` som feiler lukket), kjøre `cargo deny`, `--locked` og `just third-party`.
- Skrive etterbehandling: YuNet-dekoding (tre skalaer, `cls`/`obj`/`bbox`/`kps`, NMS) og ansiktsjustering med 5 landemerker før SFace. Utdatakontrakten i avsnitt 2 passer ikke direkte til modellenes råutdata, så det trengs en omformer per modell (som avsnitt 2 allerede sier).
- Manifest per modell med hashene over, lisens (`MIT`/`Apache-2.0`) og `source_url` som ren metadata.
- Oppdatere `THIRD_PARTY.md` og `docs/juridisk/LISENSER.md` med lisensteksten (YuNet MIT, SFace Apache-2.0, og ONNX Runtime hvis vei B).
- Måle på Windows (WIC-dekoding, ytelse, `onnxruntime.dll`-lasting hvis vei B).

## 8. Fortsatt ikke verifisert

- Treningsdatavilkår for YuNet (WIDER Face) og SFace (ukjent i README). Juridisk vurdering er eiers.
- Alle andre kandidater i avsnitt 3 (BlazeFace, UltraFace, MobileFaceNet, SCRFD/RetinaFace, ArcFace/buffalo, MediaPipe-landemerker, dlib): ikke sjekket mot kilde.
- Alt på Windows: ytelse, `onnxruntime.dll`-oppførsel, installasjonsstørrelse etter komprimering, WIC-forbehandling.
- Ekte kvalitet: ingen ekte kalibrering, ingen norske testbilder, ingen måling av falsk gruppering av personer. Kvalitetstallene i 7.4 er fra to demobilder.
- `ort`-krateet er ikke bygget eller kjørt her (bare metadata er lest); ONNX Runtime er målt via Python-hjulet, som er samme C++-motor men ikke samme Rust-binding.
- `cargo deny` er ikke kjørt (ikke installert); lisensvurderingen av `tract`-treet er gjort mot `deny.toml` med et eget skript på `cargo metadata`.
- Narrator/NVDA er ikke relevant her; ingen UI-endring.

## 9. Beslutning: vei A (2026-11-17, eier)

Robert valgte **vei A: `tract` + SFace fp32**. Dette er det som er avgjort og det som ikke er det.

**Avgjort**
- Kjøremotor er `tract` (ren Rust). ONNX Runtime/`ort` velges bort for denne omgangen (22,7 MB DLL, `rc`-krate, nedlasting ved bygging).
- Embeddingmodell er SFace fp32 (`face_recognition_sface_2021dec.onnx`, 38 696 353 byte, Apache-2.0). Kvantiserte modeller (int8) brukes ikke, siden `tract` ikke støtter dem.
- Taket per modellfil heves fra 30 til **40 MiB** (`MAX_MODEL_BYTES` i `fjell-models`). Det er en hard grense i lasteporten og dekker SFace fp32 med liten margin; en større modell krever ny beslutning (en kompileringstidssjekk i `manifest.rs` vokter det).
- Detektoren er foreløpig anbefalt, ikke bekreftet: YuNet `2026may` (se 7.5). Siden `tract` støtter den dynamiske modellen, er den kompatibel med vei A.

**Fortsatt åpent (ikke avgjort)**
- Oppløsning for ansiktsdeteksjon (256 px mot 512–640 px, avsnitt 7.4).
- Øyemodell (ingen kandidat verifisert).
- Treningsdatavilkår for YuNet og SFace (juridisk vurdering).
- Kalibrering og terskler.

**Ikke gjort i denne endringen:** `tract` er ikke lagt inn som avhengighet, og ingen modell er sjekket inn eller koblet til. Det gjenstår (se 7.6, punkt 1): legge `tract-onnx` bak feature `onnx` i `fjell-models`, `cargo deny`, `--locked` og `just third-party`; skrive YuNet-dekoding og SFace-justering; manifest med hashene i 7.1; måling på Windows. Modellen kopieres ikke inn i repoet før den siste lisens- og treningsdatavurderingen er gjort.
