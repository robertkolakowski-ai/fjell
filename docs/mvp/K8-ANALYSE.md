# K8-analyse: hash-recall med uskarpe varianter (MVP-restanse 6, forsøk)

Dato 2026-11-12. Syntetisk korpus (`core/testdata`, `blurset`), kun Linux. Ingen ekte bilder.

## Utgangspunkt
Port for visuelle duplikater: presisjon >= 0,98 og recall >= 0,90. Med Gauss-, fokusfeil-, bevegelses- og støyuskarpe varianter regnet som duplikater av originalen var recall 0,891 (hovedsett) og 0,914 (holdout), presisjon 1,000.

## Funn
1. **Presisjon er ikke flaskehalsen på syntetiske data.** Ingen negative par har avstand under 20 (max av dHash og pHash Hamming); tersklene ligger på 6. Det betyr at korpuset ikke sier noe om hvor mye terskelen tåler på ekte foto (serier, lik motivplass).
2. **Terskelen (`max_dist`) hjelper lite i gruppetall.** På parnivå gir 6 -> 7 -> 8 -> 10 en recall på 0,84 -> 0,87 -> 0,90 -> 0,94 uten falske par, men gruppenivå (transitiv sannhet) går bare fra 0,891 til 0,894 ved 7. Over 7 faller MIH-indeksen tilbake til brute force (O(n^2)), som er uakseptabelt for 10 000+ bilder.
3. **Den som biter er kjedevernets diameter (`max_span`).** Sannhetsgruppen er alle par, og to ulike uskarpe varianter (f.eks. Gauss og bevegelse) ligger langt fra hverandre selv om begge ligger nær originalen. Når gruppens diameter overstiger `max_span` (10) klynges den om med skjerpet terskel og deles.
4. dHash+pHash-kombinasjon: summeregel (d+p <= 12) og min/max-regler ga samme bilde som AND-regelen på parnivå (recall 0,90-0,96 uten falske par), men uten at de løser diameterproblemet og med et nytt, uprøvd beslutningsgrunnlag. Ikke valgt.
5. Svakeste motiv er fortsatt treverk (recall ca. 0,38 for uskarpe varianter). Ingen parameter fikser det; det er en egenskap ved hashene på periodiske teksturer.

## Endring
**Kandidat, IKKE innført** (revertert av integrator): `max_span` 10 -> 12 og `tight_dist` 4 -> 5 ga recall 0,913 (hovedsett) / 0,931 (holdout), men falske sammenslåinger kan ikke måles uten ekte bilder, og bredere grupper forhåndsmerker flere bilder for papirkurven. Standard er fortsatt 10 og 4. Tallene under gjelder kandidaten. `max_dist` er uendret (6). Klyngingen regnes fra lagrede hasher ved hver gruppering, så `ALGO_VER` økes ikke.

| Syntetisk sett | recall før | recall etter | presisjon før/etter | falske par |
|---|---|---|---|---|
| hovedsett (blurset) | 0,891 | 0,913 | 1,000 / 1,000 | 0 / 0 |
| holdout (egen seed) | 0,914 | 0,931 | 1,000 / 1,000 | 0 / 0 |
| golden-korpus K2b-3 | 0,950 | 0,950 | 1,000 / 1,000 | 0 / 0 |

Uskarphet-porten (K8 blur, `blur_score`) berøres ikke: hashene og `quality` er uendret.

## Forbehold (overtilpasning)
- `max_span` og `tight_dist` er valgt ved å se på hovedsettet; holdout er brukt til å bekrefte, ikke uavhengig. Begge er syntetiske, laget av samme generator.
- Større diameter gir større risiko for at kjeder av nesten-like, men ulike bilder slås sammen på ekte foto (serier, burst). Syntetiske negativer er for lette til å måle det. Konsekvens begrenses av at Fjell foreslår, aldri sletter automatisk, og at alt kan angres.
- Må kalibreres på etikettert korpus av ekte foto (restanse 6) før `ALGO_VER` fryses. Kandidaten 12/5 testes da mot 10/4; innfør den bare hvis falske sammenslåinger ikke øker.
- `docs/mvp/PLAN-KJERNE.md` K2 punkt 4 nevner fortsatt 10 og 4; oppdateres av den som eier planen.
