<!-- Oversikt (dashboard) på Hjem. Viser tall fra Dashboard-bridgen: brukt plass, kan ryddes per forslagstype, papirkurv, «Tømt»,
     fremdrift (ring med tekstekvivalent), «Neste beste grep», tømt per måned (tabell), største mapper og små tellere.
     Panelet foreslår bare: knapper og lenker åpner andre skjermer, ingenting flyttes, slettes eller tømmes herfra.
     «Tømt» er det Fjell har tømt (standard «Tøm» går til Windows Papirkurv): aldri påstand om frigjort diskplass.
     Tilstand i dashboardState.ts, henting i dashboardFlow.ts. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import type { KindCode } from "../bridge/types";
  import type { Dashboard } from "../bridge/types-v1";
  import { useBridge } from "../context";
  import { Button, Card, EmptyState, Ring, StatTile } from "../lib";
  import { fmt, getIntlLocale, tr as tx } from "../lib/i18n";
  import { formatHash, navigate } from "../router";
  import { DashboardSession } from "./dashboardFlow";
  import { cleanable, freedTotals, initialDashboard, kindTiles, monthDate, nextTarget, progressFraction, type DashboardView } from "./dashboardState";

  interface Props {
    bridge?: CoreBridge;
  }
  let { bridge }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);

  let view: DashboardView = $state(initialDashboard);
  const session = new DashboardSession(b, (s) => (view = s));
  onMount(() => void session.load());
  onDestroy(() => session.dispose());

  const catKey = {
    exact_dup: "home.cat.exact_dup", visual_dup: "home.cat.visual_dup",
    blurry: "home.cat.blurry", screenshot: "home.cat.screenshot", series: "home.cat.series", rule: "home.cat.rule",
  } as const satisfies Record<KindCode, string>;

  const data: Dashboard | null = $derived(view.kind === "ready" || view.kind === "empty" ? view.data : null);
  const monthName = $derived.by(() => {
    void $tx; // formatteres på nytt når språket byttes
    const f = new Intl.DateTimeFormat(getIntlLocale(), { month: "long", year: "numeric", timeZone: "UTC" });
    return (ym: number) => f.format(monthDate(ym));
  });

  function nextText(d: Dashboard): string {
    const n = d.nextStep;
    if (n.kind === "review") {
      const t = nextTarget(d);
      const kind = t && t.to === "review" && t.kind ? $tx(catKey[t.kind]) : $tx("ui.card.suggestion");
      return $tx("dashboard.next.review", { kind, count: n.count, size: fmt.bytes(n.bytes) });
    }
    if (n.kind === "empty_trash") return $tx("dashboard.next.empty_trash", { count: n.count, size: fmt.bytes(n.bytes) });
    return $tx("dashboard.next.nothing");
  }
  function goNext(d: Dashboard): void {
    const t = nextTarget(d);
    if (!t) return;
    navigate(t.to === "trash" ? { name: "trash" } : { name: "review", kind: t.kind });
  }
</script>

<Card heading={$tx("dashboard.title.label")} headingLevel={2}>
  {#if view.kind === "loading"}
    <p class="muted" role="status">{$tx("common.loading")}</p>
  {:else if view.kind === "error"}
    <p role="alert">{$tx("dashboard.error.body")}</p>
    <Button onclick={() => void session.retry()}>{$tx("common.retry")}</Button>
  {:else if view.kind === "empty"}
    <EmptyState title={$tx("dashboard.empty.title")} description={$tx("dashboard.empty.body")} headingLevel={3} />
  {:else if data}
    {@const d = data}
    {@const can = cleanable(d)}
    {@const freed = freedTotals(d.freedByMonth)}
    <div class="top">
      <div class="progress">
        <h3 class="sub">{$tx("dashboard.progress.title")}</h3>
        <Ring
          size="sm"
          fraction={progressFraction(d)}
          value={fmt.percent(progressFraction(d))}
          label={$tx("dashboard.progress.label", { done: fmt.number(d.progress.decided), total: fmt.number(d.progress.total) })}
        />
        <p class="muted center">{$tx("dashboard.progress.label", { done: fmt.number(d.progress.decided), total: fmt.number(d.progress.total) })}</p>
      </div>
      <div class="next">
        <h3 class="sub">{$tx("dashboard.next.label")}</h3>
        <p class="next-text">{nextText(d)}</p>
        {#if d.nextStep.kind !== "nothing"}
          <div class="next-act">
            <Button onclick={() => goNext(d)}>{$tx(d.nextStep.kind === "review" ? "dashboard.next.review.action" : "dashboard.next.empty_trash.action")}</Button>
          </div>
        {/if}
      </div>
    </div>

    <ul class="tiles">
      <li><StatTile label={$tx("dashboard.used.label")} value={fmt.bytes(d.activeBytes)} hint={$tx("common.count.photos", { count: d.activeCount })} /></li>
      <li><StatTile label={$tx("dashboard.cleanable.label")} value={fmt.bytes(can.bytes)} hint={$tx("common.count.photos", { count: can.count })} href={formatHash({ name: "review", kind: null })} /></li>
      <li><StatTile label={$tx("dashboard.trash.label")} value={fmt.bytes(d.trashedBytes)} hint={$tx("common.count.photos", { count: d.trashedCount })} href={formatHash({ name: "trash" })} /></li>
      <li><StatTile label={$tx("dashboard.freed.label")} value={fmt.bytes(d.freedBytes)} hint={d.freedBytes > 0 ? $tx("common.count.photos", { count: freed.count }) : undefined} /></li>
    </ul>
    <p class="muted note">{d.freedBytes > 0 ? $tx("dashboard.freed.note") : $tx("dashboard.freed.empty")}</p>

    <h3 class="sub">{$tx("dashboard.kinds.label")}</h3>
    <ul class="tiles">
      {#each kindTiles(d) as k (k.kind)}
        <li><StatTile label={$tx(catKey[k.kind])} value={fmt.bytes(k.bytes)} hint={$tx("common.count.photos", { count: k.count })} href={formatHash({ name: "review", kind: k.kind })} /></li>
      {/each}
    </ul>

    <h3 class="sub">{$tx("dashboard.counters.label")}</h3>
    <ul class="tiles small">
      <li><StatTile label={$tx("protect.title.label")} value={fmt.number(d.protectedCount)} href={formatHash({ name: "settings" })} /></li>
      <li><StatTile label={$tx("series.title.label")} value={fmt.number(d.seriesCount)} href={formatHash({ name: "review", kind: "series" })} /></li>
      <li><StatTile label={$tx("people.title.label")} value={fmt.number(d.peopleCount)} href={formatHash({ name: "people" })} /></li>
      <li><StatTile label={$tx("rules.title.label")} value={fmt.number(d.rulesActive)} href={formatHash({ name: "rules" })} /></li>
    </ul>

    <div class="lists">
      {#if d.freedByMonth.length > 0}
      <div class="block">
        <h3 class="sub">{$tx("dashboard.freedmonths.label")}</h3>
          <table>
            <thead>
              <tr>
                <th scope="col">{$tx("dashboard.col.month")}</th>
                <th scope="col" class="num">{$tx("dashboard.col.count")}</th>
                <th scope="col" class="num">{$tx("dashboard.col.size")}</th>
              </tr>
            </thead>
            <tbody>
              {#each d.freedByMonth as m (m.yearMonth)}
                <tr>
                  <th scope="row">{monthName(m.yearMonth)}</th>
                  <td class="num">{fmt.number(m.count)}</td>
                  <td class="num">{fmt.bytes(m.bytes)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
      </div>
      {/if}
      <div class="block">
        <h3 class="sub">{$tx("dashboard.folders.label")}</h3>
        <ul class="folders">
          {#each d.topFolders as f (f.path)}
            <li>
              <span class="path">{f.path}</span>
              <span class="muted">{$tx("home.cat.summary", { count: f.count, size: fmt.bytes(f.bytes) })}</span>
            </li>
          {/each}
        </ul>
      </div>
    </div>
  {/if}
</Card>

<style>
  p { margin-block: 0 var(--space-3); }
  .muted { color: var(--c-text-muted); font-size: var(--fs-sm); }
  .center { text-align: center; }
  .sub { margin: var(--space-5) 0 var(--space-3); font-size: var(--fs-md); font-weight: var(--fw-semibold); }
  .top { display: flex; flex-wrap: wrap; gap: var(--space-4) var(--space-6); align-items: flex-start; }
  .top .sub { margin-block-start: 0; }
  .progress { display: flex; flex-direction: column; align-items: center; inline-size: 9rem; max-inline-size: 100%; }
  .progress .sub { align-self: flex-start; }
  .next { flex: 1 1 14rem; min-inline-size: 0; }
  .next-text { font-size: var(--fs-lg); font-variant-numeric: var(--num); }
  .next-act { margin-block-start: var(--space-3); }
  ul { list-style: none; margin: 0; padding: 0; }
  .tiles { display: grid; gap: var(--space-3); grid-template-columns: repeat(auto-fit, minmax(min(100%, 11rem), 1fr)); margin-block-start: var(--space-4); }
  .tiles.small { grid-template-columns: repeat(auto-fit, minmax(min(100%, 8rem), 1fr)); margin-block-start: 0; }
  .sub + .tiles { margin-block-start: 0; }
  .note { margin-block: var(--space-3) 0; }
  .lists { display: grid; gap: var(--space-2) var(--space-6); grid-template-columns: repeat(auto-fit, minmax(min(100%, 18rem), 1fr)); }
  .block { min-inline-size: 0; }
  table { inline-size: 100%; border-collapse: collapse; font-variant-numeric: var(--num); }
  th, td { padding: var(--space-2) var(--space-2); border-block-end: var(--border-width) solid var(--c-border-subtle); text-align: start; font-weight: var(--fw-regular); }
  thead th { font-weight: var(--fw-medium); font-size: var(--fs-sm); color: var(--c-text-muted); }
  .num { text-align: end; }
  .folders li { display: flex; flex-direction: column; padding-block: var(--space-2); border-block-end: var(--border-width) solid var(--c-border-subtle); min-inline-size: 0; }
  .folders .muted { margin: 0; font-variant-numeric: var(--num); }
  .path { overflow-wrap: anywhere; }
</style>
