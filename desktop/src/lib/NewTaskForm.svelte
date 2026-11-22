<script lang="ts">
  import { taskTypes, starterLabels } from '$lib/types';
  import type { TaskDraft } from '$lib/types';

  let { knownLabels = [], busy = false, error = '', oncreate, oncancel, onchat }: {
    knownLabels?: string[]; busy?: boolean; error?: string;
    oncreate: (draft: TaskDraft) => void; oncancel: () => void; onchat: (text: string) => void;
  } = $props();

  let title = $state('');
  let description = $state('');
  let taskType = $state('chore');
  let priority = $state(55);
  let dueDate = $state('');
  let estimate = $state<number | undefined>();
  let labels = $state<string[]>([]);
  let labelInput = $state('');
  let localError = $state('');
  let titleField = $state<HTMLInputElement>();
  let choices = $derived([...new Set([...knownLabels, ...starterLabels])].filter(l => l !== 'canvas' && l !== 'inbox').slice(0, 14));
  const today = () => new Date().toLocaleDateString('en-CA');
  const inDays = (n: number) => { const d = new Date(); d.setDate(d.getDate() + n); return d.toLocaleDateString('en-CA'); };

  $effect(() => { titleField?.focus(); });

  function toggle(label: string) { labels = labels.includes(label) ? labels.filter(l => l !== label) : [...labels, label]; }
  function addTypedLabels() {
    const typed = labelInput.split(',').map(l => l.trim().replace(/^#/, '').toLowerCase()).filter(Boolean);
    labels = [...new Set([...labels, ...typed])];
    labelInput = '';
  }
  function submit() {
    addTypedLabels();
    localError = '';
    if (!title.trim()) { localError = 'Give the task a title.'; return; }
    if (estimate !== undefined && estimate !== null && (!Number.isInteger(estimate) || estimate < 5 || estimate > 2400)) { localError = 'Estimates are 5–2400 whole minutes.'; return; }
    oncreate({ title: title.trim(), description: description.trim(), taskType, labels, priority, dueDate: dueDate || null, estimateMinutes: estimate ?? null });
  }
  function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape') { event.preventDefault(); oncancel(); }
    if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) { event.preventDefault(); submit(); }
  }
</script>

<div class="modal-backdrop" role="presentation" onkeydown={keydown}>
  <div class="settings-modal new-task" role="dialog" aria-modal="true" aria-labelledby="new-task-title" tabindex="-1">
    <div class="modal-title"><span class="assistant-mark">＋</span><button aria-label="Close" onclick={oncancel}>×</button></div>
    <h2 id="new-task-title">New task</h2>
    <p>Add it yourself — no assistant needed. Only the title is required.</p>
    <form onsubmit={event => { event.preventDefault(); submit(); }}>
      <label>Title<input bind:this={titleField} bind:value={title} maxlength="140" placeholder="What needs doing?" required /></label>
      <label>Notes <span class="optional">(optional)</span><textarea bind:value={description} rows="3" maxlength="20000" placeholder="Outcome, links, details…"></textarea></label>
      <div class="two-col">
        <label>Type<select bind:value={taskType}>{#each taskTypes as [value, text] (value)}<option {value}>{text}</option>{/each}</select></label>
        <label>Priority<select bind:value={priority}><option value={30}>Low</option><option value={55}>Medium</option><option value={80}>High</option><option value={95}>Urgent</option></select></label>
      </div>
      <div class="two-col">
        <label>Due date<input type="date" bind:value={dueDate} min={today()} /></label>
        <label>Estimate (minutes)<input type="number" min="5" max="2400" step="5" bind:value={estimate} placeholder="Auto" /></label>
      </div>
      <div class="quick-dates" role="group" aria-label="Quick due dates">
        <button type="button" class:chosen={dueDate === today()} onclick={() => dueDate = today()}>Today</button>
        <button type="button" class:chosen={dueDate === inDays(1)} onclick={() => dueDate = inDays(1)}>Tomorrow</button>
        <button type="button" class:chosen={dueDate === inDays(7)} onclick={() => dueDate = inDays(7)}>Next week</button>
        {#if dueDate}<button type="button" onclick={() => dueDate = ''}>No date</button>{/if}
      </div>
      <fieldset class="label-picker"><legend>Labels</legend>
        <div class="chips">{#each [...new Set([...choices, ...labels])] as label (label)}<button type="button" class:chosen={labels.includes(label)} aria-pressed={labels.includes(label)} onclick={() => toggle(label)}>{label}</button>{/each}</div>
        <input bind:value={labelInput} placeholder="Add labels, separated by commas" aria-label="New labels" onkeydown={event => { if (event.key === 'Enter') { event.preventDefault(); addTypedLabels(); } }} />
      </fieldset>
      {#if localError || error}<div class="settings-error" role="alert">{localError || error}</div>{/if}
      <button class="primary-button save-settings" disabled={busy || !title.trim()}>{busy ? 'Creating…' : 'Create task'}<span>⌘↩</span></button>
      <button type="button" class="text-button chat-instead" onclick={() => onchat(title.trim() ? `Help me plan this task: ${title.trim()}` : 'Walk me through creating a task.')}>Plan it with the assistant instead →</button>
    </form>
  </div>
</div>

<style>
  .new-task form { display: grid; gap: 12px; }
  .new-task textarea { width: 100%; resize: vertical; font: inherit; font-size: 13px; }
  .optional { color: var(--muted); }
  .two-col { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .quick-dates, .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .quick-dates button, .chips button { padding: 4px 9px; border-radius: 999px; border: 1px solid var(--line); background: transparent; color: var(--secondary-text); font-size: 11px; }
  .quick-dates button.chosen, .chips button.chosen { background: var(--accent); border-color: var(--accent); color: var(--surface); font-weight: 550; }
  .label-picker { border: 0; padding: 0; margin: 0; display: grid; gap: 8px; min-width: 0; }
  .label-picker legend { font-size: 11px; color: var(--secondary-text); padding: 0 0 6px; }
  .chat-instead { justify-self: center; font-size: 11px; }
</style>
