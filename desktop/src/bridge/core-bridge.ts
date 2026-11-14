// CoreBridge: felles grensesnitt for Tauri og mock. All UI går via dette (ingen direkte invoke).
import type {
  AppSettings, AppState, CardPage, Decision, EmptyResult, KindCode, LibraryRoot, PermanentAck, RestoreResult, Thumbnail, TrashedItem, UpdateCheck,
} from "./types";
import type {
  Condition, Dashboard, Person, Protection, Rule, RuleDraft, RulePreview, Scope, SeriesCompare, SuggestionSettings, Thresholds,
} from "./types-v1";

export interface CoreBridge {
  getState(): Promise<AppState>;
  startScan(full: boolean): Promise<void>;
  listCards(kind: KindCode, limit: number, cursor?: number | null): Promise<CardPage>;
  /**
   * Papirkurv/behold. Returnerer batch-id (ett angre). Aldri hard sletting.
   * `confirmed`: konsekvenskoder brukeren har bekreftet etter en `ack_required` (f.eks. `protected_by_scope`).
   */
  decide(ids: number[], decision: Decision, confirmed?: string[]): Promise<number>;
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
  /** Reservevei når installasjonen feiler: adressen til siste utgivelse. Vises som tekst og kan kopieres; appen åpner den aldri selv. */
  getUpdateDownloadUrl(): Promise<string>;
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

  // ---- V1: beskyttelse, regler, personer, oversikt, serier og forslagsinnstillinger (docs/v1/V1-PLAN.md) ----
  // Sikkerhet: ingenting her sletter bilder. `protect(.., false)`, `deleteRule` og `mergePeople` kan angres
  // (`protect(.., true)`, `restoreRule`, `unmergePerson`). `wipeFaceIndex` fjerner bare avledede ansiktsdata og må
  // bekreftes av brukeren først. Feil er koder (`BridgeError`), aldri tekst.

  /** Slår beskyttelse av (`on`) eller fra for bilde, mappe, album eller person. Kan angres med motsatt kall. */
  protect(scope: Scope, on: boolean): Promise<void>;
  listProtections(): Promise<Protection[]>;

  /** Forslagsregler. De foreslår bare; ingenting slettes eller flyttes av dem. */
  listRules(): Promise<Rule[]>;
  /** Oppretter (`draft.id` null) eller endrer. Returnerer regelens id. Avvises med `empty_rule`, `empty_name`, `name_too_long`, `invalid_condition`, `rule_not_found`. */
  saveRule(draft: RuleDraft): Promise<number>;
  previewRule(conditions: Condition[]): Promise<RulePreview>;
  toggleRule(id: number, enabled: boolean): Promise<void>;
  /** Mykt: regelen skjules og kan hentes tilbake med `restoreRule`. */
  deleteRule(id: number): Promise<void>;
  restoreRule(id: number): Promise<void>;

  /** Personer (tom liste når ansiktsindeksen er av). */
  listPeople(): Promise<Person[]>;
  /** `null` fjerner navnet. Navnet er brukerens eget og blir på enheten. */
  namePerson(id: number, name: string | null): Promise<void>;
  /** Slår `absorbed` inn i `target`. Returnerer personen som ble stående. Angres med `unmergePerson(absorbed)`. */
  mergePeople(target: number, absorbed: number): Promise<number>;
  unmergePerson(absorbed: number): Promise<void>;
  protectPerson(id: number, protect: boolean): Promise<void>;
  /** Ansiktsindeksen er av som standard. */
  getFaceIndex(): Promise<boolean>;
  setFaceIndex(enabled: boolean): Promise<void>;
  /** Sletter ansiktsdata og personer (ikke bilder). Returnerer antall ansikter slettet. Kan ikke angres: bekreft først. */
  wipeFaceIndex(): Promise<number>;

  getDashboard(): Promise<Dashboard>;
  /** Medlemmene i en serie med forslått behold-kandidat først. Avvises med `unknown_series`. */
  compareSeries(series: number): Promise<SeriesCompare>;
  getSuggestionSettings(): Promise<SuggestionSettings>;
  setSuggestionSettings(settings: SuggestionSettings): Promise<SuggestionSettings>;
  getThresholds(): Promise<Thresholds>;
}
