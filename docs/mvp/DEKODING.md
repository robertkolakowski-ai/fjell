# Dekoding (HEIC/HEIF og RAW via WIC)

Mål: `image`-craten beholder JPEG/PNG (deterministisk, alle plattformer). HEIC/HEIF og vanlige RAW-formater dekodes på Windows via Windows Imaging Component (WIC). Andre plattformer og manglende kodek gir `Unavailable`. Beslutningen står i `ARKITEKTUR.md` nr. 6.

## Oppbygging (`core/crates/fjell-fs`)
- `thumb::FrameDecoder`: trekk med `extensions`, `probe` (dimensjoner og EXIF-orientering) og `decode` (første ramme som RGBA8).
- `thumb::make_thumb` / `oriented_dims` / `is_decodable` har uendret signatur. De bruker `platform_decoder()`: WIC på Windows, `None` ellers. `make_thumb_with` og `oriented_dims_with` tar dekoder eksplisitt (tester).
- jpg/jpeg/png går alltid via `image`, før noen ekstra dekoder (byte-lik som før, låst i `tests/decode.rs`).
- Orientering og nedskalering til 256 px gjøres av samme kode (`thumb::finish`) for begge stier. WIC leverer full oppløsning uten egen skalering, for minst mulig hashdrift. Pris: RAW med mange megapiksler bruker mye minne (grense 512 MB RGBA, over det `Failed`).
- `wic.rs` (`cfg(windows)`): COM per kall (`CoInitializeEx` MTA, `RPC_E_CHANGED_MODE` tolereres), `IWICImagingFactory`, `CreateDecoderFromFilename`, ramme 0, `IWICFormatConverter` til 32bpp RGBA. EXIF-orientering leses fra `/app1/ifd` (JPEG) eller `/ifd` (HEIF/TIFF/RAW).
- `unsafe`: kun `wic.rs`, `windows.rs` og `drives::sys` (alle `cfg(windows)`, egen `#![allow(unsafe_code)]`). Kommentaren i `Cargo.toml` er utvidet.

## Feilmapping
| Situasjon | Resultat |
|---|---|
| Kodek mangler (HEIF-utvidelse ikke installert), ukjent format, COM- eller ukjent HRESULT-feil | `SourceError::Unavailable` |
| `WINCODEC_ERR_BADIMAGE/BADHEADER/BADSTREAMDATA/...` | `Corrupt` |
| Fil/sti finnes ikke | `NotFound` |
| Ugyldig bufferstørrelse fra dekoder | `Corrupt` |
| Heic/RAW på ikke-Windows | `Unavailable` (endelsen er ikke dekodbar) |

`Unavailable` hoppes over uten feiltelling i pipelinen (`pipeline/full.rs`, `fast.rs`: `skipped_unavailable`). `SourceError::Unavailable` har ingen tekst; forklaringen UI kan vise ligger i `wic::UNAVAILABLE_HINT`, men er ikke koblet til UI ennå.

## Hva som er verifisert
- Linux: `cargo test -p fjell-fs` (trait-tester med fake-dekoder, JPEG/PNG byte-lik med referanse, heic/RAW gir `Unavailable`), clippy `-D warnings`, fmt, `just layers`.
- `wic.rs` er kompilert og clippy-sjekket mot `x86_64-pc-windows-gnu` i et isolert prøveprosjekt (samme `windows`-feature og kode, stubbet `thumb`/`fjell_core`). Hele `fjell-fs` lot seg ikke sjekke for Windows her, fordi `libsqlite3-sys` trenger `x86_64-w64-mingw32-gcc`. Linking og kjøring er ikke gjort.
- Windows-testene i `tests/windows.rs` (`wic_jpeg_and_image_jpeg_hash_within_tolerance`, `wic_garbage_heic_never_panics_and_is_unavailable_or_corrupt`) er ikke kompilert og ikke kjørt. Første kjøring er `windows-latest`. Hash-toleransen (4 av 64 biter) er et anslag.

## Ikke verifisert
- Ingen ekte HEIC/HEIF- eller RAW-fil er dekodet, med eller uten kodek.
- Orientering fra HEIF/RAW-metadata (`/ifd/{ushort=274}`) er ikke prøvd.
- Dekodetid og minnebruk (4c nr. 5) er ikke målt.
- Hashstabilitet WIC-JPEG mot `image`-JPEG er bare skrevet som test.

## Skannlisten
Ikke utvidet. `source.rs::mime_and_kind` har fortsatt bare `heic` blant disse formatene; `thumb::WIC_EXTENSIONS` (heic, heif, dng, cr2, cr3, nef, arw, orf, rw2, raf) styrer bare dekoding. Pipelinen hopper over `Unavailable` korrekt (se over), men å vise RAW/HEIF i biblioteket uten miniatyr er en produktbeslutning og ligger utenfor P1.
