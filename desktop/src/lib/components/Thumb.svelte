<!-- Miniatyr: fyller foreldrens boks. Laster lazy (IntersectionObserver), avbryter hvis den forsvinner ut av visning,
     skeleton mens den lastes, og forklart feiltilstand per kode (ikon + tekst, aldri farge alene).
     size=lg viser forklaringen som synlig tekst; size=sm (små rader) har ikon og forklaring som navn/tittel.
     Bildet er dekorativt (alt tomt) med mindre `alt` er gitt: raden/knappen rundt bærer navnet. -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { CoreBridge } from "../../bridge/core-bridge";
  import { isCancelled, thumbsFor, type ThumbHandle } from "../../bridge/thumbs";
  import type { Thumbnail } from "../../bridge/types";
  import { useBridge } from "../../context";
  import { tr } from "../i18n";

  interface Props {
    id: number;
    bridge?: CoreBridge;
    alt?: string;
    fit?: "cover" | "contain";
    size?: "lg" | "sm";
    /** Forklaringen tegnes som CSS-generert innhold (ingen tekstnode): for flis i en knapp som har eget navn med forklaringen. */
    generated?: boolean;
    /** Meldes når feilkode settes eller fjernes. */
    onerror?: (code: string | null) => void;
  }
  let { id, bridge, alt = "", fit = "cover", size = "lg", generated = false, onerror }: Props = $props();
  // svelte-ignore state_referenced_locally
  const store = thumbsFor(useBridge(bridge));

  type View = { k: "idle" } | { k: "loading" } | { k: "ready"; t: Thumbnail } | { k: "error"; code: string };
  let view: View = $state({ k: "idle" });
  let visible = $state(false);
  let host: HTMLElement | undefined = $state();

  const fromCache = (n: number): View | null => {
    const hit = store.peek(n);
    return hit ? ("ok" in hit ? { k: "ready", t: hit.ok } : { k: "error", code: hit.err.code }) : null;
  };

  onMount(() => {
    if (typeof IntersectionObserver === "undefined" || !host) { visible = true; return; }
    const io = new IntersectionObserver((es) => { for (const e of es) visible = e.isIntersecting; }, { rootMargin: "200px" });
    io.observe(host);
    return () => io.disconnect();
  });

  $effect(() => {
    const n = id;
    const v = visible;
    const cached = fromCache(n);
    if (cached) { view = cached; return; }
    if (!v) { view = { k: "idle" }; return; }
    view = { k: "loading" };
    let handle: ThumbHandle | null = store.load(n);
    handle.promise.then(
      (t) => { view = { k: "ready", t }; },
      (e) => { if (!isCancelled(e)) view = { k: "error", code: (e as { code: string }).code }; },
    );
    return () => { handle?.cancel(); handle = null; };
  });

  $effect(() => { onerror?.(view.k === "error" ? view.code : null); });

  const KNOWN = ["unavailable", "cloud_placeholder", "corrupt", "not_found", "no_library"];
  const explain = (code: string): string => $tr(`thumb.error.${KNOWN.includes(code) ? code : "unavailable"}` as "thumb.error.unavailable");
</script>

<span class="thumb" class:sm={size === "sm"} bind:this={host} data-state={view.k}>
  {#if view.k === "ready"}
    <img src={view.t.url} {alt} width={view.t.width} height={view.t.height} style:object-fit={fit} decoding="async" draggable="false" />
  {:else if view.k === "error"}
    <span class="err" data-code={view.code} role={size === "sm" ? "img" : undefined} aria-label={size === "sm" ? explain(view.code) : undefined} title={size === "sm" ? explain(view.code) : undefined}>
      <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3.5" y="5" width="17" height="14" rx="2" /><path d="m3.5 16 4.5-4.5 4 4 2.5-2.5 6 5.5M3 3l18 18" />
      </svg>
      {#if size === "lg"}<span class="txt" class:gen={generated} data-text={generated ? explain(view.code) : undefined}>{generated ? "" : explain(view.code)}</span>{/if}
    </span>
  {:else}
    <span class="skel" aria-hidden="true"></span>
  {/if}
</span>

<style>
  .thumb { position: relative; display: block; inline-size: 100%; block-size: 100%; min-block-size: 0; overflow: hidden; background: var(--c-surface-sunken); }
  img { display: block; inline-size: 100%; block-size: 100%; }
  .skel { display: block; inline-size: 100%; block-size: 100%; background: var(--c-surface-sunken); }
  .err { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--space-2); inline-size: 100%; block-size: 100%; padding: var(--space-2); box-sizing: border-box; text-align: center; color: var(--c-text); border: var(--border-width) dashed var(--c-border-strong); font-size: var(--fs-xs); line-height: 1.3; overflow-wrap: anywhere; }
  .txt.gen::after { content: attr(data-text); }
  .err svg { inline-size: 1.75rem; block-size: 1.75rem; flex: none; }
  .sm .err { padding: 0; border-width: 1px; }
  .sm .err svg { inline-size: 1.125rem; block-size: 1.125rem; }
  @media (prefers-reduced-motion: no-preference) {
    .skel { background: linear-gradient(90deg, var(--c-surface-sunken) 0%, var(--c-border-subtle) 50%, var(--c-surface-sunken) 100%); background-size: 200% 100%; animation: shimmer 1.4s linear infinite; }
  }
  @keyframes shimmer { from { background-position: 200% 0; } to { background-position: -200% 0; } }
  @media (forced-colors: active) {
    .thumb, .skel { background: Canvas; }
    .skel { border: 1px solid CanvasText; box-sizing: border-box; }
    .err { border-color: CanvasText; color: CanvasText; }
  }
</style>
