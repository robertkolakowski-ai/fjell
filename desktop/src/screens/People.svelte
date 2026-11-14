<!-- Personer (V1): rutenett av personer fra ansiktsindeksen (opt-in, av som standard). Hver person: miniatyr, navn eller
     «Uten navn», antall bilder, «Gi navn» (inline), «Slå sammen» (egen valgdialog) og beskyttelse (ProtectToggle).
     Alt som endres kan angres: sammenslåing (unmergePerson), beskyttelse og navn via «Angre» i Toast.
     Er indeksen av, forklarer skjermen hva som skjer og tilbyr «Slå på ansiktsindeks» (informert samtykke først).
     Ingen embeddings eller ansiktsdata vises, bare tall. Tilstand i peopleState.ts, flyt i peopleFlow.ts. -->
<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import type { Person } from "../bridge/types-v1";
  import { useBridge } from "../context";
  import { Banner, Button, Card, EmptyState, LiveRegion, PageHeader, ProtectToggle, Toast } from "../lib";
  import Thumb from "../lib/components/Thumb.svelte";
  import { tr } from "../lib/i18n";
  import FaceConsent from "./FaceConsent.svelte";
  import { PeopleSession } from "./peopleFlow";
  import { displayName, initialPeople, mergeRoles, PERSON_NAME_MAX, type PeopleState } from "./peopleState";
  import "./sheet.css";

  interface Props {
    bridge?: CoreBridge;
  }
  let { bridge }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const session = new PeopleSession(b, () => ({ unnamed: unnamedText }), (s) => (view = s));

  let view: PeopleState = $state(initialPeople);
  let announce = $state("");
  let draft = $state("");
  let mergeSheet: HTMLDialogElement | undefined = $state();
  let nameInput: HTMLInputElement | undefined = $state();

  const unnamedText = $derived($tr("people.unnamed"));
  const name = (p: Person): string => displayName(p, unnamedText);
  /** Navn med antall bilder: skiller personer uten navn fra hverandre for skjermleser og valg. */
  const full = (p: Person): string => $tr("people.person.pick", { name: name(p), photos: $tr("common.count.photos", { count: p.assetCount }) });

  /** Feiltekst per kode (error.<kode>); alt annet gir den generelle meldingen. */
  const errText = (code: string): string =>
    code === "unknown_person" ? $tr("error.unknown_person") : code === "not_merged" ? $tr("error.not_merged")
    : code === "name_too_long" ? $tr("error.name_too_long") : $tr("err.generic");

  const mergeFrom = $derived(view.merge ? view.people.find((p) => p.id === view.merge!.from) : undefined);
  const mergeOthers = $derived(view.merge ? view.people.filter((p) => p.id !== view.merge!.from) : []);
  const roles = $derived(view.merge ? mergeRoles(view.merge) : null);
  const roleOf = (id: number | undefined): Person | undefined => view.people.find((p) => p.id === id);

  const undoText = $derived.by(() => {
    const u = view.undo;
    if (!u) return "";
    if (u.kind === "merge") return $tr("people.merge.done", { absorbed: u.absorbedLabel, target: u.targetLabel });
    if (u.kind === "name") return $tr("people.name.changed", { person: u.label });
    return $tr(u.prev ? "people.protect.off.done" : "people.protect.on.done", { person: u.label });
  });

  // Dialogen styres av tilstand; Esc avbryter.
  $effect(() => {
    if (!mergeSheet) return;
    if (view.merge && !mergeSheet.open) mergeSheet.showModal();
    else if (!view.merge && mergeSheet.open) mergeSheet.close();
  });
  // Navnefeltet får fokus og forhåndsutfylt navn når det åpnes.
  $effect(() => {
    if (view.editing === null) return;
    const p = view.people.find((x) => x.id === view.editing);
    draft = p?.name ?? "";
    void tick().then(() => { nameInput?.focus(); nameInput?.select(); });
  });
  // Etter endring i listen: fokus til personens «Gi navn»-knapp (finnes alltid), ellers overskriften.
  $effect(() => {
    const id = view.focusId;
    if (id === null) return;
    void tick().then(() => {
      document.querySelector<HTMLElement>(`[data-rename="${id}"]`)?.closest("button")?.focus();
      session.clearFocus();
    });
  });

  async function enable(): Promise<void> {
    if (await session.enable()) announce = $tr("people.index.on.done");
  }
  async function undo(): Promise<void> {
    if (await session.undo()) announce = $tr("people.undo.done");
  }
  function onNameKey(e: KeyboardEvent): void {
    if (e.key === "Escape") { e.preventDefault(); session.cancelRename(); }
  }

  onMount(() => void session.load());
  onDestroy(() => session.dispose());
</script>

<section class="people">
  <PageHeader title={$tr("people.title.label")} description={view.faceIndex ? $tr("people.description") : undefined} />
  <LiveRegion message={announce} />

  {#if view.phase === "loading"}
    <p role="status">{$tr("common.loading")}</p>
  {:else if view.phase === "error"}
    <Banner tone="danger" role="alert"><p>{$tr("err.generic")}</p>
      {#snippet actions()}<Button onclick={() => void session.load()}>{$tr("common.retry")}</Button>{/snippet}
    </Banner>
  {:else}
    {#if view.error}<div class="gap"><Banner tone="danger" role="alert"><p>{errText(view.error)}</p></Banner></div>{/if}

    {#if !view.faceIndex}
      <Card>
        <EmptyState title={$tr("people.off.title")} description={$tr("people.off.body")}>
          {#snippet action()}<Button variant="primary" onclick={() => session.openConsent()}>{$tr("people.enable.button")}</Button>{/snippet}
        </EmptyState>
      </Card>
    {:else if view.people.length === 0}
      <Card><EmptyState title={$tr("people.on.empty.title")} description={$tr("people.on.empty.body")} /></Card>
    {:else}
      <ul class="grid" aria-label={$tr("people.list.label")}>
        {#each view.people as p (p.id)}
          <li class="person" data-person={p.id}>
            <Card>
              <div class="inner">
                <div class="cover" aria-hidden="true">{#if p.cover !== null}<Thumb id={p.cover} />{/if}</div>
                <div class="who">
                  {#if view.editing === p.id}
                    <form class="edit" onsubmit={(e) => { e.preventDefault(); void session.saveName(p.id, draft); }}>
                      <label for="pn-{p.id}">{$tr("people.name.label")}</label>
                      <input id="pn-{p.id}" type="text" bind:this={nameInput} bind:value={draft} autocomplete="off" spellcheck="false"
                        placeholder={$tr("people.name.hint")} onkeydown={onNameKey}
                        aria-invalid={view.nameError ? "true" : undefined} aria-describedby="pn-{p.id}-h{view.nameError ? ` pn-${p.id}-e` : ''}" />
                      <p id="pn-{p.id}-h" class="muted">{$tr("people.name.help")}</p>
                      {#if view.nameError}
                        <p id="pn-{p.id}-e" class="err" role="alert">{errText(view.nameError)} ({[...draft.trim()].length}/{PERSON_NAME_MAX})</p>
                      {/if}
                      <div class="row">
                        <Button variant="primary" size="sm" type="submit" disabled={view.busy}>{$tr("people.name.save")}</Button>
                        <Button size="sm" disabled={view.busy} onclick={() => session.cancelRename()}>{$tr("common.cancel")}</Button>
                      </div>
                    </form>
                  {:else}
                    <h2 class="name" class:unnamed={!p.name}>{name(p)}</h2>
                    <p class="count">{$tr("common.count.photos", { count: p.assetCount })}</p>
                  {/if}
                </div>
              </div>
              {#if view.editing !== p.id}
                <div class="acts">
                  <Button size="sm" disabled={view.busy} ariaLabel={$tr(p.name ? "people.name.change.aria" : "people.name.add.aria", { person: p.name ? name(p) : full(p) })}
                    onclick={() => session.startRename(p.id)}>
                    <span data-rename={p.id}>{$tr(p.name ? "people.name.change" : "people.name.button")}</span>
                  </Button>
                  {#if view.people.length > 1}
                    <Button size="sm" disabled={view.busy} ariaLabel={$tr("people.merge.aria", { person: full(p) })} onclick={() => session.openMerge(p.id)}>{$tr("people.merge.button")}</Button>
                  {/if}
                </div>
                <div class="protect">
                  <ProtectToggle scope={{ scope: "person", target: String(p.id) }} protected={p.protected} disabled={view.busy}
                    label={$tr("people.protect.label", { person: full(p) })} onchange={(next) => void session.toggleProtect(p.id, next)} />
                </div>
              {/if}
            </Card>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}

  {#if view.undo}
    <div class="toast-slot">
      <Toast message={undoText} undoLabel={$tr("common.undo")} onUndo={() => void undo()} dismissLabel={$tr("common.dismiss")} onDismiss={() => session.dismissUndo()} />
    </div>
  {/if}

  <FaceConsent open={view.consent} busy={view.busy} onconfirm={() => void enable()} oncancel={() => session.closeConsent()} />

  <!-- Sammenslåing: velg den andre personen og hvem som blir stående. Ingenting slettes; kan angres. -->
  <dialog class="sheet" bind:this={mergeSheet} aria-labelledby="pm-t" onclose={() => { if (view.merge) session.cancelMerge(); }}>
    {#if view.merge && mergeFrom}
      <div class="sheet-body">
        <h2 id="pm-t">{$tr("people.merge.title")}</h2>
        <p>{$tr("people.merge.body", { person: full(mergeFrom) })}</p>
        {#if mergeOthers.length === 0}
          <p class="muted">{$tr("people.merge.none")}</p>
        {:else}
          <fieldset class="pick">
            <legend>{$tr("people.merge.other")}</legend>
            {#each mergeOthers as o (o.id)}
              <label class="opt">
                <input type="radio" name="merge-other" checked={view.merge.other === o.id} onchange={() => session.pickMergeOther(o.id)} />
                <span class="mini" aria-hidden="true">{#if o.cover !== null}<Thumb id={o.cover} size="sm" />{/if}</span>
                <span>{full(o)}</span>
              </label>
            {/each}
          </fieldset>
          {#if view.merge.other !== null}
            {@const other = roleOf(view.merge.other)}
            <fieldset class="pick">
              <legend>{$tr("people.merge.keep")}</legend>
              <label class="opt">
                <input type="radio" name="merge-keep" checked={view.merge.keep === "from"} onchange={() => session.setMergeKeep("from")} />
                <span>{full(mergeFrom)}</span>
              </label>
              {#if other}
                <label class="opt">
                  <input type="radio" name="merge-keep" checked={view.merge.keep === "other"} onchange={() => session.setMergeKeep("other")} />
                  <span>{full(other)}</span>
                </label>
              {/if}
            </fieldset>
            {#if roles}
              <Banner tone="info"><p>{$tr("people.merge.summary", { absorbed: name(roleOf(roles.absorbed) ?? mergeFrom), target: name(roleOf(roles.target) ?? mergeFrom) })}</p></Banner>
            {/if}
          {/if}
        {/if}
      </div>
      <div class="sheet-actions">
        <Button disabled={view.busy} onclick={() => session.cancelMerge()}>{$tr("common.cancel")}</Button>
        <Button variant="primary" disabled={view.busy || view.merge.other === null} onclick={() => void session.confirmMerge()}>{$tr("people.merge.button")}</Button>
      </div>
    {/if}
  </dialog>
</section>

<style>
  .people { max-inline-size: var(--w-content); }
  .gap { margin-block-end: var(--space-4); }
  .grid { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 15rem), 1fr)); gap: var(--space-4); }
  .person { min-inline-size: 0; }
  .person :global(.card) { block-size: 100%; display: flex; flex-direction: column; gap: var(--space-3); }
  .inner { display: flex; flex-direction: column; gap: var(--space-3); }
  .cover { inline-size: 100%; aspect-ratio: 1; max-block-size: 11rem; overflow: hidden; border-radius: var(--radius-md); background: var(--c-surface-sunken); }
  .name { margin: 0; font-size: var(--fs-lg); font-weight: var(--fw-semibold); line-height: var(--lh-tight); overflow-wrap: anywhere; }
  .name.unnamed { color: var(--c-text-muted); font-weight: var(--fw-medium); }
  .count { margin: var(--space-0) 0 0; color: var(--c-text-muted); font-size: var(--fs-sm); font-variant-numeric: var(--num); }
  .acts { display: flex; flex-wrap: wrap; gap: var(--space-2); }
  .protect { margin-block-start: auto; }
  .protect :global(.toggle) { inline-size: 100%; }
  .edit { display: grid; gap: var(--space-2); }
  .edit label { font-weight: var(--fw-medium); }
  .edit input {
    min-block-size: var(--target-min); padding: 0 var(--space-3); font: inherit; color: var(--c-text); min-inline-size: 0;
    background: var(--c-surface); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-md);
  }
  .edit input[aria-invalid="true"] { border-color: var(--c-danger); border-width: 3px; }
  .edit p { margin: 0; }
  .muted { color: var(--c-text-muted); font-size: var(--fs-sm); }
  .err { color: var(--c-danger); font-weight: var(--fw-semibold); font-size: var(--fs-sm); }
  .row { display: flex; flex-wrap: wrap; gap: var(--space-2); }
  .toast-slot { position: fixed; inset-block-end: var(--space-4); inset-inline-start: var(--space-4); inset-inline-end: var(--space-4); z-index: 20; display: flex; justify-content: center; pointer-events: none; }
  .toast-slot :global(.toast) { pointer-events: auto; }
  fieldset.pick { display: grid; gap: var(--space-2); margin: 0; padding: 0; border: 0; min-inline-size: 0; max-block-size: 16rem; overflow: auto; }
  fieldset.pick legend { padding: 0; margin-block-end: var(--space-2); font-weight: var(--fw-semibold); }
  .mini { flex: none; inline-size: 2rem; block-size: 2rem; overflow: hidden; border-radius: var(--radius-sm); background: var(--c-surface-sunken); }
  input[type="radio"] { inline-size: 1.25rem; block-size: 1.25rem; flex: none; }
  .opt { align-items: center; }
</style>
