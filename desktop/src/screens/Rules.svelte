<!-- Regler: liste (navn, vilkår som lesbar setning, treff, av/på), «Ny regel», rediger og slett. Regler foreslår bare: de flytter,
     sletter eller godtar aldri noe selv (banneret sier det). Sletting er mykt og kan angres med «Angre» (deleteRule/restoreRule).
     Opprett og rediger skjer i RuleEditor med forhåndsvisning før lagring. Tilstand i rulesFlow.ts, hjelpere i rulesState.ts. -->
<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import type { Rule } from "../bridge/types-v1";
  import { useBridge } from "../context";
  import { Banner, Button, Card, EmptyState, LiveRegion, PageHeader, Toast } from "../lib";
  import { fmt, tr } from "../lib/i18n";
  import RuleEditor from "./RuleEditor.svelte";
  import { initialRules, RulesSession, type RulesState } from "./rulesFlow";
  import { describeRule, errorKeyOf } from "./rulesState";

  interface Props {
    bridge?: CoreBridge;
  }
  let { bridge }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const session = new RulesSession(b, (s) => (view = s));

  let view: RulesState = $state(initialRules);
  let editing: Rule | null = $state(null);
  let editorOpen = $state(false);
  let host: HTMLElement | undefined = $state();
  let lastTrigger: HTMLElement | null = null;

  const note = $derived(view.note);
  const noteText = $derived(
    !note ? ""
    : note.kind === "saved" ? $tr("rules.saved", { name: note.name })
    : note.kind === "toggled" ? $tr(note.enabled ? "rules.toggled.on" : "rules.toggled.off", { name: note.name })
    : note.kind === "deleted" ? $tr("rules.deleted", { name: note.name })
    : $tr("rules.restored", { name: note.name }),
  );
  const announce = $derived(view.error ? $tr(errorKeyOf(view.error)) : note?.kind === "deleted" ? "" : noteText);
  const sentence = (r: Rule): string => describeRule(r.conditions, $tr, fmt.bytes);

  function openEditor(rule: Rule | null, trigger: HTMLElement | null): void {
    lastTrigger = trigger;
    editing = rule;
    editorOpen = true;
  }
  async function closeEditor(): Promise<void> {
    editorOpen = false;
    await tick();
    if (lastTrigger?.isConnected) lastTrigger.focus();
    else host?.querySelector<HTMLElement>("button")?.focus();
  }
  async function saved(name: string): Promise<void> {
    await session.saved(name);
    await closeEditor();
  }
  async function removeRule(r: Rule): Promise<void> {
    if (await session.remove(r.id)) {
      await tick();
      host?.querySelector<HTMLElement>(".toast .undo")?.focus();
    }
  }
  async function undo(): Promise<void> {
    await session.undoRemove();
    await tick();
    host?.querySelector<HTMLElement>("li button, li input")?.focus();
  }

  onMount(() => void session.load());
  onDestroy(() => session.dispose());
</script>

<section class="rules" bind:this={host}>
  <PageHeader title={$tr("rules.title.label")} description={$tr("rules.note.suggest")}>
    {#snippet actions()}
      {#if view.phase === "ready"}
        <Button variant="primary" onclick={(e) => openEditor(null, e.currentTarget as HTMLElement)}>{$tr("rules.add.button")}</Button>
      {/if}
    {/snippet}
  </PageHeader>
  <LiveRegion message={announce} />

  <Banner tone="info"><p>{$tr("rules.note.never")}</p></Banner>

  {#if view.phase === "loading"}
    <p role="status">{$tr("common.loading")}</p>
  {:else if view.phase === "error"}
    <Banner tone="danger" role="alert"><p>{$tr("rules.error.load")}</p>
      {#snippet actions()}<Button onclick={() => void session.load()}>{$tr("common.retry")}</Button>{/snippet}
    </Banner>
  {:else}
    {#if view.error}<Banner tone="danger" role="alert"><p>{$tr(errorKeyOf(view.error))}</p></Banner>{/if}

    {#if note && (note.kind === "deleted")}
      <div class="toastbox">
        <Toast message={noteText} undoLabel={$tr("common.undo")} onUndo={() => void undo()} dismissLabel={$tr("common.close")} onDismiss={() => session.clearNote()} />
      </div>
    {:else if note}
      <div class="toastbox"><Banner tone="ok"><p>{noteText}</p>
        {#snippet actions()}<Button size="sm" onclick={() => session.clearNote()}>{$tr("common.dismiss")}</Button>{/snippet}
      </Banner></div>
    {/if}

    {#if view.rules.length === 0}
      <Card>
        <EmptyState title={$tr("rules.empty.note")} headingLevel={2}>
          {#snippet action()}<Button variant="primary" onclick={(e) => openEditor(null, e.currentTarget as HTMLElement)}>{$tr("rules.add.button")}</Button>{/snippet}
        </EmptyState>
      </Card>
    {:else}
      <Card padding="none">
        <ul class="list" aria-label={$tr("rules.title.label")}>
          {#each view.rules as r (r.id)}
            <li class="item">
              <div class="main">
                <h2 class="name">{r.name}</h2>
                <p class="sentence">{sentence(r)}</p>
                <p class="hits" class:off={!r.enabled}>{$tr("rules.hits.label", { count: r.hits })}</p>
              </div>
              <div class="side">
                <label class="switch">
                  <input type="checkbox" role="switch" checked={r.enabled} disabled={view.busy.has(r.id)}
                    aria-label={$tr("rules.enabled.named", { name: r.name })}
                    onchange={(e) => void session.toggle(r.id, e.currentTarget.checked)} />
                  <span aria-hidden="true">{$tr("rules.toggle.label")}</span>
                </label>
                <Button size="sm" disabled={view.busy.has(r.id)} ariaLabel={$tr("rules.edit.named", { name: r.name })}
                  onclick={(e) => openEditor(r, e.currentTarget as HTMLElement)}>{$tr("rules.edit.button")}</Button>
                <Button size="sm" disabled={view.busy.has(r.id)} ariaLabel={$tr("rules.delete.named", { name: r.name })}
                  onclick={() => void removeRule(r)}>{$tr("rules.delete.button")}</Button>
              </div>
            </li>
          {/each}
        </ul>
      </Card>
    {/if}
  {/if}

  <RuleEditor open={editorOpen} bridge={b} rule={editing} onSaved={(n) => void saved(n)} onClose={() => void closeEditor()} />
</section>

<style>
  .rules { max-inline-size: var(--w-content); display: grid; gap: var(--space-4); align-content: start; }
  .rules > :global(*) { margin: 0; }
  .list { list-style: none; margin: 0; padding: 0; }
  .item { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: var(--space-2) var(--space-4); padding: var(--space-3) var(--space-4); }
  .item + .item { border-block-start: var(--border-width) solid var(--c-border-subtle); }
  .main { flex: 1 1 18rem; min-inline-size: 0; display: grid; gap: var(--space-1); }
  .main > * { margin: 0; }
  .name { font-size: var(--fs-md); font-weight: var(--fw-semibold); line-height: var(--lh-tight); overflow-wrap: anywhere; }
  .sentence { color: var(--c-text-muted); font-size: var(--fs-sm); overflow-wrap: anywhere; }
  .hits { font-variant-numeric: var(--num); font-weight: var(--fw-medium); }
  .hits.off { color: var(--c-text-muted); }
  .side { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-3); }
  .switch { display: inline-flex; align-items: center; gap: var(--space-2); min-block-size: var(--target-min); font-weight: var(--fw-semibold); }
  /* Bryter: native avkrysning med eget utseende. Tilstand bæres av tommelposisjon og farge, i tillegg til rollen «switch». */
  input[type="checkbox"] {
    appearance: none; position: relative; flex: none; margin: 0; inline-size: 2.5rem; block-size: 1.5rem; cursor: pointer;
    border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-pill); background: var(--c-surface-sunken);
  }
  input[type="checkbox"]::before {
    content: ""; position: absolute; inset-block: 0.1875rem; inset-inline-start: 0.1875rem; inline-size: 1rem; border-radius: var(--radius-pill);
    background: var(--c-text-muted); transition: transform var(--dur-fast);
  }
  input[type="checkbox"]:checked { background: var(--c-accent); border-color: var(--c-accent); }
  input[type="checkbox"]:checked::before { transform: translateX(1rem); background: var(--c-on-accent); }
  input[type="checkbox"]:disabled { opacity: 0.6; cursor: not-allowed; }
  input[type="checkbox"]:focus-visible { outline: var(--focus-width) solid var(--c-focus); outline-offset: var(--focus-offset); }
  @media (prefers-reduced-motion: reduce) { input[type="checkbox"]::before { transition: none; } }
  @media (forced-colors: active) {
    input[type="checkbox"] { border-color: ButtonText; background: Canvas; }
    input[type="checkbox"]::before { background: ButtonText; }
    input[type="checkbox"]:checked { background: Highlight; border-color: Highlight; }
    input[type="checkbox"]:checked::before { background: HighlightText; }
    input[type="checkbox"]:disabled { border-color: GrayText; }
  }
</style>
