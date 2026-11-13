<!-- Toast med Angre. Forsvinner ALDRI av seg selv (PLAN-UX-I18N 6.1 «Tidsbegrensning»): ligger til neste handling
     eller til brukeren lukker. Angre er en ekte knapp. role="status" annonserer teksten uten fokusflytting. -->
<script lang="ts">
  import { onMount, type Snippet } from "svelte";

  interface Props {
    message: string;
    undoLabel: string;
    onUndo: () => void;
    /** Utelatt: ingen lukkeknapp (toasten byttes ut av neste handling). */
    dismissLabel?: string;
    onDismiss?: () => void;
    /** Valgfri miniatyr av det forrige bildet (dekorativ; teksten bærer meningen). */
    thumb?: Snippet;
  }
  let { message, undoLabel, onUndo, dismissLabel, onDismiss, thumb }: Props = $props();

  // Narrator og NVDA annonserer ikke pålitelig en live-region som settes inn i DOM sammen med teksten.
  // Regionen finnes derfor først (tom), og teksten kommer ~100 ms senere. Knappene er med fra start.
  let ready = $state(false);
  onMount(() => {
    const id = setTimeout(() => (ready = true), 100);
    return () => clearTimeout(id);
  });
</script>

<div class="toast" role="status" aria-live="polite">
  {#if thumb}<span class="thumb" aria-hidden="true">{@render thumb()}</span>{/if}
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
    border: var(--border-width) solid transparent; /* synlig i høykontrast */
    border-radius: var(--radius-lg);
    background: var(--c-inverse-bg);
    color: var(--c-inverse-text);
    box-shadow: var(--shadow-2);
    font-size: var(--fs-sm);
    animation: in var(--dur-2) var(--ease-out);
  }
  .thumb { flex: none; inline-size: 2.5rem; block-size: 2.5rem; overflow: hidden; border-radius: var(--radius-xs); background: var(--c-stage); }
  .thumb :global(img) { display: block; inline-size: 100%; block-size: 100%; object-fit: cover; }
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
    text-underline-offset: 0.2em;
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: background-color var(--dur-1) var(--ease-out);
  }
  .act:hover { background: color-mix(in srgb, var(--c-inverse-text) 12%, transparent); }
  .act:focus-visible { outline-color: var(--c-inverse-action); }
  @keyframes in { from { opacity: 0; transform: translateY(0.5rem); } to { opacity: 1; transform: none; } } /* glir 8 px opp */
  @media (forced-colors: active) {
    .toast { border: 2px solid CanvasText; }
    .act:hover { background: Canvas; outline: 1px solid LinkText; }
    .act { color: LinkText; }
  }
</style>
