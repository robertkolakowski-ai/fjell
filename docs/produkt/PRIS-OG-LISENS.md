# Pris og lisens (beslutningsgrunnlag)

**Beslutningen er produkteiers.** Dette er et grunnlag, ikke et vedtak. Ingen priser, tall eller markedsdata er funnet eller brukt; alle beløp må settes av produkteier. Ingenting er bygget. Appen har i dag ingen lisenskode og ingen konto.

## Rammer fra produktet (ufravikelige)
- Appen kjører uten nett og uten konto; ingen nye nettverkstillatelser (`nonet.test.ts` skal ikke svekkes). Lisenskontroll må derfor være **helt offline**.
- Sikkerheten er aldri bak betalingsmur: papirkurv, angre og «ingen automatisk sletting» gjelder alle nivåer. Det som sperres må være bekvemmelighet, ikke trygghet.
- Prøveperiode må ikke kreve nettverk eller konto.

## Alternativer
| | A. Engangskjøp | B. Abonnement | C. Gratis grunnversjon + Pro |
|---|---|---|---|
| Fordeler | Passer «alt lokalt» og enkel historie; ingen løpende kontakt med kunden; enkel offline-lisens | Jevn inntekt som finansierer vedlikehold (Windows-oppdateringer, WIC/kodeker) | Lav terskel, gir omdømme og testere; Pro kan sperre bekvemmelighet (store mapper, flere kategorier) |
| Ulemper | Ingen gjentakende inntekt; større oppgraderinger krever ny pris eller gratis for alltid | Strider mot «alt lokalt»: abonnement uten nettverk kan ikke håndheves; bruker kan oppleve det som skuffende for en app som kjøres sjelden | Må trekke en grense uten å svekke trygghet; to tilstander å teste og støtte |
| Teknisk | Signert lisensfil, offline | Krever enten nettverkssjekk (stridig) eller utløpsdato i nøkkel (omgås ved å stille klokken) | Funksjonsflagg + offline-nøkkel |
| Risiko | Lisensdeling | Churn, support | Grensen oppleves urimelig |

## Anbefaling (til vurdering)
**C (gratis grunnversjon + Pro som engangskjøp)**, men bare hvis Pro-grensen kan trekkes uten å røre trygghet. Begrunnelse: passer personvernløftet, og en rydde-app brukes sjelden så abonnement er vanskelig å forsvare. Begrunnelsen er et resonnement, ikke målt. Hvis ingen god grense finnes: A.

## Teknisk det krever (alle alternativer unntatt «helt gratis»)
1. **Offline verifisering:** lisensnøkkel = signert nyttelast (f.eks. Ed25519: kunde-ID/e-post-hash, produkt, utstedt dato, evt. utløp). Offentlig nøkkel innebygd i appen; privat nøkkel utenfor repoet (aldri i fil eller CI-logg). Verifisering i Rust-kjernen, ingen nett. Ny avhengighet krever eksplisitt godkjenning (`cargo-deny`, `THIRD_PARTY.md`).
2. **Lagring:** nøkkelen i brukerens innstillinger lokalt; ingen sending.
3. **Prøveperiode:** startdato lagres lokalt ved første kjøring. Kan omgås (slette fil, stille klokke); akseptert, ikke bekjempet med nettverk eller skjult tilstand.
4. **Utstedelse:** utenfor appen (betalingsleverandør og manuell/skriptet nøkkelutstedelse). Leverandør, MVA og personvern for kjøpsdata er ikke avklart.
5. **Grense for Pro (kun forslag):** for eksempel antall mapper eller kategorier; aldri Angre, Gjenopprett eller begrunnelsesmerker.
6. Personvernerklæringen må oppdateres hvis noe lagres eller sendes (`PERSONVERN.md`, «Endringer»).

## Åpne spørsmål til produkteier
Hvem selger (enkeltpersonforetak? MVA?) · pris og valuta · hva er Pro · gratis for beta-testere? · hva skjer ved nøkkeltap · åpen kildekode og lisens for koden (ingen LICENSE-fil finnes i repoet nå).
