<!-- Trygg modus (plassholder i kontraktfasen; grenen «backup» eier filen, docs/v1/ROBUSTHET-KONTRAKT.md): vises av App.svelte i stedet for
     skjermene når indeksen ikke er hel (integritet ≠ ok). Ingen skanning og ingen endringer før brukeren har gjenopprettet en kopi
     (med bekreftelse, kan angres) og trykket «Sjekk på nytt». Angre-banneret blir stående til brukeren selv går videre; kopien tatt rett før
     gjenopprettingen ligger i listen i Innstillinger. Bildene er ikke berørt. `onrecheck` ber App sjekke integriteten på nytt. -->
<script lang="ts">
  import type { CoreBridge } from "../bridge/core-bridge";
  import type { IntegrityCode } from "../bridge/types-robust";
  import { useBridge } from "../context";
  import { Banner, Button, PageHeader } from "../lib";
  import { tr } from "../lib/i18n";
  import SettingsBackup from "./SettingsBackup.svelte";

  interface Props {
    bridge?: CoreBridge;
    /** Gjeldende integritet (kode), til beskrivelsen. */
    integrity: IntegrityCode;
    /** Be App sjekke integriteten på nytt (knappen). Er indeksen hel, forlater App trygg modus. */
    onrecheck: () => void;
  }
  let { bridge, integrity, onrecheck }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const integrityKey = {
    ok: "backup.integrity.ok", degraded: "backup.integrity.degraded", corrupt: "backup.integrity.corrupt", unreadable: "backup.integrity.unreadable",
  } as const;
</script>

<section class="recovery">
  <PageHeader title={$tr("backup.recovery.title")} />
  <Banner tone="warn" role="alert">
    <p>{$tr("backup.recovery.banner")} {$tr(integrityKey[integrity])}</p>
  </Banner>
  <p>{$tr("backup.recovery.description")}</p>
  <p>{$tr("backup.recovery.pick")}</p>
  <SettingsBackup bridge={b} />
  <div><Button onclick={onrecheck}>{$tr("backup.recovery.recheck")}</Button></div>
</section>

<style>
  .recovery { display: grid; gap: var(--space-4); max-inline-size: var(--w-narrow, 40rem); }
  .recovery p { margin: 0; }
</style>
