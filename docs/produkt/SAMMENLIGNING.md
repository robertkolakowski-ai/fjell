# Sammenligning: Fjell mot alternativer (ærlig, delvis uverifisert)

Status: ingen konkurrent er kjørt eller sjekket mot nettsider i denne oppgaven. Alle utsagn om andre er fra generell kunnskap (som `docs/INSPIRASJON.md`, «ikke nye nettsøk») og merket **ikke verifisert**. Verifiser før noe av dette brukes utad. Ingen tall, kunder eller markedsandeler er oppgitt.

## Hva Fjell har i dag (kilde: `MVP-PLAN.md`, `PERSONVERN.md`; alt målt kun på Linux, Windows er ikke kjørt)
Eksakte kopier (BLAKE3), like bilder (visuelle fingeravtrykk), uskarphet og skjermbildesignaler · begrunnelsesmerke på hvert forslag · karantene + angre + papirkurv + «Tøm» til Windows Papirkurv · ingen automatisk sletting · helt lokalt, ingen telemetri · norsk og engelsk.

## Hva Fjell IKKE har (ennå)
Ansiktsgjenkjenning/lukkede øyne (planlagt i PRODUKT.md, ikke bygget) · verifisert HEIC/RAW (WIC-kode finnes, ikke kjørt) · kalibrert treffsikkerhet på ekte bilder · signert installasjonsprogram · Mac/iPhone/Android · skyopprydding · søk · albumtildeling · bruker-tester (BETA.md er ikke krysset av).

## Tabell
Symbolene gjelder Fjell mot det som er kjent. «?» = ikke verifisert.

| Egenskap | Fjell | dupeGuru | digiKam | Microsoft Bilder (Windows) | Google Photos | Gratis «duplikatfinner»-verktøy generelt |
|---|---|---|---|---|---|---|
| Plattform | Windows 10/11 (beta) | ? Kryssplattform (ikke verifisert) | ? Kryssplattform (ikke verifisert) | Windows | Nett/mobil | ? |
| Helt lokalt | Ja | ? Antatt ja | ? Antatt ja | ? Delvis sky (ikke verifisert) | Nei (sky) | ? |
| Eksakte duplikater | Ja | ? | ? | ? | ? | ? |
| Like bilder (visuell) | Ja (ikke kalibrert) | ? | ? | ? | ? | ? |
| Uskarphet | Ja (syntetisk testet) | ? | ? | ? | ? | ? |
| Forklarer hvorfor | Ja, merke per forslag | ? | ? | ? | ? | ? |
| Karantene og angre | Ja | ? | ? | ? | ? | ? |
| Ansiktsgjenkjenning | Nei | ? | ? | ? | ? | ? |
| HEIC/RAW | Ikke verifisert | ? | ? | ? | ? | ? |
| Norsk grensesnitt | Ja | ? | ? | ? | ? | ? |
| Pris | Ikke avklart | ? | ? | ? | ? | ? |

Mange «?» er ærlig: de må fylles ut etter at hver app er prøvd. Anbefalt: prøv hver på samme testbibliotek (`core/testdata`) og noter funn.

## Slik bruker vi tabellen
- Fjells løfte er **trygghet og personvern**, ikke flest funksjoner. Det er slik vi bør posisjonere oss; det er et valg, ikke bevist overlegenhet.
- Ikke skriv «bedre enn» noe sted før en test er kjørt og dokumentert.
- Oppdater tabellen når beta-resultater (`BETA.md`) og kalibrering finnes.
