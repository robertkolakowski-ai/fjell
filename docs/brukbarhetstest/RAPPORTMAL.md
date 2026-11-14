# Rapportmal: brukbarhetstest Fjell

**Mal. Ingen runde er gjennomført; alle felt under er tomme med vilje.** Kopiér filen til `RAPPORT-runde-N.md` per runde og fyll ut etter at alle økter er ferdige. Ingen navn eller andre personopplysninger. Oppgaver som feiler rapporteres som feil, uavhengig av mål. Rapportér antall («3 av 5»), ikke prosent.

## 1. Oppsummering (maks 10 linjer)

- Runde: __ Dato (fra–til): __ Versjon (app, Windows, WebView2): __
- Antall deltakere gjennomført (P): __ av 5. Skjermleserøkter (S): __ av 2.
- Måloppnåelse (se 3): __ av __ mål nådd.
- De tre viktigste funnene: 1) __ 2) __ 3) __
- Anbefaling: går til bred beta / ny runde etter retting / stopp. Begrunnelse: __
- Begrensninger (rekruttering, utstyr, kuttede oppgaver, tekniske feil): __

## 2. Deltakere (uten identitet)

| ID | Alderbånd | Erfaring med PC/bilder | Forstørrelse/skalering | Skjermleser | Økt avbrutt? |
|---|---|---|---|---|---|
| P1 | | | | | |

## 3. Mål mot utfall

| Mål | Terskel | Utfall | Nådd? |
|---|---|---|---|
| Onboarding til resultat ≤ 60 s (O1) | 5 av 5 | tider: __ | |
| Duplikater fjernet uten hjelp (O2) | 5 av 5 | __ av 5 | |
| Gjøre om feil (O4) | 4 av 5, 5 av 5 med hint | A: __ B: __ C: __ | |
| Gjenopprette (O5) | 4 av 5, 5 av 5 med hint | A: __ B: __ C: __ | |
| Plass ikke frigjort før tømming forstått (O6) | 5 av 5 | __ av 5 | |
| Utilsiktet permanent sletting (O6) | 0 | __ | |
| Originalene forstått (O7) | 4 av 5 | __ av 5 | |
| Kortmodus tastatur (O3) | 4 av 5 | __ av 5 | |
| Språk (O8a), hurtigtaster (O8b) | 4 av 5, 3 av 5 | __ / __ | |
| Mistet et bilde man bryr seg om | 0 | __ | |
| SUS snitt / laveste | ≥ 80 / ≥ 68 | __ / __ | |

## 4. Oppgave × deltaker

Utfall A/B/C/T og tid i sekunder, f.eks. «A 42».

| Oppgave | P1 | P2 | P3 | P4 | P5 | Uten hjelp (A) | Med hint (B) | Ikke fullført (C) |
|---|---|---|---|---|---|---|---|---|
| O1 | | | | | | | | |
| O2 | | | | | | | | |
| O3 | | | | | | | | |
| O4 | | | | | | | | |
| O5 | | | | | | | | |
| O6 | | | | | | | | |
| O7 | | | | | | | | |
| O8a | | | | | | | | |
| O8b | | | | | | | | |

Tilleggstall per deltaker: antall hint, antall feiltrykk, utilsiktede effekter.

## 5. SUS

| ID | Score |
|---|---|
| P1 | |
Snitt: __ Laveste: __ Merk: oversettelsen er ikke validert (SUS.md).

## 6. Funn

**Alvorlighetsgrad (0–4, etter Nielsen):**

| Grad | Betydning | Handling |
|---|---|---|
| 0 | Ikke et brukbarhetsproblem | Ingen |
| 1 | Kosmetisk | Rettes hvis tid |
| 2 | Mindre problem, lav prioritet | Planlegges |
| 3 | Stort problem, høy prioritet | Rettes før neste runde |
| 4 | Katastrofe: hindrer oppgaven, eller kan føre til tap av bilde eller sletting uten at brukeren forstår | Rettes før noe annet; stopper beta |

**Hyppighet:** antall deltakere av 5 som rammes (0–5), målt separat fra alvorlighet. Skjermleser rapporteres som egne linjer (n av 2).

**Prioritet:** funn som rammer ≥ 2 av 5 får en sak og rettes før neste runde (BETA.md). Funn med alvorlighet 4 får sak uansett hyppighet (1 av 5 er nok).

| ID | Funn (hva skjedde, ikke tolkning) | Oppgave | Alvorlighet 0–4 | Hyppighet (n av 5) | Sitat | Forslag | Sak |
|---|---|---|---|---|---|---|---|
| F1 | | | | | | | |

Rangér etter alvorlighet, så hyppighet. Ikke vurder bort funn som ikke passer forventningen; skriv dem ned.

## 7. Skjermleserøkt (Narrator/NVDA)

Egen del. Ikke blandet med 3–6.

- Skjermleser og versjon per deltaker: __
- Gjennomført (ja/nei): __
- Oppgaver gjennomført uten mus og uten å se: __
- Annonseres handling og utfall? __ Fokus etter handling? __ Advarsel før destruktiv knapp? __
- Funn (samme alvorlighet og hyppighet n av 2): __
- Dekning: Narrator __ NVDA __ andre __. Uten at begge er dekket, står det her at det mangler.

## 8. Det som gikk bra

Kort. Hva fungerte uten problem, med tall.

## 9. Anbefalinger og neste skritt

| Tiltak | Eier | Før runde/dato | Sak |
|---|---|---|---|

## 10. Avvik fra planen

Kuttede oppgaver, avbrutte økter, tekniske feil (T), endringer i manus eller oppgavetekst, endringer i terskler.

## 11. Underlag

`SKJEMA.csv` per deltaker (uten navn) ligger i `docs/brukbarhetstest/data/` først når produkteier har godkjent lagring i repoet; ellers utenfor repoet. Samtykkeskjema oppbevares aldri i repoet.
