<!-- «Legg til mappe»: tekstfelt for sti i et modalt ark (Esc og Avbryt lukker uten endring). Mappevelger fra Windows
     (tauri-plugin-dialog) er ikke koblet på ennå; stien skrives inn. Brukes av Hjem og Onboarding. -->
<script lang="ts">
  import { Button } from "../lib";
  import { tr } from "../lib/i18n";

  interface Props {
    open: boolean;
    /** Returnerer true når mappen er valgt. false = vis feil, behold arket. */
    onSubmit: (path: string) => Promise<boolean>;
    onClose: () => void;
  }
  let { open, onSubmit, onClose }: Props = $props();

  let dlg: HTMLDialogElement | undefined = $state();
  let path = $state("");
  let busy = $state(false);
  let failed = $state(false);

  $effect(() => {
    if (!dlg) return;
    if (open && !dlg.open) { path = ""; failed = false; dlg.showModal(); }
    else if (!open && dlg.open) dlg.close();
  });

  async function submit(e: Event): Promise<void> {
    e.preventDefault();
    if (busy || path.trim() === "") return;
    busy = true;
    failed = false;
    const ok = await onSubmit(path);
    busy = false;
    failed = !ok;
  }
</script>

<dialog bind:this={dlg} aria-labelledby="fd-t" onclose={onClose}>
  <form onsubmit={submit}>
    <h2 id="fd-t">{$tr("home.folder.title")}</h2>
    <p>{$tr("home.folder.note")}</p>
    <label class="field">
      <span>{$tr("home.folder.path")}</span>
      <input type="text" bind:value={path} autocomplete="off" spellcheck="false" aria-describedby="fd-ex" aria-invalid={failed ? "true" : undefined} />
    </label>
    <p id="fd-ex" class="muted">{$tr("home.folder.example")}</p>
    {#if failed}<p role="alert">{$tr("home.folder.error")}</p>{/if}
    <div class="actions">
      <Button onclick={onClose}>{$tr("common.cancel")}</Button>
      <Button variant="primary" type="submit" disabled={busy || path.trim() === ""}>{$tr("home.folder.submit")}</Button>
    </div>
  </form>
</dialog>

<style>
  dialog { max-inline-size: min(32rem, calc(100vw - 2rem)); padding: var(--space-5); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-lg); background: var(--c-surface); color: var(--c-text); }
  dialog::backdrop { background: var(--c-scrim); }
  h2 { margin-block-start: 0; font-size: var(--fs-lg); }
  .field { display: grid; gap: var(--space-1); }
  input { min-block-size: var(--target-min); padding: 0 var(--space-3); font: inherit; color: var(--c-text); background: var(--c-surface); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-md); }
  .muted { color: var(--c-text-muted); }
  .actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: var(--space-3); margin-block-start: var(--space-3); }
</style>
