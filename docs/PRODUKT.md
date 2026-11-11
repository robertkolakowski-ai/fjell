# Produkt

## 1. Duplikatdeteksjon (tre nivåer)
- **Eksakte duplikater:** SHA-256 eller xxHash. Trygt å slette med bekreftelse.
- **Visuelle duplikater:** annen oppløsning, komprimering, format eller beskjæring. pHash/dHash og CLIP-embeddings.
- **Serier og nesten-duplikater:** burst, gruppebilder, panoramaforsøk. Grupperes etter tidsstempel, GPS og visuell likhet.

### Valg av vinner
1. Høyest oppløsning og minst kompresjon.
2. Skarpest, best eksponering.
3. Ingen lukkede øyne, alle smiler, alle ser mot kameraet.
4. Metadata (GPS, tekst, favoritt, albummedlemskap) overføres til vinneren.

**UI:** hver gruppe er en rad med «Behold» markert og en kort forklaring («Skarpere, og øynene er åpne»). Vinneren byttes med ett trykk.

## 2. «Dårlige bilder»-filter
Hver kategori har egen score og justerbar følsomhet.
- Uskarpe: Laplacian-varians + lært modell
- Over-/undereksponerte: histogramanalyse
- Lukkede øyne og dårlige uttrykk: ansiktslandemerker
- Utilsiktede bilder: lommebilder, finger over linsen, gulv/tak, hvit vegg
- Skjermbilder, kvitteringer, dokumenter, memes, WhatsApp-videresendte (OCR for kvitteringer)
- Lav estetisk verdi: NIMA-lignende modell som rangerer, ikke dømmer
- Skjermopptak, GIF-er og store videoer, sortert etter plassbruk

**Prinsipp:** kandidater har begrunnelsesmerke («Uskarp (82 %)») og er aldri forhåndsvalgt hvis bildet har ansikter av personer du ofte fotograferer.

## 3. Smart sortering og organisering
- Automatiske samlinger: Personer, Steder, Hendelser, Dyr, Mat, Dokumenter, Natur, Skjermbilder
- Tidslinjeopprydding («Hellas, juli 2025, 412 bilder»)
- Swipe-modus: opp = slett, venstre = behold, høyre = album, lang trykk = sammenlign. Haptikk og lyd.
- Regelmotor: «Skjermbilder eldre enn 90 dager kan arkiveres»
- Søk med naturlig språk: «hund på stranden», «kvittering fra IKEA»
- Mappestruktur for filbaserte bibliotek: `ÅÅÅÅ/ÅÅÅÅ-MM-DD_Hendelse/`

## 4. Culling-modus for fotografer
- RAW+JPEG-par som ett motiv
- Seriegruppering med beste-bilde-forslag
- Sammenligning av 2–4 bilder med synkronisert zoom
- Fokusmaske og «zoom til øyne»
- Stjerner, fargeetiketter, flagg (behold/forkast) med hurtigtaster
- Eksport til Lightroom (XMP-sidecar) og Capture One

## 5. Lagringsforvalter
- Dashboard: brukt plass, plass som kan frigjøres, fremdriftsring
- Komprimeringsforslag («Konverter 4 000 HEIC/JPEG uten synlig tap»)
- Skyoptimalisering: lavoppløselig lokalt, original i skyen
- Kategori «Store filer» over terskel

## Designprinsipper
- Rolig og tydelig. Aksentfarge bare til primærhandling.
- Bildet er hovedperson. Fullskjerm som standard.
- Én ting om gangen: guidede flyter.
- WCAG AA, dynamisk tekststørrelse, VoiceOver/TalkBack, knappealternativ til alle gester.
- Norsk (bokmål, nynorsk) og engelsk fra dag én.

## Hovedskjermer
- **Hjem:** stor ring «X GB kan frigjøres», kategorikort, «Ukens opprydding» (2 min)
- **Gjennomgang:** rutenett/kortstabel, fast bunnlinje «Slett 87 (1,2 GB)»
- **Gruppevisning:** karusell, «Beste» markert, zoom for forskjeller
- **Swipe-modus:** kortstabel med handlingsfarger og haptikk
- **Sammenligning:** side om side, synkronisert zoom, skarphetsoverlegg
- **Samlinger:** personer, steder, hendelser, albumforslag
- **Papirkurv og historikk:** «Du har frigjort 42 GB siden mars», gjenopprett alt
- **Innstillinger:** følsomhet per kategori, beskyttede album/personer, regler, sikkerhetskopi

## Onboarding (60 sekunder)
Gi tilgang → raskt skann → «312 eksakte duplikater (2,1 GB)» → ett trykk for å rydde. Brukeren får en seier først.

Etter hver økt: rolig oppsummering («Du frigjorde 3,4 GB og 612 bilder»), uten påtrengende poengsystemer.
