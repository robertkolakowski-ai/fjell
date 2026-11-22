# Trygghetsmodell

Sletting er det som får folk til å miste tilliten. Sikkerhet er et hovedtema.

1. **Papirkurv først:** alt havner i «Nylig slettet» i 30 dager (konfigurerbart). Tømmes aldri automatisk uten varsel.
2. **Angre overalt:** hver gruppe og hver swipe. Angre gir samme gruppe tilbake (samme id og keeper-valg): en gruppe som
   oppløses fordi et medlem ligger i papirkurven, beholdes uten forslag til bildet er tømt (`store::groups`, testen
   `undo_brings_back_the_same_group_id_and_keeper`).
3. **Forhåndsvisning av konsekvenser:** «Du sletter 124 bilder (1,8 GB). 3 av dem er i albumet «Bryllup»».
4. **Beskyttede bilder:** favoritter, delte album, bilder med mange ansikter fra nære personer og redigerte bilder krever eksplisitt bekreftelse.
5. **Modifiser aldri originalen:** metadata i egen indeks, skrives tilbake bare når brukeren velger det.
6. **Tørrkjøring:** «Vis meg hva appen ville gjort».
7. **Sikkerhetskopi-sjekk:** advarsel før masse-sletting hvis bildene ikke er sikkerhetskopiert.

## AI-feilvurdering
Forklaring per forslag, justerbar terskel, aldri automatisk sletting.

## Plattform
iOS krever systembekreftelse ved sletting. Samle sletting i store batcher og forklar dialogen på forhånd.
