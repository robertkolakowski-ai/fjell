<!-- Innstillinger (UX-5/6): språk (nb, en eller følg Windows), papirkurvfrist (liste), karantenemappe (info) og standard for
     «Tøm» (info). Ingen skyvebrytere, ingen lyd- eller varselvalg (ikke bygget). Tilstand i settingsState.ts, flyt i settingsFlow.ts.
     Språkvalget lagres i nettleserlagring av i18n-runtime (ikke i skallet) inntil skallet får egen innstillingslagring. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { useBridge } from "../context";
  import { Button, LiveRegion } from "../lib";
  import { getLanguage, setLanguage, tr } from "../lib/i18n";
  import { SettingsSession } from "./settingsFlow";
  import { initialSettings, LANGUAGE_CHOICES, trashDayOptions, type LanguageChoice, type SettingsState } from "./settingsState";

  interface Props {
    bridge?: CoreBridge;
  }
  let { bridge }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const session = new SettingsSession(b, (s) => (view = s));

  let view: SettingsState = $state(initialSettings);
  let lang: string = $state(getLanguage());
  const langKey = {
    system: "setting.language.system", nb: "setting.language.nb", en: "setting.language.en",
  } as const satisfies Record<LanguageChoice, string>;

  const cur = $derived(view.settings);
  const announce = $derived(view.saved ? $tr("settings.saved") : view.saveFailed ? $tr("settings.error") : "");

  function pick(c: LanguageChoice): void {
    lang = c;
    setLanguage(c);
  }
  onMount(() => void session.load());
  onDestroy(() => session.dispose());
</script>

<section class="settings">
  <h1 tabindex="-1">{$tr("settings.title")}</h1>
  <LiveRegion message={announce} />

  <fieldset class="group">
    <legend>{$tr("setting.language")}</legend>
    {#each LANGUAGE_CHOICES as c (c)}
      <label><input type="radio" name="lang" checked={lang === c} onchange={() => pick(c)} /> {$tr(langKey[c])}</label>
    {/each}
    <p class="muted">{$tr("settings.language.note")}</p>
  </fieldset>

  {#if view.phase === "loading"}
    <p role="status">{$tr("common.loading")}</p>
  {:else if view.phase === "error"}
    <p role="alert">{$tr("settings.error")}</p>
    <Button onclick={() => { session.send({ type: "reload" }); void session.load(); }}>{$tr("common.retry")}</Button>
  {:else if cur}
    <div class="group">
      <label class="field" for="days">{$tr("setting.trash.days")}</label>
      <select id="days" disabled={view.saving} value={cur.trashDays} onchange={(e) => void session.setTrashDays(Number(e.currentTarget.value))}>
        {#each trashDayOptions(cur.trashDays) as d (d)}
          <option value={d} selected={d === cur.trashDays}>{$tr("setting.trash.days.value", { days: d })}</option>
        {/each}
      </select>
      <p class="muted">{$tr("settings.trash.hint")}</p>
      {#if view.saved}<p role="status">{$tr("settings.saved")}</p>{/if}
      {#if view.saveFailed}<p role="alert">{$tr("settings.error")}</p>{/if}
    </div>

    <div class="group">
      <h2>{$tr("settings.quarantine.title")}</h2>
      <p>{$tr("settings.quarantine.body")}</p>
      {#if cur.quarantineDir}
        <p class="path">{$tr("settings.quarantine.path", { path: cur.quarantineDir })}</p>
      {:else}
        <p class="muted">{$tr("settings.quarantine.none")}</p>
      {/if}
    </div>

    <div class="group">
      <h2>{$tr("settings.empty.title")}</h2>
      <p>{$tr("settings.empty.default")}</p>
    </div>
  {/if}
</section>

<style>
  .settings { max-inline-size: 40rem; }
  h1 { margin-block: 0 var(--space-4); font-size: var(--fs-xl); line-height: var(--lh-tight); }
  h1:focus { outline: none; }
  h2 { margin-block: 0 var(--space-2); font-size: var(--fs-lg); }
  .group { margin: 0 0 var(--space-5); padding: 0; border: 0; display: grid; gap: var(--space-2); }
  legend { padding: 0; font-size: var(--fs-lg); font-weight: var(--fw-semibold); margin-block-end: var(--space-2); }
  label { display: flex; align-items: center; gap: var(--space-2); min-block-size: var(--target-min); }
  .field { font-weight: var(--fw-semibold); }
  input[type="radio"] { inline-size: 1.25rem; block-size: 1.25rem; flex: none; }
  select { min-block-size: var(--target-min); padding: 0 var(--space-3); font: inherit; color: var(--c-text); background: var(--c-surface); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-md); max-inline-size: 16rem; }
  p { margin: 0; }
  .muted { color: var(--c-text-muted); }
  .path { overflow-wrap: anywhere; font-family: var(--font-mono, monospace); }
</style>
