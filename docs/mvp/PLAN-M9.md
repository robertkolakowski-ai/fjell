# M9: Windows-verifisering og ekte data

Mål: gå fra «kode, testet på Linux» (4b) til «målt på Windows med ekte bilder». Ingen ny funksjonalitet utover kobling og feilretting. Status per punkt står i `MVP-PLAN.md` 4b/4c; her er bare planen. Alt under er plan, ingenting er kjørt.

## 1. Akseptkriterier

| # | Kriterium | Hvor | Lukker (4c) |
|---|---|---|---|
| A1 | `fjell-fs` og `tests/windows.rs` grønne på `windows-latest` (Papirkurv, lange stier, UNC, stasjoner, karantene T25–T28) | CI | 1 |
| A2 | `tauri build` og `tauri dev` fungerer; CSP uten `connect-src 'self'` bekreftet mot HMR | CI + manuelt | 2 |
| A3 | NSIS bygges, stille installasjon, start, avinstaller (røyktest); norsk språk i installasjonsprogrammet sjekket | CI | 3 |
| A4 | Fil sendt til Papirkurv finnes faktisk der (manuell kontroll, 3 filer; `WINDOWS-TEST.md` del 3) | Produkteier | 1 |
| A5 | `FsSource::with_known_ids` og `thumbcache` koblet i skallet, med test | Agent A | 12 |
| A6 | `just bench` og Playwright som CI-jobber (`nightly`); deny for skallet med egen konfig | Agent D | 13, 14 |
| A7 | Handling p95 < 100 ms og kald oppstart < 1,5 s i ekte Tauri; 10 000 bilder < 10 min; RSS-topp rapportert | Referansemaskin | 5 |
| A8 | Hash-recall med uskarpe varianter ≥ 0,90 på ekte, etikettert korpus; deretter `ALGO_VER = 1` frosset | Agent B + ekte bilder | 6, 7 |
| A9 | Narrator og NVDA, skalering 100–200 %, høykontrast: `BETA.md` avkrysset (kortversjon: `WINDOWS-TEST.md`) | Produkteier | 4 |
| A10 | Brukbarhetstest med 5 personer; onboarding ≤ 60 s | Produkteier | 8 |
| A11 | Signeringsbeslutning tatt og dokumentert (ellers lukket beta, usignert) | Produkteier | 9 |
| A12 | Skymapper (iCloud/OneDrive/Dropbox for Windows): skann og forslag virker, flytting til papirkurv blokkert, plassholdere hoppes over; prøvd mot ekte synkmappe (i dag bare Linux-test av mappenavn) | Produkteier / Windows-maskin | spørsmål 14 |

Felles: `just check` grønn, én gren og én PR per agent, skjermbilder ved UI-endring, ingen test svekkes uten begrunnelse i PR.

## 2. Agentgraf

```
        [orkestrator/integrator]  eier MVP-PLAN.md, PLAN-M9.md, ARKITEKTUR.md, VEIKART.md
           |
   +-------+--------+---------+---------+
   A        B         C         D         E (dette dokumentet)
 skall    kalibr.   Windows   CI/deny   docs
 kobling  ekte bilder  fs-test  bench
   \        |         |         |
    +-------+----+----+---------+--> integrasjon -> windows-latest -> produkteier
```

| Agent | Oppgave | Eier (kun disse filene) | Kan starte |
|---|---|---|---|
| A | Skallkobling: `with_known_ids`, `thumbcache`, CSP/HMR, språklagring i skallet, skysperre | `desktop/src-tauri/**` (skall `shell/` og Tauri-lim `src/`; ikke `.github/**`, ikke deny) og broen i `desktop/src/bridge/**`, `desktop/src/lib/i18n/**` | Nå |
| B | K8-kalibrering mot ekte korpus, frysing av `ALGO_VER`, reindeksering (4c nr. 7) | `core/**` (kvalitet/hash), `testdata/real/**` (manifest, ikke bildene) | Skjelett nå; selve kalibreringen blokkert av ekte bilder |
| C | Windows-tester: lange stier, UNC, stasjoner, Papirkurv; `tests/windows.rs`; skynavn-sikkerhetsnett (`cloud.rs`) | `core/crates/fjell-fs/**` | Nå (skrives uten å kunne kjøres lokalt) |
| D | CI: `desktop.yml`, bench, Playwright, `nightly`, `deny.toml` for skallet, røyktest for NSIS | `.github/**`, `desktop/src-tauri/deny.toml`, `justfile` | Nå |
| E | Dokumentasjon | `docs/**` (ikke `docs/screens/**`, ikke `K8-ANALYSE.md`, ikke `KALIBRERING.md`), `README.md` | Nå |

### Faktisk filfordeling i denne fasen (arbeidstreet, ikke committet)

Planen over var grovere enn det som ble gjort. Slik ble det per område, etter `git status` 2026-11-12:

| Område | Filer som faktisk er endret eller nye |
|---|---|
| Kjerne | `core/crates/fjell-fs/src/cloud.rs` (ny: `CloudProvider`, `cloud_provider`), `core/crates/fjell-fs/src/lib.rs` (eksport). Ingen endring i `fjell-core`, hash eller kvalitet. |
| Skall | `desktop/src-tauri/shell/src/{dto,lib,tests_core}.rs`: `StateDto.cloud_sync`, `Shell::cloud_sync`, avvisning av papirkurv-beslutning i skysynket bibliotek (feilkode `cloud_synced_readonly`), tester |
| Tauri-lim | `desktop/src-tauri/src/lib.rs`: kommandoene `get_language`/`set_language` (speiler eksisterende mønster; **ikke kompilert**, mangler GTK) |
| UI/bro | `desktop/src/bridge/{core-bridge,tauri-bridge}.ts`, `bridge.test.ts`, `desktop/src/lib/i18n/index.ts`, `language-store.test.ts` (språk lagres i skallet). Ingen skjerm viser skyadvarselen ennå: ingen `.svelte`-fil leser `cloud_sync`. |
| CI | Ingen `.github/**`-endring i denne fasen utover forarbeidet i commit `db40adb` |
| Docs | `docs/mvp/WINDOWS-TEST.md` (ny), denne filen, `MVP-PLAN.md` (spørsmål 14), `PERSONVERN.md`, `BETA.md` |

Agent B (kalibrering, `K8-ANALYSE.md`, `KALIBRERING.md`) hører ikke til disse filene. Ekte iCloud, OneDrive og Dropbox er ikke prøvd; skynavn-sperren er en navneheuristikk testet bare på Linux.

Delte filer (kun orkestrator): `model`, `api`-flaten, migrering, `ARKITEKTUR.md`. Rekkefølge ved konflikt: D før A (CI må finnes for å teste A), C uavhengig, B sist. Windows-agenter kan ikke kjøre sin egen kode lokalt: de skriver, typesjekker mot `x86_64-pc-windows-gnu` og leverer til `windows-latest`. Rødt der er et funn, ikke noe å skjule.

## 3. Hvor hva kjøres

| `windows-latest` (automatisk) | Manuelt av produkteier | Annet |
|---|---|---|
| A1, A2, A3, A6 (bygg, tester, røyktest, deny) | A4 Papirkurv-kontroll | Windows 10 22H2: egen maskin eller VM (ikke dekket av `windows-latest`) |
| Delvis A7: bench på delt runner gir bare trend, ikke referansetall | A7 på referansemaskin, A9, A10 | Signering (A11) prøves først etter beslutning |

## 4. Blokkert av produkteier

| Blokkert | Hva som blokkerer | Agent |
|---|---|---|
| A7, A9 | Referansemaskin (åpent spørsmål 8) | alle |
| A8 | Ekte bilder | B |
| A10 | Testpersoner | produkteier |
| A11, signert bygg | Signeringsbeslutning (spørsmål 11) | D |
| NSIS-endelig, A3 | Ja til anbefalingene 12–14 | D |

Uten ekte bilder kan B bare forberede verktøy; ikke kalibrer mot syntetiske motiver og kall det ferdig.

## 5. Dette må produkteier levere

1. **Referansemaskin:** Windows 11, 8 kjerner, 16 GB, NVMe (forslag fra spørsmål 8); hvem eier den; om den kan være self-hosted runner for `nightly`.
2. **Ekte bilder:** minst 20 per format (JPEG, HEIC, PNG, RAW), pluss kjente duplikater, uskarpe og skjermbilder, med etikett. Egne eller samtykkede. Bildene skal ikke inn i repoet uten avtale; manifest og sjekksummer kan.
3. **Fem testpersoner** for brukbarhetstest, helst også én med skjermleser og én på Windows 10.
4. **Signeringsbeslutning:** sertifikat eller tjeneste, eier og frist (anbefaling i `MVP-PLAN.md` del 6, nr. 11).
5. **Ja eller nei** til anbefalingene 12–14 og deny-konfig for skallet, og svar på åpne spørsmål 5–9, 15–17.
6. Windows-maskin (eller tid) for Narrator/NVDA-avkrysning i `BETA.md`.

## 6. Rekkefølge og risiko

Uke 1: A, C, D parallelt; B skjelett. Uke 2: integrasjon, første grønne `windows-latest`-kjøring (forvent røde funn). Uke 3: rettinger; referansemaskin og ekte bilder inn; B kalibrerer. Uke 4: manuell avkrysning, brukbarhetstest, frys `ALGO_VER`. Anslag, ikke målt.

Største risiko: Windows-koden er aldri kjørt (Papirkurv via IFileOperation, UNC, WebView2). Plan for slik: små PR-er, tidlig CI-kjøring, og at integratoren justerer 4b/4c etter hva CI faktisk viser.
