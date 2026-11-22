<script lang="ts">
  import { changelog, compareVersions } from '$lib/changelog';

  /** `since` is the last version the person saw; releases after it are highlighted. */
  let { since = null, onclose }: { since?: string | null; onclose: () => void } = $props();

  let fresh = $derived(since ? changelog.filter(r => compareVersions(r.version, since) > 0) : changelog.slice(0, 1));
  let earlier = $derived(changelog.filter(r => !fresh.includes(r)));
  let latest = $derived(changelog[0]);
  let showEarlier = $state(false);
  let card = $state<HTMLDivElement>();
  const day = (iso: string) => new Date(`${iso}T12:00:00`).toLocaleDateString(undefined, { month: 'long', day: 'numeric', year: 'numeric' });

  $effect(() => { card?.focus(); });
</script>

<div class="modal-backdrop whats-new-backdrop" role="presentation" onkeydown={e => { if (e.key === 'Escape') onclose(); }} onclick={e => { if (e.target === e.currentTarget) onclose(); }}>
  <div class="whats-new" bind:this={card} role="dialog" aria-modal="true" aria-labelledby="whats-new-title" tabindex="-1">
    <header class="banner">
      <button class="close" aria-label="Close" onclick={onclose}>×</button>
      <span class="kicker">What’s new</span>
      <h2 id="whats-new-title">{latest.title}</h2>
      <p>Cadence {latest.version} · {day(latest.date)}</p>
    </header>
    <div class="body">
      {#each fresh as release (release.version)}
        <section>
          {#if fresh.length > 1 || release !== latest}<h3>{release.title} <small>{release.version}</small></h3>{/if}
          <ul>{#each release.notes as note (note.text)}<li><span class="icon" aria-hidden="true">{note.icon}</span><span>{note.text}</span></li>{/each}</ul>
        </section>
      {/each}
      {#if earlier.length}
        <button class="earlier-toggle" aria-expanded={showEarlier} onclick={() => showEarlier = !showEarlier}>{showEarlier ? 'Hide' : 'Show'} earlier updates <span aria-hidden="true">{showEarlier ? '▴' : '▾'}</span></button>
        {#if showEarlier}
          {#each earlier as release (release.version)}
            <section class="past">
              <h3>{release.title} <small>{release.version} · {day(release.date)}</small></h3>
              <ul>{#each release.notes as note (note.text)}<li><span class="icon" aria-hidden="true">{note.icon}</span><span>{note.text}</span></li>{/each}</ul>
            </section>
          {/each}
        {/if}
      {/if}
    </div>
    <footer><button class="primary-button" onclick={onclose}>Got it</button></footer>
  </div>
</div>

<style>
  .whats-new-backdrop { z-index: 80; }
  .whats-new { width: min(460px, 100%); max-height: calc(100dvh - 48px); display: flex; flex-direction: column; background: var(--surface); border: 1px solid var(--line); border-radius: 18px; overflow: hidden; box-shadow: 0 30px 100px #0004; outline: none; animation: pop .28s cubic-bezier(.2, .9, .3, 1.2); }
  .banner { position: relative; padding: 30px 28px 24px; color: #fff; background: linear-gradient(135deg, #6d5dfc, #c04dd8 55%, #ff8a5c); }
  .banner .kicker { font-size: 10px; letter-spacing: 1.4px; text-transform: uppercase; opacity: .85; font-weight: 600; }
  .banner h2 { margin: 6px 0 4px; font-size: 24px; letter-spacing: -.5px; line-height: 1.15; }
  .banner p { margin: 0; font-size: 12px; opacity: .85; }
  .close { position: absolute; top: 14px; right: 14px; width: 28px; height: 28px; border-radius: 50%; background: #fff3; color: #fff; font-size: 17px; line-height: 1; }
  .body { padding: 18px 28px 6px; overflow: auto; }
  section + section { margin-top: 18px; }
  h3 { font-size: 13px; margin: 0 0 8px; }
  h3 small { color: var(--muted); font-weight: 450; margin-left: 4px; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 10px; }
  li { display: flex; gap: 12px; align-items: flex-start; font-size: 13px; line-height: 1.45; }
  .icon { flex: 0 0 26px; height: 26px; display: grid; place-items: center; border-radius: 8px; background: var(--secondary); font-size: 14px; }
  .past { opacity: .8; }
  .earlier-toggle { margin: 20px 0 4px; font-size: 11px; color: var(--secondary-text); background: transparent; }
  footer { padding: 14px 28px 22px; display: flex; justify-content: flex-end; border-top: 1px solid var(--line); margin-top: 14px; }
  footer .primary-button { min-width: 110px; justify-content: center; }
  @keyframes pop { from { transform: scale(.94) translateY(8px); opacity: 0; } }
  @media (prefers-reduced-motion: reduce) { .whats-new { animation: none; } }
  :global(html[data-reduce-motion='true']) .whats-new { animation: none; }
</style>
