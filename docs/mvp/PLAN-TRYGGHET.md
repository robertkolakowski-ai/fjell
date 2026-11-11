# Plan: Trygghet ved sletting (MVP)

Status: forslag fra spesialist TRYGGHET. Bygger på `docs/TRYGGHET.md` og `docs/mvp/ARKITEKTUR.md` (bindende). Der denne planen krever skjemaendringer, står de i «Grensesnitt mot andre planer». Terskler er startverdier som kalibreres. Skjemakrav i «Grensesnitt» er besluttet i ARKITEKTUR (beslutning 17–19).

## 1. Prinsipper og avgrensning

| # | Prinsipp | Konsekvens |
|---|---|---|
| P1 | Ingen stille sletting | Fysisk sletting skjer bare via «Tøm», to bekreftelser (Fjell + systemet), aldri via tid, forslag eller angre. |
| P2 | Alt som kan angres, kan angres | Papirkurv er indekstilstand (`asset.state`), én transaksjon per batch. |
| P3 | Dry-run og ekte kjøring bruker samme kode | `plan()` er ren; `commit()` anvender nøyaktig det planen viste. |
| P4 | Ærlig om lagring | «Merket for sletting» frigjør ingenting. Tre tall i UI: merket, kan frigjøres, frigjort. |
| P5 | Kjernen sier hva, UI sier hvordan | Kjernen returnerer koder og tall (`ConsequenceCode`), aldri tekst. |
| P6 | Forhåndsvisning bindes til utførelsen | Bekreftelse gjelder én fast mengde (digest). Endres mengden, må brukeren bekrefte på nytt. |

Utenfor MVP: ansiktsgjenkjenning («nære personer», krever modell, jf. beslutning 3), skrive metadata tilbake, desktop/Tauri, automatisk sletting (finnes ikke, heller ikke senere uten ny beslutning).

## 2. Tilstandsmaskiner

### 2.1 Asset-tilstand (lagret i `asset.state`)

```
active(0) --trash--> trashed(1) --restore/undo--> active(0)
trashed(1) --purge (Confirmed + plattform OK)--> purged(2)   [terminal]
active(0) --avstemming: borte fra kilden--> purged(2)        [cause=external]
```

| Fra | Hendelse | Vakt | Til | Skriver |
|---|---|---|---|---|
| active | `trash` | ikke beskyttet, eller `ProtectedAck` dekker id | trashed | `trashed_at`, `expires_at = now + trash_days`, logg `op=trash` |
| trashed | `restore` / `undo` | kilden har fortsatt bildet | active | nuller `trashed_at`/`expires_at`, logg `op=restore` |
| trashed | `expire_due` | `expires_at <= now` | trashed | kun `expired_notified_at`; **ingen tilstandsendring** |
| trashed | `finish_purge` (Deleted) | gyldig `Confirmed` + plattform meldte slettet | purged | logg `op=purge` |
| trashed/active | avstemming (skanning) | `source_id` finnes ikke lenger | purged | logg `op=purge`, `cause=external` |
| purged | alt | - | feil `AlreadyPurged` | ingenting |

«Utløpt» er ikke en egen tilstand, bare `trashed` med `expires_at` passert. Dermed kan tid aldri føre til sletting.

### 2.2 Slettehandling (arbeidsflyt per batch, kjøres av `api`)

```
Draft -> Planned -> [AckPending] -> Committed -> Undone
                                        |
                                        +-> (senere) Purge-flyt
Purge-flyt:
Selected -> Explained -> Confirmed -> AwaitingSystem -> Purged | PartiallyPurged | Aborted
```

| Tilstand | Betyr | Overgang | Vakt / effekt |
|---|---|---|---|
| Draft | Brukervalg (swipe, gruppeforslag, «velg alle uskarpe») | `plan()` | Ingen skriving. |
| Planned | `Plan{ids, summary, digest, codes}` finnes, intet er endret | `commit()` | Planen er ikke eldre enn 5 min og digest stemmer med dagens tilstand, ellers `PlanStale` (ny plan). |
| AckPending | Planen inneholder beskyttede bilder eller er massesletting | `commit(acks)` | `ProtectedAck(ids)` per kategori; `MassAck` ved terskel (se 4). Uten dekkende ack: `AckRequired(codes)`. |
| Committed | Én transaksjon: tilstand + `action_log` med nytt `batch_id` | `undo()` | Alt eller ingenting. |
| Undone | Invers batch skrevet (`inverse_of = batch_id`) | - | Hopper over bilder hvis tilstand ≠ tilstand etter batch (rapporteres). |
| Selected | Brukeren åpner «Nylig slettet» og velger (standard: utløpte, øvrige må velges) | `begin_purge(ids)` | Bare `trashed`. Gir `PurgeIntent{summary, digest}`. |
| Explained | UI har vist forklaring av systemdialogen (antall, at dialogen er systemets, at den ikke kan angres i Fjell) | `confirm_purge(intent)` | Brukeren trykker eksplisitt. Gir `PurgeTicket` (omslutter `Confirmed`, gyldig 10 min, i minnet). |
| Confirmed | Fjells bekreftelse gitt | UI kaller adapter `delete(ids)` | Ticket brukes én gang. |
| AwaitingSystem | iOS `PHAssetChangeRequest.deleteAssets` | adapter returnerer per id: Deleted / Cancelled / Failed | Kjernen er ikke involvert. |
| Purged / PartiallyPurged | `finish_purge(ticket, results)` | - | Bare id-er med Deleted settes til `purged`. Resten forblir `trashed` (ingen tap). |
| Aborted | Bruker avbrøt systemdialogen, ticket utløp, eller app døde | - | Ingen tilstandsendring. Avstemming tar eventuelle slettinger som skjedde likevel. |

Krasj mellom plattformsletting og databaseskriving: neste skanning ser at `source_id` er borte og avstemmer til `purged` (`cause=external`). Ingen egen journal trengs.

## 3. Papirkurv, angre, utløp

| Tema | Beslutning |
|---|---|
| Papirkurv | `asset.state = trashed`. Originalen røres ikke. Bildet ligger fortsatt i galleriet til «Tøm». UI må si det («Merket, ikke slettet fra telefonen»). |
| Angre | Stabel av batcher. `undo()` inverterer siste ikke-angrede batch i én transaksjon. Både swipe (1 bilde) og gruppe (n bilder) er én batch. Batch som inneholder `purge` kan ikke angres (`NotUndoable`), UI skjuler knappen. Angre gjelder i økt og ved neste oppstart (siste batch ligger i `action_log`). |
| Idempotens | `trash` på allerede trashed er no-op uten loggrad. Dobbelt `undo` inverterer ikke to ganger. |
| Frist | `trash_days` = 30 (setting). Endring gjelder bare nye elementer (forkorting skal aldri plutselig utløpe eksisterende). |
| Utløp | `expire_due()` kjøres ved oppstart og i bakgrunnsskanning. Returnerer `ExpiryNotice{count, bytes}` én gang per element (`expired_notified_at`). UI viser lokalt varsel og banner. Elementene blir liggende og kan gjenopprettes. Kjernen sletter aldri. |
| Tøm | Hele eller delmengde. Utløpte er forhåndsvalgt, ikke-utløpte må velges. |
| Lagring | `TrashSummary{trashed_count, trashed_bytes, expired_count}`. «Kan frigjøres» = `trashed_bytes`. «Frigjort» telles først etter `finish_purge`. iOS: sletting havner i systemets «Nylig slettet» (30 dager), plass frigjøres først når den tømmes. Verifiseres på iPhone før tekst låses. |

## 4. Konsekvensforhåndsvisning, beskyttelse, masse-sletting

### 4.1 `ConsequenceSummary` (fra `plan()`, ren funksjon på indeksdata)

| Felt | Innhold |
|---|---|
| `count`, `bytes` | Sum over nøyaktig mengden `commit` vil endre (aktive, ikke allerede trashed). |
| `by_kind` | exact_dup, visual_dup, blurry, screenshot, manuell. |
| `albums` | `Vec<(AlbumRef, count)>`, sortert synkende («3 av dem er i albumet «Bryllup»»). |
| `protected` | Teller per `ProtectFlag`: Favorite, SharedAlbum, Edited, (Faces: reservert, ikke satt i MVP). |
| `backup` | `BackupState`: Verified, Unverified, Unknown (MVP: nesten alltid Unknown, se under). |
| `groups_untouched_keeper` | Sann hvis hver berørt gruppe beholder minst ett aktivt medlem. |
| `frees_storage_now` | Alltid `false` for trash. |
| `codes` | `Vec<ConsequenceCode>`: `ProtectedFavorite`, `ProtectedSharedAlbum`, `ProtectedEdited`, `InAlbum`, `BackupUnknown`, `MassDelete`, `FreesNoStorageYet`, `PlatformDialogNext`, `ExpiredNotice`. |

### 4.2 Beskyttede bilder

| Kategori | Kilde | MVP | Krav |
|---|---|---|---|
| Favoritt | adapter-flagg | Ja | Aldri forhåndsvalgt. Egen bekreftelse. |
| Delt album | adapter (album + `shared`) | Ja | Samme. Teksten sier at andre kan miste bildet. |
| Redigert | adapter (PhotoKit adjustments) | Ja, der plattformen gir det | Samme. |
| Mange ansikter fra nære personer | krever ansiktsmodell | Nei (post-MVP, flagget reservert) | Dokumenter i UI-plan at denne beskyttelsen mangler i MVP. |

Regler:
- Beskyttede bilder er aldri med i standardutvalget for masseforslag («slett alle uskarpe»). De vises i egen seksjon med eksplisitt valg.
- `ProtectedAck` lister id-er og kategorier. En ack for Favorite dekker ikke SharedAlbum.
- Swipe på beskyttet kort åpner bekreftelse før `commit`, med knappealternativ (a11y).
- `suggest` skal foretrekke beskyttet medlem som behold-kandidat. Hvis forslaget likevel sletter beskyttet bilde, kreves ack.

### 4.3 Masse-sletting og sikkerhetskopi-sjekk

- Terskel `MassDelete`: ≥ 50 bilder eller ≥ 1 GB i én `commit` eller `begin_purge` (setting, startverdi).
- Plattformene eksponerer ikke pålitelig opplastingsstatus (iCloud/Google Foto). MVP-ansvar: `BackupState::Unknown` gir advarsel «Fjell kan ikke se om disse er sikkerhetskopiert» og krever `MassAck` (avkrysning). Hvis adapter senere gir `Verified/Unverified`, brukes det uten kjerneendring.
- For «Tøm» gjelder terskelen alltid på mengden som sendes til systemdialogen.

### 4.4 Tørrkjøring

| Funksjon | Definisjon |
|---|---|
| `plan(req)` | Ren. Ingen skriving (verifiseres med `total_changes()` og dump-sum). Gir `Plan` med `ConsequenceSummary`, per-bilde `ReasonCode` og digest. |
| «Vis meg hva appen ville gjort» | `plan(AllPendingSuggestions)`. Viser samme kort/grupper som ekte kjøring, merket «forhåndsvisning». Ingen knapp for «utfør alt» uten egen `commit` og ack-regler. |
| Garanti | `apply(plan)` gir nøyaktig endringsmengden i planen (egenskapstest I6). |

## 5. Invarianter (kan aldri brytes)

Håndheves på tre nivåer: **typer** (`Confirmed`, `ProtectedAck` kan bare lages av `api`), **database** (triggere), **tester** (proptest).

| ID | Invariant | Håndhevelse | Property-test |
|---|---|---|---|
| I1 | Originaler skrives aldri. Eneste skrivende kall mot plattformen er adapterens `delete`, og den kalles bare med gyldig ticket. | `AssetSource` har ingen skrivemetoder; `delete` ligger i adapter | Fake-adapter logger kall. For alle sekvenser: `delete`-kall ⇒ foregående `confirm_purge`. Fixture-sjekksum lik før/etter. |
| I2 | `purged` oppstår bare via `finish_purge` med `Deleted` eller avstemming av borte kilde. Aldri via trash, expire, undo, decide, tid. | Type `Confirmed`; trigger blokkerer `0 -> 2` direkte | Tilfeldig sekvens uten `PurgeConfirmed`/`ExternalDelete` ⇒ ingen `purged`. |
| I3 | `purged` er terminal. | Trigger `BEFORE UPDATE OF state WHEN OLD.state=2` avbryter | Etter `purged` kan ingen op endre tilstand. |
| I4 | Batch er atomisk. Etter hver op (også med injisert feil midt i transaksjonen) er hver `batch_id` helt anvendt eller helt fraværende. | Én transaksjon per batch, én skrivetråd | Feilinjeksjon i `store`; sammenlign loggrader mot tilstandsendringer. |
| I5 | Angre-rundtur: for sekvenser uten purge og ekstern endring gir «alle batcher angret i omvendt rekkefølge» opprinnelig `asset.state`, `trashed_at`, `suggestion.status`. | `inverse_of`, `prev_state`, `prev_sugg_status` | Hovedtest, jf. ARKITEKTUR del 4. |
| I6 | Tørrkjøring har ingen bivirkning og `apply(plan)` = det planen viste. | `plan` tar `&Store` lesetilkobling | `total_changes()` uendret; mengde endret av `apply` = `plan.ids`; `summary.count/bytes` = faktiske summer. |
| I7 | Beskyttet bilde trashes aldri uten `ProtectedAck` som dekker id og kategori. | Vakt i `actions::trash` | Generator setter tilfeldige `protect_flags`; uten ack ⇒ `AckRequired`, ingen tilstandsendring. Standardutvalg inneholder aldri beskyttede. |
| I8 | Gruppeforslag etterlater minst ett aktivt medlem per gruppe. | Vakt i `plan` | Tilfeldige grupper + «godta alle» ⇒ hver gruppe har ≥ 1 aktivt medlem. |
| I9 | Tid sletter ikke. Vilkårlig `Clock`-fremføring og `expire_due` gir bare `active`/`trashed`. Utløpte kan gjenopprettes. | `expire_due` skriver ikke `state` | Sekvens med tick på år; ingen `purged`; `restore` virker. |
| I10 | `expire_due` er idempotent: hvert element varsles én gang. | `expired_notified_at` | Kall n ganger ⇒ summen av `ExpiryNotice.count` = antall utløpte. |
| I11 | Idempotens: `trash`/`restore` på allerede riktig tilstand gir ingen loggrad. Dobbel `undo` inverterer ikke dobbelt. | Vakt + `inverse_of` unik | Gjenta op; loggtelling uendret. |
| I12 | Bekreftelse er bundet til mengden. `Confirmed`/`ProtectedAck`/`MassAck` bærer digest over (id-er, tilstand, flagg). Endring ⇒ `PlanStale`/`PurgeStale`. | Digest sjekkes i `commit` og `finish_purge` | Muter indeks mellom `plan` og `commit` ⇒ avvist. |
| I13 | Skanning endrer aldri `state` på trashed, bortsett fra avstemming til `purged` for borte kilder. `upsert_assets` bevarer `state`, `trashed_at`, `expires_at`. | Spørring uten `state` i `DO UPDATE SET` | Skanning med tilfeldige endringer mellom ops; trashed forblir trashed. |
| I14 | `action_log` er append-only. | Triggere `BEFORE UPDATE/DELETE` avbryter | Forsøk på UPDATE/DELETE feiler. |
| I15 | Delvis plattformresultat taper ingenting: id-er uten `Deleted` forblir `trashed`. | `finish_purge` per id | Generator for utfall: Deleted/Cancelled/Failed per id, krasj før DB. Tilstand er alltid forklarbar av utfall + avstemming. |
| I16 | Ingen kolonne eller logg inneholder bildedata eller brukertekst utover id, sti/plattform-ID og albumnavn. | Skjema | Skjemadump-test. |

Modellbasert test: referansemodell (`HashMap<AssetId, State>` + stabel) kjøres parallelt med ekte `actions`. Etter hver op sammenlignes tilstand. Operasjonsalfabet: `Trash(delmengde)`, `Restore`, `Keep`, `Undo`, `Tick(dt)`, `ScanChange`, `DryRun`, `BeginPurge`, `ConfirmPurge`, `FinishPurge(utfall)`, `CrashBeforeDb`, `SetTrashDays`. `Clock` injiseres via L0-trait.

## 6. Testscenarier

| ID | Scenario | Forventet | Nivå |
|---|---|---|---|
| T1 | Swipe slett, så angre | Tilbake til `active`; én batch på hver vei; < 10 ms kjernekall | Enhet + perf |
| T2 | Gruppeforslag (5 bilder, behold 1), angre | Alle 4 tilbake som én batch | Enhet |
| T3 | Masseforslag 124 bilder (1,8 GB), 3 i album «Bryllup» | Summary: 124 / 1,8 GB, `albums=[Bryllup:3]`, `MassDelete`, `BackupUnknown`; krever `MassAck` | Enhet + UI |
| T4 | Favoritt er sletteforslag i gruppe | `AckRequired(ProtectedFavorite)`; uten ack uendret; `suggest` foretrekker favoritten som behold | Enhet |
| T5 | Delt album og redigert bilde i samme batch | Egne ack per kategori; én ack dekker ikke begge | Enhet |
| T6 | Dry-run av alle ventende forslag | Ingen skriving (`total_changes`); påfølgende `commit` gir samme mengde | Egenskap |
| T7 | Plan blir gammel (bilde gjenopprettet/endret mellom plan og commit) | `PlanStale`, ingenting skrevet | Enhet |
| T8 | 31 dager frem (`Clock`) | Elementer `trashed`, `ExpiryNotice` én gang, restore virker, ingen sletting | Egenskap |
| T9 | iOS-dialog: bruker avbryter | Alle forblir `trashed`; ingen logg `purge` | Adapter-fake + UI |
| T10 | Delvis (7 av 10 slettet) | 7 `purged`, 3 `trashed`; rapport viser 7/3 | Adapter-fake |
| T11 | App drepes etter plattformsletting, før DB | Neste skanning avstemmer til `purged` (`cause=external`) | Integrasjon (`fjell-cli`) |
| T12 | Bilde slettet utenfor Fjell mens `trashed` | Avstemmes til `purged`; angre hopper over det og rapporterer | Integrasjon |
| T13 | Angre av batch som inneholder purge | `NotUndoable`; UI viser ikke knapp | Enhet |
| T14 | Angre når tilstand er endret (restore gjort manuelt) | Hopper over avvikende id-er, rapporterer `skipped` | Enhet |
| T15 | Skanning mens bilder er `trashed` | `state`/`expires_at` urørt; trashed bilder dukker ikke opp som kort | Integrasjon |
| T16 | Ticket gjenbrukes / utløpt / mengde endret | `PurgeStale`, ingen `delete` kalt | Enhet |
| T17 | Feil midt i batch (injisert) | Ingen delvis batch (I4) | Egenskap |
| T18 | 50 000 id-er i én batch | Chunking under SQLite-variabelgrense; fortsatt én transaksjon | Enhet |
| T19 | Tom mengde, duplikat-id-er, ukjent id | Tom: no-op uten batch; duplikater dedupes; ukjent: `UnknownAsset` | Enhet |
| T20 | Kontrakt: alle koder oversatt | `ConsequenceCode` og `PlatformDialogNext` finnes i nb og en | i18n |
| T21 | UI nb (+30 %) for bekreftelser og systemdialog-forklaring | Ingen avkapping; knapper nåbare | UI |
| T22 | VoiceOver på bekreftelse og «Tøm» | Etiketter finnes; swipe og knapp gir samme resultat | UI |
| T23 | Fixture-sjekksum etter full skanning, alle ops og `FsAdapter.delete` bare på temp-kopi | Original-fixture uendret | Integrasjon |
| T24 | Migrering legger til triggere og kolonner | Fra forrige og fra tom db gir lik skjemadump; triggere virker | Migrering |

## 7. Ytelse

| Operasjon | Budsjett | Merknad |
|---|---|---|
| `trash`/`restore`/`undo` av 1–200 bilder | < 10 ms kjernekall (mål < 5) | Dekkes av 50 000-bilders perf-test |
| `plan()` for 1 000 id-er | < 10 ms | Indeks på `asset(id)`, `asset_album(asset_id)` |
| `trash_summary()` | < 5 ms | `idx(state)` |
| `trash`/`undo` av > 200 bilder (f.eks. 312 fra onboarding) | < 100 ms ende til ende med optimistisk UI. Mål: 1 000 id-er < 50 ms | Egen perf-test; unntatt 10 ms-porten |
| Purge (systemdialog) | Utenfor 100 ms-budsjettet | Styres av plattformen. UI viser fremdrift. Kjernens del (`finish_purge`) < 10 ms per 200 id-er |

## 8. Leveranser og rekkefølge

1. `model`: `ProtectFlag`, `ConsequenceCode`, `Plan`, `Acks`, `Confirmed` (privat konstruktør), `PurgeTicket`. Ingen I/O.
2. `store`: migrering (kolonner + triggere), `set_state(batch)`, `pop_batch`, avstemming.
3. `actions`: `plan`, `commit`, `undo`, `expire_due`, `begin_purge`, `confirm_purge`, `finish_purge`.
4. Modelltest + proptest (I1–I16) før noen UI. Deretter T1–T24.
5. `api`-flate for UI. `fjell-cli` kommandoer `plan`, `trash`, `undo`, `purge --fs-temp` for integrasjon.

Faser følger CLAUDE.md: én fase, én gren, én PR.

## 9. Åpne punkter

- iOS: bekreft på referanseenhet hva som frigjør plass når (tekst i UI låses etterpå).
- Terskler `MassDelete` (50 / 1 GB) og plangyldighet (5 min) kalibreres i brukertest.
- Ansiktsbeskyttelse (nære personer) er post-MVP og krever modellvalg.

## Grensesnitt mot andre planer

### Krever

| Fra plan | Krav |
|---|---|
| Arkitekt / data (`store`, `model`) | **Skjema:** `asset.protect_flags INTEGER` (bitmaske Favorite, SharedAlbum, Edited, Faces-reservert); tabeller `album(id, name, shared)` og `asset_album(asset_id, album_id)`; `asset.expired_notified_at`; `action_log.inverse_of`, `action_log.prev_sugg_status`, `action_log.cause` (user, expiry, external, undo); triggere for I2, I3, I14. `upsert_assets` skal bevare `state`, `trashed_at`, `expires_at` (I13). `set_state(batch)` må være én transaksjon. |
| Arkitekt | Bekreft at `purge(ids, Confirmed)` i `actions` betyr «oppdater indeks etter plattformens svar» (ikke selve slettingen), og at `Confirmed`/`PurgeTicket` lages i `api` via `begin_purge` / `confirm_purge`. |
| Plattformadaptere (iOS, fs) | `AssetMeta` med `is_favorite`, `in_shared_album`, `is_edited`, albumliste (id, navn, delt), valgfri `BackupState`. Adapter-operasjon `delete(ids) -> Vec<(AssetId, Deleted\|Cancelled\|Failed)>` som bare UI kaller med gyldig ticket. `list_changes` må kunne melde slettede kilder (til avstemming). |
| Pipeline (`scan`) | Avstemming av borte kilder til `purged` (`cause=external`). Skanning rører ikke `state` ellers. Trashed bilder utelates fra klynging og kort. |
| Forslag (`suggest`) | Foretrekk beskyttet medlem som behold. Aldri forslag som sletter alle aktive medlemmer i en gruppe. `ReasonCode` per bilde leveres til `Plan`. |
| UI/UX-plan | Bekreftelsesflyt per kategori, `MassAck`, forklaring før systemdialog, «Nylig slettet» med tre tall (merket / kan frigjøres / frigjort), banner for utløp, tydelig «ikke slettet fra telefonen ennå». Alle med knappealternativ til swipe. |
| i18n-plan | nb/en-tekster for alle `ConsequenceCode`, feilkoder (`AckRequired`, `PlanStale`, `PurgeStale`, `NotUndoable`, `AlreadyPurged`, `UnknownAsset`) og dialogforklaringer. Lokalt varsel for utløp. |
| Test/CI-plan | `proptest` for I1–I16 i `core`-jobben, `Clock`-fake og `FakeAdapter` (utfall per id), feilinjeksjon i `store`, migreringstest for triggere (T24), perf-test med trash/undo/plan i `nightly`. |

### Leverer

| Til plan | Leveranse |
|---|---|
| UI/UX | `api`: `plan(req) -> Plan`, `commit(plan, acks) -> BatchId`, `undo() -> UndoReport`, `trash_summary()`, `begin_purge(ids) -> PurgeIntent`, `confirm_purge(intent) -> PurgeTicket`, `finish_purge(ticket, results) -> PurgeReport`, `expire_due() -> Option<ExpiryNotice>`. Alle typer med koder, tall og id-er, ingen tekst. |
| i18n | Komplett liste over `ConsequenceCode` og feilkoder (del 4.1 og 5). |
| Plattformadaptere | Kontrakt for `delete` (bare etter ticket, per-id-utfall, deles i store batcher) og krav til flagg i `AssetMeta`. |
| Arkitekt / data | Skjemaendringer over, med trigger-SQL og migreringstester. |
| Pipeline | Regler for avstemming og for hvilke tilstander skanning kan skrive. |
| Test/CI | Invariantliste I1–I16 med generatorer, scenarier T1–T24 og ytelsesbudsjett (del 7). |
