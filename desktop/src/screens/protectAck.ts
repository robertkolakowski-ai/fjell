// Beskyttede bilder (konsekvenskode `protected_by_scope`): kjernen svarer `ack_required` til brukeren har bekreftet.
// Delt av Gjennomgang, Kortmodus og Serie. Bekreftelsen gjelder ett kall og kan angres som alle andre valg.
import type { BridgeError } from "../bridge/types";

export const PROTECTED_ACK = "protected_by_scope";

/** Kjernen ber om bekreftelse fordi minst ett av bildene er beskyttet av brukeren. */
export const isProtectedAck = (e: BridgeError | null | undefined): boolean =>
  !!e && e.code === "ack_required" && e.detail === PROTECTED_ACK;
