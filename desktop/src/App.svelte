<!-- Enkel tilstandsside: viser svaret fra Rust-kommandoen get_state. Ingen UI-komponenter (M6). All tekst via t() (tools/gen-i18n lint). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { getState } from "./bridge/bridge";
  import { t } from "./lib/i18n";
  import type { AppState, BridgeError } from "./bridge/types";

  let appState: AppState | null = $state(null);
  let error: string | null = $state(null);

  onMount(async () => {
    try {
      appState = await getState();
    } catch (e) {
      const be = e as Partial<BridgeError>;
      error = be.code ?? String(e);
    }
  });
</script>

<main>
  <h1>{t("app.name")}</h1>
  {#if error}
    <p role="alert">{t("err.generic")} ({error})</p>
  {:else if appState}
    <dl>
      <dt>{t("dev.state.backend")}</dt><dd>{appState.backend}</dd>
      <dt>{t("dev.state.scan")}</dt><dd>{appState.scanPhase}</dd>
      <dt>{t("dev.state.reclaimable")}</dt><dd>{appState.reclaimableBytes}</dd>
      <dt>{t("dev.state.trash")}</dt><dd>{appState.trashCount}</dd>
    </dl>
    <ul>
      {#each appState.perKind as k (k.kind)}
        <li>{k.kind}: {k.count}</li>
      {/each}
    </ul>
  {:else}
    <p>{t("common.loading")}</p>
  {/if}
</main>
