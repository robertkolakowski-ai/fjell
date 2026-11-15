//! V1 gjennom skallets kommandolag (`fjell_shell::Shell`, samme metoder som Tauri-kommandoene i `fjell-desktop` kaller)
//! mot ekte Fjell-kjerne (`Fjell` med `Store` i minnet og `MemSource`, ikke `MemApi`-mocken).
//!
//! Hva testene sjekker:
//! 1. Kodene og JSON-formen som sendes til frontend (feltnavn, `kind`/`reason`/`detail`-koder, aldri tekst eller stier).
//! 2. At feilkodene frontend har egne meldinger for faktisk er de som skallet sender (kontrakten i `desktop/src/bridge/types.ts`).
//! 3. At ingen V1-kommando gir frontend filsystem- eller nettverkstilgang: capabilities, Tauri-plugins, CSP, kommandooverflaten
//!    i `desktop/src-tauri/src/lib.rs` og at V1-modulene i skallet ikke bruker filsystem, nett eller prosesser.
//! 4. At hver V1-kommando svarer innenfor 100 ms på et lite bibliotek (CLAUDE.md: handlinger < 100 ms).
//!
//! Ikke verifisert her: Windows-bygg, WebView2, ekte ML-modeller (ansiktsindeksen er en tom kjerneflate uten modell) og
//! at kommandoene faktisk er registrert i en kjørende Tauri-app (fjell-desktop kan ikke bygges uten GTK/WebView2 her;
//! registreringen kontrolleres statisk mot kildeteksten i stedet).

use fjell_core::api::{Fjell, FjellApi, ScanConfig, ScanMode};
use fjell_core::model::{AssetMeta, FakeClock, Kind, MemSource, ProtectFlags, SourceId};
use fjell_core::store::Store;
use fjell_shell::dto::{DecisionDto, ErrorDto};
use fjell_shell::dto_v1::{ConditionDto, RuleDraftDto, ScopeDto};
use fjell_shell::Shell;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------------------------------------------------
// Oppsett: ekte kjerne, tre eksakte duplikater (to JPEG + én PNG som regel-treff), skannet.
// ---------------------------------------------------------------------------------------------------------------------

fn meta(name: &str, mime: &str, bytes: u64) -> AssetMeta {
    AssetMeta {
        source_id: SourceId(name.into()),
        kind: Kind::Photo,
        created_at: 1_000,
        mod_token: "t1".into(),
        width: 0,
        height: 0,
        bytes,
        mime: mime.into(),
        platform_screenshot_hint: false,
        has_camera_exif: Some(true),
        is_local: true,
        protect_flags: ProtectFlags::default(),
        albums: vec![],
        backup: None,
    }
}

fn shell() -> Shell {
    let src = MemSource::new();
    src.put(meta("a.jpg", "image/jpeg", 10), None, vec![7; 10]);
    src.put(meta("b.jpg", "image/jpeg", 10), None, vec![7; 10]);
    src.put(meta("c.png", "image/png", 14), None, vec![9; 14]);
    let f = Fjell::new(
        Store::open_in_memory().unwrap(),
        Arc::new(src),
        Arc::new(FakeClock::new(5_000_000)),
    );
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    f.wait_scan().unwrap().unwrap();
    Shell::new(Arc::new(f), "core")
}

fn asset_scope(id: u64) -> ScopeDto {
    ScopeDto {
        scope: "asset".into(),
        target: id.to_string(),
    }
}

fn err<T: Debug>(r: Result<T, ErrorDto>) -> ErrorDto {
    r.expect_err("kommandoen skulle avvises")
}

/// Alt som serialiseres til frontend i en test, samlet for lekkasjekontroll.
#[derive(Default)]
struct Wire(Vec<Value>);

impl Wire {
    fn push<T: serde::Serialize>(&mut self, v: &T) -> Value {
        let j = serde_json::to_value(v).unwrap();
        self.0.push(j.clone());
        j
    }
}

fn keys(v: &Value) -> BTreeSet<&str> {
    v.as_object().unwrap().keys().map(String::as_str).collect()
}

// ---------------------------------------------------------------------------------------------------------------------
// 1. Kode- og JSON-kontrakten mot ekte kjerne
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn v1_commands_send_codes_and_camel_case_json_from_real_core() {
    let s = shell();
    let mut wire = Wire::default();

    // Kort: eksakt duplikat har kode-begrunnelse og tom detalj; ingen sti, filnavn eller tekst.
    let page = s.cards("exact_dup", 10, None).unwrap();
    let card = &page.cards[0];
    let cj = wire.push(card);
    assert_eq!(cj["kind"], "exact_dup");
    assert_eq!(cj["detail"], json!({ "kind": "none" }));
    assert!(
        cj["reason"]
            .as_str()
            .unwrap()
            .chars()
            .all(|c| c.is_ascii_alphanumeric()),
        "begrunnelse er en kode: {}",
        cj["reason"]
    );
    assert_eq!(
        keys(&cj),
        [
            "asset",
            "bytes",
            "confidence",
            "detail",
            "group",
            "id",
            "keeper",
            "kind",
            "protectFlags",
            "reason"
        ]
        .into_iter()
        .collect()
    );

    // Beskyttelse: kommando og liste, og at papirkurv da krever bekreftelse med kode (aldri tekst).
    s.protect(&asset_scope(card.asset), true).unwrap();
    let pj = wire.push(&s.protections().unwrap());
    assert_eq!(
        pj,
        json!([{ "scope": { "scope": "asset", "target": card.asset.to_string() }, "createdAt": 5_000_000 }])
    );
    let e = err(s.decide(&[card.id], DecisionDto::Trash));
    assert_eq!(
        (e.code, e.detail.as_str()),
        ("ack_required", "protected_by_scope")
    );
    assert_eq!(
        s.state().trash_count,
        0,
        "avvist uten bekreftelse: ingenting flyttet"
    );
    let batch = s
        .decide_confirmed(
            &[card.id],
            DecisionDto::Trash,
            &["protected_by_scope".into()],
        )
        .unwrap();
    assert_eq!(s.state().trash_count, 1);
    assert_eq!(s.undo().unwrap(), batch, "alt som flyttes kan angres");
    assert_eq!(s.state().trash_count, 0);
    // Behold er alltid trygt, også for beskyttede bilder.
    s.decide(&[card.id], DecisionDto::Keep).unwrap();
    s.undo().unwrap();
    s.protect(&asset_scope(card.asset), false).unwrap();
    assert!(s.protections().unwrap().is_empty());

    // Regler: lagre, forhåndsvis, kort med regelkode, av/på, slett og gjenopprett.
    let conds = vec![ConditionDto::Mime("image/png".into())];
    let prev = wire.push(&s.preview_rule(&conds).unwrap());
    assert_eq!(prev["hits"], 1);
    assert!(prev.get("bytes").is_some());
    let id = s
        .save_rule(&RuleDraftDto {
            id: None,
            name: "Hemmelig navn".into(),
            conditions: conds,
        })
        .unwrap();
    let rj = wire.push(&s.rules().unwrap());
    assert_eq!(
        rj,
        json!([{ "id": id, "name": "Hemmelig navn", "enabled": true, "hits": 1, "conditions": [{ "kind": "mime", "value": "image/png" }] }])
    );
    let rc = wire.push(&s.cards("rule", 10, None).unwrap().cards[0]);
    assert_eq!(rc["kind"], "rule");
    assert_eq!(rc["reason"], "Rule");
    assert_eq!(rc["detail"], json!({ "kind": "rule", "id": id }));
    s.toggle_rule(id, false).unwrap();
    assert!(
        s.cards("rule", 10, None).unwrap().cards.is_empty(),
        "avslått regel foreslår ingenting"
    );
    s.toggle_rule(id, true).unwrap();
    s.delete_rule(id).unwrap();
    assert!(s.rules().unwrap().is_empty());
    s.restore_rule(id).unwrap();
    assert_eq!(s.rules().unwrap().len(), 1, "sletting av regel kan angres");

    // Ansiktsindeks: av som standard; på er et eget valg; personliste og sletting er koder og tall.
    assert!(!s.face_index().unwrap());
    wire.push(&s.people().unwrap());
    s.set_face_index(true).unwrap();
    assert!(s.face_index().unwrap());
    assert_eq!(s.wipe_face_index().unwrap(), 0);

    // Dashboard, innstillinger og terskler.
    let d = wire.push(&s.dashboard().unwrap());
    for k in [
        "activeCount",
        "freedBytes",
        "protectedCount",
        "progress",
        "nextStep",
        "perKind",
        "freedByMonth",
    ] {
        assert!(d.get(k).is_some(), "dashboard mangler {k}");
    }
    assert_eq!(
        d["freedBytes"], 0,
        "ingenting er tømt: aldri påstand om frigjort plass"
    );
    let t = wire.push(&s.thresholds().unwrap());
    assert_eq!(
        keys(&t),
        [
            "blurMaxScore",
            "closedEyesMinConfidence",
            "seriesGapSecs",
            "visualMaxHamming"
        ]
        .into_iter()
        .collect()
    );
    let st = wire.push(&s.suggestion_settings().unwrap());
    assert_eq!(
        keys(&st),
        ["faceIndex", "sensitivity", "trashDays"]
            .into_iter()
            .collect()
    );
    assert_eq!(st["sensitivity"].as_object().unwrap().len(), 6);

    // Ingenting som går til frontend inneholder filnavn eller stier (kilde-id-er forlater aldri kjernen).
    let alt = serde_json::to_string(&wire.0).unwrap();
    for forbudt in ["a.jpg", "b.jpg", "c.png", "\\\\", "C:"] {
        assert!(
            !alt.contains(forbudt),
            "{forbudt} lekket til frontend: {alt}"
        );
    }
}

#[test]
fn v1_commands_never_move_or_hide_cards_on_their_own() {
    // Appen foreslår, den dømmer ikke: ingen V1-kommando endrer antall aktive bilder eller flytter noe til papirkurv.
    let s = shell();
    let før = (s.dashboard().unwrap().active_count, s.state().trash_count);
    let id = s
        .save_rule(&RuleDraftDto {
            id: None,
            name: "PNG".into(),
            conditions: vec![ConditionDto::Mime("image/png".into())],
        })
        .unwrap();
    s.toggle_rule(id, false).unwrap();
    s.toggle_rule(id, true).unwrap();
    s.delete_rule(id).unwrap();
    s.restore_rule(id).unwrap();
    s.set_face_index(true).unwrap();
    s.wipe_face_index().unwrap();
    let card = s.cards("exact_dup", 10, None).unwrap().cards.remove(0);
    s.protect(&asset_scope(card.asset), true).unwrap();
    s.protect(&asset_scope(card.asset), false).unwrap();
    let etter = (s.dashboard().unwrap().active_count, s.state().trash_count);
    assert_eq!(før, etter);
    assert_eq!(etter.1, 0);
}

// ---------------------------------------------------------------------------------------------------------------------
// 2. Feilkoder: det frontend har meldinger for
// ---------------------------------------------------------------------------------------------------------------------

/// V1-feilkodene i `desktop/src/bridge/types.ts` (`BridgeErrorCode`, V1-blokken) og i i18n (`error.<kode>`).
const V1_FRONTEND_CODES: [&str; 10] = [
    "empty_folder",
    "unknown_target",
    "empty_rule",
    "empty_name",
    "name_too_long",
    "invalid_condition",
    "rule_not_found",
    "unknown_person",
    "not_merged",
    "unknown_series",
];

fn repo(rel: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR = <repo>/desktop/src-tauri/shell
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(rel)
}

fn read(rel: &str) -> String {
    // Windows-utsjekk kan ha CRLF; kildetekstkontrollene under forutsetter LF.
    std::fs::read_to_string(repo(rel))
        .unwrap_or_else(|e| panic!("kan ikke lese {rel}: {e}"))
        .replace("\r\n", "\n")
}

#[test]
fn every_v1_error_code_has_a_message_in_both_languages_and_a_type() {
    let ts = read("desktop/src/bridge/types.ts");
    let union = ts
        .split("export type BridgeErrorCode =")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    for kode in V1_FRONTEND_CODES {
        assert!(
            union.contains(&format!("\"{kode}\"")),
            "{kode} mangler i BridgeErrorCode"
        );
        for lang in ["nb", "en"] {
            let j: Value = serde_json::from_str(&read(&format!("i18n/{lang}.json"))).unwrap();
            assert!(
                j.get(format!("error.{kode}")).is_some(),
                "i18n/{lang}.json mangler error.{kode}"
            );
        }
    }
}

/// (kommando, feilen skallet faktisk sender)
fn v1_errors(s: &Shell) -> Vec<(&'static str, ErrorDto)> {
    let long = "x".repeat(200);
    vec![
        (
            "save_rule uten vilkår",
            err(s.save_rule(&RuleDraftDto {
                id: None,
                name: "tom".into(),
                conditions: vec![],
            })),
        ),
        (
            "save_rule uten navn",
            err(s.save_rule(&RuleDraftDto {
                id: None,
                name: "".into(),
                conditions: vec![ConditionDto::IsScreenshot],
            })),
        ),
        (
            "save_rule for langt navn",
            err(s.save_rule(&RuleDraftDto {
                id: None,
                name: long,
                conditions: vec![ConditionDto::IsScreenshot],
            })),
        ),
        ("toggle_rule ukjent", err(s.toggle_rule(9_999, true))),
        (
            "name_person ukjent",
            err(s.name_person(9_999, Some("x".into()))),
        ),
        ("merge_people ukjent", err(s.merge_people(1, 2))),
        (
            "unmerge_person ikke sammenslått",
            err(s.unmerge_person(9_999)),
        ),
        ("compare_series ukjent", err(s.compare_series(42))),
    ]
}

#[test]
fn v1_errors_are_codes_never_user_text() {
    let s = shell();
    for (hva, e) in v1_errors(&s) {
        let j = serde_json::to_value(&e).unwrap();
        assert_eq!(keys(&j), ["code", "detail"].into_iter().collect(), "{hva}");
        for felt in [e.code, e.detail.as_str()] {
            assert!(
                !felt.is_empty()
                    && felt.chars().all(|c| c.is_ascii_lowercase()
                        || c.is_ascii_digit()
                        || c == '_'
                        || c == ','
                        || c.is_ascii_uppercase()
                        || c == '('
                        || c == ')'
                        || c == '"'
                        || c == ' '),
                "{hva}: kode eller detalj ser ut som tekst: {felt:?}"
            );
        }
    }
    // Navn brukeren har gitt (regler, personer) kommer aldri tilbake i en feilmelding som kan logges.
    let e = err(s.save_rule(&RuleDraftDto {
        id: None,
        name: "Hemmelig navn".into(),
        conditions: vec![],
    }));
    assert!(!e.detail.contains("Hemmelig"), "{e:?}");
}

/// KONTRAKTSBRUDD (funnet av denne testen, se docs/v1/WINDOWS-V1.md del 8): frontend slår opp feilmelding på `BridgeError.code`
/// (rulesState.ts: `ERROR_KEYS[e.code]`) og mocken sender `code = "empty_rule"` osv. Skallet sender derimot
/// `code = "core_error"` med den stabile koden i `detail` (`ErrorDto::from(ApiError::Internal(kode))` i dto.rs), og TauriBridge
/// sender feilen videre uendret. Resultat i ekte app: regelfeil vises som generisk melding i stedet for «Regelen trenger et navn».
///
/// Testen krever riktig oppførsel og feiler derfor i dag. `should_panic` holder den grønn så lenge bruddet finnes, og slår ut den
/// dagen det er rettet (da fjernes attributtet). Rettelsen hører til delte filer (`shell/src/dto.rs` eller `tauri-bridge.ts`).
#[test]
#[should_panic(expected = "V1-feilkoder når ikke frontend")]
fn v1_error_codes_reach_frontend_as_the_code_it_has_messages_for() {
    let s = shell();
    let brudd: Vec<String> = v1_errors(&s)
        .into_iter()
        .filter(|(_, e)| {
            V1_FRONTEND_CODES.contains(&e.detail.as_str()) && !V1_FRONTEND_CODES.contains(&e.code)
        })
        .map(|(hva, e)| format!("{hva}: code={:?} detail={:?}", e.code, e.detail))
        .collect();
    assert!(
        brudd.is_empty(),
        "V1-feilkoder når ikke frontend som code:\n{}",
        brudd.join("\n")
    );
}

#[test]
fn v1_error_wire_shape_today_is_core_error_with_detail_code() {
    // Dokumenterer dagens form slik at en endring (enten rettelsen over eller en utilsiktet endring) blir synlig.
    let s = shell();
    let e = err(s.save_rule(&RuleDraftDto {
        id: None,
        name: "tom".into(),
        conditions: vec![],
    }));
    assert_eq!((e.code, e.detail.as_str()), ("core_error", "empty_rule"));
    assert_eq!(err(s.compare_series(42)).detail, "unknown_series");
    // Feil i selve kommandoparametrene (ukjent omfangskode) har egen kode.
    let b = err(s.protect(
        &ScopeDto {
            scope: "bogus".into(),
            target: "1".into(),
        },
        true,
    ));
    assert_eq!(b.code, "bad_request");
}

// ---------------------------------------------------------------------------------------------------------------------
// 3. Ingen filsystem- eller nettverkstilgang for frontend via V1
// ---------------------------------------------------------------------------------------------------------------------

/// Tauri-plugins appen får bruke, og hva de gir frontend. Alt annet er et brudd (filsystem, shell, http, opener, store, sql ...).
const ALLOWED_PERMISSIONS: [&str; 4] = [
    "core:default",
    "dialog:allow-open",
    "updater:default",
    "process:allow-restart",
];
const ALLOWED_PLUGINS: [&str; 3] = [
    "tauri-plugin-dialog",
    "tauri-plugin-process",
    "tauri-plugin-updater",
];

const V1_COMMANDS: [&str; 26] = [
    "protect",
    "list_protections",
    "list_rules",
    "save_rule",
    "preview_rule",
    "toggle_rule",
    "delete_rule",
    "restore_rule",
    "list_people",
    "name_person",
    "merge_people",
    "unmerge_person",
    "protect_person",
    "get_face_index",
    "set_face_index",
    "wipe_face_index",
    "get_dashboard",
    "compare_series",
    "get_suggestion_settings",
    "set_suggestion_settings",
    "get_thresholds",
    // Delte med MVP, men brukt av V1-skjermene (avgjørelse med bekreftelse, valg av beste, angre).
    "decide_cards",
    "set_keeper",
    "undo",
    "list_cards",
    "get_state",
];

#[test]
fn capabilities_give_frontend_only_app_commands_dialog_updater_and_restart() {
    let cap: Value =
        serde_json::from_str(&read("desktop/src-tauri/capabilities/default.json")).unwrap();
    let perms: Vec<&str> = cap["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            p.as_str()
                .expect("tillatelser er strenger (ingen scoped objekter)")
        })
        .collect();
    assert_eq!(
        perms.iter().copied().collect::<BTreeSet<_>>(),
        ALLOWED_PERMISSIONS.into_iter().collect::<BTreeSet<_>>(),
        "capabilities endret: {perms:?}"
    );
    for p in &perms {
        let plugin = p.split(':').next().unwrap();
        for farlig in [
            "fs",
            "shell",
            "http",
            "opener",
            "websocket",
            "sql",
            "store",
            "os",
            "clipboard-manager",
            "global-shortcut",
            "notification",
            "upload",
            "log",
            "persisted-scope",
        ] {
            assert_ne!(plugin, farlig, "farlig tillatelse for frontend: {p}");
        }
    }
    assert_eq!(
        cap["windows"],
        json!(["main"]),
        "bare hovedvinduet har rettigheter"
    );
    for nøkkel in ["remote", "webviews", "platforms_override"] {
        assert!(
            cap.get(nøkkel).is_none(),
            "capability gjelder eksterne adresser eller andre webviews: {nøkkel}"
        );
    }
    // Ingen V1-kommando er knyttet til en plugin-tillatelse: de er appens egne kommandoer (core:default dekker invoke av dem).
    for c in V1_COMMANDS {
        assert!(
            perms.iter().all(|p| !p.ends_with(c)),
            "{c} står som tillatelse"
        );
    }
}

#[test]
fn only_dialog_process_and_updater_plugins_are_linked() {
    let toml = read("desktop/src-tauri/Cargo.toml");
    let plugins: BTreeSet<&str> = toml
        .lines()
        .filter_map(|l| {
            l.trim()
                .split(|c: char| c == '=' || c.is_whitespace())
                .next()
        })
        .filter(|n| n.starts_with("tauri-plugin-"))
        .collect();
    assert_eq!(
        plugins,
        ALLOWED_PLUGINS.into_iter().collect::<BTreeSet<_>>(),
        "nye Tauri-plugins må vurderes mot CLAUDE.md (on-device, minimale rettigheter)"
    );
}

#[test]
fn csp_and_webview_config_allow_no_network_and_no_file_protocol() {
    let conf: Value = serde_json::from_str(&read("desktop/src-tauri/tauri.conf.json")).unwrap();
    let sec = &conf["app"]["security"];
    let csp = sec["csp"].as_str().expect("produksjons-CSP må være satt");
    assert!(csp.contains("default-src 'self'"));
    assert!(
        csp.contains("connect-src ipc: http://ipc.localhost;"),
        "connect-src skal bare tillate Tauri-IPC: {csp}"
    );
    let uten_ipc = csp.replace("http://ipc.localhost", "");
    for ekstern in [
        "http://",
        "https://",
        "ws://",
        "wss://",
        "*",
        "unsafe-eval",
        "file:",
        "asset:",
        "blob:",
    ] {
        assert!(!uten_ipc.contains(ekstern), "CSP tillater {ekstern}: {csp}");
    }
    assert!(csp.contains("object-src 'none'") && csp.contains("frame-ancestors 'none'"));
    // Asset-protokollen gir frontend lesetilgang til filer; den skal ikke være på. Global Tauri-objekt og CSP-unntak heller ikke.
    assert!(
        sec.get("assetProtocol").is_none(),
        "assetProtocol gir filtilgang fra frontend"
    );
    assert!(sec.get("dangerousDisableAssetCspModification").is_none());
    assert!(conf["app"]
        .get("withGlobalTauri")
        .is_none_or(|v| v == &json!(false)));
    assert!(conf.get("plugins").is_none_or(|p| p.get("fs").is_none()
        && p.get("shell").is_none()
        && p.get("http").is_none()));
}

/// `#[tauri::command]`-funksjonene i fjell-desktop: (navn, kropp).
fn desktop_commands() -> Vec<(String, String)> {
    let src = read("desktop/src-tauri/src/lib.rs");
    let mut out = vec![];
    let mut deler = src.split("#[tauri::command]");
    deler.next();
    for del in deler {
        let navn = del
            .split("fn ")
            .nth(1)
            .unwrap()
            .split(['(', '<'])
            .next()
            .unwrap()
            .trim()
            .to_string();
        let kropp = del.split("\n}\n").next().unwrap().to_string();
        out.push((navn, kropp));
    }
    out
}

fn registered_handlers() -> BTreeSet<String> {
    let src = read("desktop/src-tauri/src/lib.rs");
    let liste = src
        .split("generate_handler![")
        .nth(1)
        .expect("generate_handler! finnes")
        .split(']')
        .next()
        .unwrap();
    liste
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[test]
fn every_v1_command_is_registered_and_is_a_thin_wrapper_without_fs_or_net() {
    let cmds = desktop_commands();
    let names: BTreeSet<String> = cmds.iter().map(|(n, _)| n.clone()).collect();
    let reg = registered_handlers();
    assert_eq!(
        names, reg,
        "kommandoer og generate_handler! er ikke like (skjult eller uregistrert kommando)"
    );
    for c in V1_COMMANDS {
        assert!(reg.contains(c), "{c} er ikke registrert");
    }
    for (navn, kropp) in &cmds {
        for forbudt in [
            "std::fs",
            "tokio::fs",
            "std::net",
            "std::process",
            "Command::new",
            "reqwest",
            "hyper",
            "ureq",
            "TcpStream",
            "UdpSocket",
            "File::",
            "read_dir",
            "remove_file",
            "remove_dir",
        ] {
            assert!(
                !kropp.contains(forbudt),
                "kommandoen {navn} bruker {forbudt}"
            );
        }
        // Omslaget kaller bare skallet (eller spawn_blocking mot det): all logikk og all I/O ligger i fjell-shell.
        assert!(
            kropp.contains("shell.") || kropp.contains("sh."),
            "{navn} kaller ikke skallet"
        );
    }
}

#[test]
fn the_frontend_bridge_invokes_exactly_registered_commands() {
    let ts = read("desktop/src/bridge/tauri-bridge.ts");
    let mut brukt: BTreeSet<String> = BTreeSet::new();
    for del in ts.split("this.call").skip(1) {
        let etter = del
            .trim_start_matches(|c: char| c != '(')
            .trim_start_matches('(');
        if let Some(n) = etter.strip_prefix('"') {
            brukt.insert(n.split('"').next().unwrap().to_string());
        }
    }
    let reg = registered_handlers();
    let ukjente: Vec<_> = brukt.difference(&reg).collect();
    assert!(
        ukjente.is_empty(),
        "frontend kaller kommandoer som ikke finnes: {ukjente:?}"
    );
    for c in V1_COMMANDS.iter().filter(|c| {
        ![
            "get_state",
            "list_cards",
            "decide_cards",
            "set_keeper",
            "undo",
        ]
        .contains(c)
    }) {
        assert!(
            brukt.contains(*c),
            "V1-kommandoen {c} brukes ikke av TauriBridge (død kommando eller manglende kobling)"
        );
    }
    // Frontend har ingen egne sti- eller nettverkskommandoer og bruker ingen filplugin.
    for forbudt in [
        "plugin:fs|",
        "plugin:shell|",
        "plugin:http|",
        "plugin:opener|",
        "readFile",
        "writeFile",
        "fetch(",
    ] {
        assert!(!ts.contains(forbudt), "TauriBridge bruker {forbudt}");
    }
}

#[test]
fn shell_v1_modules_do_not_touch_filesystem_network_or_processes() {
    for fil in [
        "desktop/src-tauri/shell/src/v1.rs",
        "desktop/src-tauri/shell/src/dto_v1.rs",
    ] {
        let kode = read(fil);
        // Bare ikke-kommentar-linjer teller.
        for (i, l) in kode
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim_start().starts_with("//"))
        {
            for forbudt in [
                "std::fs",
                "std::net",
                "std::process",
                "fjell_fs",
                "RecycleBin",
                "File::",
                "read_dir",
                "remove_file",
                "remove_dir",
                "Command::new",
                "TcpStream",
            ] {
                assert!(
                    !l.contains(forbudt),
                    "{fil}:{} bruker {forbudt}: {l}",
                    i + 1
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// 4. Ytelse: hver V1-kommando under 100 ms
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn each_v1_command_answers_within_100_ms_on_a_small_library() {
    let s = shell();
    let budsjett = Duration::from_millis(100);
    let tid = |hva: &str, f: &dyn Fn()| {
        // Beste av tre: måler kommandoen, ikke en tilfeldig planleggingsforsinkelse på CI.
        let best = (0..3)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed()
            })
            .min()
            .unwrap();
        assert!(
            best < budsjett,
            "{hva} brukte {best:?} (budsjett {budsjett:?})"
        );
    };
    let card = s.cards("exact_dup", 10, None).unwrap().cards.remove(0);
    tid("list_cards", &|| {
        s.cards("exact_dup", 50, None).unwrap();
    });
    tid("get_dashboard", &|| {
        s.dashboard().unwrap();
    });
    tid("list_rules", &|| {
        s.rules().unwrap();
    });
    tid("list_protections", &|| {
        s.protections().unwrap();
    });
    tid("list_people", &|| {
        s.people().unwrap();
    });
    tid("get_suggestion_settings", &|| {
        s.suggestion_settings().unwrap();
    });
    tid("get_thresholds", &|| {
        s.thresholds().unwrap();
    });
    tid("preview_rule", &|| {
        s.preview_rule(&[ConditionDto::IsScreenshot]).unwrap();
    });
    tid("protect + unprotect", &|| {
        s.protect(&asset_scope(card.asset), true).unwrap();
        s.protect(&asset_scope(card.asset), false).unwrap();
    });
    tid("save_rule + delete_rule + restore_rule", &|| {
        let id = s
            .save_rule(&RuleDraftDto {
                id: None,
                name: "t".into(),
                conditions: vec![ConditionDto::IsScreenshot],
            })
            .unwrap();
        s.delete_rule(id).unwrap();
        s.restore_rule(id).unwrap();
    });
}
