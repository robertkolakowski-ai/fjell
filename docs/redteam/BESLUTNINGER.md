# Beslutninger fra red team

## F1/F2/F4: flyt mellom skall, indeks, karantene og sletter (rettet i produksjon)

Invariant: «Tøm» sletter bare filer som på Tøm-tidspunktet er i «Nylig slettet» OG som karantenen faktisk
inneholder. Etablert slik (regresjonstester gjennom `Shell`: `desktop/src-tauri/shell/src/tests_flow.rs`):
- F1: `Core::relocate` bruker resultatet fra `unstage` og fra oppstartsavstemmingen (`Reconciled::Restored`) og
  oppdaterer `asset.source_id` (`Store::relocate_source`, `Fjell::relocate_source`) når filen fikk kollisjonsnavn.
- F2: `FsDeleter::path_for` har ingen fallback til opprinnelig sti når karantene er konfigurert. `Core.flow`
  (mutex) holdes over hver sammensatt overgang (trash + flytting, angre, gjenoppretting, «Tøm», avstemming).
- F4: `Core::undo` legger filer som er `trashed` i indeksen, men ikke i karantenen, tilbake i karantenen.

### Reprodusentene F1 og F4 i `redteam_fable.rs` står fortsatt ignorert (avklar)

Hjelperne `restore_undone`/`undo` i testfilen er en kopi av skallets gamle lim (kaster `unstage`-resultatet og
re-stager ikke). De kaller ikke produksjonskoden som er rettet, og kan ikke bli grønne uten at hjelperne endres
(ikke assertions). Forslag: la hjelperne kalle `fjell.relocate_source` og re-stage slik `Core` gjør, eller flytt
limet til en delt modul som både skallet og testene bruker. Endring i `quarantine.rs` hjelper ikke.

### Brukersynlig følge av F2 (valgt trygg retning, kan omgjøres)

«Merket, ikke flyttet» (flytting til karantene feilet: låst fil, antivirus) slettes ikke lenger av «Tøm»; den
teller som `kept_trashed` og kan gjenopprettes. Før ble den slettet direkte fra biblioteket. Alternativ: tillat
sletting fra opprinnelig sti bare når størrelse og `mod_token` stemmer med indeksen. Avklar ønsket atferd og tekst.
«Tøm» holder dessuten låsen til den er ferdig, så angre/gjenopprett/trash venter mens en stor tømming pågår.

### Kjente resthull (ikke rettet, krever valg eller `quarantine.rs`)

1. Krasj mellom tilbakeflytting til kollisjonsnavn og indeksoppdatering: manifestet vet ikke det nye navnet, så
   indeksen peker på den fremmede filen til neste skanning (endret `mod_token`). Forslag: `unstage` skriver
   målnavnet i manifestet før `rename`, og avstemmingen fullfører.
2. Kollisjonsnavnet er opptatt i indeksen av en annen rad (f.eks. et bilde i karantene med nettopp det navnet):
   `relocate_source` avviser, og indeksen står igjen på gammel sti.
3. `stage` identifiserer filen bare på sti. Forslag: sammenlign størrelse/`mod_token` med indeksen før flytting
   («forslaget gjaldt et annet innhold» gir «Merket, ikke flyttet»). Det lukker også punkt 1 og 2.
4. Oppstartsavstemmingen re-stager ikke «trashed, men ikke i karantene» (krasj midt i angre av gjenoppretting);
   filen blir «Merket, ikke flyttet» og slettes ikke (trygt, men ikke selvrettende).

## RT-02: gjenbrukt id i `stage` (delvis rettet, testen står ignorert)

Gjort: `stage` svarer nå `Err(Failed)` når id allerede er i karantene for en annen sti (aldri Ok uten flytting), og
samme id to ganger i ett kall gir `Err` på det andre. Samme id og samme sti er fortsatt idempotent `Ok`.
Reprodusentens siste assertion krever likevel at `new.jpg` faktisk flyttes og at `locate(1)` peker på den. Det
forutsetter at den gamle oppføringen for id 1 erstattes, og `old.jpg` blir da foreldreløs i karantenen. Det er et
designvalg. Forslag: kallerne (indeksen) tildeler aldri id på nytt; da er avvisning riktig, og testen bør endres til å
godta `Err` uten flytting. Avklar.

## RT-04/F5: symlenker i skanning (konservativt valg)

`FsSource::walk` hopper nå over symlenker (filer og mapper) og følger dem aldri. Brukersynlig: album som er
symlenket inn i biblioteket skannes ikke lenger. Avklar om de skal støttes via kanonisk deduplisering.
`stage` avviser dessuten filer hvis kanoniserte foreldermappe ligger utenfor biblioteket (NotFound).
Skallets tekstbaserte skysperre (`cloud_provider`) er uendret (utenfor omfang).

## Kritikerrunden (KR-1..KR-8)

- KR-1 rettet: `FsQuarantine::locate_for(id, original)`; `FsDeleter::path_for` sletter bare når manifestets `original`
  er indeksens sti for id-en (reprodusent `kr1_*` i `fjell-fs/tests/redteam_review.rs`, nå grønn).
- KR-2 rettet: skanning av karantenen hopper over batchmapper som er symlenker (`kr2_*`, nå grønn). Ikke dekket:
  `.fjell-karantene` selv som symlenke, og Windows-junctions.
- KR-3 årsak rettet: `config::db_path` kanoniserer biblioteksstien (frakoblet: leksikalsk normalisering), så
  stavemåtene `lib` og `lib/.` gir samme indeks (`kr3_to_stavemater_gir_samme_indeks`, grønn). Reprodusenten
  `kr3_samme_mappe_...` står FORTSATT ignorert: oppsettet forutsetter to indekser og stopper nå på
  «oppsett: duplikatkort». Mulig årsak, ikke undersøkt: ny gruppe i samme indeks skjules av en gammel beslutning
  (per_kind sier 1 gruppe, `cards` gir 0). Hvis det stemmer er det en egen, brukersynlig feil. Bivirkning: eksisterende
  indeksfiler hashet på ukanonisk sti blir ikke funnet igjen (Windows: `\\?\`-prefiks). Avklar migrering før utgivelse.
- KR-4..KR-7 utsatt (lav alvorlighet, ikke gjort): drop_entries mister usikre manifestlinjer; skysperre på
  kanonisert sti (KR-5); NotFound for «utenfor biblioteket» og canonicalize-feil på Windows (KR-6);
  flow-lås per Core og skanning uten lås (KR-7). Ytelse for angre under stor Tøm er ikke målt.
- KR-8: F1/F4-reprodusentene i `redteam_fable.rs` er uendret og røde (hjelperne kopierer gammelt skall-lim).
