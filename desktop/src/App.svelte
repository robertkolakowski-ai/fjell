<!-- Appskall (M7): hash-ruting, navigasjon, bridge i kontekst. Skjermene i ./screens er stubber. All tekst via t()/$tr. -->
<script lang="ts">
  import { tick } from "svelte";
  import type { CoreBridge } from "./bridge/core-bridge";
  import { bridge as defaultBridge } from "./bridge/bridge";
  import { provideBridge } from "./context";
  import { tr } from "./lib/i18n";
  import { NAV_ROUTES, navFor, navHash, route, type NavRoute } from "./router";
  import Cards from "./screens/Cards.svelte";
  import Group from "./screens/Group.svelte";
  import Home from "./screens/Home.svelte";
  import Onboarding from "./screens/Onboarding.svelte";
  import Review from "./screens/Review.svelte";
  import Trash from "./screens/Trash.svelte";

  /** Overstyr bridge (tester). Standard: createBridge() i bridge/bridge.ts (Tauri hvis til stede, ellers Mock). */
  interface Props { bridge?: CoreBridge }
  let { bridge = defaultBridge }: Props = $props();
  // Bridge er fast for appens levetid; kontekst settes én gang.
  // svelte-ignore state_referenced_locally
  provideBridge(bridge);

  let main: HTMLElement | undefined = $state();
  const label: Record<NavRoute, "nav.home" | "nav.review" | "nav.trash" | "nav.settings"> = {
    home: "nav.home", review: "nav.review", trash: "nav.trash", settings: "nav.settings",
  };

  // Fokus til skjermtittel ved rutebytte (skjermleser annonserer ny skjerm); ikke ved første visning.
  let first = true;
  $effect(() => {
    void $route;
    if (first) { first = false; return; }
    void tick().then(() => main?.querySelector<HTMLElement>("h1")?.focus());
  });
  $effect(() => {
    document.title = `${$tr(label[navFor($route) ?? "home"])} · ${$tr("app.name")}`;
  });
</script>

<a class="skip" href="#main" onclick={(e) => { e.preventDefault(); main?.focus(); }}>{$tr("a11y.skip.main")}</a>

<header>
  <span class="brand">{$tr("app.name")}</span>
  <nav aria-label={$tr("nav.label")}>
    <ul>
      {#each NAV_ROUTES as n (n)}
        <li>
          <a href={navHash(n)} aria-current={navFor($route) === n ? "page" : undefined}>{$tr(label[n])}</a>
        </li>
      {/each}
    </ul>
  </nav>
</header>

<main id="main" tabindex="-1" bind:this={main}>
  {#if $route.name === "onboarding"}
    <Onboarding />
  {:else if $route.name === "home"}
    <Home />
  {:else if $route.name === "review"}
    <Review kind={$route.kind} />
  {:else if $route.name === "group"}
    <Group id={$route.id} />
  {:else if $route.name === "cards"}
    <Cards kind={$route.kind} />
  {:else if $route.name === "trash"}
    <Trash />
  {:else}
    <section><h1 tabindex="-1">{$tr("settings.title")}</h1></section>
  {/if}
</main>

<style>
  header { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-3) var(--space-5); margin-block-end: var(--space-5); }
  .brand { font-weight: var(--fw-semibold); font-size: var(--fs-lg); }
  ul { display: flex; flex-wrap: wrap; gap: var(--space-2); list-style: none; margin: 0; padding: 0; }
  nav a {
    display: inline-flex; align-items: center;
    min-block-size: var(--target-min); padding: 0 var(--space-3);
    color: var(--c-text); text-decoration: none; border-block-end: 3px solid transparent;
  }
  /* Aktiv side: understrek + vekt (aldri bare farge, og aldri aksentfarge: den er forbeholdt primærhandling). */
  nav a[aria-current="page"] { font-weight: var(--fw-semibold); border-block-end-color: var(--c-text); }
  nav a:hover { background: var(--c-surface-sunken); }
  main:focus { outline: none; }
  .skip { position: absolute; inset-inline-start: var(--space-2); inset-block-start: -10rem; background: var(--c-surface); padding: var(--space-2) var(--space-3); }
  .skip:focus { inset-block-start: var(--space-2); }
</style>
