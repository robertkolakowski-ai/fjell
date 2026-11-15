# V1: kalibrering

Gren `v1c/calibrate`. Svarer på åpent spørsmål 3 i `V1-PLAN.md` (terskler per sensitivitet) så langt det lar seg gjøre uten ekte bilder og uten modeller. Verktøy og tester ligger i `core/testdata` (egen crate). Ingen algoritme og ingen `Thresholds`/`tuning`-verdi i `fjell-core` er endret her; anbefalingene i del 6 er forslag til integratøren.

**Les dette først.** Alle tall under kommer fra et syntetisk korpus som vi har laget og merket selv. Det er ikke ekte bilder. Tallene viser hvordan terskelkurvene oppfører seg og at kjernen holder det den lover (begrunnelse, gruppering, keeper, beskyttelse), men de sier ikke hvor presis Fjell er på Roberts bibliotek. Ekte kalibrering krever Roberts eget testsett (del 8, og `docs/mvp/KALIBRERING.md` for CSV-format og beslutningsregler). Øyesikkerhet er en plassholder uten modell: fordelingen er antatt.

## 1. Hva som er laget

| Del | Hvor |
|---|---|
| Golden-tester per forslagstype, gjennom hele `Fjell` | `core/testdata/tests/golden_kinds.rs` |
| Terskelsveip og tabeller per sensitivitet | `core/testdata/tests/calibration.rs`, `core/testdata/tests/common/mod.rs` |
| Verktøy som skriver tabellene | `core/testdata/examples/calibrate.rs` |
| Syntetisk, merket korpus for serier, øyne og skjermbilder | `core/testdata/src/calib.rs` |

Visuelle duplikater og skjermbildesignaler bruker det eksisterende golden-korpuset (`corpus.rs`), uskarphet bruker det utvidede uskarphetskorpuset (`blurset.rs`, K8). Alt genereres av `image`-crate og egne rutiner, deterministisk fra seed, og skrives til `CARGO_TARGET_TMPDIR`/systemets temp-mappe. Ingenting leses fra brukerens maskin, og ingenting sendes noe sted.

## 2. Slik kjører du

Fra `core/testdata` (egen crate, ikke i workspace):

```
cargo test --locked                                   # golden-tester og kalibreringstester
cargo run --release --locked --example calibrate     # skriver tabellene under som Markdown
FJELL_UPDATE_CALIBRATION=1 cargo test --release --locked --test calibration calibration_doc_is_current
                                                     # oppdaterer tabellene i dette dokumentet
```

Testen `calibration_doc_is_current` feiler hvis tabellene under ikke er de verktøyet skriver nå (korpus, generator eller kjerne er endret uten at dokumentet er oppdatert). Kjøretiden er noen minutter i hele crate-en fordi uskarphetskorpuset (ca. 1 000 bilder) genereres; det meste er bildebehandling.

## 3. Golden-tester per forslagstype

Hver test skanner et lite syntetisk bibliotek gjennom `Fjell` (`MemSource`), og leser kortene. Forutsetninger om bildene (hashavstander, skarphetsscore) sjekkes i testen, så en endring i generatoren gir en tydelig feil.

| Forslagstype | Test | Beviser |
|---|---|---|
| Eksakt duplikat | `exact_duplicate_reason_group_keeper_and_originals_untouched` | begrunnelse `exact_copy`, konfidens 1,0, én gruppe, eldst beholdes (`older`), originalinnhold uendret |
| Visuelt duplikat | `visual_duplicate_keeps_largest_and_gives_visual_reason` | `visual_duplicate`, største beholdes selv om den er yngst (`higher_resolution`), ulike motiv grupperes ikke |
| Uskarpt | `blurry_flagged_sharp_and_low_contrast_not` | `blurry` uten gruppe, skarpt og skarpt-men-flatt flagges ikke |
| Skjermbilde | `screenshot_reason_signals_and_negatives` | `screenshot`, signalene S1..S4 bak merket, kamerabilde og redigert PNG uten S1/S2 flagges ikke |
| Serie | `series_groups_keeps_sharpest_and_excludes_far_in_time`, `series_closed_eyes_move_keeper_only_above_confidence`, `series_gap_follows_sensitivity` | `series_member` med posisjon, keeper er skarpest (`keeper_best_overall`), bilde en time senere er ikke med, lukkede øyne flytter keeper bare ved sikkerhet over kravet (`keeper_eyes_open`, `closed_eyes`), glippen følger sensitiviteten |
| Regel | `rule_suggests_with_rule_reason_and_never_moves_anything` | `rule` med regel-ID, forhåndsvisning lagrer ikke, ingenting flyttes, avslått regel gir ingen kort |
| Beskyttelse og angre | `protected_never_moved_without_ack_and_everything_can_be_undone` | beskyttet bilde (enkeltbilde og mappe) kan ikke flyttes uten bekreftelse (`AckRequired`/`ProtectedByScope`), regler treffer aldri beskyttede, med bekreftelse går bildet til «Nylig slettet» (ikke hard sletting), `undo` gjenoppretter, innhold uendret |

De fleste testene sjekker også at ingen keeper har kort og at et bilde aldri står i to kort. Kalibreringstestene (`calibration.rs`) sjekker at kurvene er monotone der de må være det, at Low/Normal/High er ordnet, og gulv for presisjon på Normal.

## 4. Korpus og fasit

- **Visuelt duplikat:** golden-korpuset, én representant per identisk innhold, flate bilder utelatt. Fasit er `visual_group` (nedskalerte, omkodede, beskårne og uskarpe varianter av samme motiv). Merk at flere av de vanskeligste variantene (kraftig beskjæring, kraftig uskarphet) ikke kan fanges av Hamming alene; det setter taket for recall.
- **Uskarphet:** `blurset` (sju motivfamilier, uskarpe og skarpe varianter inkl. lavkontrast og støy). Bare vurderbare bilder telles (som i K8).
- **Serier:** 38 serier à 4 til 8 bilder i fire bevegelsesklasser (rolig, middels, livlig, langsom med glipp opp til 19 s), 12 «samme sted senere» (to serier av samme motiv med 2 til 15 minutter imellom, skal bli to) og 12 «gange» (ulike komposisjoner få sekunder fra hverandre, skal ikke bli serie). Fasit: alle bilder i en rask serie hører sammen uansett hashavstand. Dette er vår definisjon, ikke en brukers.
- **Øyne (plassholder):** 400 serier à 3 til 6 bilder med ett ansikt. Analysatorens sikkerhet er trukket fra antatte fordelinger (se `calib::eye_cases`): lukkede øyne rapporteres riktig i 88 % av tilfellene, åpne feilrapporteres lukket i 8 %. Ingen modell er valgt eller brukt.
- **Skjermbilde:** 12 tilfellestyper med antall (typiske og vanskelige), bare metadata (S1..S4). Antallene er antakelser om en vanlig mobilsamling.
- **Seriene regnes slik appen gjør det:** bilder som alt er visuelle duplikater (Normal) går ikke videre til serier (`series_specs`), og presisjon/recall regnes på parene som er igjen.

## 5. Resultater

Tabellene skrives av `examples/calibrate.rs` og er sjekket mot korpuset av testen. «Nivå» viser hvilket nivå dagens `tuning` (V1-startverdiene) legger på raden. Presisjon/recall er på par (visuelt, serier), på bilder (uskarphet, skjermbilde, øyne). «–» betyr ingen forslag (nevner 0). Uskarphet er sveipet mot taket `BLUR_ABS_MAX`; appens `min(P10, tak)` gir lavere terskel i et korpus med mange uskarpe, så taket er det som gjelder i et vanlig bibliotek.

<!-- kalibrering:start -->

#### Visuelt duplikat: Hamming (`max_dist`)

| max_dist | nivå | presisjon | recall | falske par | serie-par slukt |
|---|---|---|---|---|---|
| 2 |  | 1.000 | 0.553 | 0 | 70/670 |
| 3 |  | 1.000 | 0.618 | 0 | 81/670 |
| 4 | Low | 1.000 | 0.653 | 0 | 176/670 |
| 5 |  | 1.000 | 0.665 | 0 | 183/670 |
| 6 | Normal | 1.000 | 0.671 | 0 | 335/670 |
| 7 |  | 1.000 | 0.688 | 0 | 335/670 |
| 8 | High | 1.000 | 0.729 | 0 | 345/670 |
| 9 |  | 1.000 | 0.747 | 0 | 345/670 |
| 10 |  | 1.000 | 0.794 | 0 | 322/670 |
| 11 |  | 1.000 | 0.794 | 0 | 322/670 |
| 12 |  | 1.000 | 0.782 | 0 | 322/670 |

#### Uskarphet: terskelfaktor (`blur_scale`, grunnlag `BLUR_ABS_MAX` = 0,006)

| faktor | nivå | terskel | presisjon | recall | feilflagg | oversett |
|---|---|---|---|---|---|---|
| 0.4 |  | 0.00240 | 0.986 | 0.679 | 4 | 134 |
| 0.5 |  | 0.00300 | 0.982 | 0.785 | 6 | 90 |
| 0.6 |  | 0.00360 | 0.981 | 0.849 | 7 | 63 |
| 0.7 | Low | 0.00420 | 0.977 | 0.902 | 9 | 41 |
| 0.8 |  | 0.00480 | 0.972 | 0.916 | 11 | 35 |
| 0.9 |  | 0.00540 | 0.963 | 0.931 | 15 | 29 |
| 1.0 | Normal | 0.00600 | 0.958 | 0.938 | 17 | 26 |
| 1.1 |  | 0.00660 | 0.901 | 0.940 | 43 | 25 |
| 1.2 |  | 0.00720 | 0.889 | 0.943 | 49 | 24 |
| 1.3 | High | 0.00780 | 0.878 | 0.943 | 55 | 24 |
| 1.4 |  | 0.00840 | 0.878 | 0.943 | 55 | 24 |
| 1.5 |  | 0.00900 | 0.870 | 0.943 | 59 | 24 |
| 1.6 |  | 0.00960 | 0.857 | 0.943 | 66 | 24 |
| 1.7 |  | 0.01020 | 0.828 | 0.947 | 82 | 22 |
| 1.8 |  | 0.01080 | 0.795 | 0.947 | 102 | 22 |
| 1.9 |  | 0.01140 | 0.751 | 0.947 | 131 | 22 |
| 2.0 |  | 0.01200 | 0.732 | 0.947 | 145 | 22 |

#### Serier: glipp i tid (`gap_secs`, `max_dist` = 10)

| gap_secs | nivå | presisjon | recall | falske par (samme sted senere) | falske par (gange) |
|---|---|---|---|---|---|
| 2 |  | 1.000 | 0.057 | 0 | 0 |
| 3 |  | 1.000 | 0.135 | 0 | 0 |
| 5 | Low | 1.000 | 0.213 | 0 | 0 |
| 8 |  | 1.000 | 0.355 | 0 | 0 |
| 10 | Normal | 1.000 | 0.362 | 0 | 0 |
| 15 |  | 1.000 | 0.447 | 0 | 0 |
| 20 | High | 1.000 | 0.525 | 0 | 0 |
| 30 |  | 1.000 | 0.525 | 0 | 0 |
| 60 |  | 1.000 | 0.560 | 0 | 0 |
| 300 |  | 1.000 | 0.574 | 0 | 0 |
| 1200 |  | 0.782 | 0.610 | 24 | 0 |

#### Serier: Hamming (`max_dist`, `gap_secs` = 10)

| max_dist | nivå | presisjon | recall | falske par (samme sted senere) | falske par (gange) |
|---|---|---|---|---|---|
| 4 |  | – | 0.000 | 0 | 0 |
| 5 |  | – | 0.000 | 0 | 0 |
| 6 |  | 1.000 | 0.028 | 0 | 0 |
| 7 |  | 1.000 | 0.028 | 0 | 0 |
| 8 | Low | 1.000 | 0.163 | 0 | 0 |
| 9 |  | 1.000 | 0.163 | 0 | 0 |
| 10 | Normal | 1.000 | 0.362 | 0 | 0 |
| 11 |  | 1.000 | 0.362 | 0 | 0 |
| 12 | High | 1.000 | 0.582 | 0 | 0 |
| 13 |  | 1.000 | 0.582 | 0 | 0 |
| 14 |  | 1.000 | 0.723 | 0 | 0 |
| 15 |  | 1.000 | 0.723 | 0 | 0 |
| 16 |  | 1.000 | 0.723 | 0 | 0 |

#### Øyesikkerhet (PLASSHOLDER, antatt fordeling, ingen modell)

| min. sikkerhet | nivå | presisjon | recall | keeper med lukkede øyne |
|---|---|---|---|---|
| 0.50 |  | 0.741 | 0.891 | 13/400 |
| 0.60 |  | 0.785 | 0.816 | 18/400 |
| 0.70 | High | 0.829 | 0.646 | 38/400 |
| 0.80 | Normal | 0.912 | 0.441 | 56/400 |
| 0.90 | Low | 1.000 | 0.234 | 75/400 |
| 0.95 |  | 1.000 | 0.120 | 82/400 |

#### Skjermbilde: `require_strong` (ingen sensitivitet)

| require_strong | presisjon | recall | feilflagg | oversett |
|---|---|---|---|---|
| ja (standard) | 0.938 | 0.809 | 5 | 18 |
| nei | 0.838 | 0.936 | 17 | 6 |

#### Samlet per sensitivitet (dagens startverdier)

| nivå | visuelt (P / R) | uskarpt (P / R) | serie (P / R) | øyne, plassholder (P / R) |
|---|---|---|---|---|
| Low | 1.000 / 0.653 | 0.977 / 0.902 | 1.000 / 0.106 | 1.000 / 0.234 |
| Normal | 1.000 / 0.671 | 0.958 / 0.938 | 1.000 / 0.362 | 0.912 / 0.441 |
| High | 1.000 / 0.729 | 0.878 / 0.943 | 1.000 / 0.816 | 0.829 / 0.646 |
<!-- kalibrering:end -->

## 6. Tolkning og anbefalte standardverdier

Anbefalingene er forslag til integratøren (jf. at `Thresholds`/`tuning` eies av `fjell-core`). Ingen verdi er endret her.

| Parameter | Low | Normal | High | Begrunnelse |
|---|---|---|---|---|
| Visuell `max_dist` | 4 | 6 | 8 | Uendret. Korpuset har ingen falske par opp til 12, men har få harde negativer, så det taler ikke for høyere. Prisen for høyere terskel er serie-par som blir duplikatkort i stedet for seriekort (176/670 ved 4, 335/670 ved 6, 345/670 ved 8). |
| Uskarphet `blur_scale` | 0,7 | 1,0 | **1,1** (i dag 1,3) | Normal: presisjon 0,958 og recall 0,938. High 1,3 gir bare +0,005 recall mot Normal og koster 0,08 presisjon (0,878). 1,1 holder presisjon over 0,90 (K8-porten) med samme recall. Low 0,7 gir 0,977/0,902. |
| Serie `gap_secs` | 5 | 10 | 20 | Uendret. Ingen falske par opp til 300 s i korpuset, så glippen er ikke det som begrenser recall; Hamming er det. |
| Serie `max_dist` | 8 | 10 | 12 | Uendret. 12 og 14 gir mye høyere recall (0,58 og 0,72 mot 0,36) uten falske par her, men korpuset har bare 12 «gange»-sett og ingen ekte «samme sted, ulikt øyeblikk». En serie som feilaktig slår sammen ulike øyeblikk kan foreslå å fjerne et bilde som var et eget øyeblikk. Ikke hev før ekte data finnes. |
| Øyesikkerhet | 0,9 | 0,8 | 0,7 | Uendret og uten dekning: plassholder, ingen modell. Kalibreres når modellen er valgt (`MODELLER.md`). |
| Skjermbilde `require_strong` | | ja | | Uendret. Ja gir presisjon 0,938 og recall 0,809; nei gir 0,838 og 0,936. Appen foreslår bare, men en for løs regel gir for mange feil kort. Gjenværende feil er nedlastede bilder i skjermstørrelse (PNG uten EXIF), som metadata alene ikke kan skille. |
| Skjermbilde og regel: sensitivitet | | | | Finnes ikke i `tuning` (bare visuelt, uskarpt og serie har sensitivitet). Det er riktig for regler (brukerens egne vilkår). |

## 7. Funn om kjernen (utenfor denne grenens omfang)

Funnene kom fram i golden-testene. Ingenting er endret i `fjell-core`; de er meldt til integratøren.

1. **Keeper i en eksisterende serie oppdateres ikke når øyeresultater kommer senere.** Øyeanalysen kjøres etter skanningen, mens serien dannes under skanningen uten øyne. `Store::apply_series` gjenbruker en lagret serie med samme medlemmer og beholder keeperen (for å respektere brukerens valg), også når keeperen aldri var valgt av brukeren. Resultat: på første skanning av et nytt bibliotek vil «øyne åpne»-logikken ikke flytte keeper i serier som alt er lagret. Golden-testen omgår dette ved å la bildene komme i to omganger, slik at øyeresultatene finnes når serien dannes. Forslag: skille automatisk og manuelt valgt keeper (egen markør), og la bare den manuelle overleve.
2. **Endret sensitivitet gjelder først ved neste utregning.** `set_settings` lagrer bare verdiene; gruppene regnes på nytt ved neste skanning (testen kjører en skanning etter endring). Passer med `V1-PLAN.md` punkt 19, men UI må utløse en utregning hvis endringen skal synes med en gang.
3. **Mappebeskyttelse gjelder mappegrenser.** `ProtectScope::Folder("/Pictures/d_")` beskyttet ikke `/Pictures/d_skjerm.png` (beslutningen gikk uten bekreftelse); `Folder("/Pictures/skjerm")` beskyttet bildene under mappen. Trolig riktig (mappe, ikke tekstprefiks), men UI bør ikke la brukeren skrive et fritt prefiks og tro at det beskytter filer med samme begynnelse.

## 8. Begrensninger og ekte kalibrering

- **Syntetisk korpus er ikke ekte bilder.** Fasit, bevegelsesklasser, antall og fordelinger er våre antakelser. Tallene gir kurveform og regresjonsvern, ikke ytelse på ekte bibliotek. Spesielt: det finnes få harde negativer for visuell klynging og serier, så presisjon 1,000 betyr ikke at terskelen er trygg.
- **Ekte kalibrering krever Roberts eget testsett** (minst 1 000 bilder med gruppeetiketter, se `docs/mvp/KALIBRERING.md`). Serier trenger i tillegg etiketter for «samme øyeblikk» og «samme sted, ulikt øyeblikk», og tidspunkter fra ekte EXIF. `fjell-cli calibrate` dekker visuelt og uskarphet; serier og øyne har ikke tilsvarende verktøy mot ekte bilder ennå.
- **Øyesikkerhet:** plassholder. Ingen modell er valgt, lastet ned eller kjørt. Tallene sier ingenting om en ekte analysator.
- **Uskarphet:** `min(P10, tak)` avhenger av hele biblioteket; sveipet bruker taket. På et bibliotek med færre uskarpe bilder enn korpuset (ca. 45 %) vil appens terskel være taket, ellers lavere.
- **Ikke verifisert her:** Windows, ekte bilder (HEIC, RAW, kamerabilder), Narrator/NVDA, kjøretid på stort korpus. Alle tall er fra Linux.
