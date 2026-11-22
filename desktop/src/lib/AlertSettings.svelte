<script lang="ts">
  import { invoke } from '$lib/bridge';
  import { reference, repeatLabels, reminderWhen, localToIso, isoToLocal } from '$lib/types';
  import type { AlertsView, AlertSettings, Repeat, Task } from '$lib/types';

  let { alerts, tasks = [], preview = false, onchange }: { alerts: AlertsView; tasks?: Task[]; preview?: boolean; onchange: (view: AlertsView) => void } = $props();

  const presets = [
    { minutes: 10080, label: '1 week' }, { minutes: 4320, label: '3 days' }, { minutes: 1440, label: '1 day' },
    { minutes: 180, label: '3 hours' }, { minutes: 60, label: '1 hour' }, { minutes: 15, label: '15 min' }, { minutes: 0, label: 'When due' },
  ];
  const leadLabel = (m: number) => presets.find(p => p.minutes === m)?.label ?? (m % 1440 === 0 ? `${m / 1440} days` : m % 60 === 0 ? `${m / 60} hours` : `${m} min`);
  const names = {
    macos: { badge: 'Dock badge', title: 'Menu bar text', tray: 'menu bar' },
    windows: { badge: 'Taskbar badge', title: '', tray: 'notification area' },
    linux: { badge: 'Launcher badge', title: 'Panel label', tray: 'panel indicator' },
  };

  let draft = $state<AlertSettings>(structuredClone($state.snapshot(alerts.settings)));
  let dirty = $state(false);
  let saving = $state(false);
  let error = $state('');
  let notice = $state('');
  let customAmount = $state<number | undefined>();
  let customUnit = $state(60);
  let newTitle = $state('');
  let newAt = $state(isoToLocal(new Date(Date.now() + 3600000)));
  let newRepeat = $state<Repeat>('none');
  let newNote = $state('');
  let label = $derived(names[alerts.platform]);
  let paused = $derived(Boolean(alerts.settings.pausedUntil && Date.parse(alerts.settings.pausedUntil) > Date.now()));
  $effect(() => { if (!dirty) draft = structuredClone($state.snapshot(alerts.settings)); });

  const touch = () => { dirty = true; notice = ''; };
  function toggleLead(minutes: number) {
    draft.leadMinutes = draft.leadMinutes.includes(minutes) ? draft.leadMinutes.filter(m => m !== minutes) : [...draft.leadMinutes, minutes].sort((a, b) => b - a);
    touch();
  }
  function addCustomLead() {
    const minutes = Math.round((customAmount ?? 0) * customUnit);
    if (!Number.isFinite(minutes) || minutes < 1 || minutes > 43200) { error = 'Choose between 1 minute and 30 days.'; return; }
    if (!draft.leadMinutes.includes(minutes)) { draft.leadMinutes = [...draft.leadMinutes, minutes].sort((a, b) => b - a); touch(); }
    customAmount = undefined; error = '';
  }
  async function run(command: string, args: Record<string, unknown>, done = '') {
    if (preview) { error = 'Open the desktop app to change alerts.'; return; }
    saving = true; error = '';
    try { const view = await invoke<AlertsView>(command, args); if (command === 'save_alert_settings') dirty = false; onchange(view); notice = done; }
    catch (e) { error = String(e); } finally { saving = false; }
  }
  const save = () => run('save_alert_settings', { settings: { ...$state.snapshot(draft), pausedUntil: alerts.settings.pausedUntil } }, 'Alert settings saved.');
  async function test() {
    if (preview) { error = 'Open the desktop app to send a test notification.'; return; }
    error = ''; try { await invoke('test_notification'); notice = 'Test notification sent. If nothing appeared, allow Cadence in your system notification settings.'; } catch (e) { error = String(e); }
  }
  async function addReminder() {
    const at = localToIso(newAt);
    if (!newTitle.trim() || !at) { error = 'Give the reminder a title and time.'; return; }
    await run('save_reminder', { input: { taskId: null, title: newTitle, note: newNote, at, repeat: newRepeat, enabled: true } }, 'Reminder added.');
    if (!error) { newTitle = ''; newNote = ''; newRepeat = 'none'; }
  }
</script>

<form class="alert-settings" onsubmit={event => { event.preventDefault(); save(); }}>
  <p class="canvas-connect-intro">Cadence uses your system’s own notifications{alerts.platform === 'macos' ? ' (Notification Center)' : alerts.platform === 'windows' ? ' (Windows notifications)' : ' (your desktop’s notification service)'}. Alerts cover your real workspace, not the demo.</p>
  <label class="canvas-auto"><input type="checkbox" bind:checked={draft.enabled} onchange={touch} /><span><strong>Allow notifications</strong></span></label>

  <fieldset disabled={!draft.enabled}>
    <legend>Due dates</legend>
    <label class="canvas-auto"><input type="checkbox" bind:checked={draft.dueAlerts} onchange={touch} /><span>Alert me before tasks are due</span></label>
    <div class="lead-chips" role="group" aria-label="Alert times before a deadline">
      {#each [...new Set([...presets.map(p => p.minutes), ...draft.leadMinutes])].sort((a, b) => b - a) as minutes (minutes)}
        <button type="button" class:chosen={draft.leadMinutes.includes(minutes)} aria-pressed={draft.leadMinutes.includes(minutes)} disabled={!draft.dueAlerts} onclick={() => toggleLead(minutes)}>{leadLabel(minutes)}{minutes ? ' before' : ''}</button>
      {/each}
    </div>
    <div class="inline-row">
      <input type="number" min="1" max="720" placeholder="Custom" aria-label="Custom alert amount" bind:value={customAmount} disabled={!draft.dueAlerts} />
      <select aria-label="Custom alert unit" bind:value={customUnit} disabled={!draft.dueAlerts}><option value={1}>minutes</option><option value={60}>hours</option><option value={1440}>days</option></select>
      <button type="button" class="text-button" disabled={!draft.dueAlerts || !customAmount} onclick={addCustomLead}>Add before due</button>
    </div>
    <div class="two-col">
      <label>Tasks without a time are due at<input type="time" bind:value={draft.allDayTime} oninput={touch} required /></label>
      <label>Alert for<select bind:value={draft.minPriority} onchange={touch}><option value={0}>Every task</option><option value={50}>Medium priority and up</option><option value={75}>High priority only</option></select></label>
    </div>
    <label class="canvas-auto"><input type="checkbox" bind:checked={draft.overdueDaily} onchange={touch} /><span>Remind me each morning while a task is overdue</span></label>
  </fieldset>

  <fieldset disabled={!draft.enabled}>
    <legend>Delivery</legend>
    <label class="canvas-auto"><input type="checkbox" bind:checked={draft.reminders} onchange={touch} /><span>Custom reminders</span></label>
    <label class="canvas-auto"><input type="checkbox" bind:checked={draft.sound} onchange={touch} /><span>Play the system alert sound</span></label>
    <label class="canvas-auto"><input type="checkbox" bind:checked={draft.quietHours} onchange={touch} /><span>Quiet hours — hold alerts and deliver them afterwards</span></label>
    {#if draft.quietHours}<div class="two-col"><label>From<input type="time" bind:value={draft.quietStart} oninput={touch} required /></label><label>Until<input type="time" bind:value={draft.quietEnd} oninput={touch} required /></label></div>{/if}
    <div class="inline-row">
      {#if paused}<span class="muted">Paused until {new Date(alerts.settings.pausedUntil || '').toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' })}</span><button type="button" class="text-button" disabled={saving} onclick={() => run('pause_alerts', { minutes: null }, 'Alerts resumed.')}>Resume now</button>
      {:else}<span class="muted">Pause alerts</span><button type="button" class="text-button" disabled={saving || !alerts.settings.enabled} onclick={() => run('pause_alerts', { minutes: 60 }, 'Alerts paused for an hour.')}>1 hour</button><button type="button" class="text-button" disabled={saving || !alerts.settings.enabled} onclick={() => run('pause_alerts', { minutes: 240 }, 'Alerts paused for 4 hours.')}>4 hours</button>{/if}
      <button type="button" class="text-button push" onclick={test}>Send test notification</button>
    </div>
  </fieldset>

  <fieldset>
    <legend>Status bar & {alerts.platform === 'macos' ? 'Dock' : alerts.platform === 'windows' ? 'taskbar' : 'dock'}</legend>
    <div class="two-col">
      <label>{label.badge}<select bind:value={draft.badge} onchange={touch}><option value="due_today">Due today & overdue</option><option value="overdue">Overdue only</option><option value="off">Off</option></select></label>
      {#if label.title}<label>{label.title}<select bind:value={draft.trayTitle} onchange={touch}><option value="timer">Focus timer</option><option value="due">Due count</option><option value="both">Timer and due count</option><option value="none">Icon only</option></select></label>{/if}
    </div>
    <label class="canvas-auto"><input type="checkbox" bind:checked={draft.dockProgress} onchange={touch} /><span>Show focus progress against the estimate on the app icon</span></label>
    {#if alerts.platform !== 'macos'}<label class="canvas-auto"><input type="checkbox" bind:checked={draft.keepRunning} onchange={touch} /><span>Keep Cadence in the {label.tray} when the window closes, so alerts keep arriving</span></label>{/if}
    <p class="settings-note">The {label.tray} item shows your focus timer, what’s due today, and your next reminder.{alerts.platform === 'linux' ? ' Badges need a launcher that supports them, such as KDE, Dash to Dock, or Plank.' : ''}</p>
  </fieldset>

  {#if error}<div class="settings-error" role="alert">{error}</div>{/if}
  {#if notice}<p class="settings-note" role="status">{notice}</p>{/if}
  <button class="primary-button save-settings" disabled={saving || preview || !dirty}>{saving ? 'Saving…' : dirty ? 'Save alert settings' : 'Saved'}<span>→</span></button>
</form>

<section class="alert-settings reminder-manager" aria-labelledby="reminders-heading">
  <h3 id="reminders-heading">Reminders</h3>
  {#if !alerts.reminders.length}<p class="settings-note">No reminders yet. Add one here, or from a task’s card.</p>{/if}
  <ul class="reminder-list">
    {#each alerts.reminders as reminder (reminder.id)}
      <li class:off={!reminder.enabled}>
        <div><strong>{reminder.title}</strong><small>{reminder.enabled ? reminderWhen(reminder.at) : 'Finished'} · {repeatLabels[reminder.repeat]}{#if reminder.taskId} · {reference(reminder.taskId)}{#if tasks.find(t => t.id === reminder.taskId)} {tasks.find(t => t.id === reminder.taskId)?.title}{/if}{/if}</small></div>
        {#if !reminder.enabled && reminder.repeat !== 'none'}<button type="button" class="text-button" disabled={saving} onclick={() => run('save_reminder', { input: { ...reminder, enabled: true } }, 'Reminder resumed.')}>Resume</button>{/if}
        {#if reminder.enabled}<button type="button" class="text-button" disabled={saving} onclick={() => run('save_reminder', { input: { ...reminder, enabled: false } }, 'Reminder turned off.')}>Turn off</button>{/if}
        <button type="button" class="text-button" disabled={saving} aria-label={`Delete reminder ${reminder.title}`} onclick={() => run('delete_reminder', { id: reminder.id }, 'Reminder deleted.')}>Delete</button>
      </li>
    {/each}
  </ul>
  <form class="new-reminder" onsubmit={event => { event.preventDefault(); addReminder(); }}>
    <label>New reminder<input bind:value={newTitle} maxlength="200" placeholder="Stretch, review notes, submit timesheet…" /></label>
    <div class="two-col"><label>When<input type="datetime-local" bind:value={newAt} required /></label><label>Repeat<select bind:value={newRepeat}>{#each Object.entries(repeatLabels) as [value, text] (value)}<option {value}>{text}</option>{/each}</select></label></div>
    <label>Note <span class="muted">(optional)</span><input bind:value={newNote} maxlength="1000" /></label>
    <button class="secondary-button" disabled={saving || preview || !newTitle.trim()}>Add reminder</button>
  </form>
</section>

<style>
  .alert-settings fieldset { border: 0; border-top: 1px solid var(--line); margin: 16px 0 0; padding: 14px 0 0; display: grid; gap: 10px; min-width: 0; }
  .alert-settings fieldset:disabled { opacity: .55; }
  .alert-settings legend { font-size: 9px; letter-spacing: 1.1px; text-transform: uppercase; color: var(--muted); padding: 0 0 4px; }
  .lead-chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .lead-chips button { padding: 5px 9px; border-radius: 999px; border: 1px solid var(--line); background: transparent; color: var(--secondary-text); font-size: 11px; }
  .lead-chips button.chosen { background: var(--accent); border-color: var(--accent); color: var(--surface); font-weight: 550; }
  .inline-row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; font-size: 11px; }
  .inline-row input { width: 86px; }
  .inline-row .push { margin-left: auto; }
  .two-col { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .muted { color: var(--muted); font-size: 11px; }
  .reminder-manager { margin-top: 22px; border-top: 1px solid var(--line); padding-top: 16px; }
  .reminder-manager h3 { font-size: 13px; margin: 0 0 8px; }
  .reminder-list { list-style: none; margin: 0 0 12px; padding: 0; display: grid; gap: 6px; }
  .reminder-list li { display: flex; align-items: center; gap: 8px; padding: 8px 10px; border: 1px solid var(--line); border-radius: 8px; }
  .reminder-list li.off { opacity: .6; }
  .reminder-list li > div { display: grid; gap: 2px; flex: 1; min-width: 0; }
  .reminder-list small { color: var(--secondary-text); font-size: 10px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .new-reminder { display: grid; gap: 10px; }
</style>
