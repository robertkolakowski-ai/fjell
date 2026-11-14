<!-- Innstillinger: «Beskyttelser». Liste over beskyttelser (bilde, mappe, album, person) med «Fjern beskyttelse» (bekreftelse, så
     «Angre»), og «Beskytt en mappe» med mappevalg etter mønsteret i FolderDialog (tekstfelt + «Bla …» via bridge.pickFolder; ingen ny
     filtilgang). Egen dialog fordi FolderDialog sier at en ny mappe erstatter biblioteket. Bildene røres aldri. Flyt i protectionsFlow.ts. -->
<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import type { CoreBridge } from "../bridge/core-bridge";
  import type { BridgeError } from "../bridge/types";
  import type { ScopeCode } from "../bridge/types-v1";
  import { useBridge } from "../context";
  import { Banner, Button, Card, LiveRegion, Toast } from "../lib";
  import Thumb from "../lib/components/Thumb.svelte";
  import { fmt, tr } from "../lib/i18n";
  import "./sheet.css";
  import { browseFolder } from "./homeFlow";
  import { describeTarget, initialProtections, ProtectionsSession, sameScope, type ProtectionsState } from "./protectionsFlow";
  import { errorKeyOf } from "./rulesState";

  interface Props {
    bridge?: CoreBridge;
  }
  let { bridge }: Props = $props();
  // svelte-ignore state_referenced_locally
  const b = useBridge(bridge);
  const session = new ProtectionsSession(b, (s) => (view = s));

  let view: ProtectionsState = $state(initialProtections);
  let host: HTMLElement | undefined = $state();
  let confirmDlg: HTMLDialogElement | undefined = $state();
  let folderDlg: HTMLDialogElement | undefined = $state();
  let folderOpen = $state(false);
  let path = $state("");
  let pathInput: HTMLInputElement | undefined = $state();
  let folderError: BridgeError | null = $state(null);
  let picking = $state(false);
  let browseFailed = $state(false);
  let busyAdd = $state(false);
  let folderNote = $state("");
  let trigger: HTMLElement | null = null;

  const scopeKey = { asset: "protect.scope.asset", folder: "protect.scope.folder", album: "protect.scope.album", person: "protect.scope.person" } as const satisfies Record<ScopeCode, string>;
  const pendingText = $derived(view.pending ? describeTarget(view.pending, $tr) : "");
  const note = $derived(view.note);
  const noteText = $derived(
    !note ? ""
    : note.kind === "added" ? $tr("settings.protections.added", { target: describeTarget(note.scope, $tr) })
    : note.kind === "removed" ? $tr("settings.protections.removed", { target: describeTarget(note.scope, $tr) })
    : $tr("settings.protections.restored", { target: describeTarget(note.scope, $tr) }),
  );
  const announce = $derived(view.failed ? $tr("settings.protections.error") : note?.kind === "removed" ? "" : noteText);

  // Bekreftelsen styres av tilstand; Esc og Avbryt lukker uten endring.
  $effect(() => {
    if (!confirmDlg) return;
    const want = view.pending !== null;
    if (want && !confirmDlg.open) confirmDlg.showModal();
    else if (!want && confirmDlg.open) confirmDlg.close();
  });
  $effect(() => {
    if (!folderDlg) return;
    if (folderOpen && !folderDlg.open) {
      path = ""; folderError = null; browseFailed = false; folderNote = "";
      folderDlg.showModal();
    } else if (!folderOpen && folderDlg.open) folderDlg.close();
  });

  async function browse(): Promise<void> {
    if (picking) return;
    picking = true; browseFailed = false; folderNote = "";
    const r = await browseFolder(b, $tr("home.folder.browse.title"));
    picking = false;
    if (r.kind === "picked") {
      path = r.path;
      folderError = null;
      folderNote = $tr("home.folder.browse.picked", { path: r.path });
    } else if (r.kind === "failed") browseFailed = true;
    pathInput?.focus();
  }
  async function submitFolder(e: Event): Promise<void> {
    e.preventDefault();
    if (busyAdd || path.trim() === "") return;
    busyAdd = true; folderError = null;
    const r = await session.addFolder(path);
    busyAdd = false;
    if (r.ok) closeFolder();
    else folderError = r.error;
  }
  function closeFolder(): void {
    folderOpen = false;
    void tick().then(() => { if (trigger?.isConnected) trigger.focus(); });
  }
  async function confirmRemove(): Promise<void> {
    if (await session.confirmRemove()) {
      await tick();
      host?.querySelector<HTMLElement>(".toast .undo")?.focus();
    }
  }
  async function cancelRemove(): Promise<void> {
    if (view.pending === null) return;
    session.cancelRemove();
    await tick();
    if (trigger?.isConnected) trigger.focus();
  }

  onMount(() => void session.load());
  onDestroy(() => session.dispose());
</script>

<div bind:this={host}>
<Card heading={$tr("settings.protections.title")}>
  <div class="body">
    <LiveRegion message={announce} />
    <p class="muted">{$tr("settings.protections.note")}</p>

    {#if view.phase === "loading"}
      <p role="status">{$tr("common.loading")}</p>
    {:else if view.phase === "error"}
      <Banner tone="danger" role="alert"><p>{$tr("settings.protections.load.error")}</p>
        {#snippet actions()}<Button onclick={() => void session.load()}>{$tr("common.retry")}</Button>{/snippet}
      </Banner>
    {:else}
      {#if view.failed}<Banner tone="danger" role="alert"><p>{$tr("settings.protections.error")}</p></Banner>{/if}

      {#if note?.kind === "removed"}
        <Toast message={noteText} undoLabel={$tr("common.undo")} onUndo={() => void session.undoRemove()} dismissLabel={$tr("common.close")} onDismiss={() => session.clearNote()} />
      {:else if note}
        <Banner tone="ok"><p>{noteText}</p>
          {#snippet actions()}<Button size="sm" onclick={() => session.clearNote()}>{$tr("common.dismiss")}</Button>{/snippet}
        </Banner>
      {/if}

      {#if view.items.length === 0}
        <p>{$tr("protect.empty.note")}</p>
      {:else}
        <ul class="list" aria-label={$tr("settings.protections.list.label")}>
          {#each view.items as p (`${p.scope.scope}:${p.scope.target}`)}
            {@const target = describeTarget(p.scope, $tr)}
            <li class="item">
              {#if p.scope.scope === "asset"}
                <span class="thumb" aria-hidden="true"><Thumb id={Number(p.scope.target)} bridge={b} size="sm" /></span>
              {/if}
              <div class="what">
                <p class="scope">{$tr(scopeKey[p.scope.scope])}</p>
                <p class="target">{target}</p>
                <p class="since">{$tr("settings.protections.since", { date: fmt.date(p.createdAt * 1000) })}</p>
              </div>
              <Button size="sm" disabled={view.busy || (view.pending !== null && !sameScope(view.pending, p.scope))}
                ariaLabel={$tr("settings.protections.remove.named", { target: `${$tr(scopeKey[p.scope.scope])} ${target}` })}
                onclick={(e) => { trigger = e.currentTarget as HTMLElement; session.askRemove(p.scope); }}>{$tr("protect.remove.button")}</Button>
            </li>
          {/each}
        </ul>
      {/if}

      <div class="add">
        <Button onclick={(e) => { trigger = e.currentTarget as HTMLElement; folderOpen = true; }}>{$tr("settings.protections.add")}</Button>
      </div>
    {/if}
  </div>
</Card>

<dialog class="sheet" bind:this={confirmDlg} aria-labelledby="pr-ct" onclose={() => void cancelRemove()}>
  <div class="sheet-body">
    <h2 id="pr-ct">{$tr("settings.protections.confirm.title")}</h2>
    <p>{$tr("settings.protections.confirm.body", { target: pendingText })}</p>
  </div>
  <div class="sheet-actions">
    <Button onclick={() => confirmDlg?.close()}>{$tr("common.cancel")}</Button>
    <Button variant="danger" disabled={view.busy} onclick={() => void confirmRemove()}>{$tr("protect.remove.button")}</Button>
  </div>
</dialog>

<dialog class="sheet" bind:this={folderDlg} aria-labelledby="pr-ft" onclose={() => { folderOpen = false; }}>
  <form onsubmit={submitFolder}>
    <div class="sheet-body">
      <h2 id="pr-ft">{$tr("settings.protections.folder.title")}</h2>
      <p>{$tr("settings.protections.folder.note")}</p>
      <div class="field">
        <label for="pr-path">{$tr("home.folder.path")}</label>
        <div class="row">
          <input id="pr-path" bind:this={pathInput} type="text" bind:value={path} autocomplete="off" spellcheck="false" aria-describedby="pr-ex" aria-invalid={folderError ? "true" : undefined} />
          <Button onclick={() => void browse()} disabled={!b.canPickFolder || picking}>{$tr("home.folder.browse")}</Button>
        </div>
        <p id="pr-ex" class="muted">{$tr("home.folder.example")}</p>
      </div>
      {#if !b.canPickFolder}<p class="muted">{$tr("home.folder.browse.unavailable")}</p>{/if}
      {#if browseFailed}<Banner tone="warn" role="alert"><p>{$tr("home.folder.browse.unavailable")}</p></Banner>{/if}
      <LiveRegion message={folderNote} />
      {#if folderError}<Banner tone="danger" role="alert"><p>{folderError.code === "empty_folder" ? $tr(errorKeyOf(folderError)) : $tr("settings.protections.folder.error")}</p></Banner>{/if}
    </div>
    <div class="sheet-actions">
      <Button onclick={() => folderDlg?.close()}>{$tr("common.cancel")}</Button>
      <Button variant="primary" type="submit" disabled={busyAdd || path.trim() === ""}>{$tr("settings.protections.folder.submit")}</Button>
    </div>
  </form>
</dialog>
</div>

<style>
  .body { display: grid; gap: var(--space-3); }
  .body p { margin: 0; }
  .muted { color: var(--c-text-muted); font-size: var(--fs-sm); }
  .list { list-style: none; margin: 0; padding: 0; border: var(--border-width) solid var(--c-border-subtle); border-radius: var(--radius-md); }
  .item { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2) var(--space-3); padding: var(--space-2) var(--space-3); }
  .item + .item { border-block-start: var(--border-width) solid var(--c-border-subtle); }
  .thumb { flex: none; overflow: hidden; inline-size: 2.5rem; block-size: 2rem; border-radius: var(--radius-sm); border: var(--border-width) solid var(--c-border-subtle); }
  .what { flex: 1 1 12rem; min-inline-size: 0; display: grid; gap: 0; }
  .scope { font-size: var(--fs-sm); color: var(--c-text-muted); font-weight: var(--fw-semibold); }
  .target { overflow-wrap: anywhere; font-weight: var(--fw-medium); }
  .since { font-size: var(--fs-sm); color: var(--c-text-muted); }
  .add { justify-self: start; }
</style>
