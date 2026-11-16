<script lang="ts">
  // The last two weeks of focus, one bar per day. Click a bar to open that day.
  let { days, selected, onpick }: { days: { date: string; ms: number }[]; selected: string; onpick?: (date: string) => void } = $props();
  const max = $derived(Math.max(30 * 60000, ...days.map(d => d.ms)));
  const avg = $derived(days.length ? days.reduce((n, d) => n + d.ms, 0) / days.length : 0);
  const label = (ms: number) => { const m = Math.round(ms / 60000); return m < 60 ? `${m}m` : `${Math.floor(m / 60)}h${m % 60 ? ` ${m % 60}m` : ''}`; };
</script>

<div class="history">
  <div class="bars" role="group" aria-label="Focus time for the last 14 days">
    {#if avg > 0}<span class="avg" style={`bottom:${(avg / max) * 100}%`}><em>avg {label(avg)}</em></span>{/if}
    {#each days as d (d.date)}
      <button class="bar" class:chosen={d.date === selected} class:empty={!d.ms} onclick={() => onpick?.(d.date)}
        aria-label={`${new Date(d.date + 'T12:00:00').toLocaleDateString(undefined, { weekday: 'long', month: 'short', day: 'numeric' })}: ${label(d.ms)}`}
        aria-pressed={d.date === selected}>
        <span class="fill" style={`height:${d.ms ? Math.max(4, (d.ms / max) * 100) : 0}%`}>{#if d.ms}<b>{label(d.ms)}</b>{/if}</span>
      </button>
    {/each}
  </div>
  <div class="labels" aria-hidden="true">
    {#each days as d, i (d.date)}<span class:chosen={d.date === selected}>{i === days.length - 1 ? 'Today' : new Date(d.date + 'T12:00:00').getDate()}</span>{/each}
  </div>
</div>

<style>
  .bars { position: relative; padding-top: 20px; display: grid; grid-template-columns: repeat(14, minmax(0, 1fr)); gap: 6px; height: 140px; align-items: end; }
  .bar { position: relative; height: 100%; padding: 0; display: flex; align-items: flex-end; border-radius: 8px; }
  .bar.empty::after { content: ''; position: absolute; left: 30%; right: 30%; bottom: 0; height: 3px; border-radius: 2px; background: var(--track); }
  .fill { position: relative; width: 100%; border-radius: 8px 8px 5px 5px; background: linear-gradient(to top, color-mix(in srgb, var(--iris) 55%, transparent), color-mix(in srgb, var(--bubblegum) 55%, transparent)); transition: height .7s cubic-bezier(.2,.8,.2,1), background .2s; }
  .fill b { position: absolute; bottom: calc(100% + 4px); left: 50%; transform: translateX(-50%); font-size: 9px; font-weight: 600; color: var(--muted); white-space: nowrap; opacity: 0; transition: opacity .15s; }
  .bar:hover .fill b, .chosen .fill b { opacity: 1; }
  .chosen .fill { background: linear-gradient(to top, var(--lagoon), var(--iris) 60%, var(--bubblegum)); box-shadow: 0 0 18px color-mix(in srgb, var(--iris) 50%, transparent); }
  .avg { position: absolute; left: 0; right: 0; border-top: 1.5px dashed color-mix(in srgb, var(--ink) 25%, transparent); pointer-events: none; z-index: 1; }
  .avg em { position: absolute; left: 0; bottom: 2px; font-style: normal; font-size: 9px; color: var(--faint); }
  .labels { display: grid; grid-template-columns: repeat(14, minmax(0, 1fr)); gap: 6px; margin-top: 6px; }
  .labels span { text-align: center; font-size: 10px; color: var(--faint); }
  .labels .chosen { color: var(--iris); font-weight: 700; }
</style>
