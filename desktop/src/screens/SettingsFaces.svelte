<!-- Innstillinger: ansikter og personer. Ansiktsindeksen er av som standard. «På» går via informert samtykke (FaceConsent).
     «Slett ansiktsindeks» er en egen farlig handling med bekreftelse som sier ærlig at den IKKE kan angres; bildene beholdes.
     Ingen embeddings eller ansiktsdata vises, bare tall. Tilstand i facesFlow.ts. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { useBridge } from "../context";
  import { Banner, Button, Card, LiveRegion, SegmentedControl } from "../lib";
  import { tr } from "../lib/i18n";
  import FaceConsent from "./FaceConsent.svelte";
  import { FacesSession, initialFaces, type FacesState } from "./facesFlow";
  import "./sheet.css";

  interface Props {
    bridge?: CoreBridge;
  }
  let { bridge }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const session = new FacesSession(b, (s) => (view = s));

  let view: FacesState = $state(initialFaces);
  let wipeSheet: HTMLDialogElement | undefined = $state();

  const options = $derived([
    { value: "on", label: $tr("settings.faces.state.on") },
    { value: "off", label: $tr("settings.faces.state.off") },
  ]);
  const announce = $derived(view.wiped !== null ? $tr("settings.faces.wipe.done", { count: view.wiped }) : "");

  $effect(() => {
    if (!wipeSheet) return;
    if (view.confirmWipe && !wipeSheet.open) wipeSheet.showModal();
    else if (!view.confirmWipe && wipeSheet.open) wipeSheet.close();
  });

  // «På» blir bare valgt etter samtykke: ved avbrudd settes den native radioknappen tilbake til faktisk tilstand.
  $effect(() => {
    const want = view.enabled ? "on" : "off";
    if (view.consent) return;
    for (const i of document.querySelectorAll<HTMLInputElement>('input[name="faces-index"]')) i.checked = i.value === want;
  });

  onMount(() => void session.load());
  onDestroy(() => session.dispose());
</script>

<!-- Card (gruppe med overskrift) i stedet for SettingsCard: kontrakten (e2e/v1-skjermer.spec.ts) navngir seksjonen som gruppe. -->
<Card heading={$tr("settings.faces.title")}>
  <div class="body">
    <p class="muted">{$tr("settings.faces.description")}</p>

    <div class="block">
      <p id="fi-l" class="lbl">{$tr("people.index.toggle")}</p>
      <SegmentedControl name="faces-index" {options} value={view.enabled ? "on" : "off"} disabled={view.phase !== "ready" || view.busy}
        ariaLabelledby="fi-l" ariaDescribedby="fi-d"
        onchange={(v) => { if (v === "on") session.requestOn(); else void session.turnOff(); }} />
      <p id="fi-d" class="muted">{view.enabled ? $tr("settings.faces.on.note") : $tr("people.index.note")}</p>
    </div>

    <div class="block wipe">
      <p id="fw-l" class="lbl">{$tr("people.index.wipe")}</p>
      <p id="fw-d" class="muted">{$tr("settings.faces.wipe.desc")}</p>
      <div><Button variant="danger" disabled={view.phase !== "ready" || view.busy} ariaDescribedby="fw-d" onclick={() => session.requestWipe()}>{$tr("people.index.wipe")}</Button></div>
    </div>

    {#if view.wiped !== null}
      <Banner tone="ok" role="status"><p>{$tr("settings.faces.wipe.done", { count: view.wiped })}</p></Banner>
    {/if}
    {#if view.error}
      <Banner tone="danger" role="alert"><p>{$tr("err.generic")}</p></Banner>
    {/if}
  </div>
</Card>
<LiveRegion message={announce} />

<FaceConsent open={view.consent} busy={view.busy} onconfirm={() => void session.confirmOn()} oncancel={() => session.cancelConsent()} />

<!-- Slettedialogen: «Avbryt» er først og standard. Den farlige knappen er sist og eneste vei til wipeFaceIndex. -->
<dialog class="sheet" bind:this={wipeSheet} aria-labelledby="fw-t" onclose={() => session.cancelWipe()}
  oncancel={(e) => { if (view.busy) e.preventDefault(); }}>
  <div class="sheet-body">
    <h2 id="fw-t">{$tr("settings.faces.wipe.title")}</h2>
    <Banner tone="danger"><p><strong>{$tr("settings.faces.wipe.irreversible")}</strong> {$tr("people.index.wipe.confirm")}</p></Banner>
    <p>{$tr("settings.faces.wipe.loses")}</p>
  </div>
  <div class="sheet-actions">
    <Button disabled={view.busy} onclick={() => session.cancelWipe()}>{$tr("common.cancel")}</Button>
    <Button variant="danger" disabled={view.busy} onclick={() => void session.confirmWipe()}>{$tr("people.index.wipe")}</Button>
  </div>
</dialog>

<style>
  .body { display: grid; gap: var(--space-4); }
  .body p { margin: 0; }
  .block { display: grid; gap: var(--space-2); justify-items: start; }
  .block.wipe { padding-block-start: var(--space-4); border-block-start: var(--border-width) solid var(--c-border-subtle); }
  .lbl { font-weight: var(--fw-medium); }
  .muted { color: var(--c-text-muted); font-size: var(--fs-sm); }
</style>
