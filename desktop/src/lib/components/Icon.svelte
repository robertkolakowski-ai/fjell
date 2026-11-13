<!-- Dekorative ikoner (aria-hidden). Meningen står alltid i tekst ved siden av. Ingen eksterne ikonfonter. -->
<script module lang="ts">
  export type IconName = "home" | "review" | "trash" | "settings" | "keyboard" | "info" | "warn" | "ok" | "danger" | "check" | "chevron" | "logo"
    | "duplicate" | "similar" | "blurry" | "screenshot" | "best" | "restore" | "cloud-off" | "folder" | "zoom" | "compare" | "image" | "undo" | "plus";
</script>

<script lang="ts">
  /** `filled` = fylt variant av samme ikon (aktiv navigasjon): flaten fylles svakt, strekene beholdes. Bare for lukkede former. */
  interface Props { name: IconName; filled?: boolean }
  let { name, filled = false }: Props = $props();
</script>

<!-- 24-rutenett med strek 1.8 ≙ 1.5 px ved 20 px (V2 2.6). Runde ender og hjørner, currentColor, ingen flerfarget. -->
<svg class="icon" class:filled viewBox="0 0 24 24" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
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
  {:else if name === "logo"}<rect x="9" y="3" width="12" height="12" rx="2" /><rect x="3" y="9" width="12" height="12" rx="2" /><path d="M3 17 15 10.07" />
  {:else if name === "duplicate"}<rect x="8" y="8" width="12" height="12" rx="2" /><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" />
  {:else if name === "similar"}<circle cx="9" cy="12" r="6" /><circle cx="15" cy="12" r="6" />
  {:else if name === "blurry"}<circle cx="12" cy="12" r="3" /><circle cx="12" cy="12" r="8" stroke-dasharray="2 3.2" />
  {:else if name === "screenshot"}<path d="M4 8V6a2 2 0 0 1 2-2h2M16 4h2a2 2 0 0 1 2 2v2M20 16v2a2 2 0 0 1-2 2h-2M8 20H6a2 2 0 0 1-2-2v-2" /><rect x="8" y="8" width="8" height="8" rx="1" />
  {:else if name === "best"}<circle cx="12" cy="12" r="9" /><path d="m12 7.5 1.4 3 3.3.4-2.4 2.3.6 3.3-2.9-1.6-2.9 1.6.6-3.3-2.4-2.3 3.3-.4z" />
  {:else if name === "restore"}<path d="M4 12a8 8 0 1 0 2.5-5.8" /><path d="M4 4v4.5h4.5" />
  {:else if name === "cloud-off"}<path d="M7 18h10a4 4 0 0 0 .6-7.95A5.5 5.5 0 0 0 7.2 8.6 4.7 4.7 0 0 0 7 18z" /><path d="M4 4l16 16" />
  {:else if name === "folder"}<path d="M3.5 7.5a2 2 0 0 1 2-2H10l2 2.5h6.5a2 2 0 0 1 2 2v7.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z" />
  {:else if name === "zoom"}<circle cx="10.5" cy="10.5" r="6" /><path d="m15 15 5 5M10.5 8v5M8 10.5h5" />
  {:else if name === "compare"}<rect x="3.5" y="5" width="7.5" height="14" rx="1.5" /><rect x="13" y="5" width="7.5" height="14" rx="1.5" />
  {:else if name === "image"}<rect x="3.5" y="5" width="17" height="14" rx="2" /><circle cx="9" cy="10" r="1.5" /><path d="m4 17 5-4.5 3.5 3 3-2.5 4.5 4" />
  {:else if name === "undo"}<path d="M9 7 4.5 11.5 9 16" /><path d="M5 11.5h9.5a4.5 4.5 0 0 1 0 9H11" />
  {:else if name === "plus"}<path d="M12 5v14M5 12h14" />
  {/if}
</svg>

<style>
  .icon { inline-size: 1.25em; block-size: 1.25em; flex: none; overflow: visible; }
  .icon.filled { fill: currentColor; fill-opacity: 0.16; }
</style>
