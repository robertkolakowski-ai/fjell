<!-- Papirkurv (UX-5, PLAN-UX-I18N 3.6): «Nylig slettet» med forklaring, utløpsvarsel, Tøm-ark (Windows Papirkurv som standard,
     permanent med eget trinn og avkrysning), resultat og historikk. Tilstand i trashState.ts, flyt i trashFlow.ts.
     Ikke med ennå: liste per bilde og «Gjenopprett» (CoreBridge har ingen listing av karantenen eller restore). -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { useBridge } from "../context";
  import { Button, LiveRegion } from "../lib";
  import { fmt, tr } from "../lib/i18n";
  import { TrashSession } from "./trashFlow";
  import { hasExpired, initialTrash, type TrashState } from "./trashState";

  interface Props {
    bridge?: CoreBridge;
  }
  let { bridge }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const session = new TrashSession(b, (s) => (view = s));

  let view: TrashState = $state(initialTrash);
  let sheet: HTMLDialogElement | undefined = $state();

  const sheetOpen = $derived(view.phase === "confirmPurge" || view.phase === "confirmPermanent" || view.phase === "purging");
  const info = $derived(view.info);
  const outcome = $derived(view.outcome);
  const announce = $derived(
    view.phase === "purged" && outcome
      ? outcome.mode === "permanent" ? $tr("trash.purged.permanent", { size: fmt.bytes(outcome.freedBytes) }) : $tr("trash.purged.recycle")
      : "",
  );

  // Arket styres av tilstand; Esc avbryter (ikke mens tømming pågår).
  $effect(() => {
    if (!sheet) return;
    if (sheetOpen && !sheet.open) sheet.showModal();
    else if (!sheetOpen && sheet.open) sheet.close();
  });
  onMount(() => void session.load());
  onDestroy(() => session.dispose());
</script>

<section class="trash">
  <h1 tabindex="-1">{$tr("trash.title")}</h1>
  <LiveRegion message={announce} />

  {#if view.phase === "loading"}
    <p role="status">{$tr("common.loading")}</p>
  {:else if view.phase === "error" && !info}
    <p role="alert">{$tr("trash.error.body")}</p>
    <Button onclick={() => { session.send({ type: "reload" }); void session.load(); }}>{$tr("common.retry")}</Button>
  {:else if info}
    {#if view.error}<p role="alert">{$tr("trash.error.body")}</p>{/if}

    {#if view.phase === "purged" && outcome}
      <div class="done" role="status">
        <p>{outcome.mode === "permanent" ? $tr("trash.purged.permanent", { size: fmt.bytes(outcome.freedBytes) }) : $tr("trash.purged.recycle")}</p>
        {#if outcome.marked > 0}<p>{$tr("trash.purge.marked", { count: outcome.marked })}</p>{/if}
        <Button onclick={() => session.send({ type: "dismiss" })}>{$tr("common.done")}</Button>
      </div>
    {/if}

    {#if info.count === 0}
      <h2>{$tr("trash.empty.title")}</h2>
    {:else}
      <p class="summary">{$tr("trash.summary", { count: info.count, size: fmt.bytes(info.bytes) })}</p>
      <p class="muted">{$tr("trash.freed.note")}</p>
      {#if hasExpired(info)}
        <p class="expired"><strong>{$tr("trash.row.expired")}</strong>: {fmt.number(info.expired)}</p>
      {/if}
      <div class="actions">
        <Button variant="primary" disabled={view.phase !== "list"} onclick={() => session.send({ type: "openPurge" })}>{$tr("trash.empty.button")}</Button>
      </div>
    {/if}

    {#if info.freedBytes > 0 && info.freedSince}
      <p class="muted history">{$tr("trash.history", { size: fmt.bytes(info.freedBytes), date: fmt.date(info.freedSince) })}</p>
    {/if}
  {/if}

  <dialog bind:this={sheet} aria-labelledby="tr-t" oncancel={(e) => { if (view.phase === "purging") e.preventDefault(); }}
    onclose={() => session.send({ type: "cancel" })}>
    {#if info}
      {#if view.phase === "confirmPermanent"}
        <h2 id="tr-t">{$tr("trash.permanent.title")}</h2>
        <p>{$tr("trash.purge.summary", { count: info.count, size: fmt.bytes(info.bytes) })}</p>
        <label class="ack">
          <input type="checkbox" checked={view.ack} onchange={(e) => session.send({ type: "setAck", value: e.currentTarget.checked })} />
          <span>{$tr("trash.permanent.ack")}</span>
        </label>
        <!-- Destruktiv knapp sist i fokusrekkefølgen. -->
        <div class="dlg-actions">
          <Button onclick={() => session.send({ type: "cancel" })}>{$tr("common.cancel")}</Button>
          <Button disabled={!view.ack} onclick={() => void session.confirm()}>{$tr("trash.permanent.go", { count: info.count })}</Button>
        </div>
      {:else}
        <h2 id="tr-t">{$tr("trash.purge.title")}</h2>
        <p>{$tr("trash.purge.summary", { count: info.count, size: fmt.bytes(info.bytes) })}</p>
        {#if view.noBin}<p role="alert">{$tr("trash.purge.nobin")}</p>{/if}
        <fieldset disabled={view.phase === "purging"}>
          <legend class="sr-only">{$tr("trash.purge.title")}</legend>
          {#if !view.noBin}
            <label><input type="radio" name="mode" checked={view.mode === "recycle"} onchange={() => session.send({ type: "setMode", mode: "recycle" })} /> {$tr("trash.purge.mode.recycle")}</label>
          {/if}
          <label><input type="radio" name="mode" checked={view.mode === "permanent"} onchange={() => session.send({ type: "setMode", mode: "permanent" })} /> {$tr("trash.purge.mode.permanent")}</label>
        </fieldset>
        <div class="dlg-actions">
          <Button disabled={view.phase === "purging"} onclick={() => session.send({ type: "cancel" })}>{$tr("common.cancel")}</Button>
          <Button variant="primary" disabled={view.phase === "purging"} onclick={() => void session.confirm()}>{$tr("common.continue")}</Button>
        </div>
        {#if view.phase === "purging"}<p role="status">{$tr("common.loading")}</p>{/if}
      {/if}
    {/if}
  </dialog>
</section>

<style>
  .trash { max-inline-size: 40rem; }
  h1 { margin-block: 0 var(--space-3); font-size: var(--fs-xl); line-height: var(--lh-tight); }
  h1:focus { outline: none; }
  h2 { font-size: var(--fs-lg); }
  .muted { color: var(--c-text-muted); }
  .expired { padding: var(--space-3); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-md); background: var(--c-badge-warn-bg); color: var(--c-badge-warn-text); }
  .actions, .dlg-actions { display: flex; flex-wrap: wrap; gap: var(--space-3); }
  .dlg-actions { justify-content: flex-end; margin-block-start: var(--space-3); }
  .done { margin-block: var(--space-3); padding: var(--space-3); border: var(--border-width) solid var(--c-border); border-radius: var(--radius-md); background: var(--c-surface); }
  .history { margin-block-start: var(--space-5); }
  dialog { max-inline-size: min(32rem, calc(100vw - 2rem)); padding: var(--space-5); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-lg); background: var(--c-surface); color: var(--c-text); }
  dialog::backdrop { background: var(--c-scrim); }
  dialog h2 { margin-block-start: 0; }
  fieldset { display: grid; gap: var(--space-2); margin: 0; padding: 0; border: 0; }
  label { display: flex; align-items: center; gap: var(--space-2); min-block-size: var(--target-min); }
  .ack { align-items: flex-start; }
  input[type="checkbox"], input[type="radio"] { inline-size: 1.25rem; block-size: 1.25rem; flex: none; }
</style>
