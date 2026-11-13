<!-- «Legg til mappe»: tekstfelt for sti i et modalt ark (Esc og Avbryt lukker uten endring). «Bla …» åpner Windows
     mappevalg (tauri-plugin-dialog via bridge.pickFolder) og fyller bare feltet; brukeren bekrefter med «Legg til».
     Uten mappevalg (nettleser/dev) er knappen deaktivert med forklaring; manuell inntasting virker alltid.
     Fokus tilbake til feltet etter valg; valgt sti annonseres i LiveRegion inne i dialogen (resten av siden er inert).
     Brukes av Hjem og Onboarding. -->
<script lang="ts">
  import { Banner, Button, LiveRegion } from "../lib";
  import "./sheet.css";
  import { tr } from "../lib/i18n";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { browseFolder } from "./homeFlow";

  interface Props {
    open: boolean;
    /** Brukes til «Bla …» (mappevalg). */
    bridge: CoreBridge;
    /** Returnerer true når mappen er valgt. false = vis feil, behold arket. */
    onSubmit: (path: string) => Promise<boolean>;
    onClose: () => void;
  }
  let { open, bridge, onSubmit, onClose }: Props = $props();

  let dlg: HTMLDialogElement | undefined = $state();
  let path = $state("");
  let busy = $state(false);
  let failed = $state(false);
  let input: HTMLInputElement | undefined = $state();
  let picking = $state(false);
  let browseFailed = $state(false);
  let announce = $state("");

  $effect(() => {
    if (!dlg) return;
    if (open && !dlg.open) { path = ""; failed = false; browseFailed = false; announce = ""; dlg.showModal(); }
    else if (!open && dlg.open) dlg.close();
  });

  async function browse(): Promise<void> {
    if (picking) return;
    picking = true;
    browseFailed = false;
    announce = "";
    const r = await browseFolder(bridge, $tr("home.folder.browse.title"));
    picking = false;
    if (r.kind === "picked") {
      path = r.path;
      failed = false;
      announce = $tr("home.folder.browse.picked", { path: r.path });
    } else if (r.kind === "failed") browseFailed = true;
    input?.focus();
  }

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

<dialog class="sheet" bind:this={dlg} aria-labelledby="fd-t" onclose={onClose}>
  <form onsubmit={submit}>
    <div class="sheet-body">
      <h2 id="fd-t">{$tr("home.folder.title")}</h2>
      <p>{$tr("home.folder.note")}</p>
      <div class="field">
        <label for="fd-path">{$tr("home.folder.path")}</label>
        <div class="row">
          <input id="fd-path" bind:this={input} type="text" bind:value={path} autocomplete="off" spellcheck="false" aria-describedby="fd-ex" aria-invalid={failed ? "true" : undefined} />
          <Button onclick={browse} disabled={!bridge.canPickFolder || picking}>{$tr("home.folder.browse")}</Button>
        </div>
        <p id="fd-ex" class="muted">{$tr("home.folder.example")}</p>
      </div>
      {#if !bridge.canPickFolder}<p class="muted">{$tr("home.folder.browse.unavailable")}</p>{/if}
      {#if browseFailed}<Banner tone="warn" role="alert"><p>{$tr("home.folder.browse.unavailable")}</p></Banner>{/if}
      <LiveRegion message={announce} />
      {#if failed}<Banner tone="danger" role="alert"><p>{$tr("home.folder.error")}</p></Banner>{/if}
    </div>
    <div class="sheet-actions">
      <Button onclick={onClose}>{$tr("common.cancel")}</Button>
      <Button variant="primary" type="submit" disabled={busy || path.trim() === ""}>{$tr("home.folder.submit")}</Button>
    </div>
  </form>
</dialog>
