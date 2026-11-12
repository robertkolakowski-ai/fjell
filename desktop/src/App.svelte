<!-- Enkel tilstandsside: viser svaret fra Rust-kommandoen get_state. Ingen UI-komponenter (M6). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { getState } from "./bridge/bridge";
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
  <h1>Fjell</h1>
  {#if error}
    <p role="alert">Feil: {error}</p>
  {:else if appState}
    <dl>
      <dt>Kilde</dt><dd>{appState.backend}</dd>
      <dt>Skanning</dt><dd>{appState.scanPhase}</dd>
      <dt>Kan frigjøres (byte)</dt><dd>{appState.reclaimableBytes}</dd>
      <dt>Nylig slettet</dt><dd>{appState.trashCount}</dd>
    </dl>
    <ul>
      {#each appState.perKind as k (k.kind)}
        <li>{k.kind}: {k.count}</li>
      {/each}
    </ul>
  {:else}
    <p>Laster …</p>
  {/if}
</main>
