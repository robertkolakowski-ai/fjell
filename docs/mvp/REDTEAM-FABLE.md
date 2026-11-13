# Red team mot sikkerhetsmodellen (Fable)

- Modell (slik sesjonen viser den): Fable 5.2 (`claude-fable-5-2`)
- Gren/arbeidstre: `m11/kjerne-herding`, 2026-11-13
- Funn: 5. Reproduserbare tester: 5 (alle feiler på dagens kode med `--ignored`, alle på selve bruddpåstanden).
- Tidsbruk: ca. 4 min veggklokke fra første kodelesing til siste testkjøring (målt 239 s), pluss rapport.
- Ingen kildekode er endret og ingenting er fikset. Den andre red-team-agentens filer er ikke lest.

## Slik kjøres reprodusentene

```
cd core
cargo test -p fjell-cli --test redteam_fable -- --ignored   # 5 feiler (forventet)
cargo test -p fjell-cli --test redteam_fable                # 0 kjørt, 5 ignorert (CI forblir grønn)
```

Fil: `core/crates/fjell-cli/tests/redteam_fable.rs`. Kjørt på Linux. Ikke kjørt på Windows.

**Forbehold om nivå:** Limet mellom indeks og filsystem (`stage_trashed`, `restore_undone`, `empty`,
`undo`) ligger i skallet (`desktop/src-tauri/shell/src/core.rs` og `lib.rs`), som er utenfor mitt
område og ikke kan nås fra `fjell-cli`. Testene gjentar derfor skallets kall i samme rekkefølge mot
ekte `Fjell`, `FsSource`, `FsQuarantine` og `FsDeleter`. F3 og F5 treffer `fjell-fs` direkte.
F1, F2 og F4 bør i tillegg bekreftes med en test gjennom `Shell` før de fikses.

## Funn

| Id | Alvor | Kort | Sted | Test |
|----|-------|------|------|------|
| F1 | Høy | Gjenoppretting ved navnekollisjon oppdaterer ikke indeksen; neste trash + «Tøm» sletter en fil som aldri ble foreslått | `shell/src/core.rs:178–193` (resultatet av `unstage` kastes, linje 191), `fjell-fs/src/quarantine.rs` `restore_target`/`move_back` | `f1_kollisjon_ved_angre_gjor_at_tom_sletter_en_uforeslatt_fil` |
| F2 | Høy | «Tøm» sletter en fil som ble gjenopprettet etter `confirm_purge` | `fjell-fs/src/delete.rs:124–129` (`path_for` faller tilbake til opprinnelig sti), `shell/src/core.rs:196–259` (ingen `check_ticket` per fil, ingen lås mot `restore`) | `f2_tom_sletter_fil_som_ble_gjenopprettet_etter_bekreftelsen` |
| F3 | Middels | Manifestfeltet `stored` valideres ikke; «Tøm» kan slette en fil utenfor karantenen og biblioteket | `fjell-fs/src/quarantine.rs` `locate` (`d.join(&e.stored)`), også `unstage`/`reconcile` | `f3_manifest_stored_med_dotdot_sletter_fil_utenfor_biblioteket` |
| F4 | Middels | Angre av «Gjenopprett» gir `trashed` i indeksen uten at filen legges tilbake i karantenen | `shell/src/lib.rs:352–359` (`undo` kaller bare `restore_undone`), `fjell-core/src/store/ops.rs:247–252` | `f4_angre_av_gjenopprett_legger_ikke_filen_tilbake_i_karantenen` |
| F5 | Middels | Symlenket/junction-koblet synkmappe inne i biblioteket omgår skysperren | `shell/src/lib.rs:236–243` (bare bibliotekstien som tekst), `fjell-fs/src/source.rs:144` (`is_dir` følger lenker), `quarantine.rs` `stage` | `f5_symlenket_synkmappe_i_biblioteket_omgar_skysperren` |

### F1 – feil fil slettes etter gjenoppretting ved kollisjon (høy)

Forløp: bilde X (`b/IMG_0001.jpg`) legges i papirkurven og flyttes til karantene. En ny fil med
samme navn havner på den opprinnelige stien (kameraet teller fra 0001 igjen, eller brukeren kopierer
inn fra sikkerhetskopi). Angre/Gjenopprett legger X som `IMG_0001 (gjenopprettet).jpg`, men
indeksens `source_id` for X peker fortsatt på `b/IMG_0001.jpg`. Kortet er ventende igjen. Tar
brukeren kortet på nytt og tømmer, flyttes og slettes den NYE filen.

Konsekvens for brukeren: et bilde som aldri er skannet eller foreslått, havner i Windows Papirkurv
(permanent borte ved permanent modus), mens det foreslåtte duplikatet blir liggende. Bryter «appen
foreslår», «undo gjenoppretter eksakt tilstand» og «indeks og karantene skal stemme». Krever at det
ikke skannes mellom angre og ny trash; skallet skanner ikke automatisk der.

### F2 – «Tøm» sletter en gjenopprettet, aktiv fil (høy)

`FsDeleter` sletter fra karantenestien hvis id-en ligger der, ellers fra opprinnelig sti. Det siste
er ment for «Merket, ikke flyttet», men gjelder også når filen nettopp er gjenopprettet. Skallets
kommandoer deler bare en lese-lås, så `restore`/`undo` kan kjøre mens `empty` står i sletteløkken
(vinduet er hele løkken, lenge ved tusenvis av filer). `finish_purge` svarer da `PurgeStale`, så
indeksen sier `active` mens filen er borte.

Konsekvens: brukeren får «gjenopprettet», men filen ligger i Papirkurv, eller er permanent slettet
i permanent modus. Testen viser rekkefølgen deterministisk i én tråd. Selve kappløpet mellom to
tråder i skallet er ikke kjørt, bare utledet av koden (`Shell::parts` tar lese-lås og kloner `Arc`).

### F3 – `stored` i manifestet er uvalidert (middels)

`original` går gjennom `resolve` (avviser `..` og absolutte stier), `stored` gjør det ikke. Med
`"stored": "../../../viktig.txt"` slettet «Tøm» (permanent modus) en fil utenfor biblioteket. Krever
at manifestet er endret (delt nettverksdisk, synkkonflikt, skadet fil, annen programvare), derfor
middels. Samme felt brukes av `unstage`/`reconcile`, som da ville flyttet en fremmed fil inn i
biblioteket (ikke testet).

### F4 – angre av «Gjenopprett» stemmer ikke med karantenen (middels)

Etter trash → Gjenopprett → Angre står X i «Nylig slettet», men filen ligger synlig i biblioteket.
Oppstartsavstemmingen svarer `NotMoved` og retter ingenting. «Tøm» sletter filen direkte fra
biblioteket. Ingen data går tapt uten «Tøm», men brukeren ser filen i Utforsker og kan tro den er
trygg. Bryter «undo gjenoppretter eksakt tilstand». Samme sted nullstilles også fristen
(`expires_at = nå + frist`, ikke den opprinnelige), i trygg retning; ikke egen test.

### F5 – skysperren omgås via lenket undermappe (middels)

Biblioteket `…/bibliotek` har undermappen `album` som er symlenke til `…/OneDrive/Bilder`.
`cloud_provider(bibliotek)` gir `None`, skanningen lister `album/ferie.jpg`, og `stage` flytter
filen ut av OneDrive-mappen. Synkklienten oppfatter det som sletting og sletter i skyen og på andre
enheter. Testet med symlenke på Linux. Windows-junction/symlenke er ikke kjørt, og om `rename` ut av
en junction lykkes der er ikke verifisert. Sperren er dokumentert som heuristikk
(`fjell-fs/src/cloud.rs`), så dette er et hull i heuristikken, ikke i en garanti.

## Undersøkt uten funn

- `actions` (plan/commit/purge): gammel plan, digest-brudd, duplikate id-er, tom liste, brukt eller
  utløpt billett, id utenfor billetten, `finish_purge` uten `Deleted`. Lest, ingen vei til `purged`
  utenom billett funnet via `FjellApi`. (`Store::apply_batch` med `RowOp::Purge` er offentlig i
  craten, men ikke nådd fra skallet.)
- Gruppevern (I8): `WouldEmptyGroup` sjekkes mot aktive medlemmer i `plan`, og `decide` planlegger
  og committer under samme lås. Fant ingen rekkefølge av trash/gjenopprett/angre som tømmer en gruppe.
- Angre etter «Tøm»: `NotUndoable` når øverste batch har purge; ingen gjenoppliving av `purged`.
- `PermanentAck`: 0, uten avkrysning, feil antall gir avvisning før noe kalles. `Unsupported` og
  frakoblet rot faller ikke tilbake til permanent.
- Utløp: `expire_due` skriver bare varselstidspunkt. `trash_days` 0 og `u32::MAX` gir ingen overflyt.
- `resolve`: `..`, tomme ledd og ledende `/` avvises. Windows-former (`C:\…`, `\` som skille) i
  `SourceId` er IKKE prøvd, fordi Windows ikke kan kjøres her.
- Krasj mellom manifest og flytting / mellom indeks og flytting: lest `reconcile`; fant ingen
  tapsvei. Dobbel feil (tapt manifest OG nullstilt indeks med gjenbrukte id-er) kan i teorien la
  `stage` overskrive en foreldreløs `<id>.jpg` i en batchmappe; ikke reprodusert, ikke talt som funn.
- `restore_target`: kollisjonsnavn uten overskriving; kappløpet mellom `exists` og `rename` er ikke testet.
- Ikke undersøkt: `windows.rs` (ekte Papirkurv), `drives.rs`, FFI-laget, miniatyrbufferen, pipeline
  utover fjerningsavstemming, og ekte parallelle skallkall.

## Til sammenligningen med P3

Tell reproduserbare funn (5 her), falske alarmer etter gjennomgang, og tokenbruk fra orkestreringen
(jeg har ikke tall for egen tokenbruk). Én kjøring sier lite om modellen.
