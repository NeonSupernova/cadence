<script lang="ts">
  import { invoke } from '$lib/bridge';
  import { repeatLabels, reminderWhen, localToIso, isoToLocal } from '$lib/types';
  import type { AlertsView, Repeat, Task } from '$lib/types';

  let { task, alerts, unavailable = '', onchange }: { task: Task; alerts: AlertsView; unavailable?: string; onchange: (view: AlertsView) => void } = $props();

  let custom = $state(false);
  let at = $state('');
  let repeat = $state<Repeat>('none');
  let busy = $state(false);
  let error = $state('');
  let mine = $derived(alerts.reminders.filter(r => r.taskId === task.id && r.enabled));
  let off = $derived(!alerts.settings.enabled || !alerts.settings.reminders);

  function preset(kind: 'hour' | 'evening' | 'tomorrow' | 'beforeDue'): Date | null {
    const d = new Date();
    if (kind === 'hour') return new Date(d.getTime() + 3600000);
    if (kind === 'evening') { d.setHours(18, 0, 0, 0); return d > new Date() ? d : null; }
    if (kind === 'tomorrow') { d.setDate(d.getDate() + 1); d.setHours(9, 0, 0, 0); return d; }
    if (!task.dueDate) return null;
    const due = task.canvas?.dueAt ? new Date(task.canvas.dueAt) : new Date(`${task.dueDate}T${alerts.settings.allDayTime}`);
    const before = new Date(due.getTime() - 86400000);
    return before > new Date() ? before : null;
  }
  async function add(when: Date | string | null, every: Repeat = 'none') {
    if (unavailable) { error = unavailable; return; }
    const iso = typeof when === 'string' ? localToIso(when) : when?.toISOString();
    if (!iso) { error = 'Choose a valid time.'; return; }
    busy = true; error = '';
    try { onchange(await invoke<AlertsView>('save_reminder', { input: { taskId: task.id, title: task.title, note: '', at: iso, repeat: every, enabled: true } })); custom = false; repeat = 'none'; }
    catch (e) { error = String(e); } finally { busy = false; }
  }
  async function remove(id: string) {
    busy = true; error = '';
    try { onchange(await invoke<AlertsView>('delete_reminder', { id })); } catch (e) { error = String(e); } finally { busy = false; }
  }
  function openCustom() { custom = !custom; at = isoToLocal(new Date(Date.now() + 3600000)); }
</script>

<section class="task-reminders linked-summary" aria-labelledby={`reminders-${task.id}`}>
  <h3 id={`reminders-${task.id}`}>Remind me</h3>
  {#each mine as reminder (reminder.id)}
    <div class="reminder-row"><span>🔔 {reminderWhen(reminder.at)}{reminder.repeat !== 'none' ? ` · ${repeatLabels[reminder.repeat].toLowerCase()}` : ''}</span><button class="text-button" disabled={busy} aria-label="Remove reminder" onclick={() => remove(reminder.id)}>Remove</button></div>
  {/each}
  <div class="reminder-presets">
    <button class="secondary-button" disabled={busy} onclick={() => add(preset('hour'))}>In 1 hour</button>
    {#if preset('evening')}<button class="secondary-button" disabled={busy} onclick={() => add(preset('evening'))}>This evening</button>{/if}
    <button class="secondary-button" disabled={busy} onclick={() => add(preset('tomorrow'))}>Tomorrow 9 AM</button>
    {#if preset('beforeDue')}<button class="secondary-button" disabled={busy} onclick={() => add(preset('beforeDue'))}>Day before due</button>{/if}
    <button class="text-button" aria-expanded={custom} onclick={openCustom}>Custom…</button>
  </div>
  {#if custom}
    <form class="reminder-custom" onsubmit={event => { event.preventDefault(); add(at, repeat); }}>
      <input type="datetime-local" aria-label="Reminder time" bind:value={at} required />
      <select aria-label="Repeat" bind:value={repeat}>{#each Object.entries(repeatLabels) as [value, text] (value)}<option {value}>{text}</option>{/each}</select>
      <button class="primary-button" disabled={busy}>Set</button>
    </form>
  {/if}
  {#if off}<p class="hint">Reminders are turned off in Settings → Alerts, so these won’t notify you yet.</p>{/if}
  {#if error}<p class="card-error" role="alert">{error}</p>{/if}
</section>

<style>
  .task-reminders { display: grid; gap: 8px; }
  .reminder-row { display: flex; align-items: center; justify-content: space-between; font-size: 12px; }
  .reminder-presets { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .reminder-presets .secondary-button { font-size: 11px; padding: 5px 10px; }
  .reminder-custom { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
  .reminder-custom input { flex: 1; min-width: 170px; }
  .hint { font-size: 10px; color: var(--muted); margin: 0; }
</style>
