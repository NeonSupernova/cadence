<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke, desktop } from '$lib/bridge';
  import { defaultMindful, type MindfulSettings, type MindfulStatus, type Reminder } from '$lib/types';

  let { onsaved }: { onsaved?: (message: string) => void } = $props();

  let draft = $state<MindfulSettings>({ ...defaultMindful });
  let quietUntil = $state<string | null>(null);
  let busy = $state(false);
  let error = $state('');
  const live = desktop();

  const minutes = [0, 10, 15, 20, 25, 30, 45, 60, 90, 120];
  const label = (m: number) => (m === 0 ? 'Off' : m < 60 ? `Every ${m} min` : m === 60 ? 'Every hour' : `Every ${m / 60} h`.replace('1.5 h', '90 min'));
  const rows: { key: 'waterMinutes' | 'eyesMinutes' | 'stretchMinutes'; kind: Reminder; icon: string; title: string; hint: string }[] = [
    { key: 'waterMinutes', kind: 'water', icon: '💧', title: 'Drink water', hint: 'A few sips keeps focus steadier.' },
    { key: 'eyesMinutes', kind: 'eyes', icon: '👀', title: 'Rest your eyes', hint: 'The 20-20-20 rule: look 20 feet away for 20 seconds.' },
    { key: 'stretchMinutes', kind: 'stretch', icon: '🧠', title: 'Brain break', hint: 'Stand, stretch, walk around for a minute.' }
  ];

  function apply(status: MindfulStatus) { draft = { ...status.settings }; quietUntil = status.quietUntil; }
  async function load() {
    if (!live) return;
    try { apply(await invoke<MindfulStatus>('get_mindful_settings')); } catch (e) { error = String(e); }
  }
  async function save() {
    if (!live) { error = 'Open the desktop app to change break reminders.'; return; }
    busy = true; error = '';
    try {
      apply(await invoke<MindfulStatus>('save_mindful_settings', { settings: { ...draft } }));
      onsaved?.(draft.enabled ? 'Break reminders saved' : 'Break reminders turned off');
    } catch (e) { error = String(e); }
    finally { busy = false; }
  }
  async function preview(kind: Reminder) {
    if (!live) { error = 'Open the desktop app to preview a reminder.'; return; }
    error = '';
    try { await invoke('mindful_preview', { kind }); } catch (e) { error = String(e); }
  }
  async function quiet(mins: number) {
    if (!live) return;
    try { apply(await invoke<MindfulStatus>('mindful_quiet', { minutes: mins })); } catch (e) { error = String(e); }
  }
  onMount(load);
</script>

<form class="settings-section" onsubmit={e => { e.preventDefault(); save(); }}>
  <label class="toggle-row"><input type="checkbox" bind:checked={draft.enabled} /><span><strong>Remind me to take small breaks</strong><small>Only while a timer is running. A pause of 5 minutes or more counts as a break.</small></span></label>

  <div class="break-rows" class:dimmed={!draft.enabled}>
    {#each rows as row (row.key)}
      <div class="break-row">
        <span class="break-icon" aria-hidden="true">{row.icon}</span>
        <span class="break-copy"><strong>{row.title}</strong><small>{row.hint}</small></span>
        <select bind:value={draft[row.key]} disabled={!draft.enabled} aria-label={`${row.title} interval`}>
          {#each minutes as m}<option value={m}>{label(m)}</option>{/each}
        </select>
        <button type="button" class="text-button" onclick={() => preview(row.kind)} title="Show this reminder now">Preview</button>
      </div>
    {/each}
  </div>

  <label class="inline-field">Keep each reminder on screen for
    <select bind:value={draft.displaySeconds} disabled={!draft.enabled}>
      {#each [10, 15, 25, 40, 60, 90] as s}<option value={s}>{s} seconds</option>{/each}
    </select>
  </label>

  <div class="quiet-row">
    {#if quietUntil}
      <span>🔕 Paused until {new Date(quietUntil).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })}</span>
      <button type="button" class="text-button" onclick={() => quiet(0)}>Resume now</button>
    {:else}
      <span>Need to focus without interruptions?</span>
      <button type="button" class="secondary-button" onclick={() => quiet(60)} disabled={!live}>Pause 1 hour</button>
      <button type="button" class="secondary-button" onclick={() => quiet(180)} disabled={!live}>Pause 3 hours</button>
    {/if}
  </div>

  <p class="settings-note">Reminders appear as a small banner at the top of your screen, above other apps and full-screen windows. Snooze brings one back in 10 minutes; Skip waits a full interval.</p>
  {#if error}<div class="settings-error" role="alert">{error}</div>{/if}
  <button class="primary-button save-settings" disabled={busy || !live}>{busy ? 'Saving…' : 'Save break reminders'}<span>→</span></button>
</form>
