# Kalibrering mot ekte bilder (MVP-restanse 6 og 7)

Mål: avgjøre `max_span` / `tight_dist` (kandidat 12/5, se `K8-ANALYSE.md`) og fryse `ALGO_VER = 1`, på et etikettert korpus av ekte foto. Verktøyet er `fjell-cli calibrate`. Det endrer ingen algoritmer: det kjører kjernens egne funksjoner (`hash::perceptual`, `cluster::visual_groups`, `cluster::exact_groups`, `quality::blur_score`, `suggest::blur_threshold`) og måler mot etikettene dine.

Status: verktøyet er testet på Linux med det syntetiske golden-korpuset (`cargo test -p fjell-cli`). Det er ikke kjørt på ekte bilder eller på Windows.

## 1. Begrensning: bare JPEG og PNG dekodes i dag
Beslutning 6: kjernen dekoder bare JPEG og PNG. HEIC, RAW, TIFF og andre telles som «ikke dekodbare» og utelates fra visuell gruppering og uskarphet (de er med i eksakte duplikater, som leser bytes). Før HEIC/RAW-dekoding (WIC) finnes, kan kalibreringen bare gjelde JPEG/PNG. Ta likevel med noen HEIC/RAW i korpuset slik at rapporten viser hvor mange som ikke ble målt.

## 2. Slik lager produkteier korpuset
**Størrelse.** Minst 1 000 bilder totalt (jo flere jo bedre; 2 000 gir tryggere falske-sammenslåing-tall). Anbefalt fordeling:

| Format | Antall | Merknad |
|---|---|---|
| JPEG fra mobil | 600 | Minst to ulike telefoner |
| JPEG fra kamera/speilløst | 250 | Inkl. noen høy-ISO |
| PNG (skjermbilder, redigerte) | 100 | |
| HEIC og RAW | 50 hver | Telles som ikke dekodbare nå; trengs når dekoder kommer |

**Innhold som må med** (ellers blir falske sammenslåinger undervurdert):
- Serier og burst (5–20 nesten like bilder; her er risikoen for 12/5).
- Samme motiv fra samme sted (hage, utsikt, tavle, rom) på ulike dager.
- Trær, gress, vann, tekstiler og tekst (svake områder i `K8-ANALYSE.md`).
- Ekte duplikater: kopier mellom mapper, sendt via meldingstjeneste (omkodet, mindre), beskåret.
- Uskarpe bilder: bevegelse, fokusfeil, mørke bilder med støy, og skarpe bilder med lite kontrast og lite detalj (himmel, vegg) som ikke skal flagges.
- Ikke fjern «dårlige» bilder på forhånd; korpuset skal ligne et vanlig bibliotek.

**Personvern.** Bildene forlater aldri maskinen. Verktøyet bruker ingen nettverk, indeksfil eller cache og leser originalene bare. Ligger korpuset på en delt disk, kopier det til en lokal mappe først. Send ikke bilder eller utklipp til utviklere; rapporten inneholder bare filnavn og tall, så gi nøytrale filnavn (`0001.jpg`) hvis navnene er sensitive. Ikke legg korpuset i Git. Slett kopien når kalibreringen er ferdig.

**Etikettering.**
1. To etikettere merker uavhengig; uenigheter avgjøres av produkteier. Mål: samsvar over 95 %.
2. Gruppe-ID: bilder som viser samme øyeblikk/motiv (inkl. omkodede, nedskalerte, beskårne og uskarpe varianter) får samme ID. Bilder uten noen kopi eller serie får tom ID. Tommelregel: «ville du ønsket at Fjell foreslo å beholde bare ett av disse?»
3. Eksakt-ID: bare byte-identiske filer deler ID. Verktøyet sjekker dette selv (BLAKE3), så avvik her avslører etikettfeil.
4. Uskarp: `ja` hvis bildet er uskarpt nok til at du ville slettet det, `nei` ellers; la stå tom hvis du ikke vil vurdere. Vurder på full størrelse, ikke på miniatyr.
5. Tips: la Fjell-appen eller en vanlig bildeviser sortere etter filnavn/tid; lag CSV i Excel.

## 3. Etikettformat (CSV)
UTF-8, første linje er overskrift. Skilletegn komma, semikolon (norsk Excel) eller tabulator (velges fra overskriften). Felt kan stå i «…» (`""` for anførselstegn); ingen linjeskift i felt. BOM er ok.

| Kolonne | Påkrevd | Betydning |
|---|---|---|
| `fil` | ja | Sti relativt til mappen, med `/` (også `\` godtas) |
| `visuell_gruppe` | minst én av tre | Valgfri tekst-ID. Samme ID = samme motiv. Tom celle = ingen kopier |
| `eksakt_gruppe` | | Samme ID = identiske bytes. Tom = ingen eksakt kopi |
| `uskarp` | | `ja` / `nei` (også j/n/true/false/1/0). Tom = ikke vurdert (utelatt) |

Eksempel:
```
fil,visuell_gruppe,eksakt_gruppe,uskarp
2024/ferie/0001.jpg,v1,e1,nei
2024/ferie/0001 (kopi).jpg,v1,e1,nei
2024/ferie/0002.jpg,v1,,ja
2024/hage/0007.jpg,,,nei
```
Regler: filer i mappen som ikke står i CSV utelates (telles og rapporteres). Filer i CSV som ikke finnes i mappen gir feil (exit 1), så skrivefeil oppdages. En fil kan ikke stå to ganger. Ugyldig `uskarp`-verdi gir feil (exit 2).

## 4. Kjøring på Windows
Bygg én gang (utvikler): `cargo build --release -p fjell-cli --manifest-path core\Cargo.toml` gir `core\target\release\fjell-cli.exe`. Kopier den til korpusmaskinen, så trengs ingenting mer.

PowerShell:
```
.\fjell-cli.exe calibrate --dir D:\korpus --labels D:\korpus-etiketter.csv --rows 100 > resultat-10-4.txt
.\fjell-cli.exe calibrate --dir D:\korpus --labels D:\korpus-etiketter.csv --rows 100 --max-span 12 --tight-dist 5 > resultat-12-5.txt
```
Valg: `--max-dist N` (6), `--max-span N` (10), `--tight-dist N` (4), `--rows N` (50, antall rader i tabellene). Uten valg brukes dagens standard (`Params::default()`). Etikettfilen bør ligge utenfor mappen. Exit 0 = målt; 1 = lesefeil eller filer mangler; 2 = bruksfeil eller feil etikettformat. Kjøretid er dominert av dekoding (anslag 20–50 ms per bilde, flere tråder; ikke målt på ekte bilder).

## 5. Rapporten
- `[eksakte duplikater]`: presisjon/recall på par. Skal være 1,000/1,000; alt annet er etikettfeil.
- `[visuell hash-gruppering]`: to linjer. «alle filer» er sammenlignbar med K8-tallene (0,891 osv.). «én per identisk innhold» fjerner eksakte kopier, som er lette treff, og er tallet som sier mest om ekte ytelse. Bilder med lav informasjon er utelatt som i skanningen.
- `[falske sammenslåinger]`: tabell over filpar Fjell ville lagt i samme gruppe, men som etiketten sier ikke hører sammen (`maks_hamming` = høyeste Hamming av dHash/pHash, `gruppestørrelse` = hvor mange bilder gruppen fikk). Sortert med minst avstand først.
- `[uskarphet]`: presisjon/recall ved skanningens terskel (`min(P10, BLUR_ABS_MAX)`, brukes i appen) og ved fast `BLUR_ABS_MAX`, pluss tabell over feilflagg (`flagget_men_skarp`) og oversette (`ikke_flagget_men_uskarp`). Ikke vurderbare (flate) bilder utelates, som i K8.
- Pardefinisjon: alle par i samme gruppe, som i `golden_blur_ext.rs`.

Merk: P10-terskelen avhenger av hele mappen (også filer uten etikett), akkurat som i appen.

## 6. Beslutningsregel for `max_span` / `tight_dist`
Kjør standard (10/4) og kandidat (12/5), eventuelt flere (11/4, 12/4) for å se hvor kurven knekker. Ta tallene fra linjen «én per identisk innhold».

Innfør 12/5 hvis alle gjelder:
1. Falske par øker med høyst 10 % (og høyst 5 par absolutt) mot 10/4, og presisjonen er minst 0,98.
2. Ingen ny falsk sammenslåing i tabellen er en serie der du ville angret på forhåndsmerkingen (bla gjennom tabellen manuelt; hver rad er ett bilde du kan åpne).
3. Recall øker målbart (minst +0,02), ellers er det ingen grunn til å bredere gruppene.
4. Brutt ned på de harde kategoriene (trær, serier, tekst) forverres ingen.

Ellers behold 10/4. Hvis recall på ekte bilder fortsatt er under 0,90 med presisjon over 0,98, er dette en algoritmeendring (nytt hashgrunnlag), ikke en parameter: ny fase, ikke del av dette. Dokumenter beslutningen og tallene i `K8-ANALYSE.md` og `MVP-PLAN.md` 4c punkt 6, og oppdater `PLAN-KJERNE.md` K2 punkt 4 hvis verdiene endres.

Uskarphet: godkjenn dagens `BLUR_ABS_MAX` hvis presisjon ≥ 0,90 og recall ≥ 0,90 ved skanningens terskel. Er presisjonen lavere, se tabellen over `flagget_men_skarp` (typisk lite kontrast eller bokeh); en terskelendring er en algoritmeendring og følger regelen under.

## 7. Frysing av `ALGO_VER`
`ALGO_VER` (nå 1, `core/crates/fjell-core/src/model/mod.rs`) frysses når alt dette er sant:
1. Kalibreringen over er kjørt, og beslutningen om `max_span`/`tight_dist` og uskarphetsterskel er tatt og dokumentert.
2. Endres bare klyngeparametere (`Params`), øker ikke `ALGO_VER`: klyngingen regnes fra lagrede hasher ved hver gruppering (`K8-ANALYSE.md`). Endres derimot hash, `blur_score`, `BLUR_ABS_MAX` eller skjermbildetabellen, må tabellene regnes om og `ALGO_VER` økes først; kalibrer deretter på nytt.
3. `just check` er grønn, og de låste vektorene (K3-6, hash-vektorene) er oppdatert ved eventuell endring.
4. Produkteier har signert av. Fra da gjelder: enhver algoritmeendring øker `ALGO_VER` og gir reindeksering ved neste fulle skann (restanse 7, testet).

Restanse 7 er allerede låst med test (reindeksering ved ny `ALGO_VER`); det som gjenstår er selve frysingen etter kalibrering.

## 8. Ikke verifisert
Windows-kjøring, ekte bilder (alle tall så langt er syntetiske), HEIC/RAW-måling, kjøretid på stort korpus og beslutningsreglene i del 6 (de er forslag, avventer produkteier).
