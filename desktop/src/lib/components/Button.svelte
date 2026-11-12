<!-- Knapp. primary = eneste aksentfarge på skjermen (én primærhandling per skjerm). Vokser i høyde ved lang tekst. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    variant?: "primary" | "secondary";
    type?: "button" | "submit";
    disabled?: boolean;
    onclick?: (e: MouseEvent) => void;
    /** Synlig tekst er også tilgjengelig navn (WCAG 2.5.3). Sett kun aria-label hvis den inneholder den synlige teksten. */
    ariaLabel?: string;
    ariaKeyshortcuts?: string;
    children: Snippet;
  }
  let { variant = "secondary", type = "button", disabled = false, onclick, ariaLabel, ariaKeyshortcuts, children }: Props = $props();
</script>

<button class="btn {variant}" {type} {disabled} {onclick} aria-label={ariaLabel} aria-keyshortcuts={ariaKeyshortcuts}>
  {@render children()}
</button>

<style>
  .btn {
    min-block-size: var(--target-min);
    min-inline-size: var(--target-min);
    padding: var(--space-2) var(--space-4);
    border: var(--border-width) solid var(--c-border-strong);
    border-radius: var(--radius-md);
    font: inherit;
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
    text-align: center;
    cursor: pointer;
    background: var(--c-surface);
    color: var(--c-text);
    transition: background-color var(--dur-fast);
  }
  .btn.primary {
    background: var(--c-accent);
    color: var(--c-on-accent);
    border-color: var(--c-accent);
  }
  .btn.primary:hover:not(:disabled) { background: var(--c-accent-hover); border-color: var(--c-accent-hover); }
  .btn.secondary:hover:not(:disabled) { background: var(--c-surface-sunken); }
  .btn:disabled {
    cursor: not-allowed;
    opacity: 0.6; /* tekst + tilstand er også kodet via disabled-attributtet; deaktivert er unntatt fra kontrastkrav */
    border-style: dashed;
  }
  @media (forced-colors: active) {
    .btn { border-color: ButtonText; }
    .btn.primary { background: ButtonFace; border-width: 3px; }
    .btn:disabled { color: GrayText; border-color: GrayText; opacity: 1; }
  }
</style>
