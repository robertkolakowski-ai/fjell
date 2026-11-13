<!-- Merkesymbol (V2 2.8): to nesten like bilder (det bakerste forskjøvet opp til høyre) der det fremste har en diagonal ryggkant
     som deler flaten i lys og skygge. Leses både som «duplikat» og «fjell». 24 px rutenett, 2 px hjørner, ryggkant 30°.
     Tegnes i currentColor (aksent i sidemenyen). Dekorativ: navnet står i ordmerket ved siden av. Kilde: docs/design/merke/. -->
<script lang="ts">
  interface Props { size?: number }
  let { size = 24 }: Props = $props();
  const uid = $props.id();
</script>

<svg class="logo" width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round">
  <defs><clipPath id="{uid}-c"><rect x="3" y="9" width="12" height="12" rx="2" /></clipPath></defs>
  <rect x="9" y="3" width="12" height="12" rx="2" />
  <g clip-path="url(#{uid}-c)" stroke="none">
    <rect x="3" y="9" width="12" height="12" fill="currentColor" fill-opacity="0.28" />
    <path d="M3 17 15 10.07V21H3z" fill="currentColor" />
  </g>
  <rect x="3" y="9" width="12" height="12" rx="2" />
</svg>

<style>
  .logo { flex: none; overflow: visible; }
</style>
