# GDPR-vurdering av ansiktsindeksen

**Utkast for juridisk gjennomgang. Ikke juridisk rådgivning.** Status 2026-11-15. Det var ikke nettilgang da dette ble skrevet (`www.datatilsynet.no` er utenfor sesjonens tillatte domener). Ingen primærkilde er lest; alle henvisninger til regelverk og veiledning er fra hukommelsen og merket **må verifiseres** (med kilde og oppslagsdato ført inn ved verifisering).

## 1. Hva ansiktsindeksen er (fra koden)

- Valgfri og av som standard (`face_index_enabled = 0`). Aktiveres via en bekreftelsesdialog (`people.consent.*`).
- Lagrer per ansikt: embedding (f32-vektor), boks (promille), kvalitet, algoritmeversjon, bilde-id, og navn brukeren gir en person. Ingen ansiktsbilder.
- Alt i lokal `index.db`. Ingen overføring, ingen server, ingen telemetri.
- **Per 2026-11-15 er ingen ansiktsmodell valgt eller inkludert** (`docs/v1/MODELLER.md`), så ingen embeddings produseres ennå. Vurderingen må gjentas når en modell er valgt (hva den lagrer, lisens, treningsdata).
- «Slett ansiktsindeks» (`people::wipe`): sletter alt, kan ikke angres, `secure_delete` og WAL-trunkering.

## 2. Omfattes behandlingen? Husholdningsunntaket og leverandørens rolle

- Art. 2(2)(c): GDPR gjelder ikke for en fysisk persons behandling i rent personlig eller husholdningsmessig virksomhet. En privatperson som ordner egne bilder på egen PC faller trolig under unntaket (**må verifiseres**, inkl. hvordan personopplysningsloven § 2 avgrenser).
- Unntaket kan falle bort ved yrkes- eller organisasjonsbruk (fotograf, bedrift, forening) eller hvis bibliotek/indeks deles. Fjell kan ikke se dette.
- Leverandørens rolle: Fjell mottar ingen data og har ingen tilgang. Argument for at utvikleren verken er behandlingsansvarlig eller databehandler (art. 4(7), 4(8)): ingen behandling hos utvikleren. Motargument: utvikleren bestemmer midlene og det innebygde formålet. Praksis om felles behandlingsansvar er vid (**må verifiseres**). Spørsmål til jurist.

## 3. Er embeddings «biometriske opplysninger» (art. 4(14), art. 9)?

- Definisjonen: personopplysninger fra teknisk behandling av fysiske trekk som muliggjør eller bekrefter entydig identifikasjon. Art. 9(1) forbyr behandling av biometriske opplysninger «for å identifisere en fysisk person entydig» uten unntak i art. 9(2).
- Fjells embeddings brukes til å gjenkjenne samme ansikt på tvers av bilder og til å knytte navn. Det er funksjonelt identifikasjon. Fotografier som sådan er ikke biometriske (fortalepunkt 51, **må verifiseres**), men embedding er teknisk behandling som muliggjør identifikasjon.
- At alt er lokalt endrer ikke kategorien, bare risikoen. **Arbeidsantakelse: embeddings behandles som særlige kategorier.**
- Dersom husholdningsunntaket gjelder, er art. 9 ikke direkte anvendelig overfor brukeren. Gjelder det ikke (yrkesbruk), trengs et art. 9(2)-grunnlag (typisk uttrykkelig samtykke fra de avbildede, upraktisk), og brukeren er ansvarlig.

## 4. Samtykkemodell i appen

- Dialogen er brukerens egen aktivering med informasjon (hva lagres, lokalt, originaler urørt, kan slås av og slettes). Det er **ikke** samtykke fra de avbildede og skal ikke kalles det overfor jurist.
- Mangler: opplysning om at andre personers ansikter (barn, venner, kolleger) også indekseres og at navn og embeddings er sensitive. Forslag: én setning i dialogen (eiers beslutning, i18n `people.consent.*`; ikke endret i denne fasen).
- Kjernen håndhever ikke aktiveringen med en type (se `BEHANDLINGSOVERSIKT.md`, funn 2).

## 5. Dataminimering og sikring

Fungerer: av som standard, ingen ansiktsbilder, kun lokalt, sletting med overskriving, ingen eksport, ingen nettverkskode. Forbedringer å vurdere:

- Indeksen er ukryptert (avhenger av Windows-konto og diskkryptering).
- «Slå av» sletter ikke data.
- Wipe når ikke planlagte sikkerhetskopier.
- Ingen utløpstid for ansiktsdata (lagringsbegrensning); vurder påminnelse.
- Embeddings må aldri inn i diagnoserapport, backup-eksport eller annen eksport; regel finnes i `DIAGNOSE.md`, bør håndheves med test når diagnose bygges.

## 6. DPIA (art. 35)

- Typiske utløsere: omfattende behandling av særlige kategorier, ny teknologi, sårbare personer. Datatilsynets liste over behandlinger som krever DPIA inneholder trolig biometri/ansiktsgjenkjenning (**må verifiseres**).
- Plikten gjelder ansvarlig. For husholdningsbruk er det ingen ansvarlig; for leverandøren ingen behandling. **Anbefaling (utkast):** gjør likevel en frivillig DPIA før første utgivelse med ansiktsmodell, som dokumentasjon også for profesjonelle brukere.

## 7. Andre personers ansikter

Biblioteket inneholder uunngåelig barn, familie og tredjeparter. Risiko: kartlegging av hvem som er sammen, uthenting hvis PC eller indeks kommer på avveie, bruk i konflikt. Avbøtende: lokalt, av som standard, wipe, ingen eksport. Åpent: egne hensyn for barn (**må verifiseres**).

## 8. Norsk rett og Datatilsynet

- Personopplysningsloven (2018) gjennomfører GDPR. Datatilsynets veiledning om biometri, ansiktsgjenkjenning og DPIA-liste er **ikke lest: må verifiseres på datatilsynet.no**. EDPB-retningslinje 05/2022 (ansiktsgjenkjenning i rettshåndhevelse) er trolig ikke direkte anvendelig (**må verifiseres**).

## 9. Foreløpig konklusjon

1. Personlig bruk: trolig utenfor GDPR; leverandør uten tilgang til data trolig ikke ansvarlig.
2. Embeddings behandles likevel som biometrisk, og designet følger privacy by design/default.
3. Før en modell leveres: jurist/DPO-gjennomgang, DPIA-vurdering, justert aktiveringstekst, avklaring av backup/wipe.

## Spørsmål til jurist/DPO

1. Gjelder husholdningsunntaket for en app som bygger et biometrisk register over tredjeparter lokalt? Hva hvis brukeren er profesjonell?
2. Er leverandøren (uten tilgang) behandlingsansvarlig, felles ansvarlig eller utenfor?
3. Er embeddings uten navn «biometriske opplysninger»? Endrer navngiving konklusjonen?
4. Hvilke vilkår/informasjon trengs overfor profesjonelle brukere?
5. Er en frivillig DPIA tilstrekkelig, og hva må den inneholde?
6. Hvilken formulering bør aktiveringsdialogen ha om tredjepersoner?
7. Krav til sletting i sikkerhetskopier og ved avinstallering.
8. Betydningen av at ansiktsmodellens treningsdata kan inneholde personopplysninger (lisens og opphav).
9. Lagring i WebView2/`shell.json`: omfattet av ekomloven om informasjonskapsler? (Fjell setter ingen kapsler; må verifiseres.)
10. Oppdateringssjekken: GitHub ser IP-adressen. Er opplysningen i `PERSONVERN.md` tilstrekkelig (tredjelandsoverføring)?

## Beslutninger for eier

1. Typet `FaceConsent` i kjernen? (Berører `api`; utenfor denne fasen.)
2. Skal wipe også omfatte sikkerhetskopier, eller skal kopier utelate ansiktsdata?
3. Tilby sletting av ansiktsdata når indeksen slås av?
4. Kryptert indeks eller kryptering av embeddings?
5. Tilleggstekst om tredjepersoner i aktiveringsdialogen (i18n nb/en).
6. Frivillig DPIA før første modell: hvem eier den?
7. Skal dokumentasjonen erklære at personfunksjonen er ment for personlig bruk?
8. Modell og kjøremotor (se `docs/v1/MODELLER.md`), med lisens og treningsdata verifisert.
9. Diagnoserapportens eksportmåte (krever eventuelt ny dialogtillatelse, eierbeslutning).
