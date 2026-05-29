<!-- web/src/routes/g/+page.svelte -->
<script lang="ts">
  import GrButton from '$lib/graphite/GrButton.svelte';
  import GrChip from '$lib/graphite/GrChip.svelte';
  import GrTypeDot from '$lib/graphite/GrTypeDot.svelte';
  import GrTypeTag from '$lib/graphite/GrTypeTag.svelte';
  import GrRow from '$lib/graphite/GrRow.svelte';
  import GrWidget from '$lib/graphite/GrWidget.svelte';
</script>

<div style="display:flex;flex-direction:column;gap:20px;padding:32px;max-width:720px;">
  <h1 style="font-size:19px;font-weight:600;">Graphite primitives</h1>
  <div style="display:flex;gap:10px;align-items:center;">
    <GrButton variant="cta">New note</GrButton>
    <GrButton>Ghost</GrButton>
    <GrButton icon="settings" aria-label="Settings" />
  </div>
  <div style="display:flex;gap:8px;align-items:center;">
    <GrChip active count={12}>Tasks</GrChip>
    <GrChip count={4}>Notes</GrChip>
  </div>
  <div style="display:flex;gap:14px;align-items:center;">
    {#each ['task','event','note','project','person','query'] as t}
      <span style="display:flex;gap:6px;align-items:center;"><GrTypeDot type={t} /> <span class="gr-mono" style="font-size:10.5px;color:var(--faint)">{t}</span></span>
    {/each}
  </div>
  <div style="display:flex;gap:8px;"><GrTypeTag type="project">project</GrTypeTag><GrTypeTag type="task">task</GrTypeTag></div>
  <GrWidget title="Today" badge="3" icon="sun">
    <GrRow icon="circle-dot" label="Write the plan" meta="2h" />
    <GrRow icon="circle-dot" label="Review PR" meta="now" urgent />
  </GrWidget>
</div>
