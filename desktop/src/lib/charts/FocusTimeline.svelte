<script lang="ts">
  // A 24-hour ribbon for one day: each focus session is a colored block at the time it happened.
  export type Session = { id: string; taskId: number; title: string; hue: string; start: number; end: number; live?: boolean };
  let { sessions, day, onpick }: { sessions: Session[]; day: string; onpick?: (taskId: number) => void } = $props();
  const dayStart = $derived(new Date(day + 'T00:00:00').getTime());
  const SPAN = 24 * 3600000;
  const pct = (t: number) => Math.max(0, Math.min(100, ((t - dayStart) / SPAN) * 100));
  const hm = (t: number) => new Date(t).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  const nowPct = $derived(pct(Date.now()));
  const showNow = $derived(day === new Date().toLocaleDateString('en-CA'));
</script>

<div class="timeline">
  <div class="ribbon" role="group" aria-label="Focus sessions by time of day">
    {#each [0, 6, 12, 18] as h}<span class="hour" style={`left:${(h / 24) * 100}%`}></span>{/each}
    {#each sessions as s (s.id)}
      <button class="block" class:live={s.live}
        style={`left:${pct(s.start)}%;width:max(4px,${pct(s.end) - pct(s.start)}%);--hue:${s.hue}`}
        title={`${s.title} · ${hm(s.start)}–${s.live ? 'now' : hm(s.end)}`}
        aria-label={`${s.title}, ${hm(s.start)} to ${s.live ? 'now' : hm(s.end)}`}
        onclick={() => onpick?.(s.taskId)}></button>
    {/each}
    {#if showNow}<span class="now" style={`left:${nowPct}%`}></span>{/if}
  </div>
  <div class="ticks" aria-hidden="true">
    {#each ['12a', '6a', '12p', '6p', '12a'] as t, i}<span style={`left:${i * 25}%`}>{t}</span>{/each}
  </div>
</div>

<style>
  .timeline { position: relative; padding-bottom: 18px; }
  .ribbon { position: relative; height: 38px; border-radius: 12px; background: var(--track); overflow: hidden; }
  .hour { position: absolute; top: 0; bottom: 0; width: 1px; background: color-mix(in srgb, var(--ink) 10%, transparent); }
  .block {
    position: absolute; top: 5px; bottom: 5px; padding: 0; border-radius: 7px;
    background: linear-gradient(180deg, color-mix(in srgb, var(--hue) 85%, #fff), var(--hue));
    box-shadow: 0 0 14px color-mix(in srgb, var(--hue) 60%, transparent);
  }
  .block:hover { filter: brightness(1.15); z-index: 1; }
  .block.live { background-image: linear-gradient(110deg, transparent 30%, rgba(255,255,255,.45) 50%, transparent 70%), linear-gradient(180deg, color-mix(in srgb, var(--hue) 85%, #fff), var(--hue)); background-size: 220% 100%, 100% 100%; animation: sheen 2.4s linear infinite; }
  .now { position: absolute; top: -2px; bottom: -2px; width: 2px; margin-left: -1px; background: var(--bubblegum); box-shadow: 0 0 8px var(--bubblegum); }
  .ticks { position: absolute; left: 0; right: 0; bottom: 0; height: 14px; }
  .ticks span { position: absolute; transform: translateX(-50%); font-size: 10px; color: var(--faint); }
  .ticks span:first-child { transform: none; }
  .ticks span:last-child { transform: translateX(-100%); }
  @keyframes sheen { from { background-position: 120% 0, 0 0; } to { background-position: -120% 0, 0 0; } }
  @media (prefers-reduced-motion: reduce) { .block.live { animation: none; } }
  :global(html[data-reduce-motion=true]) .block.live { animation: none; }
</style>
