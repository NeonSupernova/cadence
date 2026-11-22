<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke, desktop } from '$lib/bridge';
  import type { CanvasItem, CanvasMutationResult } from '$lib/canvas-types';
  import CanvasContent from '$lib/CanvasContent.svelte';
  let { courseId, assignmentId, baseUrl, preview = false, onsubmitted = () => {}, onbusy = () => {} }: {
    courseId: number; assignmentId: number; baseUrl: string; preview?: boolean; onsubmitted?: () => void; onbusy?: (busy:boolean) => void;
  } = $props();
  let assignment = $state<CanvasItem | null>(null);
  let submission = $state<CanvasItem | null>(null);
  let loading = $state(true);
  let sending = $state(false);
  let error = $state('');
  let feedbackError = $state('');
  let notice = $state('');
  let kind = $state('');
  let body = $state('');
  let url = $state('');
  let files = $state<File[]>([]);
  let comment = $state('');
  let reviewing = $state(false);
  let receipt = $state<CanvasItem | null>(null);
  let draftNote = $state('');
  let draftKey = $derived(`cadence.canvas.draft:${baseUrl}:${courseId}:${assignmentId}`);
  const types = [{id:'online_upload',label:'File upload'}, {id:'online_text_entry',label:'Text entry'}, {id:'online_url',label:'Website URL'}];
  let supported = $derived(types.filter(t => assignment?.submission_types?.includes(t.id)));
  let remaining = $derived(assignment?.allowed_attempts != null && assignment.allowed_attempts >= 0 ? Math.max(0, assignment.allowed_attempts + Math.max(0,submission?.extra_attempts || 0) - (submission?.attempt || 0)) : null);
  let unavailable = $derived(Boolean(assignment?.locked_for_user || remaining === 0 || assignment?.published === false));
  let valid = $derived(kind === 'online_text_entry' ? Boolean(body.trim()) : kind === 'online_url' ? /^https?:\/\//i.test(url.trim()) : files.length > 0);
  const time = (value?: string | null) => value ? new Date(value).toLocaleString() : 'Not set';
  async function load() {
    loading = true; error = ''; feedbackError = '';
    try {
      if (preview) {
        assignment = { id:assignmentId, name:'Iteration 4 · Implementation & review', description_text:'Build the next iteration, document your decisions, and upload your deliverable.\nUse the rubric to check your work before submitting.', points_possible:100, submission_types:['online_upload','online_text_entry','online_url'], allowed_attempts:3, due_at:new Date(Date.now()+86400000).toISOString(), rubric:[{id:'quality',description:'Implementation and validation',points:70},{id:'docs',description:'Documentation and reflection',points:30}] };
        submission = {id:0,workflow_state:'unsubmitted',attempt:0,submission_comments:[]};
      } else {
        const results = await Promise.allSettled([
          invoke<CanvasItem>('canvas_workspace_query', {query:{resource:'assignment',courseId,id:assignmentId}}),
          invoke<CanvasItem>('canvas_workspace_query', {query:{resource:'submission',courseId,id:assignmentId}})
        ]);
        if (results[0].status === 'rejected') throw results[0].reason;
        assignment = results[0].value;
        if (results[1].status === 'fulfilled') submission = results[1].value;
        else { submission = assignment.submission || null; feedbackError = `Submission history unavailable: ${String(results[1].reason)}`; }
      }
      if (!supported.some(t => t.id === kind)) kind = supported[0]?.id || '';
    } catch(e) { error = String(e); }
    finally { loading = false; }
  }
  function saveDraft() {
    reviewing = false;
    try { localStorage.setItem(draftKey, JSON.stringify({kind,body,url})); draftNote = 'Draft saved on this Mac'; }
    catch { draftNote = 'Draft could not be saved. Keep this view open to preserve it.'; }
  }
  function discardDraft() {
    body=''; url=''; files=[]; reviewing=false;
    try { localStorage.removeItem(draftKey); } catch { /* The form can still be cleared. */ }
    draftNote='Draft cleared';
  }
  function chooseFiles(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const chosen = Array.from(input.files || []);
    if (chosen.length > 20 || chosen.reduce((sum,f) => sum+f.size,0) > 50*1024*1024) {
      error='Choose up to 20 files totaling no more than 50 MB.'; input.value=''; return;
    }
    files=chosen; reviewing=false; error='';
  }
  async function submit() {
    if (sending || !valid || unavailable || !reviewing || preview) return;
    sending=true; onbusy(true); error=''; notice='';
    try {
      const payload = kind === 'online_upload' ? await Promise.all(files.map(async f => ({name:f.name,contentType:f.type || 'application/octet-stream',bytes:Array.from(new Uint8Array(await f.arrayBuffer()))}))) : [];
      const response = await invoke<CanvasMutationResult>('canvas_workspace_action', {action:{kind:'submit',courseId,assignmentId,submissionType:kind,text:body,url,files:payload}});
      receipt=response.result;
      notice = `Canvas accepted attempt ${receipt.attempt || '—'} · ${time(receipt.submitted_at)}.${response.syncWarning ? ` Local task sync needs a retry: ${response.syncWarning}` : ''}`;
      discardDraft(); onsubmitted(); await load();
    } catch(e) { error=String(e); reviewing=false; }
    finally { sending=false; onbusy(false); }
  }
  async function postComment() {
    if (sending || !comment.trim() || preview) return;
    sending=true; onbusy(true); error='';
    try {
      await invoke('canvas_workspace_action',{action:{kind:'comment',courseId,assignmentId,text:comment}});
      comment=''; notice='Comment posted to Canvas.'; await load();
    } catch(e) { error=String(e); }
    finally { sending=false; onbusy(false); }
  }
  async function openCanvas() { try { await invoke('open_canvas_link',{path:`/courses/${courseId}/assignments/${assignmentId}`}); } catch(e) { error=String(e); } }
  onMount(() => {
    try { const draft=JSON.parse(localStorage.getItem(draftKey) || 'null'); if(draft) { body=draft.body || ''; url=draft.url || ''; kind=draft.kind || ''; draftNote='Restored draft from this Mac'; } } catch { /* An unreadable draft does not block Canvas. */ }
    void load();
  });
</script>

<section class="cw-assignment" aria-busy={loading || sending}>
  {#if error}<div class="cw-error" role="alert">{error}<button disabled={sending || loading} onclick={load}>Refresh status</button></div>{/if}
  {#if notice}<div class="cw-success" role="status">{notice}</div>{/if}
  {#if loading}<p class="cw-empty">Loading assignment and your submission…</p>
  {:else if assignment}
    <div class="cw-detail-heading"><div><span class="section-kicker">CANVAS ASSIGNMENT</span><h2>{assignment.name}</h2></div><button class="secondary-button" disabled={preview || sending} onclick={openCanvas}>Open in Canvas ↗</button></div>
    <div class="cw-facts"><div><small>Deadline</small><strong>{time(assignment.due_at)}</strong></div><div><small>Your submission</small><strong>{submission?.workflow_state?.replaceAll('_',' ') || 'Not submitted'}</strong></div><div><small>Grade</small><strong>{submission?.grade ?? 'Not graded'}{assignment.points_possible != null ? ` / ${assignment.points_possible}` : ''}</strong></div><div><small>Attempts left</small><strong>{remaining ?? 'Unlimited'}</strong></div></div>
    {#if submission?.late}<p class="cw-warning">Canvas marks this submission as late.</p>{/if}
    {#if submission?.missing}<p class="cw-warning">Canvas marks this assignment as missing.</p>{/if}
    {#if assignment.lock_explanation_text}<p class="cw-warning">{assignment.lock_explanation_text}</p>{/if}
    <section class="cw-section"><h3>Requirements</h3><CanvasContent text={assignment.description_text} html={assignment.description}/></section>
    {#if assignment.rubric?.length}<section class="cw-section"><h3>Rubric</h3>{#each assignment.rubric as criterion}<div class="cw-rubric"><div><strong>{criterion.description}</strong>{#if criterion.long_description}<p>{criterion.long_description}</p>{/if}{#if criterion.ratings?.length}<small>{criterion.ratings.map(r=>`${r.description}: ${r.points}`).join(' · ')}</small>{/if}{#if submission?.rubric_assessment?.[criterion.id]?.comments}<p>{submission.rubric_assessment[criterion.id].comments}</p>{/if}</div><strong>{submission?.rubric_assessment?.[criterion.id]?.points ?? '—'} / {criterion.points}</strong></div>{/each}</section>{/if}
    <section class="cw-section cw-submit"><div class="cw-detail-heading"><h3>{submission?.submitted_at?'Submit another attempt':'Submit your work'}</h3><span>{draftNote}</span></div>
      {#if supported.length}
        <form onsubmit={event=>{event.preventDefault(); if(valid && !unavailable) reviewing=true;}}>
          <fieldset disabled={sending || unavailable || preview}>
            <label>Submission type<select bind:value={kind} onchange={saveDraft}>{#each supported as option}<option value={option.id}>{option.label}</option>{/each}</select></label>
            {#if kind==='online_text_entry'}<label>Your submission<textarea rows="9" bind:value={body} oninput={saveDraft} placeholder="Write or paste your work here. Text is submitted as paragraphs." required maxlength="200000"></textarea></label>
            {:else if kind==='online_url'}<label>Website URL<input type="url" bind:value={url} oninput={saveDraft} placeholder="https://…" required /></label>
            {:else if kind==='online_upload'}<label>Files to submit<input type="file" multiple accept={assignment.allowed_extensions?.map(ext=>'.'+ext).join(',') || undefined} onchange={chooseFiles}/></label><p class="cw-muted">Up to 20 files · 50 MB total{assignment.allowed_extensions?.length ? ` · Accepted: ${assignment.allowed_extensions.join(', ')}` : ''}</p>{#each files as file}<div class="cw-file"><span>{file.name}</span><small>{(file.size/1024).toFixed(1)} KB</small><button type="button" onclick={()=>{files=files.filter(f=>f!==file);reviewing=false;}} aria-label={`Remove ${file.name}`}>×</button></div>{/each}{/if}
            <div class="cw-actions"><button class="primary-button" disabled={!valid}>Review submission →</button><button type="button" class="secondary-button" onclick={discardDraft}>Clear draft</button></div>
          </fieldset>
        </form>
        {#if unavailable}<p class="cw-warning">Submissions are unavailable or no attempts remain. Check the assignment in Canvas.</p>{/if}
        {#if reviewing}<div class="cw-review"><h3>Ready to submit to Canvas</h3><p>{assignment.name} · {types.find(t=>t.id===kind)?.label}</p>{#if kind==='online_text_entry'}<div class="cw-prose">{body}</div>{:else if kind==='online_url'}<p>{url}</p>{:else}<ul>{#each files as file}<li>{file.name} · {(file.size/1024).toFixed(1)} KB</li>{/each}</ul>{/if}<p>This sends your work to the course and uses a submission attempt.</p><div class="cw-actions"><button class="primary-button" disabled={sending || preview} onclick={submit}>{sending?'Sending to Canvas…':'Submit to Canvas'}</button><button class="secondary-button" disabled={sending} onclick={()=>reviewing=false}>Keep editing</button></div></div>{/if}
      {:else}<p class="cw-muted">This assignment uses {assignment.submission_types?.join(', ').replaceAll('_',' ') || 'an external submission flow'}. Complete it in Canvas.</p>{/if}
    </section>
    {#if submission?.submitted_at}<section class="cw-section"><h3>Latest submission</h3><p class="cw-muted">Attempt {submission.attempt} · {time(submission.submitted_at)}</p><CanvasContent text={submission.body_text} html={submission.body}/>{#if submission.url}<p>{submission.url}</p>{/if}{#each submission.attachments || [] as file}<button class="cw-resource-link" disabled={preview} onclick={()=>invoke('open_canvas_link',{path:`/courses/${courseId}/files/${file.id}/download`}).catch(e=>error=String(e))}>{file.display_name || file.filename} ↗</button>{/each}</section>{/if}
    {#if submission?.submission_history?.length}<details class="cw-section"><summary>Submission history · {submission.submission_history.length} attempts</summary>{#each submission.submission_history as attempt}<div class="cw-history"><strong>Attempt {attempt.attempt} · {attempt.workflow_state}</strong><small>{time(attempt.submitted_at)} · Grade {attempt.grade ?? '—'}</small><CanvasContent text={attempt.body_text} html={attempt.body}/>{#each attempt.attachments || [] as file}<button class="cw-resource-link" disabled={preview} onclick={()=>invoke('open_canvas_link',{path:`/courses/${courseId}/files/${file.id}/download`}).catch(e=>error=String(e))}>{file.display_name || file.filename} ↗</button>{/each}</div>{/each}</details>{/if}
    <section class="cw-section"><h3>Feedback & comments</h3>{#if feedbackError}<p class="cw-warning">{feedbackError}</p>{/if}{#each submission?.submission_comments || [] as entry}<article class="cw-comment"><strong>{entry.author_name}</strong><small>{time(entry.created_at)}</small><div class="cw-prose">{entry.comment}</div></article>{/each}{#if !submission?.submission_comments?.length}<p class="cw-muted">No comments yet.</p>{/if}<form onsubmit={event=>{event.preventDefault();postComment();}}><label>Add a comment<textarea rows="3" bind:value={comment} placeholder="Ask a question or add context for your instructor" required disabled={sending || preview} maxlength="200000"></textarea></label><button class="secondary-button" disabled={sending || preview || !comment.trim()}>Post comment to Canvas</button></form></section>
  {/if}
</section>
