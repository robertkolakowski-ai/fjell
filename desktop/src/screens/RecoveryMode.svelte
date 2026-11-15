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
  import { robustErrorKey } from "./robustErrors";
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
  let ack = $state(false);
  let busy = $state(false);
  let failed = $state<string | null>(null);
  async function rebuild() {
    if (!ack || busy) return;
    busy = true; failed = null;
    try { await b.rebuildIndex(); ack = false; onrecheck(); }
    catch (e) { failed = (e as { code?: string })?.code ?? "err"; }
    finally { busy = false; }
  }
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
  <section class="rebuild" aria-labelledby="rb-t">
    <h2 id="rb-t">{$tr("backup.rebuild.title")}</h2>
    <p>{$tr("backup.rebuild.body")}</p>
    <label class="opt"><input type="checkbox" checked={ack} disabled={busy} onchange={(e) => (ack = e.currentTarget.checked)} /><span>{$tr("backup.rebuild.check")}</span></label>
    <div><Button disabled={busy || !ack} onclick={() => void rebuild()}>{$tr("backup.rebuild.button")}</Button></div>
    {#if failed}<Banner tone="danger" role="alert"><p>{$tr(robustErrorKey(failed))}</p></Banner>{/if}
  </section>
  <div><Button onclick={onrecheck}>{$tr("backup.recovery.recheck")}</Button></div>
</section>

<style>
  .recovery { display: grid; gap: var(--space-4); max-inline-size: var(--w-narrow, 40rem); }
  .recovery p { margin: 0; }
  .rebuild { display: grid; gap: var(--space-3); }
  .rebuild h2 { margin: 0; font-size: var(--fs-lg, 1.125rem); }
  .opt { display: flex; gap: var(--space-2); align-items: flex-start; }
</style>
