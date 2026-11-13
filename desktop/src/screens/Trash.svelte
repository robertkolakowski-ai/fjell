<!-- Papirkurv (UX-5, PLAN-UX-I18N 3.6): «Nylig slettet» med forklaring, utløpsvarsel, liste per bilde (Utløpt / Utløper snart /
     Andre) med «Gjenopprett» og «Gjenopprett alt» (umiddelbart, uten bekreftelse), Tøm-ark (Windows Papirkurv som standard,
     permanent med eget trinn og avkrysning; filer Papirkurv ikke kan ta ligger urørt i karantenen til brukeren velger), resultat og historikk. Tilstand i trashState.ts, flyt i trashFlow.ts.
     Loft (M10): sammendrag som StatTile-rad, tett liste i Card, dialoger i felles ark-stil (sheet.css).
     Ikke med ennå: ekte miniatyrer (bridge har ingen), raden har en nøytral plassholder. -->
<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { useBridge } from "../context";
  import { Banner, Button, Card, Icon, LiveRegion, PageHeader, StatTile } from "../lib";
  import "./sheet.css";
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
  const soonCount = $derived(sections.find((x) => x.id === "soon")?.items.length ?? 0);
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
      sheet?.querySelector<HTMLElement>(p === "confirmPermanent" ? ".ack input" : ".sheet-actions button")?.focus();
    });
  });
  onMount(() => void session.load());
  onDestroy(() => session.dispose());
</script>

<section class="trash">
  <PageHeader title={$tr("trash.title")} description={info && info.count > 0 ? $tr("trash.freed.note") : undefined}>
    {#snippet actions()}
      {#if info && info.count > 0}
        <Button variant="danger" disabled={view.phase !== "list"} onclick={() => session.send({ type: "openPurge" })}>{$tr("trash.empty.button")}</Button>
      {/if}
    {/snippet}
  </PageHeader>
  <LiveRegion message={announce} />

  {#if view.phase === "loading"}
    <p role="status">{$tr("common.loading")}</p>
  {:else if view.phase === "error" && !info}
    <Banner tone="danger" role="alert"><p>{$tr("trash.error.body")}</p>
      {#snippet actions()}<Button onclick={() => { session.send({ type: "reload" }); void session.load(); }}>{$tr("common.retry")}</Button>{/snippet}
    </Banner>
  {:else if info}
    {#if view.error}<Banner tone="danger" role="alert"><p>{$tr("trash.error.body")}</p></Banner>{/if}

    {#if view.phase === "purged" && outcome}
      <div class="done">
        <Banner tone="ok" role="status">
          {#if outcomeText(outcome)}<p>{outcomeText(outcome)}</p>{/if}
          {#if outcome.kept > 0}<p>{$tr("trash.purged.kept", { count: outcome.kept })}</p>{/if}
          {#if outcome.marked > 0}<p>{$tr("trash.purge.marked", { count: outcome.marked })}</p>{/if}
          {#snippet actions()}<Button onclick={() => session.send({ type: "dismiss" })}>{$tr("common.done")}</Button>{/snippet}
        </Banner>
      </div>
    {/if}

    {#if info.count === 0}
      <Card>
        <h2 class="empty">{$tr("trash.empty.title")}</h2>
      </Card>
    {:else}
      <div class="tiles">
        <StatTile label={$tr("trash.stat.count")} value={fmt.number(info.count)} />
        <StatTile label={$tr("trash.stat.size")} value={fmt.bytes(info.bytes)} />
        <StatTile label={$tr("trash.section.soon")} value={fmt.number(soonCount)} tone={soonCount > 0 ? "warn" : "default"} />
        {#if hasExpired(info)}<StatTile label={$tr("trash.row.expired")} value={fmt.number(info.expired)} tone="danger" />{/if}
      </div>

      <Card padding="none">
        <div class="toolbar">
          <p class="summary">{$tr("trash.summary", { count: info.count, size: fmt.bytes(info.bytes) })}</p>
          <div class="actions">
            <Button size="sm" disabled={!canRestore} onclick={() => session.send({ type: "selectAll" })}>{$tr("review.select.all")}</Button>
            <Button variant="primary" size="sm" disabled={!canRestore || view.selected.size === 0} onclick={() => void session.restore(false)}>{$tr("trash.restore")}</Button>
            <Button size="sm" disabled={!canRestore} onclick={() => void session.restore(true)}>{$tr("trash.restore.all")}</Button>
          </div>
          {#if view.restored !== null}
            <p class="muted note" role="status">{$tr("restore.done", { count: view.restored })} {$tr("restore.where")}</p>
          {:else if view.restoreFailed}
            <p class="note" role="alert">{$tr("restore.failed")}</p>
          {:else}
            <p class="muted note">{view.selected.size === 0 ? $tr("restore.none") : $tr("restore.selected", { count: view.selected.size })}</p>
          {/if}
        </div>

        <div role="group" aria-label={$tr("restore.list.label")}>
          {#each sections as sec (sec.id)}
            <h2 class="sec {sec.id}">
              {#if sec.id === "expired"}<Icon name="danger" />{:else if sec.id === "soon"}<Icon name="warn" />{/if}
              {$tr(sectionKey[sec.id])}
              <span class="count">{fmt.number(sec.items.length)}</span>
            </h2>
            <ul class="items">
              {#each sec.items as it (it.asset)}
                <li>
                  <label class="row">
                    <input type="checkbox" checked={view.selected.has(it.asset)} disabled={!canRestore}
                      aria-label={$tr("restore.row.select", { name: it.name })}
                      onchange={() => session.send({ type: "toggle", asset: it.asset })} />
                    <span class="thumb" aria-hidden="true"></span>
                    <span class="name">{it.name}</span>
                    <span class="meta size">{fmt.bytes(it.bytes)}</span>
                    <span class="meta moved">{$tr("trash.row.moved", { date: fmt.date(it.trashedAt * 1000) })}</span>
                    <span class="meta exp" class:warn={sec.id !== "other"}>{#if it.expired}<strong>{$tr("trash.row.expired")}</strong>{:else}{$tr("trash.row.expires", { days: daysLeft(it, nowS) })}{/if}</span>
                  </label>
                </li>
              {/each}
            </ul>
          {/each}
        </div>
      </Card>
    {/if}

    {#if info.freedBytes > 0 && info.freedSince}
      <p class="muted history">{$tr("trash.history", { size: fmt.bytes(info.freedBytes), date: fmt.date(info.freedSince) })}</p>
    {/if}
  {/if}

  <dialog class="sheet" bind:this={sheet} aria-labelledby="tr-t" oncancel={(e) => { if (view.phase === "purging") e.preventDefault(); }}
    onclose={() => session.send({ type: "cancel" })}>
    {#if info}
      {#if view.phase === "confirmPermanent"}
        <div class="sheet-body">
          <h2 id="tr-t">{$tr("trash.permanent.title")}</h2>
          <Banner tone="danger"><p>{$tr("trash.purge.summary", { count: info.count, size: fmt.bytes(info.bytes) })}</p></Banner>
          <label class="opt ack">
            <input type="checkbox" checked={view.ack} onchange={(e) => session.send({ type: "setAck", value: e.currentTarget.checked })} />
            <span>{$tr("trash.permanent.ack")}</span>
          </label>
        </div>
        <!-- Destruktiv knapp sist i fokusrekkefølgen. -->
        <div class="sheet-actions">
          <Button onclick={() => session.send({ type: "cancel" })}>{$tr("common.cancel")}</Button>
          <Button variant="danger" disabled={!view.ack} onclick={() => void session.confirm()}>{$tr("trash.permanent.go", { count: info.count })}</Button>
        </div>
      {:else if view.phase === "confirmUnsupported"}
        <!-- Filer Papirkurv ikke kan ta ligger urørt i karantenen. «La ligge» er standard og første i fokusrekkefølgen;
             den destruktive knappen er sist og fører til eget trinn med avkrysning (PermanentAck). -->
        <div class="sheet-body">
          <h2 id="tr-t">{$tr("trash.unsupported.title")}</h2>
          <Banner tone="warn" role="alert"><p>{$tr("trash.unsupported.body", { count: view.unsupported })}</p></Banner>
          <p>{$tr("trash.unsupported.options")}</p>
        </div>
        <div class="sheet-actions">
          <Button variant="primary" onclick={() => session.send({ type: "keepUnsupported" })}>{$tr("trash.unsupported.keep")}</Button>
          <Button variant="danger" onclick={() => session.send({ type: "deleteUnsupported" })}>{$tr("trash.unsupported.delete")}</Button>
        </div>
      {:else}
        <div class="sheet-body">
          <h2 id="tr-t">{$tr("trash.purge.title")}</h2>
          <p>{$tr("trash.purge.summary", { count: info.count, size: fmt.bytes(info.bytes) })}</p>
          <fieldset disabled={view.phase === "purging"}>
            <legend class="sr-only">{$tr("trash.purge.title")}</legend>
            <label class="opt"><input type="radio" name="mode" checked={view.mode === "recycle"} onchange={() => session.send({ type: "setMode", mode: "recycle" })} /> <span>{$tr("trash.purge.mode.recycle")}</span></label>
            <label class="opt"><input type="radio" name="mode" checked={view.mode === "permanent"} onchange={() => session.send({ type: "setMode", mode: "permanent" })} /> <span>{$tr("trash.purge.mode.permanent")}</span></label>
          </fieldset>
          {#if view.phase === "purging"}<p role="status">{$tr("common.loading")}</p>{/if}
        </div>
        <div class="sheet-actions">
          <Button disabled={view.phase === "purging"} onclick={() => session.send({ type: "cancel" })}>{$tr("common.cancel")}</Button>
          <Button variant="primary" disabled={view.phase === "purging"} onclick={() => void session.confirm()}>{$tr("common.continue")}</Button>
        </div>
      {/if}
    {/if}
  </dialog>
</section>

<style>
  .trash { max-inline-size: var(--w-content); }
  .muted { color: var(--c-text-muted); }
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(var(--tile-min), 1fr)); gap: var(--space-4); margin-block-end: var(--space-4); }
  .empty { margin: 0; font-size: var(--fs-lg); }
  .toolbar { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-4); padding: var(--space-3) var(--space-4); border-block-end: var(--border-width) solid var(--c-border-subtle); }
  .summary { margin: 0; flex: 1 1 14rem; font-weight: var(--fw-semibold); font-variant-numeric: var(--num); }
  .actions { display: flex; flex-wrap: wrap; gap: var(--space-2); }
  .note { margin: 0; flex-basis: 100%; font-size: var(--fs-sm); }
  .sec { display: flex; align-items: center; gap: var(--space-2); margin: 0; padding: var(--space-2) var(--space-4); background: var(--c-surface-sunken); border-block-end: var(--border-width) solid var(--c-border-subtle); font-size: var(--fs-sm); font-weight: var(--fw-semibold); }
  .sec.expired { color: var(--c-danger); }
  .count { padding-inline: var(--space-2); border-radius: var(--radius-pill); background: var(--c-surface); color: var(--c-text-muted); font-variant-numeric: var(--num); }
  .items { list-style: none; margin: 0; padding: 0; }
  .items li + li { border-block-start: var(--border-width) solid var(--c-border-subtle); }
  .row { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-1) var(--space-3); padding: var(--space-2) var(--space-4); min-block-size: var(--target-min); cursor: pointer; }
  .row:hover { background: var(--c-surface-hover); }
  .thumb { flex: none; inline-size: 2.5rem; block-size: 2rem; border-radius: var(--radius-sm); border: var(--border-width) solid var(--c-border-subtle); background: repeating-linear-gradient(45deg, var(--c-surface-sunken), var(--c-surface-sunken) 5px, var(--c-border) 5px, var(--c-border) 6px); }
  .name { flex: 1 1 10rem; min-inline-size: 0; font-weight: var(--fw-medium); overflow-wrap: anywhere; }
  .meta { color: var(--c-text-muted); font-size: var(--fs-sm); font-variant-numeric: var(--num); }
  .size { min-inline-size: 4.5rem; text-align: end; }
  .moved, .exp { min-inline-size: 8rem; }
  .exp.warn { color: var(--c-text); font-weight: var(--fw-semibold); }
  .done { margin-block-end: var(--space-4); }
  .done :global(.banner) { margin-block-end: 0; }
  .history { margin-block-start: var(--space-5); }
  fieldset { display: grid; gap: var(--space-2); margin: 0; padding: 0; border: 0; }
  input[type="checkbox"], input[type="radio"] { inline-size: 1.25rem; block-size: 1.25rem; flex: none; }
</style>
