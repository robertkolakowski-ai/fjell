# Nettside: struktur og tekst (utkast)

Status: utkast til produkteier. Ingen side er bygget eller publisert. Teksten holder seg til det appen gjør i dag (se `docs/PERSONVERN.md`, `docs/BETA.md`, `docs/mvp/MVP-PLAN.md`). Siden skal ikke bruke eksterne fonter, CDN eller sporing (samme prinsipp som appen).

## Forbudte påstander (til appen faktisk holder dem)
Ansiktsgjenkjenning · iCloud-/OneDrive-opprydding (skymapper skannes, men flytting til papirkurv der er sperret) · støtte for HEIC/RAW som «virker» (WIC-dekoding er ikke kjørt på Windows) · «frigjør X GB» som løfte · tall på treffsikkerhet (ikke kalibrert på ekte bilder) · signert installasjonsprogram (beta er usignert) · iPhone/Android/Mac (etter MVP) · «KI» (ingen modell brukes; vanlig kode).

## Struktur
1. Hero · 2. Tre fordeler · 3. Slik virker det · 4. Personvern · 5. Last ned · 6. Vanlige spørsmål

## 1. Hero
- nb: **Rydd i bildene dine. Ingenting forsvinner uten at du sier ja.** / Fjell finner like og uskarpe bilder på Windows-PC-en din og foreslår hva du kan rydde. Alt skjer på maskinen din.
- en: **Tidy your photos. Nothing disappears unless you say so.** / Fjell finds duplicate and blurry photos on your Windows PC and suggests what to clean up. Everything happens on your machine.
- Knapp: nb «Last ned beta for Windows» / en «Download the Windows beta». Under: «Beta. Windows 10/11.»

## 2. Tre fordeler
| | nb | en |
|---|---|---|
| Trygg opprydding | Fjell foreslår, du bestemmer. Hvert forslag har en begrunnelse, og originalbildene endres aldri. | Fjell suggests, you decide. Every suggestion shows why, and your original files are never edited. |
| Alt lokalt | Bildene dine forlater ikke maskinen. Ingen konto, ingen opplasting, ingen telemetri. | Your photos never leave your PC. No account, no upload, no telemetry. |
| Angre overalt | Valgte bilder flyttes til en karantenemappe du kan gjenopprette fra. Du tømmer selv, og som standard til Windows Papirkurv. | Chosen photos move to a quarantine folder you can restore from. You empty it yourself, by default into the Windows Recycle Bin. |

## 3. Slik virker det
1. Velg mapper (også eksterne disker). 2. Fjell skanner og viser grupper: eksakte kopier, like bilder, uskarpe bilder, skjermbilder. 3. Gå gjennom forslagene: ett og ett kort, eller flere samtidig. 4. «Flytt til papirkurv» i Fjell. Angre når som helst. 5. Tøm når du er sikker; plassen frigjøres først da.
- Ærlig merknad (nb): «Plassen er ikke frigjort før du har tømt papirkurven.» (en: «Space is not freed until you empty the bin.»)

## 4. Personvern
- nb: Ingen nettverkskall, bortsett fra en valgfri oppdateringssjekk i beta (standard: bare når du trykker). Den sender appversjon og operativsystem, aldri bilder, stier eller filnavn. Se personvernerklæringen og testene som feiler hvis nettverkskall legges inn.
- en: No network calls except an optional update check in the beta (default: only when you press it). It sends app version and OS, never photos, paths or file names. See the privacy statement and the tests that fail if network calls are added.
- Lenk til `PERSONVERN.md` (publisert versjon) og kildekoden (hvis repoet gjøres offentlig; ikke avklart).

## 5. Last ned
Én knapp (NSIS, per bruker, ingen administratorrettigheter). Vis versjon og SHA-256. Beta er usignert: «Windows kan vise «Windows beskyttet PC-en din». Velg «Mer info» og «Kjør likevel».» Fjern den teksten først når signering er på plass (se `SIGNERING.md`). Lenke til GitHub-releasen `beta`.

## 6. Vanlige spørsmål (nb / en kort)
- **Sletter Fjell noe av seg selv?** Nei. Ingen automatisk sletting. / No. Nothing is deleted automatically.
- **Kan jeg angre?** Ja, så lenge filene er i Fjells papirkurv (standard 30 dager, kan endres). / Yes, while files are in Fjell's bin (30 days by default, configurable).
- **Hva med OneDrive/iCloud/Dropbox-mapper?** Fjell kan skanne dem og foreslå, men flytter ikke filer til papirkurv der, så sletting ikke utløses i skyen. Gjenkjenningen går på mappenavn og er ikke prøvd mot ekte skytjenester. / Fjell can scan them but will not move files there.
- **Hvilke formater?** JPEG og PNG er testet. HEIC/RAW: ikke verifisert på Windows ennå. / JPEG and PNG tested. HEIC/RAW: not verified on Windows yet.
- **Koster det noe?** Ikke avklart (se `PRIS-OG-LISENS.md`). Fjern spørsmålet til det er bestemt.
- **Mac, iPhone, Android?** Senere, ingen dato.
- **Bruker dere KI?** Nei, vanlig bildeanalyse (kontrollsummer, visuelle fingeravtrykk, uskarphetsmål) på maskinen din.
