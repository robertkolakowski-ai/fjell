<!-- Innstillinger: sikkerhetskopi av indeksen (plassholder i kontraktfasen; grenen «backup» eier filen, docs/v1/ROBUSTHET-KONTRAKT.md).
     Kopien inneholder bare Fjells egen indeks, aldri bilder. Gjenoppretting går via en dialog med avkrysning («Avbryt» er først) og
     kan angres rett etterpå. Også brukt i trygg modus (RecoveryMode). Tilstand og flyt i backupFlow.ts. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import type { BackupKind } from "../bridge/types-robust";
  import { useBridge } from "../context";
  import { Banner, Button, Card, LiveRegion } from "../lib";
  import { fmt, tr, type MessageKey } from "../lib/i18n";
  import { BackupSession, canUndo, initialBackup, type BackupState } from "./backupFlow";
  import { robustErrorKey } from "./robustErrors";
  import "./sheet.css";

  interface Props {
    bridge?: CoreBridge;
    /** Kalles etter at indeksen kan ha endret seg (gjenoppretting, angre, sjekk). Trygg modus bruker den til å sjekke integriteten på nytt. */
    onchange?: () => void;
  }
  let { bridge, onchange }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const session = new BackupSession(b, (s) => (view = s), () => onchange?.());

  let view: BackupState = $state(initialBackup);
  let sheet: HTMLDialogElement | undefined = $state();

  const kindKey = {
    manual: "backup.kind.manual", auto: "backup.kind.auto", pre_restore: "backup.kind.pre_restore",
  } as const satisfies Record<BackupKind, MessageKey>;
  const integrityKey = {
    ok: "backup.integrity.ok", degraded: "backup.integrity.degraded", corrupt: "backup.integrity.corrupt", unreadable: "backup.integrity.unreadable",
  } as const;
  const announce = $derived(view.restored ? $tr("backup.restored") : view.created ? $tr("backup.created") : "");

  $effect(() => {
    if (!sheet) return;
    if (view.confirmId !== null && !sheet.open) sheet.showModal();
    else if (view.confirmId === null && sheet.open) sheet.close();
  });

  onMount(() => void session.load());
  onDestroy(() => session.dispose());
</script>

<Card heading={$tr("backup.title")}>
  <div class="body">
    <p class="muted">{$tr("backup.description")}</p>
    <p class="muted">{$tr("backup.privacy")}</p>
    <p class="muted">{$tr("backup.newpc")}</p>

    {#if view.phase === "loading"}
      <p role="status">{$tr("common.loading")}</p>
    {:else if view.phase === "error"}
      <Banner tone="danger" role="alert">
        <p>{$tr("backup.load.error")}</p>
        {#snippet actions()}<Button onclick={() => void session.load()}>{$tr("common.retry")}</Button>{/snippet}
      </Banner>
    {:else}
      <p role="status" class="integrity">{$tr(integrityKey[view.integrity])}</p>
      <div class="row">
        <Button variant="primary" disabled={view.busy} onclick={() => void session.create()}>{$tr("backup.create")}</Button>
        <Button disabled={view.busy} onclick={() => void session.check()}>{$tr("backup.integrity.check")}</Button>
      </div>

      {#if view.backups.length === 0}
        <p class="muted">{$tr("backup.empty")}</p>
      {:else}
        <ul class="list" aria-label={$tr("backup.list.label")}>
          {#each view.backups as bk (bk.id)}
            <li>
              <span>{$tr("backup.entry", { kind: $tr(kindKey[bk.kind]), date: fmt.date(bk.createdAt * 1000), count: bk.assetCount })}</span>
              <span class="muted">{fmt.bytes(bk.bytes)}</span>
              <Button size="sm" disabled={view.busy || bk.integrity !== "ok"} ariaLabel={$tr("backup.restore.for", { kind: $tr(kindKey[bk.kind]), date: fmt.date(bk.createdAt * 1000) })} onclick={() => session.requestRestore(bk.id)}>{$tr("backup.restore")}</Button>
            </li>
          {/each}
        </ul>
      {/if}

      {#if view.restored}
        <Banner tone="ok" role="status">
          <p>{$tr("backup.restored")}{#if !canUndo(view.restored)} {$tr("backup.restored.noundo")}{/if}</p>
          {#snippet actions()}{#if canUndo(view.restored)}<Button disabled={view.busy} onclick={() => void session.undoRestore()}>{$tr("common.undo")}</Button>{/if}{/snippet}
        </Banner>
      {/if}
      {#if view.error}
        <Banner tone="danger" role="alert"><p>{$tr(robustErrorKey(view.error))}</p></Banner>
      {/if}
    {/if}
  </div>
</Card>
<LiveRegion message={announce} />

<!-- Bekreftelsen: «Avbryt» er først. «Gjenopprett» virker bare med avkrysning (eneste vei til restoreBackup er session.confirmRestore). -->
<dialog class="sheet" bind:this={sheet} aria-labelledby="bk-t" onclose={() => session.cancelRestore()}
  oncancel={(e) => { if (view.busy) e.preventDefault(); }}>
  <div class="sheet-body">
    <h2 id="bk-t">{$tr("backup.restore.title")}</h2>
    <p>{$tr("backup.restore.body")}</p>
    <label class="opt"><input type="checkbox" checked={view.ack} onchange={(e) => session.setAck(e.currentTarget.checked)} /><span>{$tr("backup.restore.check")}</span></label>
  </div>
  <div class="sheet-actions">
    <Button disabled={view.busy} onclick={() => session.cancelRestore()}>{$tr("common.cancel")}</Button>
    <Button variant="primary" disabled={view.busy || !view.ack} onclick={() => void session.confirmRestore()}>{$tr("backup.restore")}</Button>
  </div>
</dialog>

<style>
  .body { display: grid; gap: var(--space-4); }
  .body p { margin: 0; }
  .row { display: flex; flex-wrap: wrap; gap: var(--space-3); }
  .list { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--space-2); }
  .list li { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-3); justify-content: space-between; }
  .muted { color: var(--c-text-muted); font-size: var(--fs-sm); }
</style>
