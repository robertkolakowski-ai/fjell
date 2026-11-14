# Endringslogg

Alle merkbare endringer i Fjell skrives her. Formatet følger [Keep a Changelog](https://keepachangelog.com/no/1.1.0/), versjonene følger [SemVer](https://semver.org/lang/nb/) (`0.y.z` til 1.0; se `docs/RELEASE.md` del 5). Teksten er for brukere og testere: hva appen gjør, ingen «sendt til server»-språk, for Fjell sender ingenting.

Utgivelsesworkflowen (`.github/workflows/release.yml`) henter utdraget til GitHub Release fra seksjonen som heter som versjonen, ellers fra `[Ikke utgitt]`. Prosess: `docs/RELEASE-PROSESS.md`.

## [Ikke utgitt]

Status per 2026-11-12 (MVP-PLAN 4b): alt er målt på Linux. Windows-koden og selve appen er ikke kjørt på Windows ennå; se «Kjent» nederst.

### Lagt til

- **Fundament (M0).** Repo-skjelett, CI med fmt, clippy og tester, lagsjekk (`check-layers`) som feiler på bevisst brudd, og `cargo deny` på kjernen.
- **Bibliotek og testdata (M1).** Deterministisk testkorpus med manifest, databasemigrering som gir lik struktur fra tom og fra forrige versjon, og en test som beviser at originalfiler ikke endres.
- **Gjenkjenning (M2).** Eksakte duplikater, visuelt like bilder (hash), uskarphet og skjermbilder, med en begrunnelse per forslag. Kjernen bruker omtrent 0,4 ms per bilde.
- **Trygg sletting (M3).** Alt går via appens Papirkurv (30 dager, kan endres), alt kan angres, og ingenting kan permanent slettes uten bekreftelse. Avgjørelser og angring tok under 2 ms (p95) på 50 000 rader.
- **Skanning og kommandolinje (M4).** Skanning av en mappe med fremdrift, inkrementell skanning, og CLI for skann, handling, angre og tømming. 10 000 bilder skannes på ca. 4 sekunder.
- **Windows-adapter og skall (M5).** Filsystemadapter for Windows (kode, bare type-sjekket mot Windows) med Papirkurv (IFileOperation), stasjoner, lange stier og UNC; karantenemappe med manifest og avstemming; Tauri-skall som kaller kjernen (skallets Tauri-del er ikke kompilert ennå).
- **Grensesnitt (M6).** Designsystem med tre temaer (lys, mørk, høykontrast) der alle 81 fargepar består WCAG AA, og felles komponenter.
- **Skjermer (M7).** Velkomst, Hjem, gjennomgang av grupper, kortmodus, Papirkurv. Norsk bokmål og engelsk. Tastatur, mus og knapp gir samme resultat. Handling til skjerm tok ca. 15 ms (p95, i testmiljø uten Tauri).
- **Tilgjengelighet og herding (M8).** Automatiske tester for høykontrast, redusert bevegelse, skalering 100/150/200 % og lang tekst (pseudo-språk); axe gir 0 brudd i lys/mørk × bred/smal. Windows-installasjonsprogram (NSIS, per bruker, uten administrator). Dokumentene `BETA.md`, `RELEASE.md`, `PERSONVERN.md` og `THIRD_PARTY.md`.
- Utgivelsesoppsett: workflow for utkast til GitHub Release med sjekksum, valgfri signering, og `docs/RELEASE-PROSESS.md`.

### Endret

- Plattform for MVP er Windows-PC (Windows 10/11, Tauri 2); iPhone og Android kommer etter MVP (beslutning 2026-11-11).
- Originalfilers innhold endres aldri, men filer flyttes til appens karantenemappe og kan angres. «Tøm» sender til Windows Papirkurv som standard.
- Duplikater forhåndsmerkes (alt unntatt det som beholdes). Beskyttede bilder forhåndsmerkes aldri.
- Nynorsk utsatt til etter MVP (stub med fallback til bokmål).

### Kjent

- Ikke kjørt på Windows: Papirkurv, lange stier, UNC, stasjoner, NSIS-installasjon og selve Tauri-appen. Narrator og NVDA er ikke prøvd; `BETA.md` er uutfylt.
- Uskarphetsalgoritmen: recall for visuelle duplikater med uskarpe varianter er 0,891 (mål 0,90) og er bare testet på syntetiske bilder. `ALGO_VER` er ikke frosset, så eldre indekser kan ha utdaterte uskarphetsverdier.
- Biblioteket har én mappe; «Legg til mappe» erstatter forrige. Mappevelger er ikke koblet (stien skrives inn). «Pause» avbryter og starter på nytt. Ingen miniatyrer i Papirkurv.
- Ett åpent tilgjengelighetsfunn: statusmelding i gjennomgang settes inn med ferdig tekst (`aria.spec.ts`, forventet feil).
- Ikke signert: SmartScreen viser advarsel til signering er avklart (åpent spørsmål 11).
- Ytelsesmål (kald oppstart under 1,5 s, 10 000 bilder under 10 min, handling under 100 ms i ekte app) er ikke målt på Windows.
