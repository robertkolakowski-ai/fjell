//! Robusthetstyper i `model` (L0): sikkerhetskopi av indeksen, integritet, indeksering og diagnose.
//! Se docs/v1/ROBUSTHET-KONTRAKT.md. Ren datamodell uten I/O; alt er koder, tall og ID-er, aldri UI-tekst.
//! Unntaket er `DiagReport::text`: en teknisk, redigert tekstblokk (bare koder og tall) som brukeren kan se
//! før noe eksporteres. Den inneholder aldri stier, filnavn, bilder eller navn (docs/PERSONVERN.md).
//!
//! Avhenger av: ingenting (selvstendig modul i `model`). Grensesnitt:
//! - `BackupId`, `BackupKind`, `RestoreAck` (bekreftelsesport for gjenoppretting), `IntegrityStatus`.
//! - `IndexState`, `IndexPriority`: koder for indekseringens tilstand og prioritet.
//! - `DiagMetric`, `DiagReport` (`render` lager den redigerte tekstblokken).
//! - `ROBUST_ERROR_CODES`: stabile feilkoder som `ApiError::Internal` bærer for denne flaten.

use std::fmt::Write as _;

/// Sikkerhetskopi av indeksen (`backup`-rad). Kopien inneholder bare metadata i Fjells egen indeks,
/// aldri bildefiler, og skrives bare til appens egen datamappe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BackupId(pub u64);

/// Hvorfor en kopi finnes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BackupKind {
    /// Brukeren ba om den.
    Manual,
    /// Laget av appen etter planen (f.eks. ukentlig). Aldri uten at brukeren kan se den i listen.
    Auto,
    /// Tatt rett før en gjenoppretting; er det som gjør gjenopprettingen angrebar.
    PreRestore,
}

impl BackupKind {
    pub const ALL: [BackupKind; 3] = [Self::Manual, Self::Auto, Self::PreRestore];

    /// Stabil snake_case-kode (i18n-nøkkel `backup.kind.<kode>`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
            Self::PreRestore => "pre_restore",
        }
    }

    pub fn parse(code: &str) -> Option<BackupKind> {
        Self::ALL.into_iter().find(|k| k.as_str() == code)
    }
}

/// Resultat av en integritetssjekk av indeksen. Alt annet enn `Ok` gir trygg modus i UI (ingen skriving, ingen
/// skanning) til brukeren har valgt gjenoppretting eller ny indeks. Originalbildene røres aldri.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum IntegrityStatus {
    #[default]
    Ok,
    /// Indeksen kan leses, men har avvik (f.eks. manglende rader). Appen foreslår å gjenopprette.
    Degraded,
    /// Databasen er ødelagt (`PRAGMA integrity_check` feilet).
    Corrupt,
    /// Filen kan ikke åpnes (låst, manglende tilgang eller ukjent skjemaversjon).
    Unreadable,
}

impl IntegrityStatus {
    pub const ALL: [IntegrityStatus; 4] =
        [Self::Ok, Self::Degraded, Self::Corrupt, Self::Unreadable];

    /// Stabil snake_case-kode (i18n-nøkkel `backup.integrity.<kode>`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Degraded => "degraded",
            Self::Corrupt => "corrupt",
            Self::Unreadable => "unreadable",
        }
    }

    pub fn parse(code: &str) -> Option<IntegrityStatus> {
        Self::ALL.into_iter().find(|s| s.as_str() == code)
    }

    pub fn is_ok(self) -> bool {
        self == Self::Ok
    }
}

/// Bevis for at brukeren har bekreftet at gjenoppretting erstatter dagens indeks med den valgte kopien.
/// Kan bare lages med `RestoreAck::new(.., true)`; `backup_restore` tar den som argument (samme mønster som
/// `PermanentAck`: porten håndheves av typen, ikke av konvensjon).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestoreAck {
    backup: BackupId,
}

impl RestoreAck {
    /// `checked` er brukerens avkrysning i bekreftelsesdialogen. `None` hvis den ikke er satt.
    pub fn new(backup: BackupId, checked: bool) -> Option<RestoreAck> {
        checked.then_some(RestoreAck { backup })
    }

    /// Kopien bekreftelsen gjelder. Gjenoppretting av en annen kopi krever ny bekreftelse.
    pub fn backup(&self) -> BackupId {
        self.backup
    }
}

/// Indekseringens tilstand (bakgrunnsskanning).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum IndexState {
    #[default]
    Idle,
    Running,
    /// Satt på pause av brukeren.
    Paused,
    /// Satt på pause fordi PC-en går på batteri (`IndexingSettings::pause_on_battery`). Fortsetter av seg selv
    /// når strømmen er tilbake, med mindre brukeren har pauset.
    PausedOnBattery,
}

impl IndexState {
    pub const ALL: [IndexState; 4] = [
        Self::Idle,
        Self::Running,
        Self::Paused,
        Self::PausedOnBattery,
    ];

    /// Stabil snake_case-kode (i18n-nøkkel `indexing.state.<kode>`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::PausedOnBattery => "paused_on_battery",
        }
    }

    pub fn parse(code: &str) -> Option<IndexState> {
        Self::ALL.into_iter().find(|s| s.as_str() == code)
    }
}

/// Prioritet for indekseringstrådene (OS-trådprioritet på Windows; ellers ingen virkning).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum IndexPriority {
    /// Bakgrunn: tar minst mulig fra annet arbeid (standard).
    #[default]
    Low,
    Normal,
}

impl IndexPriority {
    pub const ALL: [IndexPriority; 2] = [Self::Low, Self::Normal];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
        }
    }

    pub fn parse(code: &str) -> Option<IndexPriority> {
        Self::ALL.into_iter().find(|p| p.as_str() == code)
    }
}

/// Én måling i diagnoserapporten (bare antall og versjoner, aldri innhold).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiagMetric {
    SchemaVersion,
    AssetsActive,
    AssetsTrashed,
    Groups,
    Suggestions,
    Rules,
    Protections,
    Backups,
}

impl DiagMetric {
    pub const ALL: [DiagMetric; 8] = [
        Self::SchemaVersion,
        Self::AssetsActive,
        Self::AssetsTrashed,
        Self::Groups,
        Self::Suggestions,
        Self::Rules,
        Self::Protections,
        Self::Backups,
    ];

    /// Stabil snake_case-kode (i18n-nøkkel `diag.metric.<kode>`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SchemaVersion => "schema_version",
            Self::AssetsActive => "assets_active",
            Self::AssetsTrashed => "assets_trashed",
            Self::Groups => "groups",
            Self::Suggestions => "suggestions",
            Self::Rules => "rules",
            Self::Protections => "protections",
            Self::Backups => "backups",
        }
    }
}

/// Diagnoserapport: tall og koder, pluss en redigert tekstblokk lagd av `render`. Brukeren ser rapporten
/// (`diag_preview`) før noe eksporteres (`diag_export`); ingenting sendes noe sted av appen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagReport {
    /// Format-versjon for rapporten selv.
    pub format: u32,
    pub integrity: IntegrityStatus,
    pub indexing: IndexState,
    pub metrics: Vec<(DiagMetric, u64)>,
    /// Redigert tekstblokk (`render`): linjer på formen `kode=verdi`. Ingen stier, filnavn eller navn.
    pub text: String,
}

impl DiagReport {
    pub const FORMAT: u32 = 1;

    /// Bygger rapporten og tekstblokken ut fra tall og koder (det er ikke plass til fritekst her).
    pub fn new(
        integrity: IntegrityStatus,
        indexing: IndexState,
        metrics: Vec<(DiagMetric, u64)>,
    ) -> DiagReport {
        let mut r = DiagReport {
            format: Self::FORMAT,
            integrity,
            indexing,
            metrics,
            text: String::new(),
        };
        r.text = r.render();
        r
    }

    /// Tom rapport (standard for implementasjoner uten diagnose).
    pub fn empty() -> DiagReport {
        Self::new(IntegrityStatus::Ok, IndexState::Idle, Vec::new())
    }

    /// Den redigerte tekstblokken: én linje per kode og tall, stabil rekkefølge.
    pub fn render(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "format={}", self.format);
        let _ = writeln!(s, "integrity={}", self.integrity.as_str());
        let _ = writeln!(s, "indexing={}", self.indexing.as_str());
        for (m, v) in &self.metrics {
            let _ = writeln!(s, "{}={v}", m.as_str());
        }
        s
    }
}

/// Stabile feilkoder (snake_case) som `ApiError::Internal` bærer for backup, indeksering og diagnose.
/// UI oversetter via i18n-nøkkelen `error.<kode>`.
pub const ROBUST_ERROR_CODES: [&str; 9] = [
    "backup_not_found",
    "backup_ack_required",
    "backup_corrupt",
    "backup_failed",
    "index_busy",
    "indexing_not_running",
    "indexing_invalid_settings",
    "diag_failed",
    "not_implemented",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_roundtrip() {
        for k in BackupKind::ALL {
            assert_eq!(BackupKind::parse(k.as_str()), Some(k));
        }
        for s in IntegrityStatus::ALL {
            assert_eq!(IntegrityStatus::parse(s.as_str()), Some(s));
        }
        for s in IndexState::ALL {
            assert_eq!(IndexState::parse(s.as_str()), Some(s));
        }
        for p in IndexPriority::ALL {
            assert_eq!(IndexPriority::parse(p.as_str()), Some(p));
        }
        assert_eq!(IntegrityStatus::parse("x"), None);
    }

    #[test]
    fn restore_needs_checked_box() {
        assert_eq!(RestoreAck::new(BackupId(3), false), None);
        let ack = RestoreAck::new(BackupId(3), true).expect("avkrysset");
        assert_eq!(ack.backup(), BackupId(3));
    }

    #[test]
    fn report_text_has_only_codes_and_numbers() {
        let r = DiagReport::new(
            IntegrityStatus::Degraded,
            IndexState::Running,
            DiagMetric::ALL.iter().map(|m| (*m, 7)).collect(),
        );
        assert_eq!(r.text, r.render());
        assert!(r
            .text
            .starts_with("format=1\nintegrity=degraded\nindexing=running\n"));
        assert!(!r.text.contains('/') && !r.text.contains('\\') && !r.text.contains(':'));
        assert_eq!(r.text.lines().count(), 3 + DiagMetric::ALL.len());
        assert_eq!(DiagReport::empty().integrity, IntegrityStatus::Ok);
    }

    #[test]
    fn error_codes_are_snake_case_and_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for c in ROBUST_ERROR_CODES {
            assert!(c.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_'));
            assert!(seen.insert(c));
        }
    }
}
