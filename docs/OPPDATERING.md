# Oppdatering (beta-kanalen)

Fjell for Windows kan oppdatere seg selv fra en rullerende beta-utgivelse på GitHub. Du slipper å laste ned installasjonsfilen hver gang.

Status: hele flyten i appen (sjekk, nedlasting, NSIS `/P /UPDATE`, ny versjon starter) kjører grønt i CI på `windows-latest` (`update-e2e.yml`, se [mvp/OPPDATERING-E2E.md](mvp/OPPDATERING-E2E.md)). På en ekte maskin har oppdatering og avinstallering likevel feilet flere ganger med «Kunne ikke avslutte Fjell»; årsaken er ukjent. Mottiltakene står under «Når appen ikke lar seg lukke». Første ekte oppdatering fra beta-kanalen skal fortsatt kontrolleres manuelt (se «Første gang» under).

## Slik virker det

1. Hver sammenslåing til `main` som rører `desktop/`, `core/`, `i18n/`, `tools/` eller `beta.yml` starter workflowen `.github/workflows/beta.yml` (kan også startes for hånd med «Run workflow»).
2. Workflowen setter versjonen til `0.1.<kjørenummer>` med `tools/set-version.mjs` (alltid stigende, ren numerisk semver slik at NSIS godtar den) og bygger en NSIS-pakke med `npx tauri build --bundles nsis`. Pakken og en `.sig`-fil signeres med den private oppdateringsnøkkelen fra GitHub-secreten `TAURI_SIGNING_PRIVATE_KEY`.
3. Røyktesten fra `desktop.yml` kjøres (stille installasjon, appen lever 10 s, avinstaller). Feiler den, publiseres ingenting.
4. Workflowen lager `latest.json` (versjon, endringsnotat fra git-loggen, tidspunkt, signatur og nedlastingsadresse for `windows-x86_64`).
5. Pakken, signaturen og `latest.json` lastes opp til GitHub Release med tag `beta` (prerelease, aldri draft). Releasen finnes bare én gang og overskrives; taggen flyttes til nyeste commit, og eldre pakker fjernes. Release-notatet viser versjon, commit og SHA-256.
6. Appen leser `https://github.com/robertkolakowski-ai/fjell/releases/download/beta/latest.json`. Er versjonen der høyere enn den installerte, tilbyr appen «Installer og start på nytt». Alt er offentlig, så ingen innlogging kreves.

Lokale og dev-bygg har versjon `0.0.0`, så de ser alltid en nyere beta. CI-bygg er `0.1.N`.

### Hva brukeren ser og kan styre

- Oppdateringssjekk skjer bare når du trykker «Se etter oppdatering», eller ved oppstart hvis du har slått på «Sjekk automatisk ved oppstart». Standard er **av**.
- Forespørselen sender kun appversjon og operativsystem (HTTP GET), aldri bilder, stier eller filnavn. Se [PERSONVERN.md](PERSONVERN.md).
- Fjell installerer aldri av seg selv: du må trykke «Installer og start på nytt».
- Før du trykker, sier appen fra om at Fjell lukkes mens den nye versjonen installeres, og starter igjen etterpå.
- Appen installerer ikke mens en skanning pågår, fordi omstart ville avbryte skanningen midt i arbeidet. Vent til skanningen er ferdig, eller avbryt den.

## Når appen ikke lar seg lukke

Symptom: installeren eller avinstalleren stopper med «Kunne ikke avslutte Fjell. Lukk den først og prøv igjen». Meldingen kommer fra Tauris NSIS-mal: den spør Windows Restart Manager om `fjell-desktop.exe` er i bruk og avbryter hvis Restart Manager ikke får lukket prosessen. Rotårsaken på maskinen der dette skjedde er ikke funnet, og feilen lar seg ikke reprodusere i CI. Vi har derfor to mottiltak som ikke avhenger av Restart Manager, og en reservevei.

### 1. Installeren avslutter Fjell selv (NSIS-hooker)

`desktop/src-tauri/installer-hooks.nsh` (koblet inn med `bundle.windows.nsis.installerHooks` i `tauri.conf.json`) definerer `NSIS_HOOK_PREINSTALL` og `NSIS_HOOK_PREUNINSTALL`. Malen (tauri-bundler 2.10.4, `installer.nsi`) setter dem inn som første steg i `Section Install` og `Section Uninstall`, **rett før** `CheckIfAppIsRunning`, altså før sjekken som gir feilmeldingen. Hookene:

- kjører `taskkill /F /IM fjell-desktop.exe` i inntil 5 forsøk med 0,5 s mellom (ferdig så snart ingen slik prosess finnes),
- avslutter gjenværende `msedgewebview2.exe` som tilhører Fjell (kommandolinjen har `--webview-exe-name=fjell-desktop.exe` eller peker på `no.fjell.desktop\EBWebView`), aldri WebView2-prosesser fra andre apper,
- feiler aldri installasjonen: finnes ingen prosess, eller er `taskkill`/PowerShell sperret, går installeren videre til malens vanlige sjekk.

Bevisst **uten** `taskkill /T`: ved oppdatering fra appen er installeren et barn av `fjell-desktop.exe`, og `/T` ville avsluttet installeren selv.

Tvangsavslutning er trygt fordi Fjell ikke har ulagret tilstand: indeksen er SQLite i WAL-modus og hver beslutning er lagret når den er tatt, bildefiler skrives aldri (de flyttes bare), og et avbrutt skann fortsetter ved neste start.

Hookene hjelper **ikke** når Fjell kjører som administrator og installeren som vanlig bruker, eller når et annet program (antivirus, Utforsker) holder exe-filen. Se rådene under.

### 2. Reservevei i appen: manuell nedlasting

Feiler installasjonen, viser Innstillinger → Oppdateringer en tydelig melding og boksen «Installer manuelt» med adressen til siste utgivelse (`https://github.com/robertkolakowski-ai/fjell/releases/tag/beta`) og knappen «Kopier lenke». Det skjer i tre tilfeller:

- nedlasting eller installasjon kaster en feil (nett, signatur, installeren lar seg ikke starte),
- appen lever fortsatt 90 s etter at nedlastingen var ferdig (på Windows avslutter appen seg selv straks installeren er startet, så da har installasjonen stoppet),
- ved neste start er versjonen fortsatt eldre enn den som ble forsøkt installert («Oppdateringen til versjon X ble ikke fullført»). Appen husker bare versjonsnummeret til forsøket (i WebView-profilen) og rydder det når versjonen er ny. Meldingen vises i Innstillinger → Oppdateringer, ikke på Hjem.

Appen åpner aldri adressen selv og har ingen nettleser-, shell- eller opener-rettighet: lenken er ren tekst som du kopierer og limer inn i nettleseren. Adressen kommer fra skallet (`get_update_download_url`), så frontend har fortsatt ingen eksterne URL-er (`nonet.test.ts` er uendret). Går kopieringen ikke, merkes lenken slik at Ctrl+C virker.

### 3. Hvis installeren likevel sier «Kunne ikke avslutte Fjell»

1. Lukk Fjell. Åpne Oppgavebehandling (Ctrl+Shift+Esc), fanen Detaljer, og avslutt alle `fjell-desktop.exe`.
2. Startet du Fjell eller installeren med «Kjør som administrator»? Kjør da installeren på samme måte.
3. Hjelper ikke det: start PC-en på nytt og kjør installasjonsfilen før du åpner Fjell.
4. Skjer det fortsatt, er det trolig et annet program som holder filen (antivirus). Meld fra med Windows-versjon og antivirusprogram.

Bildene, indeksen og innstillingene ligger utenfor installasjonsmappen og berøres ikke av noe av dette.

## Engangsoppsett for produkteier

Den private nøkkelen er laget av Claude og skal ikke ligge i repoet eller i noen fil der. Den offentlige nøkkelen ligger i `desktop/src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).

1. Legg den private nøkkelen inn som GitHub-secret med nøyaktig navn **`TAURI_SIGNING_PRIVATE_KEY`**: Settings → Secrets and variables → Actions → New repository secret. Lim inn hele innholdet i nøkkelfilen.
2. Nøkkelen har **tomt passord**. La secreten `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` være tom eller helt uten (ikke opprett den).
3. Settings → Actions → General → Workflow permissions = **Read and write permissions**. Uten dette kan workflowen ikke opprette releasen.
4. Hvis Settings → Releases har «Immutable releases» slått på, slå det av for dette repoet. Beta-releasen overskrives med vilje.
5. Slett den lokale kopien av den private nøkkelen når den er lagt inn og sikkerhetskopiert på et trygt sted (se «Nøkkeltap»).

Uten secreten bygger workflowen en usignert pakke, publiserer ingenting og avslutter grønt med en tydelig advarsel i kjøringsloggen.

### Første gang

Appen kan ikke oppdatere seg før den første versjonen med oppdaterer er installert.

1. Slå sammen til `main` (eller start `beta`-workflowen manuelt fra `main`) og vent til jobben er grønn.
2. Last ned `Fjell_0.1.<N>_x64-setup.exe` fra Releases → `beta` **én gang** og installer den.
3. Slå sammen en ny endring til `main`, vent på ny grønn kjøring, åpne appen og trykk «Se etter oppdatering». Den skal tilby `0.1.<N+1>`. Dette er den første ekte kontrollen av hele kjeden og er ikke gjort ennå.

Deretter oppdaterer appen seg selv.

## Rollback

Hvis en beta er dårlig:

- Last ned en eldre pakke fra Actions (artefaktet `fjell-desktop-nsis-<versjon>-signert`, lagres i 30 dager) og installer den over. Releasen `beta` beholder bare nyeste pakke.
- Bestem så hva som skal skje med `main`: revert den dårlige endringen (da kommer en ny, høyere versjon og appen oppdaterer seg), eller start workflowen på nytt fra en eldre commit.
- Merk: en eldre pakke installert for hånd har lavere versjon enn den publiserte betaen, så appen vil tilby oppdatering til nyeste igjen. For å holde appen på en eldre versjon må `main` først få en ny, rettet versjon.
- Brukerdata (indeks, innstillinger) ligger utenfor installasjonsmappen og beholdes ved installasjon over.

## Nøkkeltap og rotasjon

- **Tapt privat nøkkel** (eller mistanke om lekkasje): da kan ingen signere oppdateringer som eksisterende installasjoner godtar. Rotasjon:
  1. Generer nytt nøkkelpar (`npx tauri signer generate`), legg den nye private nøkkelen i secreten `TAURI_SIGNING_PRIVATE_KEY`.
  2. Bytt `plugins.updater.pubkey` i `tauri.conf.json` til den nye offentlige nøkkelen og slå sammen til `main`.
  3. Installasjoner med den **gamle** offentlige nøkkelen godtar ikke pakker signert med den nye. De må installeres manuelt én gang (som «Første gang» over); deretter oppdaterer de seg selv igjen.
- Ved lekkasje: slett secreten i GitHub med en gang, så ingen kan misbruke den via workflowen, og roter som over.
- Sikkerhetskopier den private nøkkelen i en passordbehandler. Aldri i repoet, aldri i chat eller e-post.

## Dårlig nettverk

- Mislykket sjekk (ingen nett, GitHub nede, brannmur): appen viser en feilmelding og fortsetter å virke som før. Ingenting installeres, og den prøver ikke på nytt av seg selv utenom neste oppstart (hvis automatisk sjekk er på) eller neste trykk.
- Mislykket nedlasting midt i: ingenting installeres. Trykk «Installer og start på nytt» på nytt, eller bruk «Installer manuelt» som vises sammen med feilmeldingen.
- Ugyldig eller manglende signatur: oppdateringen avvises og installeres ikke.
- Appen virker fullt ut uten nett; oppdatering er helt valgfri.

## Signering og SmartScreen

Beta-kanalen er **ikke signert med Authenticode** (ingen kodesignerings-sertifikat; se åpne spørsmål i `docs/mvp/MVP-PLAN.md`). Oppdateringene er likevel kryptografisk sikret med minisign-nøkkelen over: appen installerer bare pakker som er signert av oss.

- Windows SmartScreen kan advare når du **manuelt** laster ned og kjører installasjonsfilen fra nettleseren («Windows har beskyttet PC-en din»; velg «Mer info» og «Kjør likevel»). Det gjelder bare første installasjon.
- Appens egen nedlasting går ikke via nettleseren og får ikke «Mark of the Web», så SmartScreen-advarselen gjelder ikke der.

## Kjente begrensninger

- `beta.yml` kjører parallelt med `desktop.yml` (tester, clippy, e2e) på samme sammenslåing og venter ikke på den. Røyktesten fanger ødelagte pakker, men ikke alle feil. Rett opp raskt hvis `desktop.yml` blir rød på `main`.
- Bare Windows x86_64.
- Cargo.lock og package-lock.json får ikke endret versjon i CI (bare `tauri.conf.json`, `desktop/package.json` og `Cargo.toml`). Det påvirker ikke bygget, men `cargo --locked` ville feilet etter versjonsendringen, så `beta.yml` bruker ikke `--locked`.
- NSIS-hookene er skrevet uten `makensis` lokalt; de kompileres og kjøres første gang i `update-e2e.yml`. Scenarioene med hengt instans er et vern, ikke en reprodusent av brukerfeilen (se [mvp/OPPDATERING-E2E.md](mvp/OPPDATERING-E2E.md)).
- Reserveveien peker på utgivelsessiden, ikke direkte på filen: releasen `beta` overskrives, så filnavnet endres for hver versjon.

## Feilsøking

| Symptom | Årsak |
|---|---|
| Advarsel «Secret TAURI_SIGNING_PRIVATE_KEY mangler» | Secret ikke lagt inn (eller kjøring fra gren/fork). Pakken er usignert og ikke publisert. |
| `gh release create` feiler med 403 | Workflow permissions er ikke «Read and write». |
| Appen sier «ingen oppdatering» | Installert versjon er lik eller høyere enn `latest.json`; sjekk `version` i filen. |
| «Kunne ikke avslutte Fjell» i installer eller avinstaller | Se «Når appen ikke lar seg lukke». |
| «Oppdateringen til versjon X ble ikke fullført» i Innstillinger | Forrige installasjon stoppet før appen ble byttet ut. Prøv igjen, eller bruk «Installer manuelt». |
| Appen avviser oppdateringen (signatur) | Pakken er signert med en annen nøkkel enn `pubkey` i `tauri.conf.json`; se «Nøkkeltap og rotasjon». |
