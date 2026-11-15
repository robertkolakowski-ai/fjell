<!-- Appskall (M7): hash-ruting, navigasjon, bridge i kontekst. All tekst via t()/$tr. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { CoreBridge } from "./bridge/core-bridge";
  import { bridge as defaultBridge } from "./bridge/bridge";
  import { provideBridge } from "./context";
  import { tr } from "./lib/i18n";
  import { Button, Icon, NavItem, Sidebar } from "./lib";
  import type { IconName } from "./lib/components/Icon.svelte";
  import { fmt } from "./lib/i18n";
  import type { AppState } from "./bridge/types";
  import { NAV_ROUTES, navFor, navHash, navigate, route, type NavRoute } from "./router";
  import Cards from "./screens/Cards.svelte";
  import Group from "./screens/Group.svelte";
  import Help from "./screens/Help.svelte";
  import { helpScreenOf, isHelpKey } from "./screens/helpKeys";
  import Home from "./screens/Home.svelte";
  import People from "./screens/People.svelte";
  import Onboarding from "./screens/Onboarding.svelte";
  import { shouldShowOnboarding } from "./screens/onboardingGate";
  import Review from "./screens/Review.svelte";
  import Rules from "./screens/Rules.svelte";
  import SeriesCompare from "./screens/SeriesCompare.svelte";
  import Settings from "./screens/Settings.svelte";
  import Trash from "./screens/Trash.svelte";
  import { runStartupCheck } from "./screens/updateFlow";

  /** Overstyr bridge (tester). Standard: createBridge() i bridge/bridge.ts (Tauri hvis til stede, ellers Mock). */
  interface Props { bridge?: CoreBridge }
  let { bridge = defaultBridge }: Props = $props();
  // Bridge er fast for appens levetid; kontekst settes én gang.
  // svelte-ignore state_referenced_locally
  provideBridge(bridge);

  let main: HTMLElement | undefined = $state();
  let helpOpen = $state(false);
  let counts = $state<AppState | null>(null);
  let version = $state("");
  let people = $state(0);
  let rulesActive = $state(0);

  // Tellere og versjon i sidemenyen. Hentes ved start, ved rutebytte og hvert 3. sekund mens vinduet er synlig (billig lesing).
  async function refreshCounts(): Promise<void> {
    try { counts = await bridge.getState(); } catch { /* tellere vises bare når de er kjent */ }
  }
  // Personer og aktive regler hentes ved start og rutebytte (ikke i 3-sekundersløkken; oversikten er dyrere enn getState).
  async function refreshV1Counts(): Promise<void> {
    try { const d = await bridge.getDashboard(); people = d.peopleCount; rulesActive = d.rulesActive; } catch { /* ingen teller */ }
  }
  onMount(() => {
    void refreshCounts();
    void bridge.getAppVersion().then((v) => (version = v)).catch(() => undefined);
    const timer = setInterval(() => { if (!document.hidden) void refreshCounts(); }, 3000);
    return () => clearInterval(timer);
  });
  const pending = $derived(counts ? counts.perKind.reduce((n, k) => n + k.count, 0) : 0);
  const countOf = (n: NavRoute): { count: string | null; label?: string } => {
    const v = n === "review" ? pending : n === "trash" ? (counts?.trashCount ?? 0) : n === "people" ? people : n === "rules" ? rulesActive : 0;
    const label = n === "people" ? $tr("common.count.people", { count: v }) : n === "rules" ? $tr("common.count.rules", { count: v }) : $tr("common.count.photos", { count: v });
    return v > 0 ? { count: fmt.number(v), label } : { count: null };
  };
  const iconOf = {
    home: "home", review: "review", people: "people", rules: "rules", trash: "trash", settings: "settings",
  } as const satisfies Record<NavRoute, IconName>;

  // Første gang: gå til onboarding fra Hjem. Senere starter (og avbrutt skann) går rett til Hjem.
  onMount(() => {
    void shouldShowOnboarding(bridge, $route).then((yes) => { if (yes) navigate({ name: "onboarding" }); });
  });
  // Stille oppdateringssjekk ved oppstart, bare når brukeren har slått på «Sjekk automatisk ved oppstart» (standard av). Installerer aldri.
  onMount(() => void runStartupCheck(bridge));
  // F1: hurtigtaster for gjeldende skjerm (kortmodus har egen dialog). Esc lukker via <dialog>.
  function windowKey(e: KeyboardEvent): void {
    if (isHelpKey(e, $route)) { e.preventDefault(); helpOpen = !helpOpen; }
  }
  const label: Record<NavRoute, "nav.home" | "nav.review" | "nav.people" | "nav.rules" | "nav.trash" | "nav.settings"> = {
    home: "nav.home", review: "nav.review", people: "nav.people", rules: "nav.rules", trash: "nav.trash", settings: "nav.settings",
  };

  // Fokus til skjermtittel ved rutebytte (skjermleser annonserer ny skjerm); ikke ved første visning.
  let first = true;
  $effect(() => {
    void $route;
    if (first) { first = false; return; }
    void tick().then(() => main?.querySelector<HTMLElement>("h1")?.focus());
  });
  $effect(() => {
    void $route;
    void refreshCounts();
    void refreshV1Counts();
  });
  $effect(() => {
    document.title = `${$tr(label[navFor($route) ?? "home"])} · ${$tr("app.name")}`;
  });
</script>

<svelte:window onkeydown={windowKey} />

<a class="skip" href="#main" onclick={(e) => { e.preventDefault(); main?.focus(); }}>{$tr("a11y.skip.main")}</a>

<Help open={helpOpen} screen={helpScreenOf($route)} onClose={() => (helpOpen = false)} />

<div class="app">
  <Sidebar brand={$tr("app.name")} navLabel={$tr("nav.label")}>
    {#each NAV_ROUTES as n (n)}
      {@const c = countOf(n)}
      <NavItem href={navHash(n)} label={$tr(label[n])} current={navFor($route) === n} count={c.count} countLabel={c.label}>
        {#snippet icon()}<Icon name={iconOf[n]} filled={navFor($route) === n} />{/snippet}
      </NavItem>
    {/each}
    {#snippet footer()}
      {#if $route.name !== "cards"}
        <Button variant="quiet" ariaKeyshortcuts="F1" onclick={() => (helpOpen = true)}><span class="kb"><Icon name="keyboard" />{$tr("common.shortcuts")}</span></Button>
      {/if}
      {#if version}<span class="ver">{$tr("update.version", { version })}</span>{/if}
    {/snippet}
  </Sidebar>

  <main id="main" tabindex="-1" bind:this={main}>
    {#if $route.name === "onboarding"}
      <Onboarding />
    {:else if $route.name === "home"}
      <Home />
    {:else if $route.name === "review"}
      <Review kind={$route.kind} />
    {:else if $route.name === "group"}
      <Group id={$route.id} />
    {:else if $route.name === "series"}
      <SeriesCompare id={$route.id} />
    {:else if $route.name === "cards"}
      {#key $route.kind}
        <Cards kind={$route.kind} />
      {/key}
    {:else if $route.name === "people"}
      <People />
    {:else if $route.name === "rules"}
      <Rules />
    {:else if $route.name === "trash"}
      <Trash />
    {:else}
      <Settings />
    {/if}
  </main>
</div>

<style>
  /* Appskall: ≥ 900 px sidemeny + arbeidsflate (rutenett med to kolonner), under 900 px toppmeny over arbeidsflaten. Se docs/design/LOFT.md. */
  .app { min-block-size: 100vh; background: var(--c-bg); display: grid; grid-template-columns: minmax(0, 1fr); grid-template-rows: auto minmax(0, 1fr); }
  main { min-inline-size: 0; padding: var(--space-5) var(--gutter) var(--space-6); }
  main:focus { outline: none; }
  .kb { display: inline-flex; align-items: center; gap: var(--space-2); }
  .ver { color: var(--c-text-muted); font-size: var(--fs-xs); font-variant-numeric: var(--num); padding-inline: var(--space-3); }
  .skip { border-radius: var(--radius-sm); box-shadow: var(--shadow-2); }
  @media (min-width: 56.25rem) {
    .app { grid-template-columns: var(--nav-w) minmax(0, 1fr); grid-template-rows: minmax(0, 1fr); }
    /* Sidemenyen er sticky og 100 vh; kolonnen under den males i menyens farge så den når hele dokumenthøyden (ingen lys stripe nederst). */
    .app { background: linear-gradient(to right, var(--c-nav-bg) calc(var(--nav-w) - var(--border-width)), var(--c-border-subtle) calc(var(--nav-w) - var(--border-width)), var(--c-border-subtle) var(--nav-w), var(--c-bg) var(--nav-w)); }
    main { padding-block-start: var(--space-6); }
  }
  .skip { position: absolute; z-index: 20; inset-inline-start: var(--space-2); inset-block-start: -10rem; background: var(--c-surface); padding: var(--space-2) var(--space-3); }
  .skip:focus { inset-block-start: var(--space-2); }
</style>
