<!-- Informert samtykke til ansiktsindeks. Delt av Personer og Innstillinger. Dialogen viser hva som lagres, at det aldri
     forlater enheten og at originaler aldri endres. «Avbryt» er først i fokusrekkefølgen (trygt valg). Esc avbryter. -->
<script lang="ts">
  import { Button } from "../lib";
  import { tr } from "../lib/i18n";
  import "./sheet.css";

  interface Props {
    open: boolean;
    busy?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
  }
  let { open, busy = false, onconfirm, oncancel }: Props = $props();
  let sheet: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (!sheet) return;
    if (open && !sheet.open) sheet.showModal();
    else if (!open && sheet.open) sheet.close();
  });
</script>

<dialog class="sheet" bind:this={sheet} aria-labelledby="fc-t" aria-describedby="fc-d" onclose={() => { if (open) oncancel(); }}>
  <div class="sheet-body">
    <h2 id="fc-t">{$tr("people.consent.title")}</h2>
    <p id="fc-d">{$tr("people.consent.lead")}</p>
    <ul class="points">
      <li><strong>{$tr("people.consent.stores.title")}</strong><span>{$tr("people.consent.stores.body")}</span></li>
      <li><strong>{$tr("people.consent.local.title")}</strong><span>{$tr("people.consent.local.body")}</span></li>
      <li><strong>{$tr("people.consent.originals.title")}</strong><span>{$tr("people.consent.originals.body")}</span></li>
      <li><strong>{$tr("people.consent.off.title")}</strong><span>{$tr("people.consent.off.body")}</span></li>
    </ul>
  </div>
  <div class="sheet-actions">
    <Button disabled={busy} onclick={oncancel}>{$tr("common.cancel")}</Button>
    <Button variant="primary" disabled={busy} onclick={onconfirm}>{$tr("people.consent.confirm")}</Button>
  </div>
</dialog>

<style>
  .points { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--space-3); }
  .points li { display: grid; gap: var(--space-0); padding-inline-start: var(--space-3); border-inline-start: 3px solid var(--c-border-strong); }
  .points strong { font-weight: var(--fw-semibold); }
  .points span { color: var(--c-text-muted); font-size: var(--fs-sm); }
</style>
