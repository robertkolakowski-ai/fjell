# V2: mobilfasen (iPhone og Android)

Status: plan. **Ingen kode** før MVP og V1 er stabile (beslutning 2026-11-11; `CLAUDE.md`: ingen iOS-/Android-kode i MVP). Hører til `V2-PLAN.md` del 3.5.

## 1. Utgangspunkt (ærlig status)

- `core/crates/fjell-ffi` finnes (UniFFI, `Fjell`-objekt, `AssetSourceCallback`, `PurgeTicket` som ugjennomsiktig objekt) og speiler **MVP-flaten**. Den speiler **ikke** V1-flaten (V1-PLAN spørsmål 11) og ikke V2-flaten. `docs/v1/FFI.md` er en tom plassholder.
- Kjernen er plattformnøytral (`AssetSource`-trait, ingen plattformkode i `fjell-core`). Ingen iOS-/Android-kode eller mobiladapter finnes.
- Mobil-UI blir native og tynt (SwiftUI, Jetpack Compose); all logikk ligger i kjernen.

## 2. Inngangskriterier (alle må være oppfylt)

1. MVP er ferdig inkludert M9 (Windows-verifisering og ekte data).
2. V1 er sluttreviewet uten åpne funn og Windows-verifisert; modeller og terskler er avgjort (V1-PLAN spørsmål 3, 8, 12).
3. V2-kjernen (culling, par, sammenligning, eksport) er integrert og reviewet; API-flaten er stabil nok til å speile.
4. Produkteier har frigitt mobil skriftlig (ny beslutning i `VEIKART.md`, ført av integratøren).

## 3. Faser

| Fase | Innhold | Gren | Avhenger av |
|---|---|---|---|
| FFI-speil | UniFFI-typer og metoder for V1 og V2 i `fjell-ffi`, rundtur-tester mot `MemApi`, `docs/v1/FFI.md` fylles ut | `v2/ffi` | V2-I |
| iOS | `app/ios/` (SwiftUI), PhotoKit-adapter som `AssetSource`, xcframework via UniFFI, VoiceOver, iOS 17+, macOS-runner | `mobile/ios` | FFI-speil |
| Android | `app/android/` (Kotlin, Compose), MediaStore-adapter, `cargo-ndk`/AAR, TalkBack, Android 11+ | `mobile/android` | FFI-speil |
| Mobil-integrer | Felles kontrakttester, i18n nb + en (nn er stub), ytelse | `mobile/integrate` | iOS, Android |

iOS og Android er parallelle og eier hver sin mappe; de deler ingen filer utenom `fjell-ffi` (låst etter FFI-speilet).

## 4. Mobilspesifikke regler og risiko

- **Sletting:** iOS krever systembekreftelse, og Android 11+ viser systemdialog for sletting via MediaStore. Samle i store batcher og forklar dialogen på forhånd (`TRYGGHET.md`). Der plattformen eier papirkurven (PhotoKit «Nylig slettet») må «alt kan angres» tolkes mot plattformens papirkurv og dokumenteres per plattform, ikke antas. Dette er den største åpne regelen for mobil.
- **Originaler:** PhotoKit og MediaStore gir ikke alltid filbaserte originaler. «Originalfilers innhold endres aldri» betyr at appen ikke redigerer assets. Vurderinger lagres i Fjells egen indeks på enheten. XMP-skriving finnes ikke på mobil i V2.
- **Batteri og varme:** indeksering bare ved lading og Wi-Fi (VEIKART), lav prioritet, avbrytbar.
- **On-device:** iCloud- og skyplassholdere lastes aldri ned under skanning (som `is_local = false` på PC); ingen synk av vurderinger mellom PC og telefon (V2-PLAN spørsmål 12).
- **Tilgjengelighet:** VoiceOver/TalkBack, Dynamic Type, knappealternativ til alle gester; haptikk valgfri.
- **Culling på mobil:** RAW-støtte avhenger av plattformen; forhåndsbilde-stien i `RAW-OG-PAR.md` gjelder bare PC til videre.
- **Ytelse:** samme budsjetter (< 100 ms p95, kald oppstart < 1,5 s) må måles på referanseenhet; ingen mobiltall finnes i dag.

## 5. Akseptkriterier (mobil)

- Rundtur-test: samme kommando gir samme resultat og begrunnelseskoder via UniFFI som via Rust.
- `PurgeTicket` kan ikke lages eller endres fra Swift/Kotlin (som i dag).
- VoiceOver/TalkBack-gjennomgang av hver skjerm; Dynamic Type opp til største størrelse; nb og en.
- Ingen nettverkstilgang i appen (manifest og entitlements sjekkes i CI).
- Alt dette er **ikke verifisert** før enhet og macOS-runner finnes; planen lover ingenting om det.
