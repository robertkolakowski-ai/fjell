//! Tester for diagnose (`diag.rs`): redaksjon, rullerende logg, panikkhook, rapport og lekkasjetest mot et
//! syntetisk bibliotek med norske tegn og lange Windows-stier. Kjørt på Linux.

use super::*;
use crate::Shell;
use std::sync::Arc;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("fjell-diag-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// Strenger som aldri skal stå i noen rapport eller loggfil.
const SECRETS: [&str; 7] = [
    "Åse",
    "Bjørn",
    "ferie på Ørland",
    "IMG_0042",
    "\\\\server\\",
    "C:\\Users",
    "Familiealbum",
];

fn hostile_messages() -> Vec<String> {
    let long = format!(
        "C:\\Users\\Åse Nordmann\\Bilder\\{}\\ferie på Ørland\\IMG_0042.jpg",
        "Familiealbum æøå ".repeat(20)
    );
    vec![
        long.clone(),
        format!("kunne ikke lese {long}"),
        "\\\\server\\deling\\Bjørn\\IMG_0042.jpg".to_string(),
        "/home/åse/Bilder/IMG_0042.jpg".to_string(),
        "regel «Slett Bjørns skjermbilder»".to_string(),
        "album=Familiealbum".to_string(),
        "diag_failed\nfolder.1=Åse".to_string(),
    ]
}

fn assert_clean(s: &str) {
    for secret in SECRETS {
        assert!(!s.contains(secret), "lekkasje av {secret:?} i:\n{s}");
    }
    assert!(!s.contains('\\') && !s.contains('/'), "skråstrek i:\n{s}");
    assert!(s.is_ascii(), "tegn utenfor ASCII i:\n{s}");
}

#[test]
fn unknown_messages_become_other_and_known_codes_survive() {
    for m in hostile_messages() {
        assert_eq!(known_code(&m), "other");
    }
    assert_eq!(known_code("backup_failed"), "backup_failed");
    assert_eq!(known_code("no_library"), "no_library");
    assert_eq!(known_code("Backup_Failed"), "other");
}

#[test]
fn log_record_from_hostile_message_has_only_codes_and_numbers() {
    for m in hostile_messages() {
        let rec = LogRecord::new(1_700_000_000, Category::Core, &m, 7);
        assert_clean(&rec.line());
        assert_eq!(LogRecord::parse(&rec.line()), Some(rec));
    }
}

#[test]
fn parse_rejects_tampered_lines() {
    let ok = "ts=1 cat=core code=other n=1 v=0.0.0";
    assert!(LogRecord::parse(ok).is_some());
    for bad in [
        "ts=1 cat=core code=C:\\Users\\Åse n=1 v=0.0.0",
        "ts=1 cat=core code=other n=1 v=0.0.0 extra=Åse",
        "ts=1 cat=core code=other n=Åse v=0.0.0",
        "ts=1 cat=album code=other n=1 v=0.0.0",
        "ts=1 cat=core code=other n=1 v=C:/x",
        "tull",
    ] {
        assert!(LogRecord::parse(bad).is_none(), "{bad}");
    }
}

#[test]
fn rolling_log_respects_size_cap_and_file_count() {
    let d = tmp("rull");
    let log = RollingLog::new(d.join("logs"), 300, 3);
    for i in 0..200 {
        log.append(&LogRecord::new(i, Category::Scan, "io_error", i as u64))
            .unwrap();
    }
    let files: Vec<_> = fs::read_dir(d.join("logs")).unwrap().flatten().collect();
    assert!(files.len() <= 3, "{} filer", files.len());
    for f in &files {
        assert!(f.metadata().unwrap().len() <= 300);
    }
    let tail = log.tail(5);
    assert_eq!(tail.len(), 5);
    assert_eq!(tail.last().unwrap().ts, 199);
    assert!(tail.windows(2).all(|w| w[0].ts < w[1].ts));
    let _ = fs::remove_dir_all(d);
}

#[test]
fn tail_skips_tampered_lines_in_the_file() {
    let d = tmp("tamp");
    let log = RollingLog::new(d.join("logs"), LOG_MAX_BYTES, LOG_FILES);
    log.append(&LogRecord::new(1, Category::Core, "internal", 1))
        .unwrap();
    let mut f = fs::OpenOptions::new()
        .append(true)
        .open(d.join("logs").join("fjell.log"))
        .unwrap();
    writeln!(f, "C:\\Users\\Åse\\Bilder\\IMG_0042.jpg").unwrap();
    writeln!(f, "ts=2 cat=core code=Åse n=1 v=0").unwrap();
    drop(f);
    let t = log.tail(10);
    assert_eq!(t.len(), 1);
    let _ = fs::remove_dir_all(d);
}

#[test]
fn redact_report_keeps_pairs_and_drops_everything_else() {
    let input = "format=1\nos=windows\nlog.1=1|core|other|1|0.1.0\n\
        sti=C:\\Users\\Åse\\Bilder\n\
        x=/home/åse/IMG_0042.jpg\n\
        album=Familiealbum æøå\n\
        Stor=1\n\
        fritekst uten likhetstegn\n\
        k=\\\\server\\deling\n\
        n=12345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890\n";
    let out = redact_report(input);
    assert_eq!(out, "format=1\nos=windows\nlog.1=1|core|other|1|0.1.0\n");
}

#[test]
fn folder_names_never_contain_separators() {
    assert_eq!(
        folder_label(Path::new("C:\\Users\\Åse\\Bilder\\Ferie på Ørland")),
        // På Linux er hele strengen ett filnavn: skilletegnene fjernes uansett.
        "C:UsersÅseBilderFerie på Ørland".replace(':', "").as_str()
    );
    let l = folder_label(Path::new("/a/b/Mappe=å\n:x"));
    assert!(!l.contains(['/', '\\', ':', '=', '\n']));
    assert!(folder_label(Path::new(&"å".repeat(100))).chars().count() <= 40);
}

fn input() -> ReportInput<'static> {
    ReportInput {
        core: DiagReport::empty(),
        app_version: "0.1.0",
        os: "windows",
        arch: "x86_64",
        os_version: Some("10.0.22631"),
        scale_percent: Some(150),
        log: vec![LogRecord::new(5, Category::Panic, "panic", 42)],
        perf: vec![("scan", 3, 90, 40)],
        folders: Vec::new(),
    }
}

#[test]
fn report_has_the_documented_contents_and_nothing_personal() {
    let t = build_text(&input());
    for line in [
        "app_version=0.1.0",
        "os=windows",
        "os_version=10.0.22631",
        "scale_percent=150",
        "integrity=ok",
        "perf.scan.count=3",
        "log_entries=1",
        "log.1=5|panic|panic|42|",
    ] {
        assert!(t.contains(line), "mangler {line}:\n{t}");
    }
    assert!(!t.contains("folder."));
    assert_clean(&t);
}

#[test]
fn unknown_environment_is_stated_not_guessed() {
    let mut i = input();
    i.os_version = None;
    i.scale_percent = None;
    let t = build_text(&i);
    assert!(t.contains("os_version=unknown") && t.contains("scale_percent=unknown"));
}

#[test]
fn set_env_rejects_non_version_text() {
    set_env("C:\\Windows\\Åse", 150);
    assert!(ENV.lock().unwrap().os_version.is_none());
    set_env("10.0.22631", 0);
    let e = ENV.lock().unwrap().clone();
    assert_eq!(e.os_version.as_deref(), Some("10.0.22631"));
    assert!(e.scale_percent.is_none());
    set_env("", 0);
}

/// Syntetisk bibliotek: norske tegn, mellomrom og en sti langt over 260 tegn.
fn synthetic_library(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let base = tmp(tag);
    let mut lib = base.join("Åse sine bilder");
    for _ in 0..6 {
        lib = lib.join("Familiealbum æøå ferie på Ørland ".repeat(2).trim());
    }
    assert!(lib.to_string_lossy().len() > 260);
    fs::create_dir_all(&lib).unwrap();
    fs::write(lib.join("IMG_0042 Bjørn.jpg"), b"ikke et ekte bilde").unwrap();
    (base.clone(), base.join("appdata"), lib)
}

#[test]
fn leak_test_against_synthetic_library() {
    let (base, data, lib) = synthetic_library("lekk");
    let sh = Shell::with_data_dir(&data, Arc::new(crate::NoRecycleBin));
    sh.set_library(&lib.to_string_lossy()).unwrap();
    for m in hostile_messages() {
        sh.diag_log(Category::Core, &m, 3);
        sh.diag_log(Category::Scan, &m, 0);
    }
    perf_observe("scan", 12);
    perf_observe(&hostile_messages()[0], 99_999);

    // Standard: ingen mappenavn.
    let prev = sh.diag_preview().unwrap();
    assert_clean(&prev.text);
    assert!(!prev.text.contains("folder."));
    assert!(prev.text.contains("log_entries="));

    // Filen er nøyaktig forhåndsvisningen.
    let saved = sh.diag_export_with(DiagOptions::default()).unwrap();
    let file = fs::read_to_string(&saved.saved_path).unwrap();
    assert_eq!(file, prev.text);
    assert_eq!(file, saved.report.text);
    assert!(Path::new(&saved.saved_path).starts_with(export_dir(&data)));
    // Stien står ikke i rapporten selv.
    assert!(!file.contains(&data.to_string_lossy().into_owned()));

    // Selve loggfilene er også rene.
    for f in fs::read_dir(data.join("logs")).unwrap().flatten() {
        assert_clean(&fs::read_to_string(f.path()).unwrap());
    }

    // Med samtykke: bare siste mappenavn, aldri sti.
    let on = DiagOptions {
        folder_names: FolderNamesConsent::new(true),
    };
    let with = sh.diag_preview_with(on).unwrap();
    assert!(with.text.contains("folder.1="));
    assert!(with.text.contains("Familiealbum æøå ferie på Ørland"));
    assert!(!with.text.contains("Åse sine bilder"));
    assert!(!with.text.contains(['/', '\\']));
    assert!(!with.text.contains("IMG_0042"));
    // Uten avkrysning gir samtykket ingenting.
    assert!(FolderNamesConsent::new(false).is_none());
    let _ = fs::remove_dir_all(base);
}

#[test]
fn export_without_data_dir_fails_with_code() {
    let e = Shell::mock().diag_export().unwrap_err();
    assert_eq!((e.code, e.detail.as_str()), ("core_error", "diag_failed"));
}

#[test]
fn panic_hook_writes_a_redacted_record_and_never_the_message() {
    let d = tmp("panikk");
    install_panic_hook(&d);
    let secret = "C:\\Users\\Åse\\Bilder\\IMG_0042.jpg ferie på Ørland";
    let r = std::panic::catch_unwind(|| {
        panic!("kunne ikke åpne {secret}");
    });
    assert!(r.is_err());
    // Tilbake til standardhook så senere tester ikke arver den.
    let _ = std::panic::take_hook();
    let log = RollingLog::in_data_dir(&d);
    let t = log.tail(10);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].category, Category::Panic);
    assert_eq!(t[0].code, "panic");
    assert!(t[0].n > 0);
    let raw = fs::read_to_string(d.join("logs").join("fjell.log")).unwrap();
    assert_clean(&raw);
    assert!(!raw.contains("ikke åpne"));
    let _ = fs::remove_dir_all(d);
}
