<!-- Regelbygger: navn og vilkår (alle Condition-typer) i et modalt ark. Teller treff (previewRule) FØR regelen kan lagres:
     «Lagre regel» er sperret (men fokuserbar, med forklaring) til forhåndsvisningen gjelder nøyaktig disse vilkårene.
     Regler foreslår bare; forhåndsvisningen teller og endrer ingenting. Feil fra bridgen vises som tekst fra feilkoden.
     Enter i feltene gir samme handling som knappen (forhåndsvis, eller lagre når forhåndsvisningen er oppdatert).
     Tilstand og flyt i rulesFlow.ts, rene hjelpere i rulesState.ts. -->
<script lang="ts">
  import { tick, untrack } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import type { Rule } from "../bridge/types-v1";
  import { Banner, Button, LiveRegion } from "../lib";
  import Thumb from "../lib/components/Thumb.svelte";
  import { fmt, tr } from "../lib/i18n";
  import "./sheet.css";
  import { canSave, editorFor, previewIsFresh, runPreview, saveEditor, type EditorState } from "./rulesFlow";
  import {
    CONDITION_KINDS, RULE_NAME_MAX, SIZE_UNITS, errorKeyOf, kindHasValue, kindIsNumeric, kindLabelKey, newRow,
    type ConditionKind, type SizeUnit,
  } from "./rulesState";

  interface Props {
    open: boolean;
    bridge: CoreBridge;
    /** null = ny regel. */
    rule: Rule | null;
    onSaved: (name: string) => void;
    onClose: () => void;
  }
  let { open, bridge, rule, onSaved, onClose }: Props = $props();

  let dlg: HTMLDialogElement | undefined = $state();
  let ed: EditorState = $state(editorFor(null));
  let nameInput: HTMLInputElement | undefined = $state();
  let addBtn: HTMLElement | undefined = $state();
  let list: HTMLElement | undefined = $state();
  let announce = $state("");

  const fresh = $derived(previewIsFresh(ed));
  const ready = $derived(canSave(ed));
  const stale = $derived(ed.preview !== null && !fresh);
  const unitKey = { kb: "rules.unit.kb", mb: "rules.unit.mb", gb: "rules.unit.gb" } as const satisfies Record<SizeUnit, string>;

  $effect(() => {
    if (!dlg) return;
    if (open && !dlg.open) {
      const d = dlg;
      const r = rule;
      untrack(() => {
        ed = editorFor(r);
        announce = "";
        d.showModal();
        nameInput?.focus();
        if (r) void preview();
      });
    } else if (!open && dlg.open) dlg.close();
  });

  function touch(): void {
    // Utdatert forhåndsvisning blir stående som tekst, men Lagre sperres til ny forhåndsvisning.
    ed = { ...ed, error: null };
  }

  async function preview(): Promise<void> {
    if (ed.previewing || ed.saving) return;
    ed = { ...ed, previewing: true, error: null };
    announce = $tr("rules.preview.running");
    ed = await runPreview(bridge, ed);
    announce = ed.preview
      ? ed.preview.hits === 0 ? $tr("rules.preview.zero") : $tr("rules.preview.label", { count: ed.preview.hits, size: fmt.bytes(ed.preview.bytes) })
      : $tr(errorKeyOf(ed.error));
  }

  async function save(): Promise<void> {
    if (!ready) return;
    ed = { ...ed, saving: true, error: null };
    const r = await saveEditor(bridge, ed);
    ed = r.state;
    if (r.id !== null) onSaved(ed.name.trim());
    else announce = $tr(errorKeyOf(ed.error));
  }

  function submit(e: Event): void {
    e.preventDefault();
    if (fresh) void save();
    else void preview();
  }

  async function addRow(): Promise<void> {
    ed = { ...ed, rows: [...ed.rows, newRow()], error: null };
    await tick();
    list?.querySelectorAll<HTMLElement>("select.kind")[ed.rows.length - 1]?.focus();
  }
  async function removeRow(key: number): Promise<void> {
    ed = { ...ed, rows: ed.rows.filter((r) => r.key !== key), error: null };
    await tick();
    addBtn?.querySelector<HTMLElement>("button")?.focus();
  }
  function setKind(key: number, kind: ConditionKind): void {
    ed = { ...ed, rows: ed.rows.map((r) => (r.key === key ? { ...r, kind, text: "" } : r)), error: null };
  }
  function setText(key: number, text: string): void {
    ed = { ...ed, rows: ed.rows.map((r) => (r.key === key ? { ...r, text } : r)), error: null };
  }
  function setUnit(key: number, unit: SizeUnit): void {
    ed = { ...ed, rows: ed.rows.map((r) => (r.key === key ? { ...r, unit } : r)), error: null };
  }
  const valueLabel = (k: ConditionKind): string =>
    k === "older_than_days" ? $tr("rules.value.days")
    : k === "in_album" ? $tr("rules.value.album")
    : k === "in_folder" ? $tr("rules.value.folder")
    : k === "mime" ? $tr("rules.value.mime")
    : $tr("rules.value.size");
</script>

<dialog class="sheet" bind:this={dlg} aria-labelledby="re-t" onclose={onClose}>
  <form onsubmit={submit} novalidate>
    <div class="sheet-body">
      <h2 id="re-t">{ed.id === null ? $tr("rules.editor.new.title") : $tr("rules.editor.edit.title")}</h2>

      <div class="field">
        <label for="re-name">{$tr("rules.name.label")}</label>
        <input id="re-name" bind:this={nameInput} type="text" value={ed.name} maxlength={RULE_NAME_MAX} autocomplete="off"
          placeholder={$tr("rules.name.hint")} aria-describedby="re-err"
          oninput={(e) => { ed = { ...ed, name: e.currentTarget.value, error: null }; }} />
      </div>

      <fieldset class="conds">
        <legend>{$tr("rules.cond.heading")}</legend>
        <p class="muted">{$tr("rules.cond.all")}</p>
        <div bind:this={list} class="rows">
          {#each ed.rows as row, i (row.key)}
            <div class="cond">
              <div class="cell kindcell">
                <label class="sr-only" for={`re-k-${row.key}`}>{$tr("rules.cond.kind.label", { n: i + 1 })}</label>
                <select id={`re-k-${row.key}`} class="kind" value={row.kind} onchange={(e) => setKind(row.key, e.currentTarget.value as ConditionKind)}>
                  {#each CONDITION_KINDS as k (k)}<option value={k} selected={k === row.kind}>{$tr(kindLabelKey(k))}</option>{/each}
                </select>
              </div>
              {#if kindHasValue(row.kind)}
                <div class="cell valuecell">
                  <label for={`re-v-${row.key}`}>{valueLabel(row.kind)}</label>
                  <input id={`re-v-${row.key}`} type="text" value={row.text} autocomplete="off" spellcheck="false"
                    inputmode={kindIsNumeric(row.kind) ? "decimal" : "text"}
                    oninput={(e) => setText(row.key, e.currentTarget.value)} />
                </div>
                {#if row.kind === "larger_than_bytes" || row.kind === "smaller_than_bytes"}
                  <div class="cell unitcell">
                    <label for={`re-u-${row.key}`}>{$tr("rules.value.unit")}</label>
                    <select id={`re-u-${row.key}`} value={row.unit} onchange={(e) => setUnit(row.key, e.currentTarget.value as SizeUnit)}>
                      {#each SIZE_UNITS as u (u)}<option value={u} selected={u === row.unit}>{$tr(unitKey[u])}</option>{/each}
                    </select>
                  </div>
                {/if}
              {/if}
              <Button size="sm" onclick={() => void removeRow(row.key)} ariaLabel={$tr("rules.cond.remove.named", { n: i + 1 })}>{$tr("rules.cond.remove")}</Button>
            </div>
          {/each}
        </div>
        <span bind:this={addBtn} class="add">
          <Button onclick={() => void addRow()}>{$tr("rules.cond.add")}</Button>
        </span>
      </fieldset>

      <section class="preview" aria-labelledby="re-pv">
        <h3 id="re-pv" class="sr-only">{$tr("rules.preview.button")}</h3>
        <div class="pv-head">
          <Button onclick={() => void preview()} disabled={ed.previewing || ed.saving}>{ed.previewing ? $tr("rules.preview.running") : $tr("rules.preview.button")}</Button>
          <p class="muted">{$tr("rules.preview.note")}</p>
        </div>
        {#if ed.preview}
          <div class="result" class:stale>
            <p class="count"><strong>{$tr("rules.preview.label", { count: ed.preview.hits, size: fmt.bytes(ed.preview.bytes) })}</strong></p>
            {#if ed.preview.hits === 0}<p>{$tr("rules.preview.zero")}</p>{/if}
            {#if ed.preview.sample.length > 0}
              <p class="muted">{$tr("rules.preview.samples")}</p>
              <ul class="samples" aria-hidden="true">
                {#each ed.preview.sample as id (id)}<li><Thumb {id} bridge={bridge} size="sm" /></li>{/each}
              </ul>
            {/if}
          </div>
        {/if}
        {#if stale}<p id="re-stale" class="muted">{$tr("rules.preview.stale")}</p>{/if}
      </section>

      <LiveRegion message={announce} />
      <div id="re-err">
        {#if ed.error}<Banner tone="danger" role="alert"><p>{$tr(errorKeyOf(ed.error))}</p></Banner>{/if}
      </div>
    </div>
    <div class="sheet-actions">
      {#if !ready && !ed.saving}<p id="re-need" class="need">{stale ? $tr("rules.preview.stale") : $tr("rules.save.needpreview")}</p>{/if}
      <Button onclick={onClose}>{$tr("common.cancel")}</Button>
      <Button variant="primary" type="submit" ariaDisabled={!ready} ariaDescribedby={!ready ? "re-need" : undefined}>{$tr("rules.save.button")}</Button>
    </div>
  </form>
</dialog>

<style>
  dialog.sheet { max-inline-size: var(--w-content); }
  .field, .cell { display: grid; gap: var(--space-1); min-inline-size: 0; }
  label { font-weight: var(--fw-semibold); font-size: var(--fs-sm); }
  input[type="text"], select {
    min-block-size: var(--target-min); padding: var(--space-1) var(--space-3); font: inherit; color: var(--c-text);
    background: var(--c-surface); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-sm);
    inline-size: 100%; min-inline-size: 0; box-sizing: border-box;
  }
  input:focus-visible, select:focus-visible { outline: var(--focus-width) solid var(--c-focus); outline-offset: var(--focus-offset); }
  .conds { margin: 0; padding: 0; border: 0; display: grid; gap: var(--space-2); min-inline-size: 0; }
  legend { padding: 0; font-weight: var(--fw-semibold); margin-block-end: var(--space-1); }
  .muted { margin: 0; color: var(--c-text-muted); font-size: var(--fs-sm); }
  .rows { display: grid; gap: var(--space-3); }
  .cond { display: flex; flex-wrap: wrap; align-items: end; gap: var(--space-2) var(--space-3); padding: var(--space-3); border: var(--border-width) solid var(--c-border-subtle); border-radius: var(--radius-md); background: var(--c-surface-sunken); }
  .kindcell { flex: 1 1 14rem; }
  .valuecell { flex: 1 1 10rem; }
  .unitcell { flex: 0 1 6rem; }
  .add { justify-self: start; }
  .preview { display: grid; gap: var(--space-2); }
  .pv-head { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-4); }
  .result { display: grid; gap: var(--space-1); padding: var(--space-3); border: var(--border-width) solid var(--c-border); border-radius: var(--radius-md); }
  .result.stale { border-style: dashed; }
  .result p { margin: 0; }
  .samples { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: var(--space-2); }
  .samples li { inline-size: 4rem; block-size: 3rem; overflow: hidden; border-radius: var(--radius-sm); border: var(--border-width) solid var(--c-border-subtle); }
  .need { flex: 1 1 100%; margin: 0; color: var(--c-text-muted); font-size: var(--fs-sm); }
</style>
