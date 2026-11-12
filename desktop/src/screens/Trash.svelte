<!-- Papirkurv (UX-5, PLAN-UX-I18N 3.6): «Nylig slettet» med forklaring, utløpsvarsel, liste per bilde (Utløpt / Utløper snart /
     Andre) med «Gjenopprett» og «Gjenopprett alt» (umiddelbart, uten bekreftelse), Tøm-ark (Windows Papirkurv som standard,
     permanent med eget trinn og avkrysning; filer Papirkurv ikke kan ta ligger urørt i karantenen til brukeren velger), resultat og historikk. Tilstand i trashState.ts, flyt i trashFlow.ts.
     Ikke med ennå: miniatyrer i listen (raden viser filnavn, størrelse og datoer). -->
<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { useBridge } from "../context";
  import { Button, LiveRegion } from "../lib";
  import { fmt, tr } from "../lib/i18n";
  import { TrashSession } from "./trashFlow";
  import { daysLeft, hasExpired, initialTrash, sectionsOf, type PurgeOutcome, type TrashSectionId, type TrashState } from "./trashState";

  interface Props {
    bridge?: CoreBridge;
  }
  let { bridge }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const session = new TrashSession(b, (s) => (view = s));

  let view: TrashState = $state(initialTrash);
  let sheet: HTMLDialogElement | undefined = $state();

  const sheetOpen = $derived(view.phase === "confirmPurge" || view.phase === "confirmUnsupported" || view.phase === "confirmPermanent" || view.phase === "purging");
  const info = $derived(view.info);
  const nowS = Math.floor(Date.now() / 1000);
  const sections = $derived(sectionsOf(view.items, nowS));
  const sectionKey = { expired: "trash.section.expired", soon: "trash.section.soon", other: "trash.section.other" } as const satisfies Record<TrashSectionId, string>;
  const canRestore = $derived(view.phase === "list" && !view.restoring);
  const outcome = $derived(view.outcome);
  /** Hovedlinjen i resultatet. Tom når ingenting ble flyttet eller slettet (bare «ligger urørt»). */
  const outcomeText = (o: PurgeOutcome): string =>
    o.mode === "permanent" ? $tr("trash.purged.permanent", { size: fmt.bytes(o.freedBytes) })
    : o.count > 0 ? $tr("trash.purged.recycle") : "";
  const announce = $derived(
    view.restored !== null ? $tr("restore.done", { count: view.restored })
    : view.restoreFailed ? $tr("restore.failed")
    : view.phase === "purged" && outcome
      ? [outcomeText(outcome), outcome.kept > 0 ? $tr("trash.purged.kept", { count: outcome.kept }) : ""].filter(Boolean).join(" ")
      : "",
  );

  // Arket styres av tilstand; Esc avbryter (ikke mens tømming pågår).
  $effect(() => {
    if (!sheet) return;
    if (sheetOpen && !sheet.open) sheet.showModal();
    else if (!sheetOpen && sheet.open) sheet.close();
  });
  // Innholdet i arket byttes mens det er åpent (knappen med fokus fjernes): sett fokus på første valg i det nye trinnet.
  // «La ligge i karantenen» er standard og først; i permanent-trinnet er det avkrysningen.
  $effect(() => {
    const p = view.phase;
    if (p !== "confirmUnsupported" && p !== "confirmPermanent") return;
    void tick().then(() => {
      sheet?.querySelector<HTMLElement>(p === "confirmPermanent" ? ".ack input" : ".dlg-actions button")?.focus();
    });
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
        {#if outcomeText(outcome)}<p>{outcomeText(outcome)}</p>{/if}
        {#if outcome.kept > 0}<p>{$tr("trash.purged.kept", { count: outcome.kept })}</p>{/if}
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
        <Button disabled={!canRestore || view.selected.size === 0} onclick={() => void session.restore(false)}>{$tr("trash.restore")}</Button>
        <Button disabled={!canRestore} onclick={() => void session.restore(true)}>{$tr("trash.restore.all")}</Button>
        <Button variant="primary" disabled={view.phase !== "list"} onclick={() => session.send({ type: "openPurge" })}>{$tr("trash.empty.button")}</Button>
      </div>
      {#if view.restored !== null}
        <p class="muted" role="status">{$tr("restore.done", { count: view.restored })} {$tr("restore.where")}</p>
      {:else if view.restoreFailed}
        <p role="alert">{$tr("restore.failed")}</p>
      {:else}
        <p class="muted">{view.selected.size === 0 ? $tr("restore.none") : $tr("restore.selected", { count: view.selected.size })}</p>
      {/if}

      <div class="listbar">
        <Button disabled={!canRestore} onclick={() => session.send({ type: "selectAll" })}>{$tr("review.select.all")}</Button>
      </div>
      <div role="group" aria-label={$tr("restore.list.label")}>
        {#each sections as sec (sec.id)}
          <h2 class="sec">{$tr(sectionKey[sec.id])}</h2>
          <ul class="items">
            {#each sec.items as it (it.asset)}
              <li>
                <label class="row">
                  <input type="checkbox" checked={view.selected.has(it.asset)} disabled={!canRestore}
                    aria-label={$tr("restore.row.select", { name: it.name })}
                    onchange={() => session.send({ type: "toggle", asset: it.asset })} />
                  <span class="name">{it.name}</span>
                  <span class="meta">{fmt.bytes(it.bytes)} · {$tr("trash.row.moved", { date: fmt.date(it.trashedAt * 1000) })}</span>
                  <span class="meta">{#if it.expired}<strong>{$tr("trash.row.expired")}</strong>{:else}{$tr("trash.row.expires", { days: daysLeft(it, nowS) })}{/if}</span>
                </label>
              </li>
            {/each}
          </ul>
        {/each}
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
      {:else if view.phase === "confirmUnsupported"}
        <!-- Filer Papirkurv ikke kan ta ligger urørt i karantenen. «La ligge» er standard og første i fokusrekkefølgen;
             den destruktive knappen er sist og fører til eget trinn med avkrysning (PermanentAck). -->
        <h2 id="tr-t">{$tr("trash.unsupported.title")}</h2>
        <p role="alert">{$tr("trash.unsupported.body", { count: view.unsupported })}</p>
        <p>{$tr("trash.unsupported.options")}</p>
        <div class="dlg-actions">
          <Button variant="primary" onclick={() => session.send({ type: "keepUnsupported" })}>{$tr("trash.unsupported.keep")}</Button>
          <Button onclick={() => session.send({ type: "deleteUnsupported" })}>{$tr("trash.unsupported.delete")}</Button>
        </div>
      {:else}
        <h2 id="tr-t">{$tr("trash.purge.title")}</h2>
        <p>{$tr("trash.purge.summary", { count: info.count, size: fmt.bytes(info.bytes) })}</p>
        <fieldset disabled={view.phase === "purging"}>
          <legend class="sr-only">{$tr("trash.purge.title")}</legend>
          <label><input type="radio" name="mode" checked={view.mode === "recycle"} onchange={() => session.send({ type: "setMode", mode: "recycle" })} /> {$tr("trash.purge.mode.recycle")}</label>
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
  .listbar { margin-block: var(--space-3); }
  .sec { margin-block: var(--space-4) var(--space-2); font-size: var(--fs-md); }
  .items { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--space-2); }
  .row { flex-wrap: wrap; gap: var(--space-1) var(--space-3); padding: var(--space-2) var(--space-3); border: var(--border-width) solid var(--c-border); border-radius: var(--radius-md); background: var(--c-surface); }
  .name { flex: 1 1 8rem; font-weight: var(--fw-semibold); overflow-wrap: anywhere; }
  .meta { color: var(--c-text-muted); }
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
