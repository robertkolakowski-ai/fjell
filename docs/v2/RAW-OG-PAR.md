# V2: RAW, forhåndsbilder og RAW+JPEG-par

Status: plan (ingen kode). Hører til `V2-PLAN.md` del 3.2. Bindende regler: `CLAUDE.md`. Bygger på `docs/mvp/DEKODING.md` (WIC) og ARKITEKTUR beslutning 6 og 21.

## 1. Prinsipp

Culling trenger rask og stabil visning, ikke fullverdig RAW-fremkalling. Derfor:
1. RAW-filer vises og analyseres via det **innebygde JPEG-forhåndsbildet** (de fleste kameraer legger en stor preview i filen).
2. Full dekoding via WIC brukes bare ved eksplisitt 100 %-zoom og fokusmaske, og gir `Unavailable` med forklaring når kodeken mangler.
3. Originalfilene leses, aldri skrives.

## 2. Forhåndsbilde

- Trekk: `embedded_preview(asset) -> Preview { jpeg_bytes, width, height, orientation, source: PreviewSource }`. `PreviewSource` er `Embedded`, `Wic` eller `Unavailable`.
- TIFF-baserte formater (DNG, CR2, NEF, ARW, ORF, RW2): les IFD-kjeden og velg den største JPEG-strømmen. CR3 (ISO-BMFF) og RAF har egen struktur og trenger egen leser (V2-PLAN spørsmål 5).
- Les bare det som trengs (header og forhåndsbildets bytes), ikke hele filen: en 50 MB RAW skal ikke leses ferdig for en 256 px miniatyr.
- Resultatet går inn i eksisterende `thumb::finish` (EXIF-orientering, nedskalering til 256 px), slik at hash og uskarphet bruker samme kode som for JPEG. `fingerprint.preview_source` lagres; endret kilde utløser reanalyse (beslutning 14).
- Feil: korrupt struktur gir `Corrupt`, manglende preview gir `Unavailable` (ingen feiltelling, som i dag), aldri panikk (`catch_unwind` per bilde som i pipeline).
- Deterministisk: samme bytes gir samme hash på alle plattformer, fordi WIC ikke er med i stien.

## 3. Full dekoding

WIC som i dag (`wic.rs`) med minnegrense 512 MB RGBA. RAW med 45+ MP kan overskride grensen; grensen må tas opp (V2-PLAN R3). Forslag: bare ett fullbilde om gangen ved 100 %-zoom i sammenligning.

## 4. RAW+JPEG-par

Regel: to aktive bilder i **samme mappe** med **samme grunnnavn** (uten endelse, uavhengig av store/små bokstaver) der den ene er RAW og den andre JPEG/HEIF, utgjør et par. Lagres i `asset_pair(pair_id, primary_id, secondary_id, kind)`.

- `primary` er det som vises (JPEG hvis den finnes, ellers RAW-forhåndsbildet; konfigurerbart). Vurderinger lagres på `primary_id` og gjelder paret.
- Flere RAW-endelser (CR2 + DNG) eller to JPEG med samme navn gir **ikke** et par (flertydig); filene vises hver for seg.
- Idempotent: ny skanning gir samme `pair_id` så lenge begge filene finnes.
- Beslutninger: «Behold begge», «Flytt begge til Nylig slettet», «Bare JPEG», «Bare RAW». Alt går via `actions` som én batch; `ConsequenceCode::PairHalf` vises når bare den ene halvparten røres. Beskyttelse av ett bilde i paret gjelder paret. «Gjenopprett» gjenoppretter det som ble flyttet i samme batch.
- Kortmodus og Gjennomgang viser paret som ett kort med begge filnavn og formater. «Del opp paret» finnes og kan angres.
- Flytting utenfor Fjell: filID-sporing finnes ikke (beslutning 21). Et omdøpt par blir «borte + ny» og mister vurderinger (som i V1-PLAN spørsmål 5). Akseptert i V2, men varsles i UI; en XMP/JSON-eksport før omdøping gir sikkerhetskopi av vurderingene.

## 5. Tester

- Syntetiske TIFF-fixtures laget i test (ingen ekte kameraeiendom sjekkes inn): gyldig preview, uten preview, korrupt IFD, sykliske IFD-pekere, svært stor offset, 0 byte, avkuttet fil.
- Fuzz-lignende test: tilfeldige byteflips gir aldri panikk og alltid `Ok`, `Corrupt` eller `Unavailable`.
- Par: golden-mappe med par, enkeltfiler, store/små bokstaver, to RAW-endelser, to mapper med samme navn, Unicode, lange stier.
- Hash: forhåndsbilde fra fixture gir samme `dhash`/`phash` som tilsvarende JPEG-bytes.
- Ikke verifisert: ekte RAW fra noe kamera, WIC-RAW, Windows-ytelse.
