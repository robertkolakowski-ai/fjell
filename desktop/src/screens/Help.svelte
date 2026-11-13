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
  /** Tastekombinasjon delt i kapsler: «Ctrl+A» blir to kapsler, «↑ / Delete» to. Skilletegn står som vanlig tekst mellom dem. */
  const parts = (k: string): { text: string; sep: boolean }[] =>
    k.split(/\s*([+/])\s*/).map((text, n) => ({ text, sep: n % 2 === 1 }));
</script>

<dialog bind:this={dlg} aria-labelledby="help-t" onclose={onClose}>
  <h2 id="help-t">{$tr("common.shortcuts")}</h2>
  <p class="muted">{$tr("help.for", { screen: $tr(SCREEN_LABEL[screen]) })}</p>

  <div class="cols">
  <section>
  <h3>{$tr("help.section.everywhere")}</h3>
  <dl>
    {#each EVERYWHERE as r (r.keys)}
      <div><dt>{#each parts(keyText(r.keys)) as p, n (n)}{#if p.sep}<span class="sep">{p.text}</span>{:else}<kbd>{p.text}</kbd>{/if}{/each}</dt><dd>{$tr(r.action)}</dd></div>
    {/each}
  </dl>
  </section>

  {#if rows.length > 0}
    <section>
    <h3>{$tr("help.section.screen")}</h3>
    <dl>
      {#each rows as r (r.keys)}
        <div><dt>{#each parts(keyText(r.keys)) as p, n (n)}{#if p.sep}<span class="sep">{p.text}</span>{:else}<kbd>{p.text}</kbd>{/if}{/each}</dt><dd>{$tr(r.action)}</dd></div>
      {/each}
    </dl>
    </section>
  {/if}
  </div>

  <div class="actions"><Button variant="primary" onclick={onClose}>{$tr("common.close")}</Button></div>
</dialog>

<style>
  dialog { inline-size: min(40rem, calc(100vw - 2rem)); max-block-size: calc(100dvh - 2rem); overflow: auto; padding: var(--space-5); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-lg); background: var(--c-surface); color: var(--c-text); box-shadow: var(--shadow-pop); }
  dialog::backdrop { background: var(--c-scrim); }
  h2 { margin-block: 0 var(--space-1); font-size: var(--fs-lg); }
  h3 { margin-block: 0 var(--space-2); font-size: var(--fs-sm); font-weight: var(--fw-semibold); color: var(--c-text-muted); }
  .muted { margin: 0; color: var(--c-text-muted); font-size: var(--fs-sm); }
  .cols { display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--space-4) var(--space-5); margin-block-start: var(--space-4); align-items: start; }
  @media (min-width: 40rem) { .cols { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
  dl { margin: 0; border: var(--border-width) solid var(--c-border-subtle); border-radius: var(--radius-md); background: var(--c-bg); }
  dl div { display: grid; grid-template-columns: minmax(5rem, 8rem) minmax(0, 1fr); gap: var(--space-1) var(--space-3); align-items: center; padding: var(--space-2) var(--space-3); border-block-start: var(--border-width) solid var(--c-border-subtle); }
  dl div:first-child { border-block-start: 0; }
  dt { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-1); }
  dd { margin: 0; min-inline-size: 0; overflow-wrap: anywhere; }
  .sep { color: var(--c-text-muted); }
  @media (max-width: 30rem) { dl div { grid-template-columns: minmax(0, 1fr); } }
  kbd { display: inline-block; min-inline-size: 1.75rem; padding: var(--space-0) var(--space-2); text-align: center; font-family: var(--font-mono); font-size: var(--fs-sm); line-height: var(--lh-body); border: var(--border-width) solid var(--c-border-strong); border-block-end-width: 3px; border-radius: var(--radius-sm); background: var(--c-surface); }
  .actions { display: flex; justify-content: flex-end; margin-block-start: var(--space-4); }
</style>
