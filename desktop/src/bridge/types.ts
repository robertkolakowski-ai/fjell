// Typer som speiler fjell-shell::dto (camelCase). Kjernen sender koder, aldri tekst.
export type KindCode = "exact_dup" | "visual_dup" | "blurry" | "screenshot";
export const KIND_CODES: readonly KindCode[] = ["exact_dup", "visual_dup", "blurry", "screenshot"];

/** Speiler fjell-core `ScanPhase` (phase_code i dto.rs). */
export type ScanPhase = "idle" | "listing" | "exact" | "exact_ready" | "analyzing" | "done";

/** Speiler fjell-core `ReasonCode` (variantnavn). Begrunnelsesmerke vises per forslag. */
export type ReasonCode =
  | "ExactCopy" | "VisualDuplicate" | "Blurry" | "Screenshot"
  | "HigherResolution" | "LessCompressed" | "Sharper" | "Older"
  | "ShotPlatformFlag" | "ShotScreenSize" | "ShotNoCameraExif" | "ShotPng";

export interface KindSummary {
  kind: KindCode;
  count: number;
  bytes: number;
}

export interface ScanStatus {
  phase: ScanPhase;
  done: number;
  total: number;
}

/** «Nylig slettet». */
export interface TrashSummary {
  count: number;
  bytes: number;
  expired: number;
  freedBytes: number;
  days: number;
}

/** Kjent skyleverandør (speiler fjell-fs `CloudProvider::code`). */
export type CloudCode = "icloud" | "onedrive" | "dropbox";

export interface AppState {
  backend: "mock" | "core";
  perKind: KindSummary[];
  reclaimableBytes: number;
  trashCount: number;
  trashBytes: number;
  trashExpired: number;
  freedBytes: number;
  scanPhase: ScanPhase;
  scanDone: number;
  scanTotal: number;
  trashDays: number;
  /** Skann satt på pause av brukeren (avbrutt, kan fortsettes). Mangler = ikke pauset. */
  scanPaused?: boolean;
  /** Tidspunkt (ms siden epoch) for første permanente sletting, til historikklinjen. Kjernen leverer det ikke ennå. */
  freedSince?: number | null;
  /** Gjeldende bibliotek ligger i en skysynket mappe (etter mappenavn, ikke en garanti). Papirkurv-valg gir `cloud_synced_readonly`. Mangler/null = ikke skysynket. */
  cloudSync?: CloudCode | null;
  /** Bibliotekets mapper i rekkefølge (første er `primary`). Her uten tall (`online`/`count`/… mangler); `listLibraryRoots` fyller dem ut. Mangler = eldre skall/mock uten bibliotek. */
  roots?: LibraryRoot[];
}

/**
 * Én mappe (rot) i biblioteket. Speiler RootDto. Flere mapper: docs/mvp/FLERE-MAPPER.md.
 * `cloudSync` på en rot sperrer papirkurv bare for bildene i den roten (`cloud_synced_readonly`);
 * `AppState.cloudSync` er satt bare når ALLE røtter er skysynket.
 */
export interface LibraryRoot {
  /** Stabil nøkkel (1 = første mappe). Brukes i `removeLibraryRoot`. Gjenbrukes aldri. */
  key: number;
  path: string;
  /** Første mappe: eier indeksen og kan ikke fjernes (bytt bibliotek med `setLibrary`). */
  primary: boolean;
  cloudSync: CloudCode | null;
  /** Mappens egen karantenemappe (`<mappe>/.fjell-karantene`). */
  quarantineDir: string;
  /** Mappen er tilgjengelig nå (ekstern disk tilkoblet). Bare fra `listLibraryRoots`. */
  online?: boolean | null;
  /** Aktive filer (bilder og video) og samlet størrelse. Bare fra `listLibraryRoots`. */
  count?: number | null;
  bytes?: number | null;
  /** Herav video (telles og listes; får bare forslag som eksakt duplikat). */
  videos?: number | null;
  videoBytes?: number | null;
  /** Antall i «Nylig slettet» fra denne mappen. Over 0: mappen kan ikke fjernes (`root_has_trash`). */
  trashed?: number | null;
}

/** Brukerens ekstra bekreftelse for permanent sletting (avkrysning + antallet på knappen). Speiler PermanentAckDto. */
export interface PermanentAck {
  checked: boolean;
  count: number;
}

/** Resultat av «Tøm». Alt som ikke er `purged` ligger fortsatt i «Nylig slettet». Speiler EmptyDto. */
export interface EmptyResult {
  purged: number;
  keptTrashed: number;
  /** Volum uten Papirkurv: ikke slettet. UI kan tilby permanent med egen advarsel. */
  unsupported: number;
  cancelled: number;
  unavailable: number;
  freedBytes: number;
}

/** Ett forslag (kort). `group`/`keeper` er satt for dubletter. */
export interface Card {
  id: number;
  kind: KindCode;
  asset: number;
  group: number | null;
  keeper: number | null;
  reason: ReasonCode;
  confidence: number;
  bytes: number;
  protectFlags: number;
}

export interface CardPage {
  cards: Card[];
  next: number | null;
}

/** Gruppe avledet av kort med samme `group` (rent, se groups.ts). */
export interface Group {
  id: number;
  keeper: number | null;
  cards: Card[];
  bytes: number;
}

export type Decision = "trash" | "keep";

export type BridgeErrorCode =
  | "ack_required" | "unknown_card" | "unknown_asset" | "nothing_to_undo" | "not_undoable"
  | "already_purged" | "purge_stale" | "plan_stale" | "not_trashed" | "core_error"
  | "bad_request" | "unsupported" | "cloud_synced_readonly"
  // Flere mapper: overlapp (detail = forklaring), bilder i «Nylig slettet» (detail = antall),
  // første mappe kan ikke fjernes, mappen har en Fjell-karantene fra et annet bibliotek (detail = sti).
  | "root_overlap" | "root_has_trash" | "root_is_primary" | "root_has_quarantine" | "not_a_folder"
  | ThumbErrorCode;

export interface BridgeError {
  code: BridgeErrorCode | string;
  detail: string;
}

/** Ett bilde i «Nylig slettet». Tider i sekunder siden epoch. `name` er bare filnavnet. Speiler TrashedDto. */
export interface TrashedItem {
  asset: number;
  name: string;
  bytes: number;
  trashedAt: number;
  expiresAt: number;
  expired: boolean;
}

/** Resultat av «Gjenopprett». Bare bilder som lå i «Nylig slettet» telles. Speiler RestoreDto. */
export interface RestoreResult {
  restored: number;
}

/** Innstillinger skallet viser. `quarantineDir` er null til en mappe er valgt. `emptyDefault` er alltid «recycle». Speiler SettingsDto. */
export interface AppSettings {
  trashDays: number;
  quarantineDir: string | null;
  emptyDefault: "recycle";
}

// ---- Beta-kanal: oppdatering (M9). Egen blokk nederst for enkel sammenslåing. ----

/** Svar fra oppdateringssjekken. `version`, `notes` og `date` finnes bare når `available` er true. `date` er ISO 8601. */
export interface UpdateCheck {
  available: boolean;
  version?: string;
  notes?: string;
  date?: string;
}

// ---- Miniatyrer (M12). Egen blokk nederst for enkel sammenslåing. ----

/** Feilkoder fra `get_thumbnail` (speiler ErrorDto.code). `unavailable` = ingen kodek / ikke lokal fil. Skyplassholdere leses aldri. */
export type ThumbErrorCode = "unavailable" | "cloud_placeholder" | "corrupt" | "not_found" | "no_library";
export const THUMB_ERROR_CODES: readonly ThumbErrorCode[] = ["unavailable", "cloud_placeholder", "corrupt", "not_found", "no_library"];

/** Miniatyr klar til visning. `url` er en `data:`-URL (CSP: img-src 'self' data:). Bildedata forlater aldri maskinen. */
export interface Thumbnail {
  url: string;
  width: number;
  height: number;
}

/** Svar fra skallet (camelCase). `data` er base64 uten `data:`-prefiks. Speiler ThumbDto. */
export interface ThumbDto {
  mime: string;
  data: string;
  width: number;
  height: number;
}
