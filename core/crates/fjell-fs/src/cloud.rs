//! Sikkerhetsnett mot skysynkede mapper (iCloud, OneDrive, Dropbox).
//!
//! Avhenger av: ingenting (ren strengbehandling, ingen filtilgang).
//! Grensesnitt: `CloudProvider`, `cloud_provider`.
//!
//! Dette er en heuristikk basert på kjente mappenavn, ikke en garanti: en synkmappe med
//! egendefinert navn oppdages ikke, og en vanlig mappe kan i teorien hete det samme.
//! Deteksjonen er derfor kun en advarsel/sperre mot sletting, aldri bevis på synk.

/// Kjent skyleverandør for en synkmappe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudProvider {
    ICloud,
    OneDrive,
    Dropbox,
}

impl CloudProvider {
    /// Stabil kode som UI slår opp i nøkkelkatalogen.
    pub fn code(self) -> &'static str {
        match self {
            CloudProvider::ICloud => "icloud",
            CloudProvider::OneDrive => "onedrive",
            CloudProvider::Dropbox => "dropbox",
        }
    }
}

fn strip_verbatim(path: &str) -> &str {
    // `\\?\UNC\server\share` og `\\?\C:\...`: komponentene under er uansett det som teller.
    path.strip_prefix(r"\\?\UNC\")
        .or_else(|| path.strip_prefix(r"\\?\"))
        .unwrap_or(path)
}

fn classify(component: &str) -> Option<CloudProvider> {
    let c = component.to_lowercase();
    match c.as_str() {
        "icloud photos" | "iclouddrive" | "icloud drive" | "icloud-bilder" => {
            return Some(CloudProvider::ICloud)
        }
        "onedrive" | "dropbox" => {
            return Some(if c == "onedrive" {
                CloudProvider::OneDrive
            } else {
                CloudProvider::Dropbox
            })
        }
        _ => {}
    }
    if c.starts_with("onedrive -") || c.starts_with("onedrive-") {
        Some(CloudProvider::OneDrive)
    } else if c.starts_with("dropbox (") {
        Some(CloudProvider::Dropbox)
    } else {
        None
    }
}

/// Leverandøren hvis en sti-komponent har et kjent synkmappenavn (første treff fra roten).
/// Hele komponenter sammenlignes; «MyOneDriveNotes» treffer ikke.
pub fn cloud_provider(path: &str) -> Option<CloudProvider> {
    strip_verbatim(path)
        .split(['/', '\\'])
        .filter(|c| !c.is_empty())
        .find_map(classify)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_folders_case_insensitively() {
        assert_eq!(
            cloud_provider("/Users/a/OneDrive/Bilder"),
            Some(CloudProvider::OneDrive)
        );
        assert_eq!(cloud_provider("/u/ONEDRIVE"), Some(CloudProvider::OneDrive));
        assert_eq!(cloud_provider("/u/dropbox/x"), Some(CloudProvider::Dropbox));
        assert_eq!(
            cloud_provider("/u/iCloudDrive/x"),
            Some(CloudProvider::ICloud)
        );
        assert_eq!(
            cloud_provider("/u/ICLOUD PHOTOS"),
            Some(CloudProvider::ICloud)
        );
        assert_eq!(
            cloud_provider("/u/iCloud Drive"),
            Some(CloudProvider::ICloud)
        );
        assert_eq!(
            cloud_provider("/u/iCloud-Bilder/x"),
            Some(CloudProvider::ICloud)
        );
    }

    #[test]
    fn detects_named_variants() {
        assert_eq!(
            cloud_provider(r"C:\Users\a\OneDrive - Firma AS\Bilder"),
            Some(CloudProvider::OneDrive)
        );
        assert_eq!(
            cloud_provider("/u/OneDrive-Personal"),
            Some(CloudProvider::OneDrive)
        );
        assert_eq!(
            cloud_provider("/u/Dropbox (Personal)/x"),
            Some(CloudProvider::Dropbox)
        );
    }

    #[test]
    fn handles_windows_separators_and_prefixes() {
        assert_eq!(
            cloud_provider(r"C:\Users\a\Dropbox\x"),
            Some(CloudProvider::Dropbox)
        );
        assert_eq!(
            cloud_provider(r"\\?\C:\Users\a\OneDrive\x"),
            Some(CloudProvider::OneDrive)
        );
        assert_eq!(
            cloud_provider(r"\\?\UNC\server\share\OneDrive\x"),
            Some(CloudProvider::OneDrive)
        );
        assert_eq!(
            cloud_provider(r"\\server\share\Dropbox\x"),
            Some(CloudProvider::Dropbox)
        );
        assert_eq!(cloud_provider(r"\\?\UNC\server\share\bilder"), None);
        assert_eq!(
            cloud_provider(r"C:/Users/a/OneDrive/x"),
            Some(CloudProvider::OneDrive)
        );
    }

    #[test]
    fn false_positives_do_not_match() {
        for p in [
            "/u/MyOneDriveNotes",
            "/u/OneDriveBackup/x",
            "/u/NotDropbox",
            "/u/Dropbox2/x",
            "/u/dropboxes",
            "/u/icloud",
            "/u/Bilder/ferie",
            "",
            "/",
            r"\\?\",
        ] {
            assert_eq!(cloud_provider(p), None, "{p}");
        }
    }

    #[test]
    fn codes_are_stable() {
        assert_eq!(CloudProvider::ICloud.code(), "icloud");
        assert_eq!(CloudProvider::OneDrive.code(), "onedrive");
        assert_eq!(CloudProvider::Dropbox.code(), "dropbox");
    }
}
