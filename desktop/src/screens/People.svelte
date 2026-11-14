<!-- Personer. Monteringspunkt for people-grenen: fyll denne filen. Ansiktsindeksen er av som standard; tom liste
     betyr «slå på indeksen». Navn, sammenslåing og beskyttelse via `useBridge()` (namePerson, mergePeople + unmergePerson, protectPerson). -->
<script lang="ts">
  import { PageHeader } from "../lib";
  import { tr } from "../lib/i18n";
</script>

<section class="people">
  <PageHeader title={$tr("people.title.label")} />
  <p class="todo">{$tr("placeholder.todo")}</p>
</section>

<style>
  .people { max-inline-size: var(--w-content); }
  .todo { margin: 0; color: var(--c-text-muted); }
</style>
