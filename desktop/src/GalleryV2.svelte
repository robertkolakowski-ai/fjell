<!-- V2-komponenter i galleriet (dev): merke, tomme tilstander, skjelett, knapper på scene, bannere. All tekst via $tr (finnes fra før). -->
<script lang="ts">
  import { Banner, Button, EmptyState, Icon, Logo, Skeleton } from "./lib/index.ts";
  import { tr } from "./lib/i18n";
</script>

<section class="v2" aria-label={$tr("ui.empty.title")}>
  <p class="row"><Logo size={32} /> <Icon name="duplicate" /> <Icon name="best" /> <Icon name="restore" /> <Icon name="cloud-off" /> <Icon name="home" filled /></p>
  <div class="empties">
    <EmptyState motif="topp" headingLevel={3} title={$tr("ui.empty.title")} description={$tr("review.empty.body")} />
    <EmptyState motif="vann" headingLevel={3} title={$tr("trash.empty.title")} />
    <EmptyState motif="sti" headingLevel={3} title={$tr("review.empty.title")}>
      {#snippet action()}<Button variant="primary">{$tr("common.continue")}</Button>{/snippet}
    </EmptyState>
  </div>
  <div class="skels" aria-busy="true">
    <Skeleton variant="thumb" width="8rem" />
    <Skeleton variant="block" width="12rem" height="4rem" />
    <Skeleton variant="text" lines={3} width="16rem" />
  </div>
  <div class="stage">
    <Button onStage>{$tr("common.cancel")}</Button>
    <Button onStage variant="ok">{$tr("group.keep.all")}</Button>
    <Button onStage variant="danger">{$tr("trash.empty.button")}</Button>
  </div>
  <p class="row"><Button variant="ok">{$tr("group.keep.all")}</Button><Button variant="danger">{$tr("trash.empty.button")}</Button><Button variant="quiet">{$tr("common.dismiss")}</Button></p>
  <Banner tone="warn" title={$tr("review.empty.title")}><p>{$tr("review.empty.body")}</p></Banner>
  <Banner tone="ok" title={$tr("trash.empty.title")} headingLevel={3}><p>{$tr("review.empty.body")}</p></Banner>
</section>

<style>
  .v2 { display: grid; gap: var(--space-4); }
  .row { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-3); margin: 0; color: var(--c-accent); }
  .empties { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 14rem), 1fr)); gap: var(--space-4); }
  .skels { display: flex; flex-wrap: wrap; align-items: flex-start; gap: var(--space-4); }
  .stage { display: flex; flex-wrap: wrap; gap: var(--space-3); padding: var(--space-4); background: var(--c-stage); border-radius: var(--radius-md); }
</style>
