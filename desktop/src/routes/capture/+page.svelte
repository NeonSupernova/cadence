<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke, listen, progress, desktop } from '$lib/bridge';
  import type { Snapshot } from '$lib/types';
  import { reference } from '$lib/types';
  let input = $state('');
  let busy = $state(false);
  let error = $state('');
  let phase = $state('');
  let saved = $state('');
  let field: HTMLTextAreaElement;
  async function capture() {
    if (!input.trim() || busy) return;
    busy = true; error = ''; saved = ''; phase = 'Capturing…';
    try {
      const result = await invoke<Snapshot>('capture_task', { message: input });
      const task = result.tasks.find(t => t.id === result.focusIds[0]);
      saved = task ? `Added to Next · ${reference(task.id)} · ${task.title}` : 'Task captured.';
      input = '';
    } catch (e) { error = String(e); }
    finally { busy = false; field?.focus(); }
  }
  onMount(() => {
    field?.focus();
    let disposed = false;
    const cleanup: (() => void)[] = [];
    for (const pending of [progress(text => phase = text), listen('capture-ready', () => field?.focus())]) {
      pending.then(fn => disposed ? fn() : cleanup.push(fn));
    }
    const escape = (e: KeyboardEvent) => { if (e.key === 'Escape' && !busy) invoke('dismiss_capture'); };
    window.addEventListener('keydown', escape);
    return () => { disposed = true; cleanup.forEach(fn => fn()); window.removeEventListener('keydown', escape); };
  });
</script>
<svelte:head><title>Quick Capture — Cadence</title></svelte:head>
<main class="quick-capture-window">
  <header><img src="/favicon.png" alt="" /><strong>Quick Capture</strong><span>INBOX FOR YOUR ATTENTION</span></header>
  <form onsubmit={e => { e.preventDefault(); capture(); }}>
    <textarea bind:this={field} bind:value={input} maxlength="19000" rows="3" aria-label="Task to capture" placeholder="What needs doing? Include a deadline if there is one." disabled={busy} onkeydown={e => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); capture(); } }}></textarea>
    <footer><span>{busy ? phase : 'Enter to capture · Esc to dismiss'}</span><button class="primary-button" disabled={busy || !input.trim() || !desktop()}>{busy ? 'Adding…' : 'Capture task'} <span>↵</span></button></footer>
  </form>
  {#if saved}<p class="capture-success" role="status">✓ {saved}</p>{/if}
  {#if error}<p class="capture-error" role="alert">{error} <button onclick={() => invoke('show_main')}>Open Cadence</button></p>{/if}
</main>
