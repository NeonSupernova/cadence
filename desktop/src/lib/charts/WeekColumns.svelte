<script lang="ts">
  import type { CapacityDay } from '$lib/types';
  let { days, onpick }: { days: CapacityDay[]; onpick?: (date: string) => void } = $props();
  const todayKey = new Date().toLocaleDateString('en-CA');
  const max = $derived(Math.max(0.1, ...days.map(d => Math.max(d.capacityUnits, d.spentUnits, d.blocks.reduce((n, b) => n + b.units, 0)))));
  const plannedOf = (d: CapacityDay) => d.blocks.reduce((n, b) => n + b.units, 0);
  const fmt = (n: number) => (Math.round(n * 10) / 10).toFixed(1);
</script>

<div class="week-columns" role="group" aria-label="Work units for the next seven days">
  {#each days as d (d.date)}
    {@const isToday = d.date === todayKey}
    <button class="col" class:today={isToday} onclick={() => onpick?.(d.date)}
      aria-label={`${new Date(d.date + 'T12:00:00').toLocaleDateString(undefined, { weekday: 'long' })}: ${fmt(d.capacityUnits)} work units available, ${fmt(plannedOf(d))} planned, ${fmt(d.spentUnits)} done`}>
      <span class="bar">
        <span class="cap" style={`height:${(d.capacityUnits / max) * 100}%`}></span>
        <span class="plan" style={`height:${(Math.min(plannedOf(d), d.capacityUnits) / max) * 100}%`}></span>
        <span class="done" style={`height:${(d.spentUnits / max) * 100}%`}></span>
      </span>
      <span class="day">{new Date(d.date + 'T12:00:00').toLocaleDateString(undefined, { weekday: 'short' })}</span>
      <span class="num">{fmt(d.capacityUnits)}</span>
    </button>
  {/each}
</div>

<style>
  .week-columns { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: 10px; align-items: end; height: 148px; }
  .col { display: grid; grid-template-rows: 1fr auto auto; justify-items: center; gap: 5px; height: 100%; padding: 0; border-radius: 12px; }
  .bar { position: relative; width: 100%; max-width: 34px; height: 100%; border-radius: 10px; overflow: hidden; background: var(--track); }
  .bar > span { position: absolute; left: 0; right: 0; bottom: 0; border-radius: 10px; transition: height .8s cubic-bezier(.2,.8,.2,1); }
  .cap { background: color-mix(in srgb, var(--iris) 16%, transparent); }
  .plan { background: repeating-linear-gradient(135deg, color-mix(in srgb, var(--iris) 45%, transparent) 0 4px, color-mix(in srgb, var(--iris) 25%, transparent) 4px 8px); }
  .done { background: linear-gradient(to top, var(--lagoon), var(--iris)); }
  .day { font-size: 11px; color: var(--muted); font-weight: 600; }
  .num { font-family: var(--display); font-size: 12px; font-variant-numeric: tabular-nums; color: var(--ink); }
  .today .bar { box-shadow: 0 0 0 2px var(--iris), 0 0 18px color-mix(in srgb, var(--iris) 55%, transparent); }
  .today .day { color: var(--iris); }
  .col:hover .bar { filter: brightness(1.15); }
</style>
