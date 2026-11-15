# V2: eksport til Lightroom (XMP) og katalogvennlige formater

Status: plan (ingen kode). Hører til `V2-PLAN.md` del 3.4. Bindende regler: `CLAUDE.md` (originalers innhold endres aldri; metadata skrives tilbake bare når brukeren velger det; alt kan angres; on-device).

## 1. Hva eksporteres

Fra `rating`: `stars` (0–5), `flag` og `color`. Sidefilen hører til RAW-filen i et par (V2-PLAN spørsmål 6). Ingenting annet: ingen hasher, ansiktsdata, embeddings eller stier til andre bilder.

## 2. Formater

| Format | Innhold | Status |
|---|---|---|
| XMP-sidefil `<grunnnavn>.xmp` | `xmp:Rating` (−1 = avvist, 0–5 etter XMP-spesifikasjonen) og `xmp:Label` | Standard |
| CSV | én rad per bilde: relativ sti, stjerner, flagg, etikett, tidsstempel | Reserve |
| JSON | samme felt, versjonert (`schema: 1`); brukes også som sikkerhetskopi av vurderinger | Reserve |
| Capture One | leser trolig `xmp:Rating` ved import (ikke verifisert); egen katalog | Stretchmål |

**Flagg:** `xmp:Rating = -1` for «forkast» er standardisert. «Behold» (pick) har ingen sikker XMP-form som Lightroom leser, fordi flagg bor i katalogen. Eksporten skriver bare det som er verifisert; «behold» havner i CSV/JSON og, hvis brukeren velger det, som `xmp:Label`. Ikke verifisert mot Lightroom Classic eller Bridge her.

## 3. Moduser og trygghet

1. **Til egen mappe (standard):** sidefiler skrives i en mappe brukeren velger, speilet etter relativ sti. Ingen fil i bibliotekene røres.
2. **Ved siden av originalene (eget valg):** det Lightroom forventer. Forhåndsvisning først («412 sidefiler skrives, 3 finnes fra før og flettes, 0 overskrives») og bekreftelse.
3. Skriving er atomisk (temp + rename i samme mappe) og rører aldri bildefilen. Skrivebeskyttet mappe eller fil gir feil per fil, ikke avbrudd av hele jobben; rapporten viser utfall per fil.
4. **Flett, aldri erstatt:** en eksisterende `.xmp` leses; ukjente navnerom og noder (f.eks. `crs:` fra Lightroom) bevares; bare feltene Fjell eier endres. Korrupt XML gir «hoppet over», aldri overskriving.
5. **Angre:** `export_file` husker om hver fil var ny eller flettet. Før en eksisterende sidefil endres kopieres den til appens historikkmappe (`%LOCALAPPDATA%\Fjell\export-historikk\<jobb>\`). Angre flytter sidefiler Fjell selv opprettet til karantene (ikke hard sletting) og gjenoppretter flettede fra historikken byte-lik (A15). Historikken tømmes først ved eksplisitt «Tøm» med bekreftelse.
6. Ingen embedded XMP: ingen skriving inn i JPEG, DNG eller andre originaler. Ingen nettverk.

## 4. Lightroom i praksis (ikke verifisert)

Lightroom Classic leser ikke endrede sidefiler automatisk; brukeren velger «Read Metadata from Files» eller importerer på nytt. UI må si dette («Slik leser du dem inn i Lightroom») på nb og en. Hvilke felt som faktisk kommer inn må testes manuelt i V2-W.

## 5. Tester

- Golden XMP: ny fil, eksisterende fra Lightroom med `crs:`-noder (bevares), korrupt, tom, skrivebeskyttet, Unicode, lang sti.
- Rundtur: skriv, les, lik. Streng XML-parser, ikke regex.
- Angre: sjekksum av alle sidefiler og kataloginnhold før og etter er lik.
- Originaler: sjekksum uendret (A12). Test at ingen skrivekall går mot filer med bildeendelse.
- CSV: komma, anførselstegn, linjeskift og Unicode i sti; ingen CSV-injeksjon (felt som starter med `=`, `+`, `-`, `@` prefikses).
- Avhengighet: ny XML-crate må være MIT/Apache og bestå `cargo deny` (ikke verifisert her).
