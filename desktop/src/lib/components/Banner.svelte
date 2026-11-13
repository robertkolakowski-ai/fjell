<!-- Banner: melding øverst i arbeidsflaten (info, advarsel, fare, OK). Ikon + tekst + kant, aldri bare farge.
     Brukes for skymappe-advarsel, utløpt papirkurv og oppdatering. `role` bestemmer annonsering: «status» for ting som dukker opp,
     «alert» bare for feil, «group» (standard med tittel) for statiske bannere som er der fra skjermen åpnes. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    tone?: "info" | "warn" | "danger" | "ok";
    /** Fet overskrift. Utelatt: bare brødtekst. */
    title?: string;
    headingLevel?: 2 | 3;
    /** id på selve banneret og på overskriften (for aria-describedby / e2e). */
    id?: string;
    titleId?: string;
    role?: "status" | "alert" | "group";
    children?: Snippet;
    /** Handlinger/lenker til høyre (bryter under ved smal bredde). */
    actions?: Snippet;
  }
  let { tone = "info", title, headingLevel = 2, id, titleId, role, children, actions }: Props = $props();
  const r = $derived(role ?? (title ? "group" : undefined));
  const tag = $derived(`h${headingLevel}`);
  const icon = $derived(({ info: "info", warn: "warn", danger: "danger", ok: "ok" } as const)[tone]);
</script>

<div class="banner {tone}" {id} role={r} aria-labelledby={r === "group" && title ? titleId : undefined}>
  <span class="ico"><Icon name={icon} /></span>
  <div class="body">
    {#if title}<svelte:element this={tag} id={titleId} class="title">{title}</svelte:element>{/if}
    {@render children?.()}
  </div>
  {#if actions}<div class="act">{@render actions()}</div>{/if}
</div>

<style>
  .banner {
    display: flex; flex-wrap: wrap; align-items: flex-start; gap: var(--space-2) var(--space-3);
    margin-block-end: var(--space-4); padding: var(--space-3) var(--space-4);
    background: var(--c-info-bg); color: var(--c-text);
    border: var(--border-width) solid var(--c-border-strong); border-inline-start-width: 4px; border-radius: var(--radius-md);
  }
  .warn { background: var(--c-badge-warn-bg); color: var(--c-badge-warn-text); }
  .danger { background: var(--c-danger-bg); }
  .ok { background: var(--c-ok-bg); }
  .ico { display: inline-flex; padding-block-start: var(--space-0); }
  .danger .ico { color: var(--c-danger); }
  .ok .ico { color: var(--c-ok); }
  .body { flex: 1 1 16rem; min-inline-size: 0; }
  .body :global(p) { margin: 0 0 var(--space-2); overflow-wrap: anywhere; }
  .body :global(p:last-child) { margin-block-end: 0; }
  .body :global(.muted) { color: inherit; opacity: 1; }
  .title { margin: 0 0 var(--space-1); font-size: var(--fs-md); font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .danger .title { color: var(--c-danger); }
  .act { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2); }
  .banner :global(a) { color: inherit; }
</style>
