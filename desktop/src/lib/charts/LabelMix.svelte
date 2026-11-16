<script lang="ts">
  // Where the time went, by label: one stacked bar plus a legend.
  let { parts }: { parts: { label: string; ms: number; hue: string }[] } = $props();
  const total = $derived(parts.reduce((n, p) => n + p.ms, 0));
  const label = (ms: number) => { const m = Math.round(ms / 60000); return m < 60 ? `${m}m` : `${Math.floor(m / 60)}h ${m % 60}m`; };
</script>

{#if total > 0}
  <div class="mix">
    <div class="stack" role="img" aria-label={parts.map(p => `${p.label} ${label(p.ms)}`).join(', ')}>
      {#each parts as p (p.label)}<span style={`flex:${p.ms};--hue:${p.hue}`}></span>{/each}
    </div>
    <ul>
      {#each parts as p (p.label)}<li style={`--hue:${p.hue}`}><i></i>{p.label}<b>{label(p.ms)}</b><small>{Math.round((p.ms / total) * 100)}%</small></li>{/each}
    </ul>
  </div>
{/if}

<style>
  .stack { display: flex; gap: 3px; height: 14px; border-radius: 99px; overflow: hidden; }
  .stack span { background: var(--hue); min-width: 4px; box-shadow: inset 0 1px 0 rgba(255,255,255,.35); }
  ul { list-style: none; padding: 0; margin: 14px 0 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 210px)); gap: 8px 16px; }
  li { display: flex; align-items: center; gap: 8px; font-size: 12px; }
  i { width: 10px; height: 10px; border-radius: 4px; background: var(--hue); box-shadow: 0 0 8px var(--hue); flex: none; }
  b { margin-left: auto; font-family: var(--display); font-weight: 600; font-variant-numeric: tabular-nums; }
  small { color: var(--faint); font-size: 10px; width: 30px; text-align: right; }
</style>
