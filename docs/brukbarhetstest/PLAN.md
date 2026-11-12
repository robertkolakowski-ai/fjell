# Brukbarhetstest: plan

**Status: ingenting er gjennomført.** Dette er et kit. Det finnes ingen deltakere, ingen resultater og ingen målinger. Alle tall under er mål, ikke funn. Gjelder MVP-restanse punkt 8 i [`../mvp/MVP-PLAN.md`](../mvp/MVP-PLAN.md) (4c) og DoD for M8 (del 4). Kitet utdyper `docs/BETA.md` del 7; ved avvik gjelder BETA.md til den er oppdatert.

Filer i kitet:

| Fil | Innhold |
|---|---|
| `PLAN.md` | Denne filen: mål, rekruttering, utstyr, roller, skjermleserøkt |
| `OPPGAVER.md` | 8 oppgaver (O1–O8) med suksesskriterium og målepunkter |
| `MANUS.md` | Moderatormanus |
| `SAMTYKKE.md` | Samtykkeskjema (skrives ut, ett per deltaker) |
| `SUS.md` | System Usability Scale på norsk, med utregning |
| `SKJEMA.csv` | Notatark, én rad per observasjon |
| `RAPPORTMAL.md` | Mal for oppsummering etter alle økter |

## 1. Mål

1. Finne ut om en vanlig PC-bruker klarer de viktigste flytene i Fjell uten hjelp.
2. Måle de målene MVP-planen setter, på ekte maskin med ekte Windows (se «Ikke dekket» under).
3. Avdekke misforståelser om det som er tryggest og lettest å misforstå: at ting går til appens appens Papirkurv og kan angres, at plass ikke er frigjort før tømming, og at originalfilene ikke endres.

Vi tester appen, ikke deltakeren.

## 2. Mål-målinger (fra MVP-PLAN og BETA.md)

| Mål | Terskel | Kilde | Oppgave |
|---|---|---|---|
| Onboarding fra første start til resultatskjerm | ≤ 60 s, 5 av 5 uten hjelp | MVP-PLAN del 4 (M8), PLAN-UX-I18N 3.1 | O1 |
| Første opprydding (duplikater fjernet) | 5 av 5 uten hjelp | BETA.md T2 | O2 |
| Forstår appens Papirkurv: finner fram, kan gjenopprette | 4 av 5 uten hjelp, 5 av 5 med ett hint | BETA.md T3 | O5 |
| Forstår angre (rask vei tilbake rett etter feil) | 4 av 5 uten hjelp, 5 av 5 med ett hint | BETA.md T3 | O4 |
| Forstår at plass ikke er frigjort før tømming | 5 av 5 riktig forklart | PLAN-UX-I18N del 8 | O6 |
| Tømming velges ikke permanent ved uhell | 0 utilsiktet permanent sletting | BETA.md T7 | O6 |
| Originalene: forstår at de ikke endres | 4 av 5 | BETA.md T8 | O7 |
| Ingen mister et bilde de bryr seg om | 0 deltakere | BETA.md mål | alle |
| SUS | snitt ≥ 80, ingen enkeltscore < 68 | BETA.md mål | SUS.md |
| Funn som rammer ≥ 2 av 5 | Sak opprettes og rettes før neste runde | BETA.md mål | RAPPORTMAL |

Terskler for kortmodus (4 av 5, ingen utilsiktet «Flytt»), språkbytte (4 av 5) og hurtigtaster (3 av 5 finner F1 uten hjelp) er satt i `OPPGAVER.md`. Hurtigtastterskelen er foreslått her og finnes ikke i MVP-PLAN; produkteier kan endre den.

Ærlighet: oppgaver som feiler rapporteres som feil, uavhengig av mål. Små utvalg gir ikke statistikk; vi rapporterer antall («3 av 5»), ikke prosent.

## 3. Rekruttering

Hvem (ikke utviklere, ikke teamet):
- 5 personer som bruker Windows-PC privat og har mange bilder (omtrent 5 000+ på PC), og som ikke har brukt Fjell.
- Ikke yrke innen programvare, IT, design eller UX. Ikke ansatte, ikke venner eller familie av teamet.
- Norsk (bokmål) som hovedspråk.
- Spredning: minst 2 over 55 år; minst 1 som bruker forstørrelse eller høy skalering (150 % eller mer); blanding av erfaren og lite erfaren med filhåndtering; helst begge kjønn.
- Skjermleserbrukere rekrutteres separat, se del 8.

Praktisk:
- Rekrutter via nettverk utenfor teamet, lokale lag eller en rekrutteringstjeneste. Ikke offentliggjør appen som kandidat.
- Screening-spørsmål (skriftlig, uten navn i dette kitet): enhet og Windows-versjon, omtrentlig antall bilder, alder i bånd (under 35, 35–54, 55+), bruk av forstørrelse, yrke (utelukk IT). Kandidatlisten med kontaktinfo oppbevares utenfor repoet.
- Vederlag (f.eks. gavekort) avklares av produkteier. Ikke bestemt.
- Reserve: rekrutter 7 for å få 5 gjennomførte.
- Deltaker-ID: `P1`–`P5` (vanlig økt), `S1`–`S2` (skjermleserøkt). Koblingen mellom ID og person finnes ikke i repoet.

## 4. Utstyr

- Referansemaskin med Windows 11 (mellomklasse, lokal SSD). Gjerne en ekstra maskin med Windows 10 22H2 for én deltaker.
- Fjell installert fra installasjonsprogrammet (NSIS), ikke `tauri dev`. Notér versjon, Windows-bygg og WebView2-versjon i SKJEMA.csv (rad med oppgave `OPPSETT`).
- **Testbibliotek, ikke deltakerens egne bilder:** tre mapper, ca. 3 000 bilder, generert av `core/testdata` eller en kopi av dem, med eksakte duplikater, like bilder, uskarpe bilder og skjermbilder. Mappen «Ferie» (navn kan velges) skal inneholde kjente duplikater til O2. Ikke bruk bilder av ekte personer.
- Nullstilling mellom deltakere: ny kopi av testbiblioteket, tømt app-data (indeks, karantene), onboarding ikke gjennomført. Dokumentér hvordan du gjorde det i økten.
- Stoppeklokke (eller klokke på observatørens maskin), notatark (SKJEMA.csv på bærbar PC eller utskrift), utskrift av SAMTYKKE.md og SUS.md.
- Egen skjerm eller delt visning for observatør (bare skjermvisning, ingen opptak, med mindre eget samtykke er gitt).
- Stille rom. Skriftstørrelse og skalering stilles til deltakerens vanlige verdi før start (notér den).
- Hurtigtaster: aldri nevnt av moderator (se O8b).

## 5. Varighet

| Del | Minutter |
|---|---|
| Innledning, samtykke | 5 |
| Oppgaver O1–O8 | 30 |
| SUS og avsluttende samtale | 8 |
| Buffer | 7 |
| **Totalt** | **ca. 50 (planlegg 60)** |

BETA.md foreslår 30 min per deltaker. Med 8 oppgaver, samtykke og SUS er det ikke realistisk; planlegg 60 minutter. Hvis tiden er knapp: O8 kan kuttes først (rapportér at de ble kuttet), aldri O1–O7.

Fast rekkefølge O1–O8. Pilot først: kjør hele kitet med én kollega som ikke er med i tallene, og rett manus og oppgavetekst.

## 6. Roller

| Rolle | Oppgave | Antall |
|---|---|---|
| Moderator | Leser manus, leser oppgaver, stiller nøytrale oppfølgingsspørsmål. Hjelper ikke. | 1 |
| Observatør | Noterer i SKJEMA.csv: tid, hint, feiltrykk, sitater, alvorlighet. Tar tid. Sier ingenting under oppgaver. | 1 |
| Teknisk ansvarlig | Nullstiller maskin og bibliotek, rydder opp i feil som ikke er deltakerens. Ikke i rommet under økt hvis mulig. | 1 (kan være observatør) |

Minst to personer. Moderator og observatør bør ikke være den som har skrevet UI-tekstene.

Hint-regler: ett hint betyr at moderator sier en forhåndsskrevet setning fra OPPGAVER.md. Etter hint teller oppgaven som «med hint». Etter 3 minutter uten framdrift avbrytes oppgaven og noteres «ikke fullført».

## 7. Etter hver økt og etter alle økter

- Samme dag: renskriv notater, avgjør alvorlighet sammen (observatør og moderator), fyll SKJEMA.csv.
- Når alle 5 er ferdige: fyll RAPPORTMAL.md. Hver feil som rammer ≥ 2 av 5 blir en sak.
- Papirskjema (samtykke, SUS) oppbevares låst; elektroniske notater uten navn. Slettefrist avtales og står i SAMTYKKE.md.
- Etter retting: ny runde med 3–5 nye deltakere.

## 8. Tilgjengelighetstest med skjermleser (Narrator/NVDA)

**Egen økt, egne deltakere. Ingenting er gjennomført.** Denne delen erstatter ikke manuell avkrysning i `docs/BETA.md` del 1 og 2 (Narrator, NVDA); den supplerer den med ekte brukere.

Deltakerkrav:
- 2 personer (`S1`, `S2`) som bruker skjermleser til daglig på Windows, helst én med NVDA og én med Narrator (eller JAWS hvis det er alt de bruker; notér og rapportér at det ikke er dekket av kravene i CLAUDE.md). Er det bare NVDA-brukere, rapportér at Narrator ikke er dekket.
- Blinde eller sterkt svaksynte som ikke kan klare seg uten skjermleser. Ikke sansede som «later som» med lukkede øyne.
- Ikke utviklere av tilgjengelighetsverktøy eller ansatte i teamet.
- Bruker egen maskin og egne innstillinger hvis mulig (leserhastighet, stemme, utvidede tastekommandoer), ellers ber vi dem komme til referansemaskinen og la dem sette opp skjermleseren selv først.
- Må kunne kommunisere under tenke-høyt (skjermleserstemmen går samtidig). Avtal muntlig signal for «jeg vil ha en pause».

Opplegg:
- 60–75 minutter. Lengre pauser, ingen tidspress. Tidskrav fra del 2 (60 s onboarding) gjelder ikke og måles ikke her.
- Oppgaver: O1, O2, O3, O4, O6 (tøm) og O8a (språk), pluss O5 (gjenopprett) og O8b (hurtigtaster via F1-dialogen) hvis tid. O3 gjøres med knappene og, i fokusmodus, piltaster. Tekstene leses opp av moderator og er identiske med vanlig økt.
- Målepunkter i tillegg: kan fullføre uten mus og uten å se skjermen; hver handling og dens utfall annonseres (ikke stille); fokus havner et forståelig sted etter handling; destruktive handlinger har advarsel før knappen i lesrekkefølge; navn på knapper forstås uten å se; deltakeren uttrykker usikkerhet om hva som har skjedd.
- Notér: skjermleser og versjon, leserhastighet, moduser brukt (surf/fokus; Narrator skannmodus), tastekommandoer deltakeren brukte.
- SUS gis muntlig eller i tilgjengelig skjema; resultatet rapporteres separat, ikke i snittet for de 5 (n = 2 er for få).
- Funn tilordnes alvorlighet som vanlig. Feil som hindrer fullføring uten sikkerhetsnett (f.eks. ingen annonsering ved sletting) er alvorlighet 4.
- Samtykke: samme skjema, men dokumentet må leses opp eller leveres i tilgjengelig format (docx/tekst) på forhånd, med muntlig bekreftelse.

Dette tilsvarer ikke at appen er «tilgjengelig». To deltakere gir kvalitative funn, ikke dekning av alle hjelpemidler.

## 9. Ikke dekket / ikke gjort

- Ingen rekruttering er gjort. Ingen økt er kjørt. Ingen SUS-score finnes.
- Ingen måling av 60 s-onboarding er gjort på ekte Windows-maskin (restanse 8).
- Kitet er ikke prøvekjørt (pilot).
- Terskler og vederlag må godkjennes av produkteier.
- Appens knappetekster er sjekket mot `desktop/src/lib/i18n/strings/nb.ts` (2026-11-12): skjermen heter «Papirkurv» (ikke «Nylig slettet»), og «Windows Papirkurv» er en egen ting. Sjekk igjen mot bygget som testes før oppgavetekstene låses, slik at moderator ikke bruker appens ord.
