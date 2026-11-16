<script lang="ts">
  // Today's focus as a ring: tracked time fills in, planned blocks show as a lighter arc.
  let { spent, planned, capacity, label, sub }: { spent: number; planned: number; capacity: number; label: string; sub: string } = $props();
  const R = 52, C = 2 * Math.PI * R;
  const cap = $derived(Math.max(capacity, 0.0001));
  const spentFrac = $derived(Math.min(1, spent / cap));
  const plannedFrac = $derived(Math.min(1 - spentFrac, planned / cap));
  const over = $derived(spent > capacity && capacity > 0);
</script>

<figure class="day-ring" class:over aria-label={`${label}. ${sub}`}>
  <svg viewBox="0 0 128 128" role="img" aria-hidden="true">
    <defs>
      <linearGradient id="ring-grad" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="var(--lagoon)" />
        <stop offset=".55" stop-color="var(--iris)" />
        <stop offset="1" stop-color="var(--bubblegum)" />
      </linearGradient>
    </defs>
    <circle class="track" cx="64" cy="64" r={R} />
    <circle class="planned" cx="64" cy="64" r={R}
      stroke-dasharray={`${plannedFrac * C} ${C}`}
      stroke-dashoffset={-spentFrac * C} />
    <circle class="spent" cx="64" cy="64" r={R} stroke="url(#ring-grad)"
      stroke-dasharray={`${spentFrac * C} ${C}`} />
    {#if spentFrac > 0.02}
      <circle class="tip" cx={64 + R * Math.cos(2 * Math.PI * spentFrac - Math.PI / 2)} cy={64 + R * Math.sin(2 * Math.PI * spentFrac - Math.PI / 2)} r="5" />
    {/if}
  </svg>
  <figcaption><strong>{label}</strong><span>{sub}</span></figcaption>
</figure>

<style>
  .day-ring { position: relative; margin: 0; width: 148px; height: 148px; flex: none; }
  svg { width: 100%; height: 100%; transform: rotate(-90deg); overflow: visible; }
  circle { fill: none; stroke-width: 12; stroke-linecap: round; transition: stroke-dasharray .9s cubic-bezier(.2,.8,.2,1), stroke-dashoffset .9s cubic-bezier(.2,.8,.2,1); }
  .track { stroke: var(--track); stroke-linecap: butt; }
  .planned { stroke: color-mix(in srgb, var(--iris) 30%, transparent); }
  .tip { fill: #fff; stroke: none; filter: drop-shadow(0 0 6px var(--bubblegum)); }
  .over .spent { stroke: var(--mango); }
  figcaption { position: absolute; inset: 0; display: grid; place-content: center; text-align: center; gap: 2px; }
  strong { font-family: var(--display); font-size: 25px; font-weight: 650; letter-spacing: -.8px; font-variant-numeric: tabular-nums; line-height: 1; }
  span { font-size: 10px; color: var(--muted); max-width: 90px; line-height: 1.3; }
</style>
