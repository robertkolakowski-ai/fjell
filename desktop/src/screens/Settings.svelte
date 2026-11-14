<!-- Innstillinger (UX-5/6): språk (nb, en eller følg Windows), papirkurvfrist (liste), karantenemappe (info) og standard for
     «Tøm» (info). Ingen skyvebrytere (unntak: «Sjekk automatisk ved oppstart» i Oppdateringer), ingen lyd- eller varselvalg (ikke bygget). Tilstand i settingsState.ts, flyt i settingsFlow.ts.
     Språkvalget lagres i nettleserlagring av i18n-runtime (ikke i skallet) inntil skallet får egen innstillingslagring. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import { useBridge } from "../context";
  import { Banner, Button, Card, LiveRegion, PageHeader, SegmentedControl } from "../lib";
  import { getLanguage, setLanguage, tr } from "../lib/i18n";
  import UpdateSection from "./UpdateSection.svelte";
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

  const langOptions = $derived(LANGUAGE_CHOICES.map((c) => ({ value: c, label: $tr(langKey[c]) })));
  const daysOptions = $derived(cur ? trashDayOptions(cur.trashDays).map((d) => ({ value: String(d), label: $tr("setting.trash.days.value", { days: d }) })) : []);

  function pick(v: string): void {
    const c = v as LanguageChoice;
    lang = c;
    setLanguage(c);
  }
  onMount(() => void session.load());
  onDestroy(() => session.dispose());
</script>

<section class="settings">
  <PageHeader title={$tr("settings.title")} />
  <LiveRegion message={announce} />

  {#if view.phase === "loading"}
    <p role="status" class="state">{$tr("common.loading")}</p>
  {:else if view.phase === "error"}
    <Banner tone="danger" role="alert">
      {$tr("settings.error")}
      {#snippet actions()}
        <Button onclick={() => { session.send({ type: "reload" }); void session.load(); }}>{$tr("common.retry")}</Button>
      {/snippet}
    </Banner>
  {/if}

  <div class="grid">
    <Card heading={$tr("setting.language")}>
      <div class="body">
        <SegmentedControl name="lang" options={langOptions} value={lang} onchange={pick} ariaLabel={$tr("setting.language")} ariaDescribedby="lang-note" />
        <p id="lang-note" class="muted">{$tr("settings.language.note")}</p>
      </div>
    </Card>

    {#if cur}
      <Card heading={$tr("setting.trash.days")}>
        <div class="body">
          <SegmentedControl name="days" options={daysOptions} value={String(cur.trashDays)} disabled={view.saving} ariaLabel={$tr("setting.trash.days")} ariaDescribedby="days-note"
            onchange={(v) => void session.setTrashDays(Number(v))} />
          <p id="days-note" class="muted">{$tr("settings.trash.hint")}</p>
          {#if view.saved}<p role="status">{$tr("settings.saved")}</p>{/if}
          {#if view.saveFailed}<p role="alert">{$tr("settings.error")}</p>{/if}
        </div>
      </Card>

      <Card heading={$tr("settings.quarantine.title")}>
        <div class="body">
          <p>{$tr("settings.quarantine.body")}</p>
          {#if cur.quarantineDir}
            <p class="path">{$tr("settings.quarantine.path", { path: cur.quarantineDir })}</p>
          {:else}
            <p class="muted">{$tr("settings.quarantine.none")}</p>
          {/if}
        </div>
      </Card>

      <Card heading={$tr("settings.empty.title")}>
        <div class="body"><p>{$tr("settings.empty.default")}</p></div>
      </Card>
    {/if}

    <div class="wide"><UpdateSection bridge={b} /></div>
  </div>
</section>

<style>
  .settings { max-inline-size: var(--w-content); }
  .grid { display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--space-4); align-items: start; }
  .wide { min-inline-size: 0; }
  @media (min-width: 68.75rem) {
    .grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .wide { grid-column: 1 / -1; }
  }
  .body { display: grid; gap: var(--space-2); justify-items: start; }
  .body p { margin: 0; }
  .state { margin: 0 0 var(--space-4); }
  .muted { color: var(--c-text-muted); font-size: var(--fs-sm); }
  .path { overflow-wrap: anywhere; font-family: var(--font-mono); font-size: var(--fs-sm); padding: var(--space-2) var(--space-3); background: var(--c-surface-sunken); border-radius: var(--radius-sm); max-inline-size: 100%; }
</style>
