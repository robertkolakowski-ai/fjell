<!-- Komponentgalleri (dev): nås med ?gallery i dev-serveren. Data fra MockBridge, all produkttekst via t().
     Kontrollene øverst (tema, språk, skala) er verktøy, ikke produkt-UI; derfor er denne filen unntatt hardkodet-tekst-lint. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { Badge, BottomBar, Button, Card, LiveRegion, PhotoTile, Ring, Toast } from "./lib/index.ts";
  import { fmt, getLanguage, reasonLabel, setLanguage, tr, type LocaleId } from "./lib/i18n";
  import { MockBridge } from "./bridge/mock-bridge";
  import { groupCards } from "./bridge/groups";
  import type { AppState, Card as BCard } from "./bridge/types";

  let theme = $state<"auto" | "light" | "dark" | "hc">("auto");
  let loc = $state<LocaleId>((getLanguage() === "system" ? "nb" : getLanguage()) as LocaleId);
  let scale = $state<100 | 200>(100);

  let appState = $state<AppState | null>(null);
  let tiles = $state<BCard[]>([]);
  let selected = $state<Record<number, boolean>>({});
  let toastOpen = $state(true);

  onMount(async () => {
    const bridge = new MockBridge();
    await bridge.startScan(false);
    appState = await bridge.getState();
    const page = await bridge.listCards("visual_dup", 6);
    const blurry = await bridge.listCards("blurry", 2);
    tiles = [...page.cards.slice(0, 4), ...blurry.cards];
    selected = { [tiles[1]?.id ?? 0]: true };
  });

  const snake = (s: string) => s.replace(/([a-z])([A-Z])/g, "$1_$2").toLowerCase();
  const pctOf = (c: BCard) => fmt.percent(c.confidence);
  const badgeFor = (c: BCard) => [{ label: reasonLabel(snake(c.reason), { pct: pctOf(c) }), tone: c.kind === "blurry" ? ("warn" as const) : ("neutral" as const) }];
  const count = $derived(tiles.filter((c) => selected[c.id]).length);
  const sumBytes = $derived(tiles.filter((c) => selected[c.id]).reduce((s, c) => s + c.bytes, 0));
  const groups = $derived(groupCards(tiles));
  const ringFraction = $derived(appState ? Math.min(1, appState.reclaimableBytes / (appState.reclaimableBytes + 4e7)) : 0);

  $effect(() => {
    setLanguage(loc);
  });
  $effect(() => {
    const root = document.documentElement;
    if (theme === "auto") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", theme);
    root.style.fontSize = `${scale}%`;
    return () => {
      root.removeAttribute("data-theme");
      root.style.fontSize = "";
    };
  });
</script>

<main>
  <h1>{$tr("app.name")}</h1>

  <form class="ctl" aria-label="Gallery" onsubmit={(e) => e.preventDefault()}>
    <label>Theme
      <select bind:value={theme}><option value="auto">auto</option><option value="light">light</option><option value="dark">dark</option><option value="hc">hc</option></select>
    </label>
    <label>Locale
      <select bind:value={loc}><option value="nb">nb</option><option value="en">en</option><option value="pseudo">pseudo</option></select>
    </label>
    <label>Scale
      <select bind:value={scale}><option value={100}>100 %</option><option value={200}>200 %</option></select>
    </label>
  </form>

  {#if appState}
    <div class="grid">
      <Card heading={$tr("home.title")}>
        <p>{$tr("consequence.frees_no_storage_yet")}</p>
        <p class="row"><Badge label={$tr("ui.badge.protected")} /><Badge label={reasonLabel("blurry", { pct: fmt.percent(0.82) })} tone="warn" /></p>
        <p class="row">
          <Button variant="primary">{$tr("home.quick.button")}</Button>
          <Button>{$tr("common.cancel")}</Button>
          <Button disabled>{$tr("common.continue")}</Button>
        </p>
      </Card>
      <Card>
        <Ring
          fraction={ringFraction}
          label={$tr("home.ring.label", { size: fmt.bytes(appState.reclaimableBytes), trash: fmt.bytes(appState.trashBytes) })}
          value={fmt.bytes(appState.reclaimableBytes)}
        />
      </Card>
    </div>

    <h2>{$tr("group.title")}{groups[0] ? ` ${groups[0].id}` : ""}</h2>
    <ul class="tiles">
      {#each tiles as c, i (c.id)}
        <li>
          <PhotoTile
            label={$tr("a11y.photo.label", { n: i + 1, total: tiles.length, date: fmt.date(new Date(2025, 6, 14)), size: fmt.bytes(c.bytes) })}
            selected={!!selected[c.id]}
            onToggle={(v) => (selected[c.id] = v)}
            bestLabel={i === 0 ? $tr("group.best") : undefined}
            badges={badgeFor(c)}
          />
        </li>
      {/each}
    </ul>

    {#if toastOpen}
      <Toast
        message={$tr("group.committed", { count: 87 }) + " " + $tr("consequence.frees_no_storage_yet")}
        undoLabel={$tr("common.undo")}
        onUndo={() => (toastOpen = false)}
        dismissLabel={$tr("common.dismiss")}
        onDismiss={() => (toastOpen = false)}
      />
    {/if}

    <LiveRegion message="" />
    <BottomBar label={$tr("review.bar.trash", { count, size: fmt.bytes(sumBytes) })} disabled={count === 0}>
      {#snippet secondary()}<Button onclick={() => (selected = {})}>{$tr("group.keep.all")}</Button>{/snippet}
    </BottomBar>
  {:else}
    <p>{$tr("common.loading")}</p>
  {/if}
</main>

<style>
  main { max-inline-size: 60rem; margin-inline: auto; padding: var(--space-4); display: grid; gap: var(--space-4); }
  h1, h2 { margin: 0; }
  h1 { font-size: var(--fs-xl); }
  h2 { font-size: var(--fs-lg); }
  .ctl { display: flex; flex-wrap: wrap; gap: var(--space-4); }
  select { font: inherit; min-block-size: var(--target-min); margin-inline-start: var(--space-2); background: var(--c-surface); color: var(--c-text); border: var(--border-width) solid var(--c-border-strong); border-radius: var(--radius-sm); }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 18rem), 1fr)); gap: var(--space-4); align-items: start; }
  .row { display: flex; flex-wrap: wrap; gap: var(--space-2); }
  .tiles { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 11rem), 1fr)); gap: var(--space-3); }
</style>
