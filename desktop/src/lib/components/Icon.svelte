<!-- Dekorative ikoner (aria-hidden). Meningen står alltid i tekst ved siden av. Ingen eksterne ikonfonter. -->
<script module lang="ts">
  export type IconName = "home" | "review" | "trash" | "settings" | "keyboard" | "info" | "warn" | "ok" | "danger" | "check" | "chevron" | "logo";
</script>

<script lang="ts">
  interface Props { name: IconName }
  let { name }: Props = $props();
</script>

<svg class="icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
  {#if name === "home"}<path d="M4 11.5 12 4l8 7.5V20H4z" /><path d="M10 20v-5h4v5" />
  {:else if name === "review"}<rect x="4" y="4" width="7" height="7" rx="1.5" /><rect x="13" y="4" width="7" height="7" rx="1.5" /><rect x="4" y="13" width="7" height="7" rx="1.5" /><rect x="13" y="13" width="7" height="7" rx="1.5" />
  {:else if name === "trash"}<path d="M5 7h14M10 7V4.5h4V7M7 7l1 13h8l1-13" /><path d="M10 11v6M14 11v6" />
  {:else if name === "settings"}<path d="M4 7h9M17 7h3M4 17h3M11 17h9" /><circle cx="15" cy="7" r="2" /><circle cx="9" cy="17" r="2" />
  {:else if name === "keyboard"}<rect x="3" y="6" width="18" height="12" rx="2" /><path d="M7 10h.01M11 10h.01M15 10h.01M7 14h10" />
  {:else if name === "info"}<circle cx="12" cy="12" r="9" /><path d="M12 11v5M12 7.6v.4" />
  {:else if name === "warn"}<path d="M12 3.5 21.5 20h-19z" /><path d="M12 10v4.5M12 17v.4" />
  {:else if name === "ok"}<circle cx="12" cy="12" r="9" /><path d="M8 12.5l2.8 2.8L16 9.6" />
  {:else if name === "danger"}<circle cx="12" cy="12" r="9" /><path d="M9 9l6 6M15 9l-6 6" />
  {:else if name === "check"}<path d="M5 12.5l4.5 4.5L19 7.5" />
  {:else if name === "chevron"}<path d="M9 6l6 6-6 6" />
  {:else if name === "logo"}<path d="M2.5 19 9 8l4 6 2.5-3.5L21.5 19z" fill="currentColor" stroke="none" />
  {/if}
</svg>

<style>
  .icon { inline-size: 1.25em; block-size: 1.25em; flex: none; overflow: visible; }
</style>
