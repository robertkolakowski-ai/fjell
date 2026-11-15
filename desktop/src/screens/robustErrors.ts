// Feilkode -> i18n-nøkkel for robusthetsflaten (sikkerhetskopi, indeksering, diagnose). Ukjent kode gir den generelle meldingen.
import type { MessageKey } from "../lib/i18n";

const KEYS: Readonly<Record<string, MessageKey>> = {
  backup_not_found: "error.backup_not_found",
  backup_ack_required: "error.backup_ack_required",
  backup_corrupt: "error.backup_corrupt",
  backup_failed: "error.backup_failed",
  index_busy: "error.index_busy",
  indexing_not_running: "error.indexing_not_running",
  indexing_invalid_settings: "error.indexing_invalid_settings",
  diag_failed: "error.diag_failed",
  not_implemented: "error.not_implemented",
};

export function robustErrorKey(code: string | null): MessageKey {
  return (code !== null && KEYS[code]) || "err.generic";
}
