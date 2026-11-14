# Utgivelse: Windows-installasjon, signering og versjonering

Status: beslutningsgrunnlag for MVP-PLAN åpne spørsmål 11–13. **Ingenting her er bygget eller signert på Linux**; NSIS-bygget kjøres i CI (`windows-latest`, jobb `desktop`) og er kun røyktestet der. Priser er veiledende (2025/2026, USD), må bekreftes mot leverandør før kjøp.

## 1. Anbefaling (kort)

| Valg | Anbefaling for beta og 1.0 |
|---|---|
| Format | **NSIS per bruker** (`installMode: currentUser`, ingen administrator). MSIX vurderes etter 1.0 (Microsoft Store). |
| WebView2 | Bootstrapper i installasjonsprogrammet (`embedBootstrapper` i dag). Offline-variant bare for bedrifter/isolerte maskiner. |
| Signering | **Azure Trusted Signing** (hvis kvalifisert utgiver), ellers OV-sertifikat i skybasert HSM. Usignert bare i intern alfa. |
| Oppdatering | Beta-kanalen oppdaterer fra en rullerende GitHub Release («beta»); brukeren trykker selv «Installer og start på nytt» (aldri automatisk installasjon). Se `docs/OPPDATERING.md`. Uten kanal: nytt installasjonsprogram over gammelt. |
| Versjonering | SemVer `MAJOR.MINOR.PATCH`, én kilde, `0.x` til brukbarhetsmålene i BETA.md er nådd. |

## 2. NSIS mot MSIX

| Kriterium | NSIS (Tauri-bundler) | MSIX |
|---|---|---|
| Installasjon uten admin | Ja (`currentUser`) | Ja per bruker via sideloading, men krever aktivert sideloading eller signert pakke |
| Filsystemtilgang | Full (appen er vanlig Win32) | Pakkeidentitet; broad file access (`broadFileSystemAccess`) må søkes/begrunnes, virtualisering av `%LOCALAPPDATA%` kan flytte indeksen og `.fjell-karantene`-semantikken. **Stor risiko for en bildeapp som flytter filer på eksterne disker og UNC.** |
| Nettverksstasjoner/UNC, eksterne disker | Uproblematisk | Må vurderes per capability |
| Tauri-støtte | Innebygd (`--bundles nsis`) | Ikke innebygd i Tauri 2-bundleren (eksternt verktøy/`makeappx`) |
| Oppdateringer | Egen løsning: beta-kanal via GitHub Release (`docs/OPPDATERING.md`) | Store/App Installer gir det gratis, men er et nettverkskall utenfor appen |
| Avinstallasjon | Programmer og funksjoner, beholder brukerdata (vår innstilling) | Rent, men brukerdata følger pakken |
| SmartScreen | Trenger signering og omdømme | Signert pakke/Store gir bedre omdømme |
| Distribusjon | Nedlasting fra egen side | Store (gebyr, gjennomgang) eller sideload |

**Beslutningsforslag:** NSIS nå. MSIX når filtilgang er avklart (ARKITEKTUR åpne punkter: skyspeilede mapper, backup-ekskludering). Åpent spørsmål 12 til produkteier.

### Dagens oppsett (`desktop/src-tauri/tauri.conf.json`)

`targets: ["nsis"]`, `installMode: currentUser`, `compression: lzma`, `displayLanguageSelector: false`, `webviewInstallMode: embedBootstrapper`.
- **Avvik fra ARKITEKTUR beslutning 23** som sier `downloadBootstrapper`. `embedBootstrapper` legger Microsofts lille bootstrapper (ca. 1,8 MB) inn i installasjonsprogrammet, så pakken fungerer uten ekstra nedlasting av bootstrapperen, men bootstrapperen trenger fortsatt nett for å hente WebView2 hvis det mangler. Oppdater ARKITEKTUR eller konfig slik at de sier det samme.
- Språk i installasjonsprogrammet: Tauri/NSIS gir språkvelger; satt av (følger Windows). Norsk bokmål finnes i NSIS-språkpakkene og bør slås på før beta (ikke gjort, ikke verifisert).

## 3. WebView2

| Spørsmål | Svar |
|---|---|
| Hva er det | Edge/Chromium som systemkomponent. Appen bundler ikke Chromium (liten pakke). |
| Windows 11 | Forhåndsinstallert. |
| Windows 10 22H2 | Normalt installert via Edge/Windows Update, men ikke garantert på ferske eller isolerte maskiner. |
| Installasjonsmodi i Tauri | `downloadBootstrapper` (henter ved behov), `embedBootstrapper` (bootstrapper inni pakken; **valgt nå**), `offlineInstaller` (Evergreen-installasjon inni pakken, **+ ca. 130 MB**), `fixedRuntime` (fast versjon, ca. 180 MB, vi vedlikeholder oppdateringer selv), `skip`. |
| Anbefaling | `embedBootstrapper` som standard; bygg i tillegg en egen offline-variant (`offlineInstaller`) for bedrifts-/offline-maskiner. |
| Oppdatering | WebView2 Evergreen oppdateres av Microsoft; det er **ikke** Fjell som snakker med nettet. Forklares i [PERSONVERN.md](PERSONVERN.md). |
| Skjermleser | Narrator/NVDA mot WebView2 må testes manuelt hver beta ([BETA.md](BETA.md)); versjonsforskjeller i Chromium kan endre oppførsel. Logg WebView2-versjon i hver testrunde. |
| Minstekrav | Windows 10 versjon 1809 eller nyere for Evergreen; vi støtter Windows 10 22H2 og 11 (forslag, åpent spørsmål 13). |

## 4. Kodesignering: alternativer og kostnader

Usignerte installasjonsprogrammer gir SmartScreen-advarsel («Windows beskyttet PC-en din») og noen antivirus-treff. Siden juni 2023 må OV/EV-nøkler ligge på maskinvaresikre moduler (HSM/token), så «PFX-fil i CI» er ikke lenger mulig for nye sertifikater.

| Alternativ | Veiledende pris | SmartScreen | Praktisk | Merknad |
|---|---|---|---|---|
| **Azure Trusted Signing** (Microsoft) | ca. 10 USD/mnd (Basic) | Bygger omdømme raskt; ingen «EV-bonus», men korte sertifikater og tilgjengelig for CI | Bare for organisasjoner/utviklere som oppfyller Microsofts identitetskrav (tilgjengelighet per land må sjekkes; Norge er ikke bekreftet her) | Beste pris/ytelse hvis kvalifisert |
| **OV-sertifikat** (Sectigo, DigiCert, SSL.com m.fl.) | ca. 200–450 USD/år | Omdømme må bygges opp over tid (uker til måneder) | Lagres på token eller skybasert HSM; CI via leverandørens skysigner | Billigst for vanlig sertifikat |
| **EV-sertifikat** | ca. 350–700 USD/år | Ingen automatisk umiddelbar SmartScreen-fritak lenger, men sterkere start | Maskinvaretoken eller skysigner | Kun hvis OV gir for treg start |
| **Skysignering** (SSL.com eSigner, DigiCert KeyLocker, Certum cloud) | ca. 20–100 USD/mnd eller inkludert | som sertifikattypen | Egnet for GitHub Actions | Hemmeligheter i CI (legges i repo-secrets, aldri i repo) |
| **Microsoft Store (MSIX)** | engangsgebyr ca. 19 USD (privat) / 99 USD (firma) | Microsoft signerer; ingen SmartScreen-advarsel | Krever MSIX og Store-gjennomgang | Se avsnitt 2 om filtilgang |
| **Usignert** | 0 | Advarsel; noen blokkerer | Bare intern alfa og nærmeste testere med instruks | Ikke til beta ute |

Krav uansett: tidsstempling (RFC 3161), SHA-256, signer **både** installasjonsprogrammet og `fjell.exe`/uninstaller. I Tauri: `bundle.windows.signCommand` eller `certificateThumbprint` + `timestampUrl`. **Tidsstempel-URL er et nettverkskall i byggeprosessen, ikke i appen.**

Åpent spørsmål 11 til produkteier: hvem eier sertifikatet/kontoen, og når (senest før beta). Ikke avklart; CI bygger usignert (`desktop.yml`).

## 5. Versjonering

- **SemVer**: `MAJOR.MINOR.PATCH`. `0.y.z` til 1.0. Beta merkes `0.y.0-beta.n` (NSIS krever numerisk fil-/produktversjon; bruk `0.y.0` i installasjonsprogrammet og la `-beta.n` stå i filnavnet/release-navnet).
- **Én kilde:** `desktop/src-tauri/tauri.conf.json` `version` styrer produktversjonen; `Cargo.toml` (`fjell-desktop`) og `desktop/package.json` holdes lik (i dag alle `0.0.0`). Bump alle tre i samme commit og tagg `v0.y.z`. Foreslått test: Vitest som sjekker at de tre er like (ikke bygget ennå).
- **Algoritmeversjon** `ALGO_VER` (core) er **uavhengig** av appversjonen og endres bare når hash/kvalitetsalgoritmer endres (ARKITEKTUR 14). Frosset i M8 (K8).
- **Databaseskjema** (`PRAGMA user_version`) migreres framover automatisk. Ingen nedgradering: nyere indeks i eldre app åpnes ikke, og brukeren får beskjed uten tap.
- **Endringslogg:** `CHANGELOG.md` (ikke opprettet ennå) med brukervendt norsk tekst. Ingen «sendt til server»-språk.
- **Oppdatering:** bare på brukerens kommando (beta-kanalen, `docs/OPPDATERING.md`), aldri automatisk installasjon. Alternativt laster brukeren ned nytt installasjonsprogram; installasjon over gammel versjon beholder brukerdata (BETA.md I12).

## 6. Utgivelsessjekkliste

1. Grønne: `just check`, `just desktop-check`, `just third-party-check`, `just contrast`, `npx playwright test` (axe, ytelse, UX-6, nettfri), CI `desktop` på `windows-latest`.
2. Versjon bumpet og tagget; `THIRD_PARTY.md` regenerert (`just third-party`).
3. NSIS bygget i CI, signert (når avklart), røyktestet (stille installasjon, start, avinstaller).
4. [BETA.md](BETA.md) punkt 1–7 gjennomgått på referansemaskin; resultat arkivert.
5. [PERSONVERN.md](PERSONVERN.md) stemmer med hva appen faktisk lagrer (BETA.md I6/I7).
6. SHA-256 av installasjonsprogrammet publiseres ved siden av nedlastingen.
7. Kjente begrensninger skrives i utgivelsesnotatet: HEIC/RAW krever Windows-kodek, nettverksstasjoner kan gi «Merket, ikke flyttet», usignert gir SmartScreen-advarsel.
