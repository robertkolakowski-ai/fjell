# Utgivelse V1: endringslogg, sjekkliste og versjonsforslag

Status: utkast 2026-11-15, skrevet som forberedelse. **Ingenting er tagget, publisert, signert eller pushet, og ingen versjonsnummer er endret.** Utgivelse er en ytre handling Robert tar selv etter [RELEASE-PROSESS.md](../RELEASE-PROSESS.md). Alt er målt på Linux; **Windows, Narrator, NVDA, ekte modell og ekte kalibrering er ikke verifisert** (del 3).

Kilder: [V1-PLAN.md](V1-PLAN.md), [SKJERMER.md](SKJERMER.md), [MODELLER.md](MODELLER.md), [KALIBRERING-V1.md](KALIBRERING-V1.md), [WINDOWS-V1.md](WINDOWS-V1.md), [../RELEASE.md](../RELEASE.md), [../PERSONVERN.md](../PERSONVERN.md).

## 1. Endringslogg for V1

### Brukervendt (kan limes inn i `CHANGELOG.md` ved utgivelse)

**Nytt**

- **Serier.** Bilder som er nesten like og tatt tett i tid samles i en serie. Fjell foreslår beste bilde og forklarer hvorfor (skarphet). Du kan selv velge et annet som beste («Gjør dette til beste»), og sammenligne bildene med felles zoom.
- **Beskyttelse.** Du kan beskytte enkeltbilder, mapper, album eller personer. Beskyttede bilder krever alltid en egen bekreftelse før de flyttes til «Nylig slettet».
- **Personer (valgfritt, av som standard).** Hvis du selv slår på ansiktsindeksen og bekrefter, kan Fjell gruppere bilder etter hvem som er på dem, og du kan navngi, slå sammen og beskytte personer. Alt skjer på din PC. Se personvern under.
- **Regler.** Lag egne regler, for eksempel «skjermbilder eldre enn 90 dager», og se forhåndsvisning av treff. Regler foreslår bare; ingenting slettes automatisk.
- **Oversikt (Hjem).** Bibliotek, forslag, papirkurv og tømt plass, regnet direkte fra indeksen på maskinen.
- **Følsomhet.** Velg Lav, Normal eller Høy per forslagstype. Normal er lik MVP.
- **Forklaring på hvert kort** viser hvorfor Fjell foreslår det, også for serier og regler.
- Norsk bokmål og engelsk for alle nye skjermer.

**Uendret og fortsatt gjeldende:** alt som slettes går via Nylig slettet (30 dager, kan endres) og kan angres; originalfiler endres aldri; «Tøm» sender til Windows Papirkurv som standard; ingen bilder forlater maskinen; ingen telemetri.

**Personvern.** Ansiktsindeksen er av til du slår den på. Den lagrer tallfingeravtrykk (ikke ansiktsbilder) og navn lokalt. «Slett ansiktsindeksen» fjerner alt, **kan ikke angres**, og bildene beholdes. Se [PERSONVERN.md](../PERSONVERN.md).

### Teknisk

- **Kjerne (`core/`):** nye moduler `protect`, `series`, `people`, `suggest_rules`, `dashboard`; migrering `005_v1.sql` (tabellene `protection`, `eye_result`, `person`, `face_embedding`, `rule`, `rule_hit`; `grp.kind` og `suggestion.kind` utvidet via trygg tabellomskriving; lik dump fra tom og fra forrige). Databaseskjema går bare framover; ingen nedgradering.
- **Forslagstyper:** seks (eksakt, visuell, serie, regel, skjermbilde, uskarpt); kortprioritet i `store/cards.rs` sørger for at samme bilde aldri vises i to kort. Fem nye begrunnelseskoder og `ConsequenceCode::ProtectedByScope`.
- **Sikkerhetsrettelse under integrasjonen:** bekreftelsesporten (`needs_ack` i `api/fjell_ops.rs`) gjelder nå beskyttede bilder både ved beslutning og ved permanent tømming. Før dette kunne en beskyttet kandidat flyttes uten bekreftelse. Testet.
- **Pipeline:** `refresh_groups` regner serier og regler etter duplikatgrupper; `run_analyzers` kjører øyne og (bare med ansiktsindeks på) ansikter. `Unavailable` avbryter steget uten å lagre noe.
- **`fjell-models`:** ramme for lokale ONNX-modeller (manifest, lisens-tillatliste, størrelse ≤ 30 MB, BLAKE3-kontroll, feiler lukket). **Ingen modell og ingen kjøremotor er inkludert**; `Null`-analysatorene gjelder.
- **Kalibrering:** verktøy og golden-tester per forslagstype i `core/testdata` (syntetisk korpus). Standardterskler er uendret.
- **Skall og skjermer:** V1-kommandoer i Tauri-skallet, `CoreBridge`/`TauriBridge`/`MockBridge` med felles konformitetsfixtur mot både kjerne og mock; skjermer for serie, regler, personer, beskyttelser, innstillinger og Hjem.
- **UniFFI-speil (`fjell-ffi`)** av V1-flaten med paritetstest. Ingen iOS-/Android-kode.
- **Windows-verifiseringssett** (`WINDOWS-V1.md`, e2e-jobbene `v1-mock` og `v1-shell`): skrevet, ikke kjørt.
- `ALGO_VER` er uendret (1).

## 2. Sjekkliste mot RELEASE-PROSESS og RELEASE del 6

Status: [x] gjort, [ ] gjenstår, [~] delvis. «Linux» betyr målt i utviklingsmiljøet, ikke på Windows.

| # | Punkt | Status | Merknad |
|---|---|---|---|
| 1 | Kjernen grønn (`fmt`, `clippy -D warnings`, `cargo test --workspace --locked`, `check-layers`, `gen_i18n --check`, `cargo deny`) | [~] | Kjørt på Linux ved V1-fullføring; **kjøres på nytt rett før tagging**. Denne dokumentfasen endrer ingen kode. |
| 2 | Frontend grønn (`npm run check`, `npm test`, `contrast`, `build`, Playwright inkl. axe, ytelse, nonet) | [~] | Som over: Linux/Chromium med MockBridge. |
| 3 | CI `desktop` på `windows-latest` inkl. skallet (`fjell-desktop`) | [ ] | Skallet kan ikke bygges på Linux (GTK); bare gjennomgått. Jobbene `v1-mock` og `v1-shell` er ikke kjørt. |
| 4 | Versjon bumpet i alle tre filer (`tauri.conf.json`, `desktop/src-tauri/Cargo.toml` + `Cargo.lock`, `desktop/package.json` + lock) | [ ] | Alle er `0.0.0` nå (og `core/Cargo.toml`). **Ikke bumpet her**, per oppdraget. Forslag i del 5. |
| 5 | `THIRD_PARTY.md` regenerert (`just third-party`) | [ ] | Gjøres i versjonsbump-committen. Lisenstekster uten nettilgang er ikke verifisert her. |
| 6 | `CHANGELOG.md`: seksjon `[Ikke utgitt]` omdøpt til versjonen, V1-teksten (del 1) limt inn, «Kjent» oppdatert | [ ] | `CHANGELOG.md` har fortsatt bare MVP-innhold og er **ikke endret** her (utenfor omfanget). |
| 7 | Merge til main via PR (én fase = én PR) | [ ] | Robert. |
| 8 | Tagg `vX.Y.Z` og push | [ ] | **Ikke gjort. Robert.** |
| 9 | Release-workflow (`prepare`, `build`, `sign`, `release`) kjørt og utkast gjennomgått | [ ] | Workflowen er aldri kjørt i GitHub (RELEASE-PROSESS del 5). |
| 10 | **Signering** (åpent spørsmål 11): tjeneste og eier valgt; secrets/variabler (`AZURE_*`, `TRUSTED_SIGNING_*`) opprettet; `fjell.exe` og avinstaller signert via `signCommand` | [ ] | Ikke avklart. Uten dette: usignert, SmartScreen-advarsel, bare lukket beta med navngitte testere. Ikke egnet for åpen beta eller «1.0» (RELEASE del 4). Handlingsversjonen `azure/trusted-signing-action@v0.5.1` må verifiseres. |
| 11 | **Manuell Windows-verifisering**: MVP-sjekklisten (`WINDOWS-TEST.md`, `BETA.md` 1–7) og V1 (`WINDOWS-V1.md`) | [ ] | Aldri kjørt på Windows. |
| 12 | **Narrator og NVDA** på alle V1-skjermer, høykontrast (de fire ekte temaene), skalering 100–200 % | [ ] | Bare emulert i Chromium (`forced-colors`, rot-skrift). Ikke verifisert. |
| 13 | **Ekte modell** (øye og ansikt): valgt av Robert (lisens, ≤ 30 MB, ONNX), kjøremotor bak `onnx` som passerer `cargo deny` og `--locked`, per-modell-omformer, `THIRD_PARTY.md`, installasjonsløype for modellmappen, ev. økt `ALGO_VER` | [ ] | Ingen modell valgt, lastet ned eller inkludert. Personer er tom og øyesignal mangler. |
| 14 | **Ekte kalibrering** på Roberts testsett (`mvp/KALIBRERING.md`) | [ ] | Tall kommer i dag fra syntetisk korpus. Øyesikkerhet er en plassholder. |
| 15 | **Personvernerklæring oppdatert for ansiktsindeks** (`PERSONVERN.md`) | [x] | Utvidet 2026-11-15: av som standard, samtykke, hva som lagres, kun på enheten, wipe kan ikke angres, ikke kryptert. **Må leses på nytt når en ekte modell velges.** |
| 16 | SHA-256 av installasjonsprogrammet publiseres ved siden av nedlastingen | [ ] | Gjøres av workflowen; ikke sett i drift. |
| 17 | Røyktest av NSIS (stille installasjon, start, avinstaller) | [ ] | Bare i CI på Windows; ikke kjørt. |
| 18 | Ytelsesmål på ekte Windows (kald oppstart < 1,5 s, handlinger < 100 ms p95 i ekte app, ytelse på ekte bibliotek) | [ ] | Målt bare i testmiljø uten Tauri. |
| 19 | Åpne kjernefunn avgjort eller akseptert (del 4) | [ ] | Produktvalg for Robert. |
| 20 | Utkast gjennomgått, installert på ren Windows-maskin, så publisert manuelt | [ ] | Robert. Workflowen publiserer aldri selv. |

## 3. Hva som er verifisert og ikke

| Verifisert (Linux, syntetiske data) | Ikke verifisert |
|---|---|
| Kjernens logikk, migrering, trygghetsløype (karantene, angre, bekreftelsesport), golden-tester per forslagstype | Alt på Windows: Papirkurv (IFileOperation), lange stier, UNC, WIC (HEIC/RAW), NSIS, ekte Tauri-app og IPC |
| Frontend mot MockBridge i Chromium: axe, emulert høykontrast og skalering, tastatur, nonet, konformitet kjerne mot mock | Narrator, NVDA, ekte Windows-kontrasttemaer og skalering |
| `fjell-models`-rammen med mock-sesjon | Enhver ekte ML-modell, kjøremotor, presisjon og tid per bilde |
| Terskelkurver på syntetisk korpus | Presisjon og recall på ekte bilder; ekte harde negativer |
| Bridge-kontrakten og DTO-ene (skallets kommandolag mot ekte kjerne) | `fjell-desktop` (GTK-feil på Linux); V1-kommandoene i skallet er bare gjennomgått |

## 4. Kjente begrensninger i V1

Fra MVP (fortsatt): HEIC og RAW krever Windows-kodek og er aldri prøvd mot ekte filer; nettverksstasjoner kan gi «Merket, ikke flyttet»; usignert gir SmartScreen-advarsel; skymapper tillater skann men ikke flytting.

Nye eller bekreftet for V1:

- **Ingen ansikts- eller øyemodell.** Personer er tom også med indeksen på. Serier rangeres på skarphet alene, og `series.eyes.unavailable` vises. Bør stå øverst i utgivelsesnotatet.
- **Terskler er ikke kalibrert mot ekte bilder.** Standardene er uendret; anbefalingen High `blur_scale` 1,3 til 1,1 er ikke tatt inn.
- **Manuelt valgt beste i en serie** blir liggende; en senere øyeanalyse flytter det ikke (`apply_series` beholder lagret keeper).
- **Endret følsomhet** gir ny gruppering først ved neste skanning; endring av visuell følsomhet bygger klyngeindeksen på nytt (engangskostnad).
- **Mappebeskyttelse** følger hele stikomponenter, ikke tegn, og er ikke uavhengig av store/små bokstaver på Windows (`C:\bilder` beskytter ikke `C:\Bilder`): under-beskyttelse. Flyttes et bilde utenfor Fjell, ses det som «borte + ny» og er ubeskyttet. To mappebeskyttelser (keeper-regel `ProtectFolder` og `protection.folder`) lever side om side.
- **Regler:** `InFolder` er rent prefiks (over-beskyttende, trygg retning); «0 dager» godtas av kjernen; `listAlbums` mangler, så «i album» bør skjules i regelbyggeren i den ekte appen.
- **Bilder uten ansikter analyseres på nytt** ved hver kjøring med indeksen på (ingen `face_scan`-markør); må løses før en ekte modell.
- **«Tømt» i stedet for «frigjort»:** kjernen vet ikke om «Tøm» gikk til Windows Papirkurv eller var permanent, så Hjem viser «Tømt».
- **Wipe av ansiktsindeksen kan ikke angres** (bevisst, personvern). Indeksfilen er ikke kryptert.
- `protect_flags` bit 16 mangler i dokumentkommentaren (`api/types.rs`, `types.ts`).
- Fra MVP-loggen, ikke fulgt opp her: ett ukjent antall MVP-punkter i «Kjent» i `CHANGELOG.md` (ett bibliotek per nå, mappevelger, miniatyrer i Papirkurv) er ikke kontrollert på nytt i denne fasen.

## 5. Forslag til versjonsnummer

**Forslag: `0.2.0`** for første utgivelse som inneholder V1 (lukket beta, tag `v0.2.0-beta.1` hvis den går ut som beta; NSIS-versjonen er `0.2.0`, `-beta.1` står bare i taggen og release-navnet).

Begrunnelse:

1. RELEASE del 5: SemVer, `0.y.z` til brukbarhetsmålene i BETA.md er nådd. De er ikke målt, siden ingenting er kjørt på Windows, så `1.0.0` er ikke forsvarlig.
2. Alle tre versjonsfiler står på `0.0.0`; det har aldri vært en utgivelse. `0.1.0` er det naturlige første nummeret for MVP-flaten, men V1 er allerede integrert i samme gren, og én første utgivelse som bærer begge er enklere enn å lage en `0.1.0` ingen har testet. Er MVP ønsket ute først og alene, bruk `0.1.0` for den og `0.2.0` for V1.
3. V1 er en ny brukersynlig funksjonsflate (nytt skjema `005`, nye skjermer), altså MINOR, ikke PATCH.
4. Tallet er ikke et løfte om kvalitet: usignert, uten ekte modell og uten Windows-verifisering bør det merkes beta.

Ved bump (ikke gjort): `desktop/src-tauri/tauri.conf.json`, `desktop/src-tauri/Cargo.toml` (+ `Cargo.lock`), `desktop/package.json` (+ lock) i samme commit; `core/Cargo.toml` står også på `0.0.0` og bør avklares om den skal følge (RELEASE del 5 nevner bare de tre). `ALGO_VER` endres ikke med appversjonen; den økes først når en ekte modell eller ny hash-/kvalitetsalgoritme tas inn.

## 6. Beslutninger som venter på Robert

1. Signeringstjeneste, eier og frist (åpent spørsmål 11). Uten svar: usignert lukket beta.
2. Ekte modell for øyne og ansikter, og testsett for kalibrering.
3. Om V1 skal ut som beta uten modell (Personer tom, serier på skarphet), eller vente.
4. Versjonsnummer (del 5) og om MVP og V1 utgis samlet.
5. De kjernefunnene i del 4 som krever produktvalg.
