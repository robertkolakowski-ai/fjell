// CoreBridge: felles grensesnitt for Tauri og mock. All UI går via dette (ingen direkte invoke).
import type {
  AppSettings, AppState, CardPage, Decision, EmptyResult, KindCode, PermanentAck, RestoreResult, TrashedItem,
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
  /** Første gang appen kjøres: onboarding skal vises. */
  getFirstRun(): Promise<boolean>;
  /** Onboarding er gjennomført. Vises ikke på nytt. */
  completeOnboarding(): Promise<void>;
  getSettings(): Promise<AppSettings>;
  /** Papirkurvfrist i dager (1 til 365). Gjelder nye elementer. */
  setTrashDays(days: number): Promise<AppSettings>;
  /** Lagret språkvalg i skallet («system», «nb», «en»), eller null = aldri valgt (frontend migrerer sitt eget). */
  getLanguage(): Promise<string | null>;
  /** Lagrer språkvalget i skallet. Returnerer lagret verdi. «pseudo» avvises av skallet. */
  setLanguage(language: string): Promise<string>;
}
