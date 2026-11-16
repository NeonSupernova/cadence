<script lang="ts">
  // The window's backdrop. Its color leans toward whatever you're focused on,
  // so the room itself tells you what's running.
  let { hue = null, active = false }: { hue?: string | null; active?: boolean } = $props();
</script>

<div class="aurora" class:active style={hue ? `--focus:${hue}` : ''} aria-hidden="true">
  <span class="blob b1"></span>
  <span class="blob b2"></span>
  <span class="blob b3"></span>
  <span class="blob b4"></span>
  <span class="grain"></span>
</div>

<style>
  .aurora {
    --focus: var(--iris);
    position: fixed; inset: 0; z-index: 0; overflow: hidden; pointer-events: none;
    background: var(--base);
    transition: background 1.2s ease;
  }
  .blob {
    position: absolute; border-radius: 50%; filter: blur(70px); opacity: var(--blob-opacity);
    will-change: transform; transition: background 1.6s ease, opacity 1.6s ease;
  }
  .b1 { width: 58vmax; height: 58vmax; left: -14vmax; top: -22vmax; background: var(--iris); animation: drift1 46s ease-in-out infinite alternate; }
  .b2 { width: 46vmax; height: 46vmax; right: -12vmax; top: -10vmax; background: var(--bubblegum); animation: drift2 54s ease-in-out infinite alternate; }
  .b3 { width: 50vmax; height: 50vmax; left: 22vw; bottom: -30vmax; background: var(--lagoon); animation: drift3 60s ease-in-out infinite alternate; }
  .b4 { width: 34vmax; height: 34vmax; right: 8vw; bottom: -6vmax; background: var(--sky); opacity: calc(var(--blob-opacity) * .6); animation: drift2 70s ease-in-out infinite alternate-reverse; }
  /* While a timer runs, two of the lights take on that task's color. */
  .active .b1, .active .b3 { background: var(--focus); }
  .active .b2 { opacity: calc(var(--blob-opacity) * .7); }
  .grain {
    position: absolute; inset: 0; opacity: var(--grain-opacity); mix-blend-mode: overlay;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='160' height='160'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='.9' numOctaves='2' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E");
  }
  @keyframes drift1 { to { transform: translate(12vw, 10vh) scale(1.08); } }
  @keyframes drift2 { to { transform: translate(-10vw, 14vh) scale(.92); } }
  @keyframes drift3 { to { transform: translate(-14vw, -12vh) scale(1.12); } }
  :global(html[data-reduce-motion=true]) .blob { animation: none; }
  @media (prefers-reduced-motion: reduce) { .blob { animation: none; } }
  :global(html[data-reduce-transparency=true]) .aurora .blob { opacity: calc(var(--blob-opacity) * .5); }
</style>
