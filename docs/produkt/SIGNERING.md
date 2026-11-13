# Kodesignering av NSIS-installasjonsprogrammet

Status: veiledning. Ingenting er signert, kjøpt eller kjørt. `docs/RELEASE.md` (del 4) har kostnadstabellen; priser der er veiledende og må bekreftes. Beslutningen om konto/sertifikat er produkteiers (åpent spørsmål 11 i `MVP-PLAN.md`). `docs/RELEASE-PROSESS.md` finnes ikke i dette treet, og `beta.yml` har ingen Authenticode-signering; den signerer bare oppdateringsfilene (minisign, `TAURI_SIGNING_PRIVATE_KEY`). Ikke bland sammen de to: minisign sikrer appens selvoppdatering, Authenticode gjelder Windows/SmartScreen.

## Sammenligning
| | Azure Trusted Signing | OV-sertifikat | EV-sertifikat |
|---|---|---|---|
| Kostnad | Ca. 10 USD/mnd (Basic), jf. RELEASE.md; bekreft | Ca. 200–450 USD/år (RELEASE.md); bekreft | Ca. 350–700 USD/år (RELEASE.md); bekreft |
| Tilgjengelighet i Norge, enkeltpersonforetak | **Ikke verifisert.** Microsoft har identitets- og landkrav som må sjekkes mot gjeldende dokumentasjon; ikke avklart for Norge | Utstedes normalt til registrert organisasjon (Brønnøysundregistrene); enkeltpersonforetak: spør leverandøren, ikke verifisert | Som OV, strengere kontroll; ikke verifisert for enkeltpersonforetak |
| Nøkkel | Hos Microsoft, ingen token | Må ligge i HSM/token eller leverandørens skysigner (krav fra 2023) | Samme |
| SmartScreen | Omdømme bygges; ingen garantert umiddelbar fjerning | Omdømme bygges over tid (uker til måneder, ikke målt her) | Ingen automatisk umiddelbar fritak lenger; bedre start (påstand fra RELEASE.md, ikke målt) |
| CI | Egnet for GitHub Actions | Via skysigner | Via skysigner/token |

## Steg for steg
1. **Produkteier velger** utsteder og hvem som eier kontoen (juridisk enhet).
2. **Verifiser identitet** hos valgt utsteder (organisasjonsnummer, adresse, ev. telefonsamtale). Tidsbruk varierer; ikke estimert her.
3. **Opprett signeringsressurs** (Azure: kontoen + sertifikatprofil; sertifikatutsteder: skysigner/token).
4. **Legg legitimasjon i GitHub-secrets**, aldri i repoet, aldri i loggen. Navn avtales ved implementering.
5. **Tauri-konfig:** `bundle.windows.signCommand` (eller `certificateThumbprint` + `timestampUrl`). Signer både installasjonsprogrammet og `Fjell.exe`/avinstallerer. SHA-256 og RFC 3161-tidsstempel (tidsstempel-URL er et nettkall i bygget, ikke i appen).
6. **`beta.yml`:** nytt, vaktet steg etter bygget, som `Sjekk signeringsnøkkel`: kjører bare når secrets finnes og refs/heads/main; ellers bygges usignert med tydelig advarsel (samme mønster som i dag). Fork-PR-er får ikke secrets. Rekkefølge: Authenticode-signer først, så lag minisign `.sig` og `latest.json`, fordi minisign-signaturen og SHA-256 i releasenotatet gjelder den endelige filen. Ikke endre `nonet.test.ts`; byggesteg er utenfor appen.
7. **Verifiser:** `Get-AuthenticodeSignature` på pakken i CI (feil bygget hvis ikke gyldig), og manuell sjekk på ren Windows 10/11 (BETA.md I1).
8. **Oppdater tekst:** fjern «usignert»-advarselen i `PERSONVERN.md`, `OPPDATERING.md`, `RELEASE.md` og på nettsiden når (og først når) det er bekreftet.
9. **Rotasjon og utløp:** noter utløpsdato; plan for fornyelse. Nytt sertifikat nullstiller ikke nødvendigvis omdømme (ikke verifisert).

## Risiko
Tap av konto/nøkkel stopper bare nye utgivelser, ikke eksisterende installasjoner. Signering fjerner ikke garantert SmartScreen-advarselen.
