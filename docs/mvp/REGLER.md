# Keeper-regler (kjerne, bølge 1)

Brukerstyrte regler som justerer hvilket bilde i en gruppe som foreslås beholdt. Koden bor i
`core/crates/fjell-core/src/rules/` (rene regler), `pipeline/keeper.rs` (limet mot K5) og
`api/fjell_rules.rs` (API). Skall og UI kommer i bølge 2.

## Prinsipper
- Reglene velger bare keeper og utelater beskyttede bilder fra forslag. De flytter, sletter og godtar aldri noe.
  Brukeren bekrefter som før; papirkurv, angre og Tøm-invarianten er urørt.
- Uten regler er resultatet K5 uendret (byte-likt; testet).
- Samme input gir samme output: sortert på id, uavhengig av medlemsrekkefølge.

## Regler (`KeeperRule`)
`PreferFolder{prefix, priority}`, `PreferNewest`, `PreferOldest`, `PreferLargest` (filstørrelse),
`PreferHighestResolution`, `PreferSharpest` (samme 10 %-terskel som K5), `PreferShortestPath`,
`PreferShortestName`, `ProtectFolder{prefix}`.

Mappetreff: hele stikomponenter, `\` og `/` likestilt, uavhengig av store/små bokstaver
(`/a/bo` ligger ikke i `/a/b`). Sti = `asset.source_id`.

## Rekkefølge og uavgjort
1. `ProtectFolder` først: finnes beskyttede kandidater, er bare de med.
2. Reglene i listerekkefølge snevrer kandidatene inn. Alle `PreferFolder` vurderes samlet der den første står:
   høyest `priority` vinner, lik prioritet skiller ikke.
3. En regel som ikke skiller (alle like, ingen treff) hoppes over. Står flere igjen etter alle regler,
   avgjør K5 blant dem.
Begrunnelsen er regelen som først snevret inn kandidatene; ingen regel skilte = K5 (vanlig `ReasonCode`).

## Beskyttede mapper
Et bilde under en `ProtectFolder` får aldri forslag om papirkurv (heller ikke uskarp/skjermbilde):
- Er minst ett medlem beskyttet, blir et beskyttet medlem keeper; øvrige beskyttede utelates fra gruppen.
- Er alle medlemmer beskyttet, lages ingen gruppe.
- Regruppering gjenbruker ikke en gammel gruppe der en annen keeper ville gi et beskyttet bilde et forslag
  (`GroupSpec::keeper_locked`). Beskyttelse går dermed foran et manuelt keeper-valg ved regruppering.

## Typet begrunnelse
`RuleReason` (enum + parameter): `PreferFolder{prefix}`, `PreferNewest`, ..., `ProtectedFolder{prefix}`.
`key()` gir `prefer_folder` osv.; UI oversetter `rule.<key>` og setter inn mappen. Lagres i `grp.keeper_rule`
som `nøkkel[:param]`; `keeper_reason` er da NULL. Ingen ferdig tekst i kjernen.

## Lagring (migrering 4)
Regellisten ligger i `setting` (`keeper_rules`, én regel per linje, tab-separert, escapet). Migrering 4
(`004_keeper_rule.sql`) legger til `grp.keeper_rule`. En indeks uten regler migreres uten endring i oppførsel.

## API (`Fjell`)
- `set_keeper_rules(regler)` / `keeper_rules()`: lagrer/leser listen. Lagring regner om grupper, men eksisterende
  grupper beholder keeper; bare nye grupper og forslag får reglene.
- `preview_keeper_rules(regler) -> Vec<KeeperChange{group, old_keeper, new_keeper, rule}>`: ren beregning,
  skriver ingenting.
- `apply_keeper_rules()`: «bruk regler på gjenstående grupper». Berører bare grupper der alle medlemmer er
  aktive, alle forslag er ventende og keeper ikke er valgt manuelt. Avviste/godtatte forslag, manuelle valg og
  papirkurv røres aldri. Tomme regler tilbakestiller til K5. `rule = None` i en endring betyr K5.

## Forholdet til V1 (serier og forslagsregler)
V1 legger forslagsregler (`suggest_rules`, tabellene `rule`/`rule_hit`, docs/v1/V1-PLAN.md) og serier oppå. Keeper-reglene
her er uendret i oppførsel og API, og de gjelder foran:
- Et bilde under en `ProtectFolder` får heller ikke serie- eller regelforslag, og telles ikke i forhåndsvisningen av en
  forslagsregel. `pipeline` og `api` gir beskyttelsen videre som en sperre (`suggest_rules::Guard`).
- Forslagsreglene endrer aldri keeper, og `apply_keeper_rules` rører aldri serier (`grp.kind = 'series'`).
- Migrering 5 (`005_v1.sql`) skriver om `grp` og `suggestion`; `keeper_reason`, `keeper_rule` og `setting.keeper_rules`
  følger med uendret (testet i `store/migrate_tests.rs`).

## Kjente begrensninger
- «Manuelt valgt» kjennes på `keeper_reason` og `keeper_rule` begge NULL.
- Å bytte keeper og lagre regel er to skrivinger; brudd imellom gir en gruppe som ser manuell ut (trygt).
