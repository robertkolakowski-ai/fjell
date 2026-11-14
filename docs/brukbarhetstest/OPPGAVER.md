# Brukbarhetstest: oppgaver

**Status: ikke gjennomført.** Terskler er mål, ikke resultater.

Regler for alle oppgaver:
- Moderator leser teksten ordrett. Teksten inneholder ikke appens ord (ikke «papirkurv», «angre», «duplikat», «kortmodus», «hurtigtast» osv.). Ikke forklar oppgaven.
- Rekkefølgen O1–O8 er fast. O1 krever fersk app (onboarding uten forkunnskap).
- Hint er forhåndsskrevne (under hver oppgave) og gis først etter 90 s uten framdrift, eller når deltakeren ber om det. Teller som «med hint».
- Avbryt etter 3 min uten framdrift: «ikke fullført». Ingen skyld på deltakeren.
- Observatør noterer i `SKJEMA.csv` per oppgave: `tid_s`, `utfall` (A = uten hjelp, B = med hint, C = ikke fullført), `hint`, `feiltrykk`, `sitat`, `alvorlighet`.
- Feiltrykk = klikk/tast som ikke leder mot målet eller som må rulles tilbake. Utilsiktet «flytt»/«permanent» noteres alltid separat.
- Etter siste oppgave: sjekk at ingen deltaker har mistet et bilde de brydde seg om (alt er gjenopprettbart i økten).

Appens faktiske tekster skal sjekkes mot bygget før testen (se PLAN.md del 9). Suksesskriterier under bruker funksjon, ikke ordlyd.

| # | Tema | Terskel |
|---|---|---|
| O1 | Onboarding | ≤ 60 s, 5 av 5 uten hjelp |
| O2 | Finn og fjern duplikater i en mappe | 5 av 5 uten hjelp |
| O3 | Rask opprydding med bare tastatur | 4 av 5 uten hjelp |
| O4 | Gjør om en feil | 4 av 5 uten hjelp, 5 av 5 med ett hint |
| O5 | Hent tilbake fra appens Papirkurv | 4 av 5 uten hjelp, 5 av 5 med ett hint |
| O6 | Tøm | 5 av 5 forstår, 0 utilsiktet permanent sletting |
| O7 | Originalene | 4 av 5 |
| O8 | Språk (a) og hurtigtaster (b) | a: 4 av 5; b: 3 av 5 uten hjelp (foreslått, ikke i MVP-PLAN) |

Prioritet hvis tiden er knapp: O1–O7 først, så O8. Rapportér det som kuttes.

---

## O1. Komme i gang

**Tekst:** «Du har nettopp installert dette programmet for å få orden på bildene dine på PC-en. Gjør det du ville gjort for å komme i gang.»

**Oppsett:** Ny installasjon, onboarding ikke gjort. Testbibliotek tilgjengelig i standard bildemappe (eller mappe deltakeren må velge; avtal på forhånd og notér).

**Suksess:** Deltakeren kommer til første resultatskjerm (liste over funn) uten hjelp. Tid fra første klikk i appen til resultatskjerm vises.

**Målepunkter:** tid til resultatskjerm (mål ≤ 60 s); tid til første tall vises (mål ≤ 5 s etter start skann, PLAN-UX-I18N 3.1); antall klikk; om deltakeren leste forklaringen om at bildene blir på maskinen (spør etterpå: «Hva tenker du skjer med bildene dine her?»); feiltrykk; sitat.

**Tidtaking:** stoppeklokken stoppes når resultatskjermen er synlig. Tid brukt på å lese samtykkeinfo hører ikke til; tid brukt på å lete etter mappen gjør.

**Hint:** «Se deg rundt på skjermen. Hva ville du trykket på?» (ikke sett navn på knapp)

## O2. Finn og fjern duplikater i en mappe

**Tekst:** «I mappen som heter “Ferie” ligger det flere bilder som er nøyaktig like. Få bort dem, men behold ett av hvert.»

**Oppsett:** Mappen inneholder kjente duplikatgrupper (antall notert).

**Suksess:** Alle kjente duplikater i mappen er flyttet til appens Papirkurv; ett bilde av hver gruppe er beholdt (ingen original uten kopi). Bekreftelse som vises håndteres uten hjelp.

**Målepunkter:** tid; om deltakeren finner mappen via gruppevisning eller sammendrag; om hen gjennomgår før flytting; om «behold» (keeper) forstås; om bekreftelsesark (≥ 50 bilder eller ≥ 1 GB) leses og forstås; feiltrykk; utilsiktet flytting av feil bilde (alvorlighet 3–4).

**Hint:** «Leter du etter en oversikt over bilder som ligner hverandre?»

## O3. Raske avgjørelser med bare tastatur

**Tekst:** «Ta avgjørelser på ti bilder, ett om gangen, bare med tastaturet. Ikke bruk musen.» (Fjern eller dekk til musepeker om nødvendig; notér om deltakeren bruker mus likevel.)

**Oppsett:** Kortbunke med minst 10 kort. Moderator nevner ikke taster eller modusnavn.

**Suksess:** Deltakeren tar 10 avgjørelser med tastatur (Tab/Enter på knapper eller piltaster/Delete) uten mus og forstår hva som skjer med hvert bilde (spør: «Hva skjedde med det siste bildet du tok avgjørelse om?»).

**Målepunkter:** tid til første avgjørelse; hvilke taster som ble brukt (Tab+Enter eller piler); om hen oppdager hvilke taster som gjelder uten hjelp (F1/knappetekst/tooltip); utilsiktet «papirkurv» (spør: «Skjedde det noe du ikke mente?»); om fokus går tapt; sitat.

**Hint:** «Hva ville du sett etter på skjermen for å få vite hvordan du gjør det?»

## O4. Gjør om en feil

**Tekst (etter at deltakeren har tatt en avgjørelse i O3 eller flyttet noe i O2):** «Du ser at du tok feil av det siste bildet. Få det slik det var før.»

**Alternativ hvis deltakeren ikke gjorde feil:** moderator ber om å ta én ny avgjørelse først («Ta en avgjørelse på ett bilde til»), og leser så teksten.

**Suksess:** Bildet er tilbake (siste handling gjort om) uten at andre ting påvirkes. Både Angre-knapp og Ctrl+Z regnes som suksess. Notér hvilken vei.

**Målepunkter:** tid; metode (knapp / Ctrl+Z / via papirkurv); om hen finner Angre uten å lete; om hen forstår hva «siste» omfatter (spør: «Hva tror du skjedde med de andre bildene?»); feiltrykk.

**Hint:** «Er det noe på skjermen akkurat nå som ser ut til å høre til det du nettopp gjorde?»

## O5. Hent tilbake fra appens Papirkurv

**Tekst:** «Forrige uke flyttet du noen bilder bort. Nå vil du ha ett av dem tilbake. Finn det og få det tilbake.»

**Oppsett:** Minst 20 bilder er flyttet til appens papirkurv tidligere i økten (kjør gjerne O2 først); ett navngitt bilde skal gjenopprettes (skrives på lapp: filnavn).

**Suksess:** Det angitte bildet er gjenopprettet til opprinnelig sted (verifisér i Filutforsker etter oppgaven: filen finnes i mappen og er uendret).

**Målepunkter:** tid; hvor hen leter først (notér sti); om hen finner appens Papirkurv i appen uten hjelp; om hen forveksler med Windows Papirkurv; om hen forstår at det finnes en frist (spør: «Hvor lenge tror du bildene blir liggende der?»; riktig svar: til de tømmes eller fristen går ut, standard 30 dager); feiltrykk.

**Hint:** «Hvor ville du lett etter noe du allerede har flyttet bort?»

## O6. Tøm

**Tekst:** «Du ønsker å få mer ledig plass på disken. Er plassen frigjort nå? Hvis ikke, gjør det som trengs.»

**Oppsett:** Bilder ligger i appens papirkurv. Ikke nevn «tømme».

**Suksess:** To deler.
1. Deltakeren svarer riktig på spørsmålet (nei, ikke før det er tømt), med egne ord.
2. Hen tømmer med standardvalget (Windows Papirkurv) og forklarer forskjellen på det og «Slett permanent» etterpå (spør: «Hva er forskjellen på de to valgene du så?»). Velger hen permanent: la det skje bare på testbiblioteket; noter som utilsiktet hvis hen ikke forsto at det ikke kan angres.

**Målepunkter:** svar på del 1 (riktig/feil, ordrett sitat); tid; om hen leste advarselen i arket; om hen ser appens tekst om at plass frigjøres først når Windows Papirkurv tømmes; utilsiktet permanent sletting (mål 0); om hen kunne tenkt seg å avbryte (Avbryt brukt?); sitat.

**Hint:** «Hva sier skjermen at som skjer hvis du går videre?»

## O7. Forklare hva som skjer med originalene

**Tekst:** «Forklar med egne ord: hvor ligger bildene dine nå, og har programmet endret noen av dem?»

**Oppsett:** Etter O2–O6. Deltakeren kan se på skjermen og i Filutforsker om hen vil.

**Suksess:** Sier at originalene ikke er endret (innholdet), og at flyttede bilder ligger trygt i appens papirkurv/Windows Papirkurv til de tømmes. Ikke kreves: nøyaktig terminologi.

**Målepunkter:** ordrett forklaring; riktig/delvis/feil (A/B/C); om hen tror at appen sender bildene noe sted (spør: «Forlater bildene maskinen din på noe tidspunkt?», riktig: nei); hva hen stoler mest/minst på; sitat.

**Hint:** Ingen. Dette er en forståelsesoppgave.

## O8a. Bytt språk

**Tekst:** «Du ønsker at programmet skal være på engelsk. Gjør det.» (Etterpå: «Sett det tilbake til norsk.»)

**Suksess:** Språket er byttet til engelsk via appens Innstillinger uten hjelp; tilbake til norsk.

**Målepunkter:** tid; veien (Innstillinger-lenke, hovedmeny, søk); om hele skjermen er oversatt (observatør noterer halvoversatt tekst); om hen blir forvirret av at språket følger Windows som standard; feiltrykk.

**Hint:** «Finnes det et sted hvor man vanligvis stiller inn ting?»

## O8b. Finn hurtigtaster

**Tekst:** «Du vil gjøre dette raskere, uten å klikke så mye. Finn ut hvilke taster du kan bruke.»

**Oppsett:** Ikke nevn F1. Deltakeren må ikke ha fått oppgitt hurtigtaster tidligere (O3 kan ha avslørt dem: notér i så fall).

**Suksess:** Åpner oversikten over hurtigtaster (F1-dialogen eller knapp «Hurtigtaster») uten hjelp og nevner minst én (f.eks. Ctrl+Z for angre, Esc for å lukke).

**Målepunkter:** tid; metode (F1, knapp, meny, hjelp); om hen tror det finnes slike (spør: «Hva forventet du å finne?»); om dialogen kan lukkes med Esc; sitat.

**Hint:** «Hvor ville du lett etter hjelp i et program?»

---

## Spørsmål etter alle oppgaver (alle deltakere)

Nøytrale. Se MANUS.md. Notér ordrett.

1. «Hvordan vil du beskrive programmet for en venn?»
2. «Hva var lettest? Hva var vanskeligst?»
3. «Var det noe du var usikker på om du kunne angre?»
4. «Er det noe som fikk deg til å nøle før du trykket?»

## Samlet suksess (utfallskoder)

- **A**: uten hjelp og uten utilsiktet effekt.
- **B**: etter ett hint.
- **C**: ikke fullført, flere hint, eller skadelig utilsiktet effekt.

Går en deltaker glipp av en oppgave på grunn av teknisk feil (ikke deltakerens), noteres «T» og oppgaven regnes ikke med i nevneren.
