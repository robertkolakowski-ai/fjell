# ADR: Ansiktsdeteksjon og øyne/smil for «beste bilde» (spike M19)

Status: forslag etter spike, 2026-11-13. Kode: `experiments/faces/` (frittstående, ikke i CI).
Alt under er merket **målt** (kjørt her), **lest** (hentet fra kilde via proxy) eller **ikke verifisert**.

## 1. Beslutning

**DELVIS.**

- **GO** for ansiktsdeteksjon + 5 landemerker lokalt: YuNet (MIT) kjørt med tract (ren Rust). Prototypen fungerer.
- **NO-GO for øyne åpne / smil i MVP.** Vi fant ingen modell med verifisert tillatende lisens, kjørbar i tract, og målt
  nøyaktighet for øyestatus. Beste kandidat (MediaPipe Face Landmarker) er TFLite og ikke prøvd i tract. Egen oppfølgingsspike.
- «Alle ser mot kamera» kan bare anslås grovt fra landemerkegeometri. Ikke målt, ikke påstått nøyaktig.

## 2. Kandidatmodeller

| Modell | Lisens | Størrelse | Nøyaktighet | Status |
|---|---|---|---|---|
| YuNet `face_detection_yunet_2023mar.onnx` (opencv_zoo) | MIT (zoo-mappen, © Shiqi Yu); treningsrepoet `libfacedetection.train` er BSD-3-Clause | 232 589 B (målt) | WIDER Face val AP easy/medium/hard 0,884/0,866/0,750 (lest, zoo-README; samme README nevner også 0,834/0,824/0,708, så tallet er usikkert) | **Kjørt i tract.** sha256 `8f2383e4…2552fa4` |
| UltraFace RFB-320 (Linzaer) | MIT (lest) | 1 270 727 B (målt) | ikke hentet | Lastet ned, ikke kjørt |
| MediaPipe Face Landmarker `.task` | Apache-2.0 i repoet (lest); modellkortets egen lisens ikke lest | 3 758 596 B (målt): detektor 230 KB, 478 landemerker 2,55 MB, blendshapes 0,96 MB | Ikke lest. Blendshapes har `eyeBlink*` og `mouthSmile*`, altså akkurat det vi trenger | TFLite, ikke ONNX. **Ikke kjørt.** sha256 `64184e22…bc9ff` |
| ONNX-modell for øyestatus/smil med tillatende lisens | – | – | – | **Ikke funnet.** HuggingFace var blokkert av proxyen (403), og vi søkte ikke bredt |

Begrensning ved YuNet: den gir bare boks + 5 punkter (øyne, nese, munnvik), ikke øyelokk. Øyne-åpne krever en landemerkemodell (typisk
EAR over øyelokkspunkter) eller blendshapes. Prototypen skalerer bildet til 640×640 uten letterbox (formen forvrenges);
effekt på nøyaktighet er ikke målt.

Veien videre for øyne/smil: konverter MediaPipe-modellene offline til ONNX (f.eks. tf2onnx), sjekk at tract kjører dem
(operatorstøtte ikke verifisert), og avklar om den konverterte modellen arver Apache-2.0 (ikke avklart).

## 3. Kjøremiljø

| | tract 0.23.10 | ort / ONNX Runtime | Windows ML / DirectML |
|---|---|---|---|
| Lisens | MIT/Apache-2.0 (lest via cargo metadata) | ort MIT/Apache-2.0 (lest); ONNX Runtime MIT (lest) | Microsoft-komponent, vilkår ikke lest |
| Bygg | **Målt:** kompilerer og kjører på Linux, ca. 130 crates. Ren Rust, ingen build.rs mot native kode forventet. Windows ikke verifisert | Laster ned eller krever ferdige binærfiler: ikke verifisert | Krever WinRT/Windows App SDK-kobling: ikke verifisert |
| Installasjonsstørrelse | Release ikke målt (debug-binær 38 MB er ikke representativ). Modell +0,2 MB | `onnxruntime.dll` ca. titalls MB (anslag fra hukommelse, **ikke verifisert**) | Avhenger av system/pakke, ikke verifisert |
| cargo-deny | **Målt:** `cargo deny --config deny.toml check licenses bans` ga «bans ok, licenses ok» på spikens lockfile. Advisories ikke kjørt (krever nett mot RustSec) | Native binærfil ligger utenfor cargo-deny; må inn i `THIRD_PARTY.md` manuelt | Samme |
| Nettverk | **Målt:** ingen reqwest/ureq/hyper/tokio/openssl i treet. Forenlig med `nonet.test.ts` | Binærnedlasting ved bygg: må vurderes mot «ingen nett»-prinsippet | – |
| Ytelse | Se under | Ikke målt | GPU-varians gir ikke-deterministiske resultater |

Anbefalt stack: **tract + YuNet**. Minst ny flate: ingen native DLL, ingen nye Tauri-tillatelser (inferens skjer i kjernen),
ingen CSP-endring, `deny.toml` urørt. Krever at toolchain ≥ Rust 1.91 (tract 0.23.10 `rust-version`); sjekk mot kjernens toolchain.

### Ytelse (målt, Linux, 4 vCPU, `dev`-profil med opt-level 1, 1 tråd, 640×640)

- Inferens: median 61,8 ms, p95 76,7 ms per bilde (30 kjøringer). Modell last+optimer: 40 ms.
- JPEG-dekoding + skalering av 2048×1150: 129 ms (opt-level 1).
- Regnestykke (antatt lineær skalering over 4 kjerner, **ikke målt parallelt**): 10 000 bilder, bare inferens ≈ 2,6 min;
  med full dekoding ≈ 8 min. Innenfor 10 min, men med tynn margin og uten release-profil.
- Derfor: kjør **bare på kandidatgrupper** (nesten-duplikater som skal rangeres), ikke hele biblioteket, og bruk eksisterende
  miniatyrer der det rekker. Per-handling-budsjettet (< 100 ms p95) holder bare hvis kjøringen er bakgrunnsjobb med
  resultater i indeksen, aldri synkront i UI.
- Release-måling og Windows-referansemaskin gjenstår.

Prototypresultat: testbildet (folkemengde, 2048×1150) ga 32 treff ved terskel 0,6; ikke sammenlignet mot fasit.

## 4. Visning som begrunnelse uten å dømme

Prinsipp fra CLAUDE.md: appen foreslår, hvert forslag har begrunnelsesmerke, ingenting er forhåndsvalgt eller slettes automatisk.

- Merker er korte og forsiktige: «Ansikter funnet: 3», «Øynene ser åpne ut», «Alle ser ut til å se mot kamera».
  Aldri «Dårlig bilde», aldri poengsum per person, aldri rangering av enkeltpersoner.
- Lav sikkerhet eller manglende data gir **ingen merke** («Ikke vurdert»), aldri et negativt merke.
- Forslaget gjelder bare valg mellom nesten-like bilder, og brukeren velger selv. Knapp for «Hvorfor?» viser hva som ble målt.
- Alle merker har knappealternativ og skjermleserstreng (nb og en; nb er ~15–30 % lengre, test layout med nb).
- **Konflikt å avklare:** `docs/PRODUKT.md` (linje 26) sier «aldri forhåndsvalgt hvis bildet har ansikter av personer du ofte fotograferer».
  Det forutsetter gjenkjenning/embedding, noe denne ADR-en utelukker. Enten omformuleres regelen til «aldri forhåndsvalgt når bildet har ansikter»
  (ingen identitet), eller så trengs egen personvernvurdering. Jeg har ikke endret PRODUKT.md (utenfor spikens omfang).
- Unngå ord som «glad», «lykkelig», «sint». Smil bør heller ikke presenteres som følelse (se punkt 6).

## 5. Feilmodi og skjevhet

Forventede feilmodi (alle **ikke målt** her, bare dokumentert som risiko): profil og delvis skjulte ansikter, solbriller og briller med refleks,
hodeplagg og munnbind, små ansikter (modellen er trent for ca. 10–300 px), barn og spedbarn, dårlig lys, motlys, rask bevegelse, tegninger/dukker (falske treff),
folkemengder (mange små treff, se 32 treff over), naturlig smale øyne som feiltolkes som lukkede, blunking midt i bevegelse.

Skjevhet: WIDER Face-AP er ikke stratifisert på hudtone, alder eller kjønn. Vi har ingen demografisk evaluering og skal **ikke** påstå
nøyaktighet vi ikke har målt, hverken i UI, README eller markedsføring. Før øyne/smil kan leveres kreves et evalueringssett med samtykke,
rapportering per undergruppe (hudtone, alder, briller), og en terskel der appen heller sier ingenting enn feil. Datasett til dette (f.eks. FairFace) er ikke verifisert for lisens og bruk.

## 6. Personvern og regulatorisk (ikke juridisk rådgivning, må bekreftes av jurist)

- Alt lokalt. Ingen bilder eller utledede data forlater enheten. Ingen nye nettverkskall (verifisert at tract-treet ikke har nettverkskrates).
- Kun deteksjon + landemerker + øyetilstand. **Ingen embedding, ansiktsmal, klyngebygging eller identifikasjon.** Resultater som lagres i indeksen
  er boks, landemerker og tilstandsflagg per bilde (ikke per person), og kan slettes med indeksen.
- GDPR art. 9: biometriske opplysninger er de som behandles for å **identifisere** en person entydig. Ren deteksjon uten identifikasjon er etter vår lesing
  trolig utenfor, men dette er ikke avklart, og lagring av landemerker bør vurderes som potensielt sensitivt (ikke verifisert).
- Fjell er lokal programvare og samler ikke inn data (`docs/PERSONVERN.md`); brukerens egne bilder faller normalt under privatbrukerunntaket. Ikke verifisert juridisk.
- EU AI-forordningen har forbud/krav rundt emosjonsgjenkjenning og biometrisk kategorisering. Smil-/uttrykksmerker kan komme nær dette.
  Anbefaling: ingen følelsesmerking; bare «smil» som geometrisk trekk, og først etter juridisk vurdering. Ikke verifisert.
- Modell- og bibliotekslisenser må inn i `THIRD_PARTY.md`. Modellfiler er ikke crates, så cargo-deny dekker dem ikke.

## 7. Anbefaling og innsats

1. **Fase A (anbefalt, ca. 5–8 dagsverk):** YuNet + tract i kjernen som bakgrunnsjobb på kandidatgrupper. Merker: «Ansikter funnet», «Ansikt i bildet».
   Letterbox/skalering, release-måling på Windows, tråder, THIRD_PARTY, nonet-/deny-sjekk, nb/en-tekster, tester.
2. **Fase B (spike 2, ca. 10–15 dagsverk, usikker):** konverter MediaPipe Landmarker/Blendshapes til ONNX, test i tract, bygg evalueringssett,
   mål nøyaktighet per undergruppe. Gå videre bare hvis målt nøyaktighet er akseptabel og juristen sier ok.
3. Ikke ort/Windows ML i MVP: ekstra DLL/system-avhengigheter, byggrisiko på windows-latest og svakere personvernhistorie (nedlasting ved bygg).

Risiko: Windows-bygg av tract ikke verifisert; tract-operatorstøtte for konverterte TFLite-modeller ukjent; ytelse uten release og uten Windows-maskin ukjent;
lisens for konvertert modell uavklart; juridisk status for landemerkelagring og smil uavklart; ingen målt skjevhetsprofil.

## 8. Ikke verifisert (samlet)

Windows-bygg (`windows-latest`), release-størrelse og -ytelse, parallell skalering på 4 kjerner, nøyaktighet på norske/egne bilder, nøyaktighet per hudtone/alder,
UltraFace-nøyaktighet og kjøring, MediaPipe i tract, lisens for modellkort og konvertering, ort-/DirectML-størrelser, cargo-deny advisories, juridisk vurdering (GDPR art. 9, AI-forordningen),
lisens for testbildet (kun lokal test, ikke i git).
