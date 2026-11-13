# Krasj og diagnose uten nettverk (designutkast)

Status: designutkast, ingenting er bygget. Fjell har ingen telemetri og ingen feilrapportering (`PERSONVERN.md`); det skal forbli slik. Diagnose er derfor en tekst brukeren selv kopierer og sender. Ingen nye nettverkstillatelser, avhengigheter eller CSP-endringer.

## Prinsipper
- **Ingenting forlater maskinen** med mindre brukeren selv limer teksten inn et sted.
- **Allowlist, ikke blocklist:** rapporten bygges av et lukket sett felter med faste typer (tall, enum, versjonsstreng). Aldri fri tekst fra filsystemet.
- Brukeren ser hele teksten før kopiering (forhåndsvisning i dialogen).
- Alt som genereres lagres bare i minnet (ingen loggfil med stier; jf. «Fjell skriver ikke loggfiler med filstier»).

## Hva rapporten SKAL inneholde
| Felt | Eksempel |
|---|---|
| Appversjon, kjerneversjon, `ALGO_VER`, skjema-/migreringsversjon | `0.1.57`, `1` |
| Windows-bygg, arkitektur, WebView2-versjon | `10.0.22631`, `x86_64` |
| Språk (nb/en), skalering, høykontrast av/på | `nb`, `150 %` |
| Antall: filer i indeks, grupper, i papirkurv; størrelser avrundet til grov klasse | `10 000–100 000` |
| Skannstatus og siste feilkode (fra kjernens feilkoder, f.eks. `cloud_placeholder`, `corrupt`, `not_found`, `no_library`) med antall | `corrupt: 3` |
| Disktyper (lokal/ekstern/nettverk/skymappe) som antall | `nettverk: 1` |
| Siste panikk: kilde-modul og linjenummer fra Rust (ikke meldingsteksten) | `fjell-fs::delete:124` |
| Tidtakinger (p50/p95 for handlinger, oppstart) | `p95 41 ms` |
| Utdrag av innstillinger som ikke er personlige (papirkurvfrist, auto-oppdateringssjekk av/på) | `30 d` |

## Hva rapporten IKKE skal inneholde
Stier (heller ikke delvise eller diskbokstaver) · filnavn og mappenavn · bilder, miniatyrer, hasher og fingeravtrykk · EXIF/opptakstidspunkt/posisjon · brukernavn, maskinnavn, e-post, IP, konto-ID, lisensnøkkel · innholdet i indeksen eller manifest · fri tekst fra feilmeldinger fra OS (kan inneholde stier; erstattes med feilkode) · stakkspor med argumentverdier.

## Brukerflyt
1. Innstillinger → Hjelp → «Kopier diagnoserapport» (knapp; ikke en hurtigtast). Dialogen viser teksten og «Kopier» + «Lukk». Teksten nb/en; selve feltnavnene alltid på engelsk slik at støtte kan lese dem.
2. Etter krasj (neste oppstart): banner «Fjell avsluttet uventet sist. Vil du se en diagnoserapport du kan kopiere?» Ingen automatisk sending. Krever at kjernen lagrer en minimal «krasjmarkør» (kodeord + modul:linje, ingen sti) i innstillingsfilen; ny lagring, må med i `PERSONVERN.md`.
3. Panikkhåndtering i Rust: hook som bare setter markøren, aldri skriver melding med data.

## Test (forslag)
- Enhetstest: rapporten for et bibliotek med kjente «giftstrenger» (sti, filnavn, e-post) inneholder ingen av dem.
- Typetest: kun allowlist-felter kompilerer inn i strukturen.
- `nonet.test.ts` urørt; ingen ny tillatelse.

## Åpent
Format (klartekst vs. kodet blokk) · maks lengde · hvor støtte tar imot (se `HJELP-OG-STOTTE.md`) · om krasjmarkør er verdt ny lagring.
