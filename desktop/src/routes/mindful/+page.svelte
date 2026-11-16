<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke, listen, desktop } from '$lib/bridge';
  import { nudge, reducedMotion } from '$lib/celebrate';
  import type { Reminder, ReminderResponse, ShowingReminder } from '$lib/types';

  const COPY: Record<Reminder, { icon: string; title: string; lines: string[]; done: string; hue: string }> = {
    water: {
      icon: '💧', title: 'Water break', hue: '#3fa7e8', done: 'Drank some',
      lines: ['Have a few sips of water.', 'Your brain is mostly water. Top it up.', 'Grab your glass, a few big sips.']
    },
    eyes: {
      icon: '👀', title: 'Rest your eyes', hue: '#2fb383', done: 'Start 20s',
      lines: ['Look at something 20 feet away for 20 seconds.', 'Find the farthest thing you can see and hold it.', 'Look out a window or across the room.']
    },
    stretch: {
      icon: '🧠', title: 'Brain break', hue: '#9a6bf0', done: 'Took one',
      lines: ['Stand up, stretch, roll your shoulders.', 'Walk around for a minute. The task will wait.', 'Shake out your hands and take three slow breaths.']
    }
  };

  let current = $state<ShowingReminder | null>(null);
  let left = $state(0);
  let paused = $state(false);
  let eyesLeft = $state(0);
  let error = $state('');
  let fresh = $state(false);
  let ticker: ReturnType<typeof setInterval> | undefined;
  let eyesTicker: ReturnType<typeof setInterval> | undefined;

  const copy = $derived(current ? COPY[current.kind] : null);
  const line = $derived(copy && current ? copy.lines[current.variant % copy.lines.length] : '');

  function present(next: ShowingReminder) {
    current = next;
    left = next.seconds;
    paused = false;
    eyesLeft = 0;
    error = '';
    clearInterval(eyesTicker);
    fresh = false;
    requestAnimationFrame(() => (fresh = !reducedMotion()));
    nudge();
    clearInterval(ticker);
    ticker = setInterval(() => {
      if (paused || eyesLeft > 0) return;
      left -= 1;
      if (left <= 0) respond('timeout');
    }, 1000);
  }

  async function respond(response: ReminderResponse) {
    if (!current) return;
    const kind = current.kind;
    clearInterval(ticker);
    clearInterval(eyesTicker);
    current = null;
    if (!desktop()) return;
    try { await invoke('mindful_respond', { kind, response }); } catch (e) { error = String(e); }
  }

  async function quiet() {
    clearInterval(ticker);
    current = null;
    if (desktop()) try { await invoke('mindful_quiet', { minutes: 60 }); } catch (e) { error = String(e); }
  }

  function primary() {
    if (current?.kind !== 'eyes') return respond('done');
    // Guide the 20-second look-away, then count it as done.
    eyesLeft = 20;
    eyesTicker = setInterval(() => {
      eyesLeft -= 1;
      if (eyesLeft <= 0) { nudge(); respond('done'); }
    }, 1000);
  }

  onMount(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    if (desktop()) {
      listen<ShowingReminder>('mindful-show', present).then(fn => (disposed ? fn() : (unlisten = fn)));
      invoke<ShowingReminder | null>('mindful_current').then(r => r && present(r)).catch(e => (error = String(e)));
    } else {
      // Browser preview: ?kind=eyes shows that reminder.
      const kind = (new URLSearchParams(location.search).get('kind') ?? 'water') as Reminder;
      present({ kind: COPY[kind] ? kind : 'water', seconds: 25, variant: 0, preview: true });
    }
    return () => { disposed = true; unlisten?.(); clearInterval(ticker); clearInterval(eyesTicker); };
  });
</script>

<svelte:head><title>Break — Cadence</title></svelte:head>

{#if current && copy}
  <div
    class="mindful-card"
    class:fresh
    style="--hue:{copy.hue}"
    role="alertdialog"
    aria-labelledby="mindful-title"
    aria-describedby="mindful-line"
    tabindex="-1"
    onmouseenter={() => (paused = true)}
    onmouseleave={() => (paused = false)}
  >
    <div class="mindful-icon" aria-hidden="true"><span>{copy.icon}</span></div>
    <div class="mindful-copy">
      <strong id="mindful-title">{copy.title}{#if current.preview}<small>&nbsp;· preview</small>{/if}</strong>
      {#if eyesLeft > 0}
        <p id="mindful-line" aria-live="polite">Keep looking far away… <b>{eyesLeft}s</b></p>
      {:else}
        <p id="mindful-line">{line}</p>
      {/if}
      {#if error}<p class="mindful-error">{error}</p>{/if}
      <div class="mindful-actions">
        {#if eyesLeft > 0}
          <button class="go" onclick={() => respond('done')}>I'm done</button>
        {:else}
          <button class="go" onclick={primary}>{copy.done}</button>
          <button onclick={() => respond('snooze')} title="Remind me in 10 minutes">Snooze</button>
          <button onclick={() => respond('skip')}>Skip</button>
          <button onclick={quiet} title="No reminders for an hour">Quiet 1h</button>
        {/if}
      </div>
    </div>
    <div class="mindful-time" aria-hidden="true">
      <span style="width:{eyesLeft > 0 ? (eyesLeft / 20) * 100 : (left / current.seconds) * 100}%"></span>
    </div>
  </div>
{/if}

<style>
  :global(html), :global(body) { background: transparent !important; overflow: hidden; }
  .mindful-card {
    position: fixed; inset: 6px; display: flex; gap: 13px; align-items: center;
    padding: 12px 15px 14px 13px; border-radius: 18px; overflow: hidden;
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    border: 1.5px solid color-mix(in srgb, var(--hue) 55%, var(--line));
    box-shadow: 0 6px 22px #0003, 0 0 0 0 var(--hue);
    backdrop-filter: blur(18px); -webkit-backdrop-filter: blur(18px);
    color: var(--ink); outline: 0;
    animation: drop-in .35s cubic-bezier(.2, 1.3, .4, 1);
  }
  /* The "flashing thing": a glowing ring that pulses a few times when it appears. */
  .mindful-card.fresh { animation: drop-in .35s cubic-bezier(.2, 1.3, .4, 1), flash 1.1s ease-in-out .3s 4; }
  .mindful-icon {
    position: relative; flex: none; width: 50px; height: 50px; border-radius: 50%;
    display: grid; place-items: center; font-size: 26px;
    background: color-mix(in srgb, var(--hue) 18%, var(--surface));
  }
  .mindful-icon::after {
    content: ''; position: absolute; inset: -4px; border-radius: 50%;
    border: 2px solid var(--hue); opacity: 0; animation: ring 1.6s ease-out infinite;
  }
  .mindful-icon span { animation: bob 2.2s ease-in-out infinite; }
  .mindful-copy { min-width: 0; flex: 1; }
  .mindful-copy strong { font-size: 13px; font-weight: 650; letter-spacing: -.2px; }
  .mindful-copy strong small { font-weight: 400; color: var(--faint); font-size: 10px; }
  .mindful-copy p { margin: 2px 0 7px; font-size: 12px; line-height: 1.45; color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .mindful-copy p b { color: var(--hue); font-variant-numeric: tabular-nums; }
  .mindful-copy .mindful-error { color: var(--red); white-space: normal; }
  .mindful-actions { display: flex; gap: 5px; }
  .mindful-actions button {
    font-size: 11px; padding: 5px 9px; border-radius: 7px; color: var(--muted);
    background: var(--secondary); white-space: nowrap;
  }
  .mindful-actions button:hover { color: var(--ink); }
  .mindful-actions .go { background: var(--hue); color: #fff; font-weight: 600; }
  .mindful-actions .go:hover { color: #fff; filter: brightness(1.08); }
  .mindful-time { position: absolute; left: 0; right: 0; bottom: 0; height: 3px; background: color-mix(in srgb, var(--hue) 15%, transparent); }
  .mindful-time span { display: block; height: 100%; background: var(--hue); transition: width 1s linear; }
  @keyframes drop-in { from { transform: translateY(-120%); opacity: 0; } }
  @keyframes flash {
    0%, 100% { box-shadow: 0 6px 22px #0003, 0 0 0 0 transparent; }
    50% { box-shadow: 0 6px 22px #0003, 0 0 0 4px color-mix(in srgb, var(--hue) 55%, transparent); border-color: var(--hue); }
  }
  @keyframes ring { 0% { transform: scale(.85); opacity: .9; } 100% { transform: scale(1.35); opacity: 0; } }
  @keyframes bob { 50% { transform: translateY(-2px) scale(1.06); } }
  @media (prefers-reduced-motion: reduce) {
    .mindful-card, .mindful-card.fresh, .mindful-icon::after, .mindful-icon span { animation: none; }
    .mindful-icon::after { opacity: .6; }
  }
</style>
