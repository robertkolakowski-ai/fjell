<!-- F1-hjelp: modalt ark med hurtigtaster for gjeldende skjerm (og «Overalt»). Esc og Lukk lukker. Tabellen ligger i helpKeys.ts. -->
<script lang="ts">
  import { Button } from "../lib";
  import { tr } from "../lib/i18n";
  import { EVERYWHERE, SCREEN_KEYS, SCREEN_LABEL, type HelpScreen } from "./helpKeys";

  interface Props {
    open: boolean;
    screen: HelpScreen;
    onClose: () => void;
  }
  let { open, screen, onClose }: Props = $props();
  let dlg: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (!dlg) return;
    if (open && !dlg.open) dlg.showModal();
    else if (!open && dlg.open) dlg.close();
  });
  const rows = $derived(SCREEN_KEYS[screen]);
  const keyText = (k: string): string => (k === "Space" ? $tr("cards.key.space") : k);
</script>

<dialog bind:this={dlg} aria-labelledby="help-t" onclose={onClose}>
  <h2 id="help-t">{$tr("common.shortcuts")}</h2>
  <p class="muted">{$tr("help.for", { screen: $tr(SCREEN_LABEL[screen]) })}</p>

  <h3>{$tr("help.section.everywhere")}</h3>
  <dl>
    {#each EVERYWHERE as r (r.keys)}
      <div><dt><kbd>{keyText(r.keys)}</kbd></dt><dd>{$tr(r.action)}</dd></div>
    {/each}
  </dl>

  {#if rows.length > 0}
    <h3>{$tr("help.section.screen")}</h3>
    <dl>
      {#each rows as r (r.keys)}
        <div><dt><kbd>{keyText(r.keys)}</kbd></dt><dd>{$tr(r.action)}</dd></div>
      {/each}
    </dl>
  {/if}

  <div class="actions"><Button variant="primary" onclick={onClose}>{$tr("common.close")}</Button></div>
</dialog>

<style>
  dialog { max-inline-size: min(32rem, calc(100vw - 2rem)); padding: var(--space-5); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-lg); background: var(--c-surface); color: var(--c-text); }
  dialog::backdrop { background: var(--c-scrim); }
  h2 { margin-block: 0 var(--space-1); font-size: var(--fs-lg); }
  h3 { margin-block: var(--space-4) var(--space-2); font-size: var(--fs-md); }
  .muted { margin: 0; color: var(--c-text-muted); }
  dl { margin: 0; display: grid; gap: var(--space-2); }
  dl div { display: flex; flex-wrap: wrap; gap: var(--space-1) var(--space-3); align-items: baseline; }
  dt { flex: 0 0 8rem; }
  dd { margin: 0; flex: 1 1 10rem; }
  kbd { font-family: var(--font-mono); padding: 0 var(--space-2); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-sm, 4px); background: var(--c-surface-sunken); }
  .actions { display: flex; justify-content: flex-end; margin-block-start: var(--space-4); }
</style>
