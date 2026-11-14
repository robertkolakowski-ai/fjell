# Utgivelsesprosess: slik lager du en versjon

Status: oppsett laget 2026-11-12. **Ikke kjørt i GitHub, ingen ekte signering, ingen ekte Windows-test.** Workflowen er bare syntakssjekket lokalt.

Dette dokumentet beskriver *hvordan*. Beslutningsgrunnlaget (NSIS mot MSIX, WebView2, priser, SemVer-regler) ligger i [RELEASE.md](RELEASE.md) og er uendret; les det først. Workflow: `.github/workflows/release.yml`. Endringslogg: `CHANGELOG.md`.

## 1. Stegvis: lage en versjon

1. **Sjekk at main er grønn**: CI `desktop`, `core`, `layers`, `i18n`. Gå gjennom sjekklisten i [RELEASE.md del 6](RELEASE.md) (`just check`, `just desktop-check`, `just third-party-check`, `just contrast`, Playwright).
2. **Bump versjon i alle tre filer i samme commit** (må være lik, ellers feiler workflowen i `prepare`):
   - `desktop/src-tauri/tauri.conf.json` (`version`)
   - `desktop/src-tauri/Cargo.toml` (`[workspace.package] version`), og oppdater `Cargo.lock`
   - `desktop/package.json` (`version`) og `package-lock.json`
   Bruk bare kjerneversjonen (`0.1.0`), også for beta. NSIS krever numerisk versjon; `-beta.1` står kun i taggen og release-navnet.
3. **Regenerer** `THIRD_PARTY.md` (`just third-party`).
4. **Oppdater `CHANGELOG.md`**: gi seksjonen `[Ikke utgitt]` et nytt navn `[0.1.0] - ÅÅÅÅ-MM-DD` (eller `[0.1.0-beta.1]`), legg inn en ny tom `[Ikke utgitt]` over, og oppdater «Kjent». Workflowen henter teksten fra seksjonen som heter som versjonen (uten `v`); finnes den ikke, brukes `[Ikke utgitt]` med en advarsel.
5. **Merge** via PR til main (én fase = én PR).
6. **Tagg main** og push taggen:
   ```
   git tag v0.1.0        # eller v0.1.0-beta.1
   git push origin v0.1.0
   ```
   Alternativt: Actions, «release», «Run workflow», skriv versjonen uten `v`. Utkastet lages da mot valgt gren/commit.
7. **Følg kjøringen** i Actions. Jobber: `prepare` (versjonskontroll, finnes hemmeligheter?), `build` (tester, NSIS, røyktest), `sign` (hoppes over uten hemmeligheter), `release` (utkast).
8. **Gå gjennom utkastet** under Releases: endringslogg, SHA-256, om det står «Usignert». Last ned og installer på en ren Windows-maskin; kryss av [BETA.md](BETA.md) punkt 1–7 på referansemaskin.
9. **Publiser manuelt** («Publish release») når alt stemmer. Workflowen publiserer aldri selv. Beta merkes som pre-release automatisk når taggen har `-`.
10. Feil i en utkastkjøring: slett utkastet (og taggen hvis den ble laget) og kjør på nytt. Omkjøring av samme tag oppdaterer eksisterende utkast og overskriver filene.

## 2. Hemmeligheter og variabler som må opprettes

Settes i GitHub: *Settings, Secrets and variables, Actions*. **Ingen verdier skal stå i repoet.** Uten dem bygges usignert og CI forblir grønn.

Repository secrets:

| Navn | Hva det er |
|---|---|
| `AZURE_TENANT_ID` | Katalog-ID (tenant) i Microsoft Entra der signeringskontoen ligger |
| `AZURE_CLIENT_ID` | Applikasjons-ID til en app-registrering (tjenesteprinsipal) som har rollen «Artifact Signing Certificate Profile Signer» (tidligere «Trusted Signing Certificate Profile Signer») på sertifikatprofilen |
| `AZURE_CLIENT_SECRET` | Klienthemmelighet for den app-registreringen (har utløpsdato; sett påminnelse) |

Repository variables (ikke hemmelige):

| Navn | Hva det er |
|---|---|
| `TRUSTED_SIGNING_ENDPOINT` | Regional endepunkt-URL for signeringskontoen, f.eks. for West Europe `https://weu.codesigning.azure.net` |
| `TRUSTED_SIGNING_ACCOUNT` | Navnet på signeringskontoen i Azure |
| `TRUSTED_SIGNING_PROFILE` | Navnet på sertifikatprofilen (Public Trust) |

Alle seks må finnes for at `sign` kjører. Mangler én, hoppes signering over med en notis i `prepare`-loggen.

Fremgangsmåte hos Azure (ikke gjort): opprett signeringskonto, fullfør identitetsvalidering som utgiver, opprett sertifikatprofil, registrer app og gi rollen over. Microsofts identitetskrav og landtilgjengelighet (Norge er ikke bekreftet, se RELEASE.md del 4) avgjør om dette er mulig; alternativet er en skysigner (SSL.com eSigner, DigiCert KeyLocker) og da må `sign`-jobben byttes. Bruk gjerne OIDC (federert legitimasjon) i stedet for klienthemmelighet når det er satt opp; da forsvinner `AZURE_CLIENT_SECRET`.

Sjekk også: `azure/trusted-signing-action` er festet til `@v0.5.1`. Verifiser gjeldende versjon og eventuelt nytt navn på handlingen før første signerte kjøring; dette er ikke kjørt.

### Begrensning

`sign` signerer kun installasjonsprogrammet (`*-setup.exe`). `fjell.exe` og avinstalleren *inni* pakken signeres ikke, siden de lages under `tauri build`. RELEASE.md del 4 krever at begge signeres. Det krever `bundle.windows.signCommand` i `tauri.conf.json` (utenfor dette oppsettets omfang) som kaller signeringsverktøyet under bygget. Til det er gjort, er en signert utgivelse bare delvis i tråd med kravet.

## 3. Usignert utgivelse og SmartScreen (lukket beta)

Dette må du forvente uten signering:

- Windows SmartScreen viser blå advarsel «Windows beskyttet PC-en din». Brukeren må velge «Mer info», så «Kjør likevel». Noen nettlesere (Edge) kan i tillegg merke nedlastingen som uvanlig; noen antivirusprogrammer kan gi treff.
- Et usignert program bygger ikke SmartScreen-omdømme, så advarselen forsvinner ikke av seg selv.
- Derfor: del bare med navngitte testere, direkte (ikke offentlig lenke), legg ved SHA-256 fra utgivelsesnotatet og instruksjonen over. Utgivelsesnotatet for usignerte bygg inneholder instruksjonen automatisk.
- Brukeren kontrollerer filen i PowerShell: `Get-FileHash .\Fjell_0.1.0_x64-setup.exe -Algorithm SHA256` og sammenligner med notatet (filnavnet kan avvike).
- Ikke egnet for åpen beta eller 1.0 (RELEASE.md del 4).

Sjekksummen sier at filen er den samme som CI laget; den sier ikke at CI er uskyldig, og erstatter ikke signering.

## 4. Hva produkteier må bestemme (åpent spørsmål 11)

MVP-PLAN del 6, spørsmål 11: *hvilket sertifikat eller signeringstjeneste, hvem eier det, og når må det være på plass (senest før beta)?* Ikke avklart. Underpunkter:

1. **Tjeneste**: Azure Trusted Signing (ca. 10 USD/mnd, krever at utgiver oppfyller Microsofts krav), OV-sertifikat med skybasert HSM, eller Store/MSIX (spørsmål 12). Anbefaling i RELEASE.md: Trusted Signing hvis kvalifisert, ellers OV i skybasert HSM. Priser er veiledende.
2. **Eier**: hvilken juridisk enhet står som utgiver (navnet synes i Windows-dialogen), og hvem administrerer Azure-abonnement, app-registrering og utløpsdatoer på hemmelighet/sertifikat.
3. **Frist**: signering senest før *åpen* beta; lukket beta kan gå usignert med instruksen over. Beslutt også hvem som får lukket beta.
4. **Hvem kan lage utgivelser**: hvem skal ha skrivetilgang til taggene og godkjenne «Publish release». Vurder et GitHub *environment* med påkrevd godkjenning for `sign` (da legges hemmelighetene i environmentet, og `prepare`-sjekken må flyttes dit).
5. **Nært beslektet** (spørsmål 12 og 13): NSIS eller MSIX, og om Windows 10 22H2 støttes. Påvirker bygg- og signeringsoppsettet.

Frem til svar: utgivelser er usignerte, alltid utkast, og publiseres av et menneske.

## 5. Hva som er og ikke er verifisert

| Verifisert (lokalt, Linux) | Ikke verifisert |
|---|---|
| YAML-syntaks i `release.yml` (Python `yaml`) | Kjøring i GitHub Actions (uttrykk, rettigheter, `needs`/`if`-logikk) |
| Versjonskontroll- og endringslogg-skriptene kan leses som bash/awk | Faktisk NSIS-bygg og røyktest i dette workflowet |
| | Ekte signering, handlingens parametere og versjon, RBAC-roller i Azure |
| | At GitHub-utkastet ser riktig ut og at sjekksum-filen følger med |
