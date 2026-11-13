<!-- Toast med Angre. Forsvinner ALDRI av seg selv (PLAN-UX-I18N 6.1 «Tidsbegrensning»): ligger til neste handling
     eller til brukeren lukker. Angre er en ekte knapp. role="status" annonserer teksten uten fokusflytting. -->
<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    message: string;
    undoLabel: string;
    onUndo: () => void;
    /** Utelatt: ingen lukkeknapp (toasten byttes ut av neste handling). */
    dismissLabel?: string;
    onDismiss?: () => void;
  }
  let { message, undoLabel, onUndo, dismissLabel, onDismiss }: Props = $props();

  // Narrator og NVDA annonserer ikke pålitelig en live-region som settes inn i DOM sammen med teksten.
  // Regionen finnes derfor først (tom), og teksten kommer ~100 ms senere. Knappene er med fra start.
  let ready = $state(false);
  onMount(() => {
    const id = setTimeout(() => (ready = true), 100);
    return () => clearTimeout(id);
  });
</script>

<div class="toast" role="status" aria-live="polite">
  {#if ready}<p class="msg">{message}</p>{:else}<p class="msg" aria-hidden="true">&nbsp;</p>{/if}
  <div class="actions">
    <button type="button" class="act undo" onclick={onUndo}>{undoLabel}</button>
    {#if dismissLabel}<button type="button" class="act" onclick={onDismiss}>{dismissLabel}</button>{/if}
  </div>
</div>

<style>
  .toast {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-4);
    max-inline-size: min(100%, 36rem);
    padding: var(--space-3) var(--space-4);
    border: var(--border-width) solid var(--c-inverse-text);
    border-radius: var(--radius-lg);
    background: var(--c-inverse-bg);
    color: var(--c-inverse-text);
    box-shadow: var(--shadow-pop);
    animation: in var(--dur-base) ease-out;
  }
  .msg { margin: 0; flex: 1 1 12rem; min-inline-size: 0; }
  .actions { display: flex; flex-wrap: wrap; gap: var(--space-2); }
  .act {
    min-block-size: var(--target-min);
    min-inline-size: var(--target-min);
    padding: var(--space-2) var(--space-3);
    border: 0;
    background: transparent;
    color: var(--c-inverse-action);
    font: inherit;
    font-weight: var(--fw-semibold);
    text-decoration: underline;
    cursor: pointer;
  }
  .act:focus-visible { outline-color: var(--c-inverse-action); }
  @keyframes in { from { opacity: 0; transform: translateY(0.5rem); } to { opacity: 1; transform: none; } }
  @media (forced-colors: active) {
    .toast { border-width: 2px; }
    .act { color: LinkText; }
  }
</style>
