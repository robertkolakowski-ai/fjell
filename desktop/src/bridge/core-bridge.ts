// CoreBridge: felles grensesnitt for Tauri og mock. All UI går via dette (ingen direkte invoke).
import type {
  AppSettings, AppState, CardPage, Decision, EmptyResult, KindCode, LibraryRoot, PermanentAck, RestoreResult, Thumbnail, TrashedItem, UpdateCheck,
} from "./types";

export interface CoreBridge {
  getState(): Promise<AppState>;
  startScan(full: boolean): Promise<void>;
  listCards(kind: KindCode, limit: number, cursor?: number | null): Promise<CardPage>;
  /** Papirkurv/behold. Returnerer batch-id (ett angre). Aldri hard sletting. */
  decide(ids: number[], decision: Decision): Promise<number>;
  /** Bytter keeper i en gruppe (K2). `asset` må være gruppens keeper eller et av gruppens forslag. Ingen filer røres. */
  setKeeper(group: number, asset: number): Promise<void>;
  /** Angrer siste batch. Returnerer batch-id. */
  undo(): Promise<number>;
  /** Tømmer «Nylig slettet» via Papirkurv. Returnerer antall tømt. */
  purge(): Promise<number>;
  /** «Tøm» med valg: Windows Papirkurv (standard) eller permanent (krever `PermanentAck`: avkrysning + riktig antall). */
  emptyTrash(permanent?: PermanentAck | null): Promise<EmptyResult>;
  /** Alle bilder i «Nylig slettet», eldste først. */
  listTrashed(): Promise<TrashedItem[]>;
  /** Gjenoppretter valgte bilder fra «Nylig slettet». Umiddelbart, uten bekreftelse. Filer flyttes tilbake fra karantenen. */
  restore(assets: number[]): Promise<RestoreResult>;
  /** Gjenoppretter alt i «Nylig slettet». */
  restoreAll(): Promise<RestoreResult>;
  /** Pause: avbryter skannet og husker det. Det som er funnet beholdes. */
  pauseScan(): Promise<void>;
  /** Fortsett etter pause (ny skanning; ferdig analyserte bilder hoppes over). */
  resumeScan(): Promise<void>;
  /** Avbryt skann. Det som er funnet beholdes. */
  cancelScan(): Promise<void>;
  /** Valgt biblioteksmappe, eller null. */
  getLibrary(): Promise<string | null>;
  /** Finnes et system-mappevalg («Bla …»)? Bare i Tauri (eller mock med testalternativ). Ellers skjules/deaktiveres knappen med forklaring. */
  readonly canPickFolder: boolean;
  /** Åpner systemets mappevalg. Returnerer valgt sti, eller null hvis brukeren avbrøt. Setter ikke biblioteket: kall `setLibrary` etter bekreftelse. Avvises med `unsupported` uten mappevalg. */
  pickFolder(title?: string): Promise<string | null>;
  /** Velger mappe (sti). Kjernen skanner én mappe om gangen: en ny mappe erstatter den forrige. */
  setLibrary(path: string): Promise<void>;

  // ---- Flere mapper i ett bibliotek (docs/mvp/FLERE-MAPPER.md). UI kommer i bølge 2. ----
  /** Bibliotekets mapper med tall per mappe (antall, størrelse, video, «Nylig slettet», tilkoblet). */
  listLibraryRoots(): Promise<LibraryRoot[]>;
  /** Legger til en mappe. Avvises med `not_a_folder`, `root_overlap` (ligger i/rundt en annen mappe) eller `root_has_quarantine`. Bildene kommer ved neste skanning. Ingen filer røres. */
  addLibraryRoot(path: string): Promise<LibraryRoot[]>;
  /** Tar ut en mappe (`key` fra `LibraryRoot`). Avvises med `root_is_primary` eller `root_has_trash` (gjenopprett eller tøm først). Ingen filer røres. */
  removeLibraryRoot(key: number): Promise<LibraryRoot[]>;
  /** Første gang appen kjøres: onboarding skal vises. */
  getFirstRun(): Promise<boolean>;
  /** Onboarding er gjennomført. Vises ikke på nytt. */
  completeOnboarding(): Promise<void>;
  getSettings(): Promise<AppSettings>;
  /** Papirkurvfrist i dager (1 til 365). Gjelder nye elementer. */
  setTrashDays(days: number): Promise<AppSettings>;

  // ---- Beta-kanal: oppdatering (M9) ----
  /** Installert appversjon. «dev» utenfor den installerte appen. */
  getAppVersion(): Promise<string>;
  /** Spør oppdateringstjenesten (sender bare appversjon og operativsystem). Kalles bare når brukeren ber om det, eller ved oppstart når auto er på. */
  checkForUpdate(): Promise<UpdateCheck>;
  /** Laster ned, installerer og starter appen på nytt. `total` er null når størrelsen er ukjent. Kalles aldri uten at brukeren har trykket. */
  installUpdate(onProgress: (done: number, total: number | null) => void): Promise<void>;
  /** «Sjekk automatisk ved oppstart». Standard av. */
  getAutoUpdate(): Promise<boolean>;
  setAutoUpdate(on: boolean): Promise<boolean>;
  /** Lagret språkvalg i skallet («system», «nb», «en»), eller null = aldri valgt (frontend migrerer sitt eget). */
  getLanguage(): Promise<string | null>;
  /** Lagrer språkvalget i skallet. Returnerer lagret verdi. «pseudo» avvises av skallet. */
  setLanguage(language: string): Promise<string>;

  // ---- Miniatyrer (M12) ----
  /** Miniatyr (256 px lengste side) for et bilde. `id` er `asset` på kort og Papirkurv-rader (AssetId). Avvises med en `ThumbErrorCode`. Cache, kø og avbryting ligger i `thumbs.ts`, ikke her. */
  getThumbnail(id: number): Promise<Thumbnail>;
}
