# Loft: visuell oppgradering av Fjell for Windows (M10)

Eier: tokens (`desktop/src/lib/tokens.css`), skall (`App.svelte`, `app.css`), komponenter (`desktop/src/lib/components/**`). Tokens er eneste kilde for farge, radius, skygge, avstand og font-size. Skisse: PC-skissen produkteier godkjente (retning). Mål i dette dokumentet gjelder foran skissen.

## Komponent-API (Svelte 5; importer fra `../lib`)
| Komponent | Props | Bruk |
|---|---|---|
| `Sidebar` | `brand`, `navLabel`, `children` (NavItem), `footer?` | Rot er `<header>` (ett banner), `<nav aria-label>` inni. ≥ 900 px sidemeny, under toppmeny, samme DOM. Brukes bare av App.svelte. |
| `NavItem` | `href`, `label`, `current?`, `count?` (formatert), `countLabel?` (SR), `icon?` snippet | `<li>`; lenketeksten er bare etiketten, telleren er søsken og kobles med aria-describedby. |
| `PageHeader` | `title`, `sub?`, `description?`, `back? {href,label}`, `actions?` snippet | Erstatter skjermens `<h1 tabindex="-1">`. App fokuserer første h1 ved rutebytte. |
| `Card` | `heading?`, `headingLevel?`, `tone? raised\|sunken`, `padding? md\|none` | Flate nivå 1 (raised) eller nivå 2 (sunken). Gruppe-landemerke, ikke region. |
| `StatTile` | `label`, `value`, `hint?`, `href?`, `tone? default\|ok\|warn\|danger` | Tall + etikett. Med `href` er hele flisen lenke (kategorikort på Hjem). |
| `SegmentedControl` | `options [{value,label}]`, `value`, `onchange(v)`, `ariaLabel`/`ariaLabelledby`, `ariaDescribedby?`, `disabled?`, `name?` | Native radiogruppe: piltaster og Tab fra nettleseren. Valgt = fylt flate + hake + vekt. |
| `SettingsCard` | `heading?`, `headingLevel?`, `description?`, `children` | Kort med SettingRow-rader. Containerspørring: rader stables under 34 rem. |
| `SettingRow` | `label`, `description?`, `labelFor?`, `control?` snippet(`{labelId,descId}`), `children?` | Tittel + forklaring venstre, kontroll høyre. `labelFor` gir `<label for>`. |
| `Banner` | `tone info\|warn\|danger\|ok`, `title?`, `headingLevel?`, `id?`, `titleId?`, `role? status\|alert\|group`, `children`, `actions?` | Ikon + tekst + kant. `CloudWarning` er bygget på den (id-ene `cloud-warning/-title/-why` er uendret). |
| `Button` | `variant primary\|secondary\|quiet\|danger`, `size md\|sm`, + som før | Alle varianter har kant (overlever høykontrast). Én `primary` per skjerm. |
| `Ring` | `fraction`, `label`, `value?`, `caption?`, `size sm\|md\|lg` | Standard `md` = 11 rem (var 15). |
| `BottomBar` | som før | Fast bunnlinje står til høyre for sidemenyen (`--shell-inset`); handlingstekst venstre, primær høyre. |
| `Icon` | `name` | Dekorative linjeikoner (aria-hidden). |
Uendret: `Badge`, `PhotoTile`, `Toast`, `LiveRegion`. Skjermagenter skal bare bygge om layout og bruke disse; ikke lag egne varianter.

## 1. Problemer i dag (m7 til m9)
- **Alle skjermer**: ett smalt innholdsspor (`max-inline-size` 40 rem) i venstre kant på 1280 px; over halve vinduet er tomt. Toppmeny som på mobil. Alt er hvitt kort på lysegrått, ingen hierarki.
- **Hjem**: ringen (15 rem) står alene øverst og skyver kategoriene ned; fire kategorier som fullbredde rader; «Legg til mappe», papirkurvlenke og mappe ligger som løs tekst nederst. Skannstatus er løs tekst og knapper.
- **Gjennomgang**: kategorivalg er en stabel lenker; gruppevisning er en lang liste med rader uten forhåndsvisning.
- **Gruppe**: bildene står i et smalt rutenett; sammenligning side om side utnytter ikke bredden. Ingen liste over andre grupper.
- **Kortmodus**: kortet og knappene står under hverandre; knappene er fullbredde.
- **Papirkurv**: ren liste, sammendrag og «Tøm» er spredt.
- **Innstillinger**: skjema i én kolonne: radioknapper, select og tekstavsnitt løse. Oppdateringer er en egen lang blokk. Ingen gruppering.
- **Onboarding, ark/dialoger**: enkeltkolonne uten rammer; dialoger helt flate.

## 2. Layoutsystem
- **Skall**: `.app` er et rutenett. ≥ 900 px (56.25 rem; media-spørring i rem så Windows-skalering gir smal modus tidlig): sidemeny `--nav-w` (15 rem), sticky, full høyde, + arbeidsflate. Under 900 px: toppmeny (merke, valg som flyter og brytes, Hurtigtaster) over arbeidsflaten, ikke sticky (høyden er knapp ved 520 px). Versjon vises bare i sidemenyen.
- **Arbeidsflate**: padding `--gutter` (12 til 32 px, `clamp`), tak `--w-wide` (84 rem) for innhold; `--w-content` (64 rem) for lister, `--w-narrow` (40 rem) for skjema/innstillinger. Innhold venstrejustert mot menyen, ikke sentrert.
- **Rutenett**: `repeat(auto-fit, minmax(var(--card-min), 1fr))` for kort og StatTile; `minmax(var(--tile-min), 1fr)` for bildeflis. Tosporsflater («liste + detalj») bruker `grid-template-columns: minmax(12rem, 16rem) minmax(0, 1fr)` og faller til ett spor under 900 px.
- **Tetthet/avstand**: skala `--space-0..8` (2 til 64 px). Sidemarg `--gutter`, mellom kort `--space-4`, innvendig kort `--space-4`/`--space-5`, mellom seksjoner `--space-5`/`--space-6`. Trykkmål min `--target-min` 40 px (`--control-sm` 32 px for `size="sm"`; WCAG 2.5.8 krever 24 px).
- Ingen horisontal sideskroll ved noen bredde; tekst bryter (`overflow-wrap: anywhere` fra base.css).

## 3. Typografi
- Systemfont (`Segoe UI Variable`, `Segoe UI`, system-ui). Ingen eksterne fonter.
- Skala (rem): xs 0.8125, sm 0.875, md 0.9375 (brødtekst), lg 1.125, xl 1.5 (sideoverskrift), stat 1.625 (tall), display 2.25. Vekter: 400, 500 (nav, segment), 600 (overskrift, etikett, tall).
- Tall som står i kolonner, tellere, størrelser og summer bruker `font-variant-numeric: var(--num)` (tabulære).
- Forklaringstekst `--fs-sm` i `--c-text-muted`. Aldri under `--fs-xs`.

## 4. Farge og dybde
- **Flatenivåer**: `--c-bg` (nivå 0, app), `--c-nav-bg` (sidemeny, litt mørkere i lys), `--c-surface` (nivå 1: kort, aktiv nav, segment), `--c-surface-sunken` (nivå 2: innfelt, tellere, segmentbakgrunn), `--c-surface-hover`.
- **Kanter og skygger**: dekorative kanter `--c-border-subtle`; kontroller har `--c-border-strong` (3:1). Skygger: `--shadow-card` (nesten ikke synlig), `--shadow-pop` (dialoger, menyer), `--shadow-lg` (bunnlinje oppover). Ingen skygge i høykontrast.
- **Én aksent** `--c-accent` forbeholdt primærhandling og ring. Aktiv navigasjon er tekstfarge + kantmarkering, ikke aksent.
- **Semantikk**: `--c-danger`/`--c-danger-bg`, `--c-ok`/`--c-ok-bg`, advarsel `--c-badge-warn-*`, info `--c-info-bg`. Aldri farge alene: ikon, tekst og kant følger alltid.
- **Mørk modus** har egne nivåer (nav mørkest, bg, surface lysest) og samme semantikk; alle par i `scripts/contrast.mjs` testes i lys, mørk og høykontrast.
- **Forced-colors**: tokens mappes til Canvas/CanvasText/Highlight; tilstand bæres av kantbredde og ikon (aktiv nav 5 px, valgt segment 3 px). Ingen `forced-color-adjust: none` på tekst.

## 5. Komponentdetaljer
Page header: h1 `--fs-xl`, støttetekst under, handlinger høyre, brytes til rad 2. Card: radius-lg, subtil kant, valgfri overskrift lg. StatTile: etikett 600, tall `--fs-stat`, hint muted, chevron høyre. SegmentedControl: sunken bånd med `--space-1` luft, valgt segment hevet. SettingRow: to kolonner, 12 px vertikal luft, tynn skillelinje. Sidebar: merke 2 rem rundt logo, NavItem 40 px høy, ikon 20 px, teller som pille (`--radius-pill`) til høyre. Banner: 4 px venstrekant, ikon, tittel 600, handlinger til høyre eller under.

## 6. Per skjerm (bredde i px; arbeidsflaten er vindu minus 240 px ≥ 900)
- **Hjem**: toppsone med kompakt ring (md) + «X kan ryddes», «Rask opprydding» (primær) og papirkurvsum i ett Card; 1280: ring-kort venstre + kategorikort (StatTile) i `auto-fit` (to kolonner, 2×2) høyre; 1024: samme, kortene 2 kolonner; 800: ring over, kort 2 kolonner; 480: alt ett spor, StatTile fullbredde. Mappe og «Legg til mappe» som tynn stripe over (som skissen), skanning (Pause/Fortsett/Avbryt) som stripe med fremdrift. Banner for oppdatering/utløpt øverst.
- **Gjennomgang**: oversikt = StatTile-rutenett for fire kategorier (4 kol ved 1280, 2 ved 1024/800, 1 ved 480). Kategori: gruppert (duplikater) = liste venstre (grupperader) + detalj; ellers bildeflis `auto-fill` min 11 rem (≈ 5 kol ved 1280, 4 ved 1024, 3 ved 800, 2 ved 480). Bunnlinje: markering/angre venstre, primær høyre.
- **Gruppe**: 1280/1024: venstre liste over grupper (14 rem) + tre sammenligningsbilder side om side (`repeat(auto-fit, minmax(14rem, 1fr))`) med «Beste»-ramme (hake + tekst + `--c-ok`), mer enn tre bilder bryter til ny rad. 800: liste øverst som horisontal stripe; 480: bilder i to kolonner. Bunnlinje: Behold alle/gjør beste venstre, primær høyre.
- **Kortmodus**: 1280/1024: kort til venstre (maks 22 rem), handlingsknapper (Behold, Papirkurv, Hopp over, Angre) som kolonne høyre med tastehint; 800 og under: kort over, knapper under i rad. Ingen sidemeny-konflikt: Hurtigtaster skjules som før.
- **Papirkurv**: sammendragskort øverst (StatTile: antall, størrelse, utløper) + «Tøm» som sekundær; radliste i Card (padding none) med miniatyr, utløpsdato og Gjenopprett; 480: rader stables.
- **Innstillinger**: SettingsCard-er, maks `--w-content`; ved ≥ 1024 to kolonner (Generelt: språk, papirkurvfrist | Lagring: karantenemappe, Tøm-standard) med Oppdateringer som kort i full bredde under; under 1024 én kolonne. Språk og frist som SegmentedControl, auto-oppdatering som bryter-rad.
- **Onboarding**: sentrert Card (maks `--w-narrow`) med steg øverst; resultat som StatTile-rad. Hurtigtaster/Help og dialoger: `<dialog>` med `--shadow-pop`, radius-lg, maks 32 rem, knapper høyre.
- **Dialoger/ark**: uendret semantikk (fokusfelle, Esc). Primær høyre, avbryt venstre for den.

## 7. Aksept (M10-skall)
`just i18n`, `npx vitest run`, `npx svelte-check`, `node scripts/contrast.mjs`, `npm run build`; e2e a11y, aria, flows, herding, cloud, perf grønne. Skjermbilder: `docs/screens/m10/`.
