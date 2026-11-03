<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { invoke, desktop, progress, listen } from '$lib/bridge';
  import { preview } from '$lib/preview';
  import { defaultView, reference } from '$lib/types';
  import type { Snapshot, Settings, Task, View, CanvasSettings, CanvasReport, Estimate, Organization } from '$lib/types';

  let data = $state<Snapshot | null>(null);
  let clock = $state(Date.now());
  let snapshotAt = $state(Date.now());
  let running = $derived(data?.tasks.find(t => !t.deleted && t.timerStartedAt));
  const elapsed = (task: Task) => (task.trackedMs || 0) + (task.timerStartedAt ? Math.max(0, clock - Date.parse(task.timerStartedAt)) : 0);
  const duration = (ms: number) => { const sec = Math.floor(ms / 1000); return `${Math.floor(sec / 3600)}:${String(Math.floor(sec / 60) % 60).padStart(2, '0')}:${String(sec % 60).padStart(2, '0')}`; };
  let totalTracked = $derived(data?.tasks.filter(t => !t.deleted).reduce((sum, task) => sum + elapsed(task), 0) || 0);
  let todayTracked = $derived((data?.stats.trackedTodayMs || 0) + (running ? Math.max(0, clock - snapshotAt) : 0));
  let view = $state<View>(defaultView());
  let selectedId = $state<number | null>(null);
  let input = $state('');
  let pending = $state('');
  let busy = $state(false);
  let chatBusy = $state(false);
  let chatStarted = $state(0);
  let assessmentBusy = $state(false);
  let phase = $state('Organizing your request…');
  let error = $state('');
  let demo = $state(false);
  let browserPreview = $state(false);
  let settingsOpen = $state(false);
  let settingsTab = $state<'assistant' | 'canvas'>('assistant');
  let canvas = $state<CanvasSettings>({ baseUrl: 'https://pcci.instructure.com', configured: false, autoSync: true, report: null, error: null });
  let canvasFormUrl = $state('https://pcci.instructure.com');
  let canvasFormToken = $state('');
  let canvasFormAuto = $state(true);
  let canvasBusy = $state(false);
  let canvasSaving = $state(false);
  let canvasSettingsError = $state('');
  let canvasNotice = $state('');
  let canvasCount = $derived(data?.tasks.filter(t => t.canvas && !t.deleted && t.status !== 'done').length || 0);
  async function refreshCanvas() { if (browserPreview) return; try { canvas = await invoke<CanvasSettings>('get_canvas_settings'); if (canvas.error && !canvasNotice) canvasNotice = canvas.error; } catch (e) { canvasSettingsError = String(e); } }
  function openCanvasSettings() { void openSettings(); settingsTab = 'canvas'; }
  function showCanvas() { if (busy) return; if (!canvas.configured) { openCanvasSettings(); return; } demo = false; selectedId = null; navigate('all', 'canvas'); }
  async function syncCanvas() {
    if (canvasBusy || browserPreview) return;
    if (!canvas.configured) { openCanvasSettings(); return; }
    canvasBusy = true; canvasSettingsError = ''; canvasNotice = '';
    try {
      const report = await invoke<CanvasReport>('sync_canvas');
      canvasNotice = `Canvas synced · ${report.created} added · ${report.updated} updated · ${report.assignments} assignments in ${report.courses} courses`;
      await refreshCanvas(); await refresh();
    } catch (e) { canvasSettingsError = String(e); canvasNotice = String(e); }
    finally { canvasBusy = false; }
  }
  async function saveCanvas() {
    canvasSaving = true; canvasSettingsError = '';
    try {
      canvas = await invoke<CanvasSettings>('save_canvas_settings', { baseUrl: canvasFormUrl, token: canvasFormToken || null, autoSync: canvasFormAuto });
      canvasFormToken = ''; await syncCanvas();
    } catch (e) { canvasSettingsError = String(e); }
    finally { canvasSaving = false; }
  }
  async function openCanvasTask(task: Task) { try { await invoke('open_canvas_assignment', { id: task.id }); } catch (e) { error = String(e); } }
  let settings = $state<Settings>({ configured: false, model: 'google/gemini-2.5-flash', jevModel: '~typesafe/jev-latest', reasoningEffort: 'low', labelThreshold: 0.65 });
  let apiKey = $state('');
  let settingsBusy = $state(false);
  let settingsError = $state('');
  let chatEnd: HTMLDivElement;
  let composer: HTMLTextAreaElement;
  let screen = $state<'now' | 'next' | 'journal' | 'week'>('now');
  let chatOpen = $state(false);
  let feedbackTaskId = $state<number | null>(null);
  let notice = $state('');
  let estimateDraft = $state<Estimate>({minutes:60,technical:3,tedium:3,ambiguity:3,switching:3,source:'user'});
  let estimateDirty = $state(false);
  let actualDraft = $state<number | undefined>();
  let formTaskId = $state<number | null>(null);
  let capacityDraft = $state([120,120,120,120,120,60,60]);
  let learningDraft = $state(true);
  let capacityDirty = $state(false);
  let dayMinutes = $state(120);
  let dayEnergy = $state(3);
  let dayDirty = $state(false);
  let budgetOpen = $state(false);
  const weekdays = ['Monday','Tuesday','Wednesday','Thursday','Friday','Saturday','Sunday'];
  const factors = [{key:'technical',label:'Technical difficulty',hint:'Skill and reasoning'}, {key:'tedium',label:'Tedium',hint:'Repetition and boredom'}, {key:'ambiguity',label:'Uncertainty',hint:'Unknown scope'}, {key:'switching',label:'Context switching',hint:'Setup and recovery'}] as const;
  const mins = (n:number) => n>=60 ? `${Math.floor(n/60)}h${Math.round(n%60)>0 ? ` ${Math.round(n%60)}m` : ''}` : `${Math.max(0,Math.round(n))}m`;
  const wu = (n:number) => Number.isFinite(n) ? n.toFixed(1) : '0.0';
  const assessment = (id:number) => data?.workload?.assessments[id];
  let day = $derived(data?.workload?.days[0]);
  let planned = $derived(day?.blocks || []);
  let riskTasks = $derived((data?.workload?.riskIds || []).map(id => data?.tasks.find(t=>t.id===id)).filter((t): t is Task=>Boolean(t)));
  let taskRows = $derived(screen==='now' ? planned.map(block=>data?.tasks.find(t=>t.id===block.taskId)).filter((t):t is Task=>Boolean(t)) : data?.visible || []);
  const emptyOrganization = ():Organization => ({role:'task',parentId:null,milestoneId:null,cycleId:null,relatedIds:[],startsOn:null,endsOn:null,external:[]});
  let organizationDraft = $state<Organization>(emptyOrganization());
  let organizationDirty = $state(false);
  let organizationTaskId = $state<number|null>(null);
  let relatedDraft = $state('');
  let externalProvider = $state('');
  let externalId = $state('');
  let externalUrl = $state('');
  let cardSection = $state('overview');
  $effect(()=>{
    if(selectedId!==organizationTaskId){organizationTaskId=selectedId;organizationDirty=false;cardSection='overview';externalProvider='';externalId='';externalUrl='';}
    if(selected && !organizationDirty){const org=structuredClone($state.snapshot(selected.organization || emptyOrganization()));organizationDraft=org;relatedDraft=org.relatedIds.map(reference).join(', ');}
  });
  async function saveOrganization(){
    if(!selectedId||busy)return;
    if(browserPreview){error='Open the desktop app to save links.';return;}
    const refs=relatedDraft.trim()?relatedDraft.split(/[,\s]+/).filter(Boolean).map(s=>Number(s.replace(/^TASK-/i,'').replace(/^#/,''))):[];
    const external=[...organizationDraft.external];
    if(externalProvider.trim()||externalId.trim()||externalUrl.trim())external.push({provider:externalProvider.trim(),remoteId:externalId.trim(),url:externalUrl.trim()});
    busy=true;error='';
    try{
      const snapshot=await invoke<Snapshot>('change_tasks',{request:request(),actions:[{kind:'update',taskIds:[selectedId],title:null,description:null,labels:null,taskType:null,priority:null,dueDate:null,organization:{...organizationDraft,relatedIds:refs,external}}]});
      organizationDirty=false;externalProvider='';externalId='';externalUrl='';accept(snapshot);notice='Links saved.';
    }catch(e){error=String(e);}finally{busy=false;}
  }
  function focusCard(node:HTMLElement){
    const previous=document.activeElement as HTMLElement|null;
    node.focus();
    const trap=(event:KeyboardEvent)=>{
      if(event.key!=='Tab')return;
      const items=[...node.querySelectorAll<HTMLElement>('button:not(:disabled),a[href],input:not(:disabled),select:not(:disabled),textarea:not(:disabled)')].filter(el=>el.offsetParent!==null);
      if(!items.length){event.preventDefault();return;}
      if(event.shiftKey&&(document.activeElement===items[0]||document.activeElement===node)){event.preventDefault();items.at(-1)?.focus();}
      else if(!event.shiftKey&&(document.activeElement===items.at(-1)||document.activeElement===node)){event.preventDefault();items[0].focus();}
    };
    node.addEventListener('keydown',trap);
    return {destroy(){node.removeEventListener('keydown',trap);previous?.focus();}};
  }
  async function openReference(id:number,index:number){
    if(browserPreview){error='Open the desktop app to follow saved references.';return;}
    try{await invoke('open_task_reference',{request:request(),id,index});}catch(e){error=String(e);}
  }
  function createGuided(){selectedId=null;compose('Walk me through creating a task.');}
  function inspect(id:number) { selectedId=id; chatOpen=false; }
  function openChat() { chatOpen=true; tick().then(()=>composer?.focus()); }
  async function workloadUpdate(command:string,args:Record<string,unknown>) {
    if (busy) return;
    if(browserPreview){error='Open the desktop app to save workload estimates and feedback.';return;}
    busy=true;error='';
    try { accept(await invoke<Snapshot>(command,{request:request(),...args})); }
    catch(e) {error=String(e);} finally {busy=false;}
  }
  async function saveEstimate() { if(!selectedId)return;estimateDirty=false;await workloadUpdate('save_task_estimate',{id:selectedId,estimate:{...estimateDraft,source:'user'}}); }
  async function assessEstimate() {
    if(!selectedId||assessmentBusy)return;
    if(browserPreview){error='Open Cadence to assess effort.';return;}
    if(!data?.configured){openSettings();return;}
    assessmentBusy=true;error='';
    try{accept(await invoke<Snapshot>('assess_task_effort',{request:request(),id:selectedId}));}catch(e){error=String(e);}finally{assessmentBusy=false;}
  }
  async function saveCapacity() {capacityDirty=false;await workloadUpdate('save_capacity_profile',{profile:{weekdayMinutes:capacityDraft,learningEnabled:learningDraft}});}
  async function saveDay() {dayDirty=false;await workloadUpdate('save_day_checkin',{checkin:{date:today(),minutes:dayMinutes,energy:dayEnergy}});budgetOpen=false;}
  async function rateEffort(id:number,rating:number) {await workloadUpdate('rate_task_effort',{id,rating,actualMinutes:selectedId===id ? actualDraft ?? null : null});if(!error){feedbackTaskId=null;notice='Effort feedback recorded. Your next estimates will use it.';}}
  async function reviewDay(date:string,rating:string) {await workloadUpdate('review_capacity_day',{date,rating});if(!error)notice='Day review saved. Future capacity for this weekday will adapt gradually.';}
  $effect(()=>{ if(selectedId!==formTaskId){formTaskId=selectedId;estimateDirty=false;actualDraft=undefined;} if(selectedId && !estimateDirty){const current=data?.workload?.assessments[selectedId];if(current)estimateDraft={...current.estimate};} });
  let journalDate = $state(new Date().toLocaleDateString('en-CA'));
  let searchField = $state<HTMLInputElement>();
  let nativeNote = $state('');
  let suggested = $derived(data?.tasks.find(t => t.id === data?.nextTaskId));
  let focusTask = $derived(running || suggested);
  let queue = $derived([...(suggested ? [suggested] : []), ...(data?.tasks || []).filter(t => !t.deleted && t.status !== 'done' && !t.timerStartedAt && !t.labels.includes('blocked') && t.id !== suggested?.id).sort((a,b) => b.priority - a.priority)].slice(0,3));
  let journal = $derived((data?.tasks || []).filter(t => !t.deleted).flatMap(task => (task.timeEntries || []).filter(entry => new Date(entry.endedAt).toLocaleDateString('en-CA') === journalDate).map(entry => ({ ...entry, task }))).sort((a,b) => b.endedAt.localeCompare(a.endedAt)));
  let journalTotal = $derived(journal.reduce((sum, entry) => sum + entry.durationMs, 0));
  const clockTime = (s: string) => new Date(s).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
  async function captureWindow() { if (browserPreview) { compose('I need to '); return; } try { await invoke('quick_capture'); } catch (e) { error = String(e); } }
  async function taskMenu(event: MouseEvent, task: Task) { event.preventDefault(); selectedId = task.id; if (!browserPreview) { try { await invoke('task_context_menu', { id: task.id, demo }); } catch (e) { error = String(e); } } }
  async function nativeAction(event: { action: string; id?: number; demo?: boolean }) {
    if (event.demo !== undefined && event.demo !== demo) return;
    switch (event.action) {
      case 'settings': await openSettings(); break;
      case 'new-task': createGuided(); break;
      case 'find-task': screen = 'next'; await tick(); searchField?.focus(); break;
      case 'focus-view': screen = 'now'; break;
      case 'next-view': navigate('open'); break;
      case 'journal-view': screen = 'journal'; break;
      case 'toggle-timer': { const task = screen === 'now' ? focusTask : selected || running || suggested; if (task) await mutate(task.timerStartedAt ? 'pause' : 'start', task.id); break; }
      case 'complete-selected': { const task = selected || running; if (task) await mutate('complete', task.id); break; }
      case 'edit': if (event.id) { selectedId = event.id; compose(`Update ${reference(event.id)}: `); } break;
    }
  }
  let selected = $derived(data?.tasks.find(t => t.id === selectedId));
  let title = $derived(view.label ? view.label : ({ open: 'Next', all: 'All tasks', today: 'Today & overdue', high: 'High priority', done: 'Completed', in_progress: 'In progress', deleted: 'Trash' }[view.status] || 'Your tasks'));
  let labels = $derived(Object.entries(data?.stats.labels || {}).sort((a,b) => b[1] - a[1]));
  const today = () => new Date().toLocaleDateString('en-CA');
  const dateLabel = (value: string | null) => !value ? 'No date' : value === today() ? 'Today' : new Date(value + 'T12:00:00').toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  const overdue = (task: Task) => task.dueDate && task.dueDate < today() && task.status !== 'done';
  const priorityName = (n: number) => n >= 75 ? 'High' : n >= 50 ? 'Medium' : 'Low';
  const statusName = (s: string) => s === 'in_progress' ? 'In progress' : s === 'done' ? 'Done' : 'To do';
  const request = () => ({ view: { ...view }, selectedId, demo });
  function accept(snapshot: Snapshot) {
    clock = Date.now(); snapshotAt = clock; data = snapshot; view = snapshot.view;
    if(snapshot.workload){if(!capacityDirty){capacityDraft=[...snapshot.workload.profile.weekdayMinutes];learningDraft=snapshot.workload.profile.learningEnabled;}if(!dayDirty){dayMinutes=snapshot.workload.days[0].minutes;dayEnergy=snapshot.workload.days[0].energy;}} if (snapshot.showList) screen = 'next';
    if (snapshot.focusIds.length) selectedId = snapshot.focusIds[0];
    if (selectedId && !snapshot.tasks.some(t => t.id === selectedId)) selectedId = null;
  }
  async function refresh(patch: Partial<View> = {}) {
    if (busy) return;
    view = { ...view, ...patch };
    error = '';
    if (browserPreview && data) {
      const tasks = data.tasks.filter(t => {
        if (view.status === 'deleted') return t.deleted;
        if (t.deleted) return false;
        if (view.status === 'open' && t.status === 'done') return false;
        if (view.status === 'done' && t.status !== 'done') return false;
        if (view.status === 'in_progress' && t.status !== 'in_progress') return false;
        if (view.status === 'high' && (t.priority < 75 || t.status === 'done')) return false;
        if (view.status === 'today' && (!t.dueDate || t.dueDate > today() || t.status === 'done')) return false;
        return (!view.label || t.labels.includes(view.label)) && (!view.taskType || t.type === view.taskType) && (!view.query || `${reference(t.id)} ${t.title} ${t.description} ${t.labels.join(' ')}`.toLowerCase().includes(view.query.toLowerCase()));
      });
      tasks.sort((a,b) => Number(a.status === 'done') - Number(b.status === 'done') || (view.sort === 'fit' ? (assessment(b.id)?.fitScore || 0)-(assessment(a.id)?.fitScore || 0) : view.sort === 'newest' ? b.id-a.id : view.sort === 'oldest' ? a.id-b.id : view.sort === 'title' ? a.title.localeCompare(b.title) : view.sort === 'time' ? elapsed(b)-elapsed(a) : view.sort === 'due' ? (a.dueDate || '9999').localeCompare(b.dueDate || '9999') : b.priority-a.priority));
      data = { ...data, visible: tasks, view }; return;
    }
    try { accept(await invoke<Snapshot>('get_snapshot', { request: request() })); }
    catch (e) { error = String(e); }
  }
  function navigate(status: string, label = '') { screen = 'next'; selectedId = null; refresh({ status, label, query: '', taskType: '' }); }
  async function scrollChat() { await tick(); chatEnd?.scrollIntoView({ behavior: 'smooth', block: 'end' }); }
  function compose(text: string) { input = text; openChat(); }
  function mention(id: number) { selectedId = id; input = `${input}${input && !input.endsWith(' ') ? ' ' : ''}${reference(id)} `; openChat(); }
  async function send() {
    if (busy || chatBusy || !input.trim()) return;
    if (browserPreview) { error = 'This is a visual preview. Open the Tauri desktop app to use chat and save changes.'; return; }
    if (!data?.configured && !/^(sync|refresh|import)( my)? canvas/i.test(input.trim())) { await openSettings(); return; }
    error = ''; chatStarted=Date.now(); chatBusy = true; pending = input.trim(); input = ''; phase = 'Organizing your request…';
    scrollChat();
    try { accept(await invoke<Snapshot>('chat', { request: request(), message: pending })); pending = ''; }
    catch (e) { error = String(e); input = pending; pending = ''; }
    finally { chatBusy = false; scrollChat(); }
  }
  async function mutate(kind: string, id: number) {
    if (busy) return;
    error = ''; busy = true;
    try {
      accept(await invoke<Snapshot>('change_tasks', { request: request(), actions: [{ kind, taskIds: [id], title: null, description: null, labels: null, taskType: null, priority: null, dueDate: null }] }));
    } catch (e) { error = String(e); }
    finally { busy = false; if(kind==='complete')feedbackTaskId=id; }
  }
  async function toggleDemo() {
    if (busy || browserPreview) return;
    demo = !demo; selectedId = null; view = defaultView(); await refresh();
  }
  async function openSettings() {
    settingsOpen = true; settingsTab = 'assistant'; settingsError = ''; apiKey = ''; canvasFormToken = ''; canvasSettingsError = '';
    if (browserPreview) return;
    try { [settings, canvas] = await Promise.all([invoke<Settings>('get_settings'), invoke<CanvasSettings>('get_canvas_settings')]); canvasFormUrl = canvas.baseUrl; canvasFormAuto = canvas.autoSync; }
    catch (e) { settingsError = String(e); }
  }
  async function saveSettings() {
    settingsBusy = true; settingsError = '';
    try {
      settings = await invoke<Settings>('save_settings', { apiKey: apiKey || null, model: settings.model, jevModel: settings.jevModel, reasoningEffort: settings.reasoningEffort });
      apiKey = ''; settingsOpen = false; await refresh();
    } catch (e) { settingsError = String(e); }
    finally { settingsBusy = false; }
  }
  onMount(() => {
    browserPreview = !desktop();
    if (browserPreview) { data = preview(); demo = true; }
    else { refresh(); refreshCanvas(); }
    const syncAppearance = async () => {
      if (!desktop()) return;
      try {
        const prefs = await invoke<{ nativeGlass: boolean; reduceTransparency: boolean; reduceMotion: boolean; increaseContrast: boolean }>('get_appearance');
        Object.assign(document.documentElement.dataset, { nativeGlass: String(prefs.nativeGlass), reduceTransparency: String(prefs.reduceTransparency), reduceMotion: String(prefs.reduceMotion), increaseContrast: String(prefs.increaseContrast) });
      } catch { document.documentElement.dataset.nativeGlass = 'false'; }
    };
    syncAppearance();
    document.documentElement.dataset.macos = String(desktop() && /Mac/.test(navigator.platform));
    const timerTick = window.setInterval(() => { clock = Date.now(); }, 1000);
    const capacityPoll = window.setInterval(()=>{if(!busy && !browserPreview && !estimateDirty) refresh();},60000);
    const onFocus = () => { syncAppearance(); if (!browserPreview) refresh(); };
    window.addEventListener('focus', onFocus);
    const nativeListeners: (() => void)[] = [];
    let unlisten: (() => void) | undefined;
    let disposed = false;
    progress(text => { phase = text; }).then(fn => { if (disposed) fn(); else unlisten = fn; });
    for (const pending of [
      listen<{ action: string; id?: number; demo?: boolean }>('native-action', nativeAction),
      listen<string>('native-error', text => error = text),
      listen<{ syncing: boolean; report?: CanvasReport; error?: string }>('canvas-sync-status', event => {
        canvasBusy = event.syncing;
        if (event.report) { canvas.report = event.report; canvas.error = null; canvasNotice = `Canvas synced · ${event.report.created} added · ${event.report.updated} updated`; }
        if (event.error) { canvas.error = event.error; canvasNotice = event.error; }
      }),
      listen('appearance-changed', syncAppearance),
      listen<{ demo?: boolean; reason?: string }>('tasks-changed', event => {
        if (event.demo !== undefined && event.demo !== demo) return;
        if (event.reason === 'sleep') nativeNote = 'Focus paused when your Mac went to sleep. Resume when you’re ready.';
        if (!busy) refresh();
      }),
    ]) pending.then(fn => disposed ? fn() : nativeListeners.push(fn));
    const shortcut = (event: KeyboardEvent) => { if ((event.metaKey || event.ctrlKey) && event.key === 'k') { event.preventDefault(); openChat(); } };
    window.addEventListener('keydown', shortcut);
    return () => { window.clearInterval(timerTick); window.clearInterval(capacityPoll); nativeListeners.forEach(fn => fn()); window.removeEventListener('focus', onFocus); disposed = true; unlisten?.(); window.removeEventListener('keydown', shortcut); };
  });
</script>

<svelte:head><title>Cadence — Now, next, done</title><meta name="description" content="Say what needs doing. Keep your tasks, priorities and progress in one calm place." /></svelte:head>

<div class="app-shell" class:with-drawer={chatOpen}>
  <aside class="sidebar">
    <button class="brand" onclick={() => screen='now'}><img src="/favicon.png" alt="" /><span>Cadence</span></button>
    <button class="capture-sidebar" onclick={createGuided}>＋ <span>Create with chat</span><kbd>⌘N</kbd></button>
    <nav aria-label="Workspace">
      <button class:active={screen==='now'} onclick={() => screen='now'}><span>◉</span>Today<kbd>⌘1</kbd></button>
      <button class:active={screen==='next'} onclick={() => navigate('open')}><span>▤</span>All tasks<kbd>⌘2</kbd></button>
      <button class:active={screen==='week'} onclick={() => screen='week'}><span>▦</span>Capacity</button>
      <button class:active={screen==='journal'} onclick={() => screen='journal'}><span>◷</span>Focus journal<kbd>⌘3</kbd></button>
    </nav>
    <div class="nav-label">CONNECTED</div>
    <div class="canvas-sidebar"><button disabled={busy} onclick={showCanvas}><span class="canvas-glyph">◉</span><span>Canvas<small>{canvasBusy?'Syncing…':canvas.error?'Needs attention':canvas.configured?`${canvasCount} remaining`:'Connect courses'}</small></span></button><button disabled={canvasBusy||browserPreview} onclick={syncCanvas} aria-label="Sync Canvas">↻</button></div>
    <details class="labels-disclosure"><summary>Labels <span>{labels.length}</span></summary>{#each labels as [label,count]}<button onclick={() => navigate('all',label)}><span class="label-dot"></span>{label}<small>{count}</small></button>{/each}</details>
    <div class="sidebar-bottom"><button class="utility" onclick={() => navigate('done')}>✓ Completed<span>{data?.stats.completed || 0}</span></button><button class="utility" onclick={() => navigate('deleted')}>♧ Trash</button><button class="utility" onclick={openSettings}>⚙ Connections</button><div class="local-workspace"><span class="online-dot"></span>Saved on this Mac</div></div>
  </aside>
  <main>
    <header class="topbar" data-tauri-drag-region><span data-tauri-drag-region>{screen==='now'?'Today':screen==='week'?'Capacity':screen==='journal'?'Focus journal':title}</span><div>{#if chatBusy}<span class="assistant-status">Assistant working · {Math.max(0,Math.floor((clock-chatStarted)/1000))}s</span>{/if}{#if demo}<span class="demo-pill">{browserPreview?'Visual preview':'Demo'}</span>{/if}<button disabled={busy} onclick={toggleDemo}>{demo?'My tasks':'Demo'}</button><button class="chat-toggle" onclick={openChat}>Conversation <kbd>⌘K</kbd></button></div></header>
    <div class="main-content">
      <div class="page-heading"><div><p class="eyebrow">{new Date().toLocaleDateString(undefined,{weekday:'long',month:'long',day:'numeric'})}</p><h1>{screen==='now'?'A plan that fits your day':screen==='week'?'Your week, at your pace':screen==='journal'?'Your focus journal':title}</h1><p>{screen==='now'?'Time and effort are both limited. This plan respects both.':screen==='week'?'Start with your schedule. Let real work refine it.':screen==='journal'?'Recorded work, session by session.':'Inspect an estimate, adjust it, or start a focused block.'}</p></div><button class="primary-button" onclick={captureWindow}>＋ Capture</button></div>
      {#if error}<div class="error-banner" role="alert">{error}<button onclick={() => error=''} aria-label="Dismiss error">×</button></div>{/if}
      {#if notice || nativeNote || canvasNotice}<div class="notice" role="status">{notice || nativeNote || canvasNotice}<button onclick={() => {notice='';nativeNote='';canvasNotice='';}} aria-label="Dismiss status">×</button></div>{/if}
      {#if feedbackTaskId}<div class="completion-feedback"><strong>Task finished. How demanding did it feel?</strong><div><button disabled={busy} onclick={() => rateEffort(feedbackTaskId!,1)}>Lighter</button><button disabled={busy} onclick={() => rateEffort(feedbackTaskId!,3)}>As expected</button><button disabled={busy} onclick={() => rateEffort(feedbackTaskId!,5)}>Draining</button><button class="text-button" onclick={() => feedbackTaskId=null}>Later</button></div></div>{/if}
      {#if screen==='now' && day}
        <section class="capacity-summary"><div><span class="section-kicker">TODAY’S CAPACITY</span><h2>{mins(day.remainingMinutes)} <small>and</small> {wu(day.remainingUnits)} <small>WU left</small></h2><p>{mins(day.spentMinutes)} tracked · {wu(day.spentUnits)} WU used · {day.energy<=2?'Lower energy':day.energy>=4?'Higher energy':'Usual energy'}</p></div><button class="secondary-button" onclick={() => budgetOpen=!budgetOpen}>Adjust today</button></section>
        {#if budgetOpen}<form class="today-checkin" onsubmit={event=>{event.preventDefault();saveDay();}}><label>Focused minutes available today<input type="number" min="0" max="720" step="1" required bind:value={dayMinutes} oninput={()=>dayDirty=true}/></label><label>Energy today<select bind:value={dayEnergy} onchange={()=>dayDirty=true}><option value={1}>Low</option><option value={3}>Usual</option><option value={5}>High</option></select></label><button class="primary-button" disabled={busy}>Update plan</button><p>Total daily budget, including time already tracked.</p></form>{/if}
        <div class="week-strip" aria-label="Seven day capacity">{#each data?.workload.days || [] as d}<button onclick={()=>screen='week'}><span>{new Date(d.date+'T12:00:00').toLocaleDateString(undefined,{weekday:'short'})}</span><div class="capacity-bar"><span class="spent" style={`width:${Math.min(100,d.spentUnits/Math.max(.1,d.capacityUnits)*100)}%`}></span><span class="planned" style={`width:${Math.min(100,d.blocks.reduce((n,b)=>n+b.units,0)/Math.max(.1,d.capacityUnits)*100)}%`}></span></div><strong>{wu(d.capacityUnits)} <small>WU</small></strong></button>{/each}</div>
        {#if riskTasks.length}<section class="deadline-alert"><strong>{riskTasks.length} deadline{riskTasks.length===1?'':'s'} exceed the plan</strong><p>These tasks need smaller scope, a changed deadline, or an explicit capacity adjustment.</p><div>{#each riskTasks.slice(0,4) as task}<button onclick={()=>inspect(task.id)}>{task.title} <span>{dateLabel(task.dueDate)} ↗</span></button>{/each}</div></section>{/if}
        {#if focusTask && assessment(focusTask.id)}{@const a=assessment(focusTask.id)!}{@const block=planned.find(b=>b.taskId===focusTask.id)}<section class="focus-card"><div class="focus-card-copy"><span class="section-kicker">{running?'IN PROGRESS':'RECOMMENDED NEXT'}</span><button class="focus-title" onclick={()=>inspect(focusTask.id)}>{focusTask.title}</button><div class="work-tags"><span>{mins(a.remainingMinutes)} left</span><span>{wu(a.units)} WU</span><span>{a.loadFactor>=1.25?'Heavy load':a.loadFactor<=.85?'Light load':'Moderate load'}</span>{#if a.estimate.source==='starter'}<span class="provisional">Starter estimate</span>{/if}</div><p>{a.reason}</p></div><div class="focus-card-control">{#if running}<strong class="live-clock">{duration(elapsed(running))}</strong>{:else}<strong>{block?mins(block.minutes):'—'}</strong><small>{block?.partial?'suggested work block':'suggested focus'}</small>{/if}<button class="primary-button" disabled={busy} onclick={()=>mutate(running?'pause':'start',focusTask.id)}>{running?'Ⅱ Pause':'▷ Start block'}</button>{#if running}<button class="text-button" disabled={busy} onclick={()=>mutate('complete',running.id)}>✓ Finish task</button>{/if}</div></section>{:else}<section class="rest-card"><h2>{day.remainingUnits<.2?'Your planned capacity is used.':'A little space in your day.'}</h2><p>{data?.stats.active?'There’s no ready block that fits right now. Review your estimates or take a break.':'Capture something to work on when you’re ready.'}</p></section>{/if}
      {/if}
      {#if screen==='now'||screen==='next'}
        <section class="task-panel"><div class="task-toolbar"><h2>{screen==='now'?'Today’s suggested blocks':'Task list'} <span>{taskRows.length}</span></h2><div class="table-tools">{#if screen==='next'}<input class="search" bind:this={searchField} aria-label="Search tasks" placeholder="Search tasks…" value={view.query} oninput={event=>refresh({query:event.currentTarget.value})}/><select aria-label="Sort tasks" value={view.sort} onchange={event=>refresh({sort:event.currentTarget.value})}><option value="fit">Personal fit</option><option value="priority">Priority</option><option value="due">Due date</option><option value="time">Time tracked</option><option value="newest">Newest</option><option value="oldest">Oldest</option><option value="title">Title</option></select>{:else}<button class="text-button" onclick={()=>navigate('open')}>All tasks →</button>{/if}</div></div>
          {#if screen==='next'}<div class="filter-row"><select aria-label="Task status" value={view.status} onchange={event=>refresh({status:event.currentTarget.value})}><option value="open">Open</option><option value="all">All</option><option value="done">Completed</option><option value="deleted">Trash</option><option value="today">Due today</option></select><select aria-label="Task type" value={view.taskType} onchange={event=>refresh({taskType:event.currentTarget.value})}><option value="">All types</option>{#each ['bug','feature','chore','research','meeting','personal','assignment'] as kind}<option>{kind}</option>{/each}</select>{#if view.label}<button class="filter-chip" onclick={()=>refresh({label:''})}>{view.label} ×</button>{/if}</div>{/if}
          <div class="table-scroll"><table><thead><tr><th>Task</th><th>Work estimate</th><th>Due</th><th></th></tr></thead><tbody>{#each taskRows as task (task.id)}{@const a=assessment(task.id)}{@const block=planned.find(b=>b.taskId===task.id)}<tr class:selected-row={selectedId===task.id}><td><button class="task-title" onclick={()=>inspect(task.id)} oncontextmenu={event=>taskMenu(event,task)}>{task.title}</button><div class="task-meta"><span>{reference(task.id)}</span><span>·</span><span>{task.canvas?.courseCode || task.type}</span>{#if task.status==='done'}<span class="done-label">✓ Done</span>{/if}{#if task.timerStartedAt}<span class="live-label">● Running</span>{/if}</div></td><td><button class="estimate-cell" onclick={()=>inspect(task.id)}><strong>{(task.organization?.role || 'task')!=='task'?task.organization?.role:a?`${wu(task.status==='done'?a.totalUnits:a.units)} WU`:'—'}</strong><span>{(task.organization?.role || 'task')!=='task'?[task.organization?.startsOn,task.organization?.endsOn].filter(Boolean).join(' → ') || 'Planning item':screen==='now'&&block?.partial?`${mins(block.minutes)} block · ${mins(a?.remainingMinutes || 0)} total`:mins(task.status==='done'?(a?.predictedMinutes || 0):(a?.remainingMinutes || 0))}</span>{#if a?.estimate.source==='starter'&&(task.organization?.role || 'task')==='task'}<small>Provisional</small>{/if}</button></td><td><span class:overdue={overdue(task)} class="due-date">{dateLabel(task.dueDate)}</span></td><td><div class="row-actions">{#if task.deleted}<button disabled={busy} onclick={()=>mutate('restore',task.id)} aria-label={`Restore ${reference(task.id)}`}>↶</button>{:else if task.status!=='done'}{#if (task.organization?.role || 'task')==='task'}<button disabled={busy} onclick={()=>mutate(task.timerStartedAt?'pause':'start',task.id)} aria-label={`${task.timerStartedAt?'Pause':'Start'} ${reference(task.id)}`}>{task.timerStartedAt?'Ⅱ':'▷'}</button>{/if}<button disabled={busy} onclick={()=>mutate('complete',task.id)} aria-label={`Complete ${reference(task.id)}`}>✓</button>{/if}<button onclick={event=>taskMenu(event,task)} aria-label={`Actions for ${reference(task.id)}`}>···</button></div></td></tr>{/each}</tbody></table></div>
          {#if !taskRows.length}<div class="empty-state"><h3>{screen==='now'?'No blocks planned':'No tasks in this view'}</h3><p>{screen==='now'?'Adjust your available time, refine an estimate, or let the day stay light.':'Try another filter, connect Canvas, or capture a task.'}</p></div>{/if}
        </section><div class="model-note"><span>{data?.workload.calibration.stage || 'Starter estimates'}</span><span>{data?.workload.calibration.timeSamples || 0} timed completions · {data?.workload.calibration.effortSamples || 0} effort ratings</span><button onclick={()=>screen='week'}>How it learns ↗</button></div><p class="unit-note">1 work unit ≈ 25 focused minutes at moderate load. Hard, tedious, uncertain work can cost more. Urgency changes order, not effort.</p>
      {/if}
      {#if screen==='week'}
        <section class="week-planner"><header><h2>The next seven days</h2><p>Suggested blocks fit both focused time and personal work units. They are estimates, not commitments.</p></header>{#each data?.workload.days || [] as d}<div class="week-day"><div><strong>{new Date(d.date+'T12:00:00').toLocaleDateString(undefined,{weekday:'long'})}</strong><small>{new Date(d.date+'T12:00:00').toLocaleDateString(undefined,{month:'short',day:'numeric'})}</small></div><div><strong>{mins(d.minutes)} · {wu(d.capacityUnits)} WU</strong><small>{mins(d.spentMinutes)} tracked · {d.blocks.length} suggested blocks</small></div><div class="week-blocks">{#each d.blocks as block}<button onclick={()=>inspect(block.taskId)}>{data?.tasks.find(t=>t.id===block.taskId)?.title}<small>{mins(block.minutes)}{block.partial?' · partial':''}</small></button>{/each}{#if !d.blocks.length}<span>Room to breathe</span>{/if}</div></div>{/each}</section>
        <section class="capacity-settings"><header><h2>Your starting schedule</h2><p>Your starting availability, in focused minutes. Weekday load capacity adjusts gradually from your day reviews.</p></header><form onsubmit={event=>{event.preventDefault();saveCapacity();}}><div class="weekday-inputs">{#each weekdays as name,i}<label>{name.slice(0,3)}<input type="number" min="0" max="720" step="1" required bind:value={capacityDraft[i]} oninput={()=>capacityDirty=true}/><small>minutes</small></label>{/each}</div><label class="checkbox-label"><input type="checkbox" bind:checked={learningDraft} onchange={()=>capacityDirty=true}/> Learn from reliable completions and my feedback</label><button class="primary-button" disabled={busy||!capacityDirty}>Save schedule</button></form></section>
        <section class="learning-panel"><header><h2>Personal calibration</h2><span>{data?.workload.calibration.stage}</span></header><p>Time is learned from completed timers. Perceived load is learned from effort ratings. Weekday capacity changes only after day reviews. Missing work logs never lower your capacity.</p><div class="learning-counts"><strong>{data?.workload.calibration.timeSamples || 0}<small>timed tasks</small></strong><strong>{data?.workload.calibration.effortSamples || 0}<small>effort ratings</small></strong><strong>{data?.workload.calibration.daySamples || 0}<small>day reviews</small></strong></div><div class="weight-list">{#each factors as factor,i}<div><span>{factor.label}</span><div><span style={`width:${Math.min(100,(data?.workload.calibration.loadWeights[i+1] || 0)/.8*100)}%`}></span></div><strong>{(data?.workload.calibration.loadWeights[i+1] || 0).toFixed(2)}</strong></div>{/each}</div><p class="muted">Small samples stay close to starting weights. Older observations fade gradually. Pausing learning stops new outcomes and ratings from being collected.</p><div class="day-review"><h3>How sustainable was today?</h3><p>After at least 15 tracked minutes, tell the planner how the day felt.</p><div>{#each ['light','sustainable','overloaded'] as rating}<button class="secondary-button" disabled={busy||(day?.spentMinutes || 0)<15||!learningDraft} onclick={()=>reviewDay(today(),rating)}>{rating}</button>{/each}</div></div></section>
      {/if}
      {#if screen==='journal'}<section class="focus-journal"><header><div><h2>Recorded focus</h2><p>Sessions ending on the selected day.</p></div><input type="date" aria-label="Journal date" bind:value={journalDate}/></header><div class="journal-summary"><strong>{duration(journalTotal)}</strong><span>{journal.length} completed sessions</span></div>{#each journal as entry}<button class="journal-entry" onclick={()=>inspect(entry.task.id)}><span class="journal-time">{clockTime(entry.startedAt)}<small>to {clockTime(entry.endedAt)}</small></span><span class="journal-task"><strong>{entry.task.title}</strong><small>{reference(entry.task.id)}</small></span><span class="journal-duration">{duration(entry.durationMs)}</span></button>{/each}{#if !journal.length}<div class="empty-state"><h3>No sessions recorded</h3><p>Start a task and pause or finish it to record a session here.</p></div>{/if}</section>{/if}
    </div>
  </main>
  {#if selected && !chatOpen}{@const a=assessment(selected.id)}<div class="task-card-backdrop" role="presentation"><div class="inspector task-card" use:focusCard role="dialog" aria-modal="true" aria-label={selected.title} tabindex="-1" onkeydown={event=>{if(event.key==='Escape')selectedId=null;}}><header><span>{reference(selected.id)}</span><button onclick={()=>selectedId=null} aria-label="Close inspector">×</button></header><div class="inspector-content"><span class="section-kicker">{selected.organization?.role || 'task'} · {statusName(selected.status)}</span><h2>{selected.title}</h2>{#if error}<p class="card-error" role="alert">{error}</p>{/if}<nav class="card-tabs"><button class:active={cardSection==='overview'} onclick={()=>cardSection='overview'}>Overview</button><button class:active={cardSection==='links'} onclick={()=>cardSection='links'}>Links & planning</button>{#if (selected.organization?.role || 'task')==='task'}<button class:active={cardSection==='effort'} onclick={()=>cardSection='effort'}>Effort & time</button>{/if}</nav>{#if cardSection==='overview'}<p class="detail-description">{selected.description || 'No description yet.'}</p><div class="work-tags">{#each selected.labels as label}<button class={`label-pill label-${label}`} onclick={()=>navigate('all',label)}>{label}</button>{/each}</div><div class="inspector-actions">{#if selected.deleted}<button class="secondary-button" disabled={busy} onclick={()=>mutate('restore',selected.id)}>Restore</button>{:else if selected.status==='done'}<button class="secondary-button" disabled={busy} onclick={()=>mutate('reopen',selected.id)}>Reopen</button>{:else}{#if (selected.organization?.role || 'task')==='task'}<button class="primary-button" disabled={busy} onclick={()=>mutate(selected.timerStartedAt?'pause':'start',selected.id)}>{selected.timerStartedAt?'Ⅱ Pause':'▷ Start'}</button>{/if}<button class="secondary-button" disabled={busy} onclick={()=>mutate('complete',selected.id)}>✓ Complete</button>{/if}<button class="text-button" onclick={()=>compose(`Update ${reference(selected.id)}: `)}>Edit in chat</button></div>
{#each selected.organization?.external || [] as link,i}<button class="linked-item" onclick={()=>openReference(selected.id,i)}><span>{link.provider}</span>{link.remoteId} ↗</button>{/each}
<section class="linked-summary"><h3>Connected work</h3>{#each [selected.organization?.parentId,selected.organization?.milestoneId,selected.organization?.cycleId,...(selected.organization?.relatedIds || []),...(data?.tasks.filter(t=>!t.deleted&&t.id!==selected.id&&(t.organization?.parentId===selected.id||t.organization?.milestoneId===selected.id||t.organization?.cycleId===selected.id||t.organization?.relatedIds.includes(selected.id))).map(t=>t.id)||[])] .filter((id):id is number=>Boolean(id)).filter((id,i,ids)=>ids.indexOf(id)===i) as id}<button class="linked-item" onclick={()=>inspect(id)}><span>{reference(id)}</span>{data?.tasks.find(t=>t.id===id)?.title || 'Unavailable item'} →</button>{/each}<button class="text-button" onclick={()=>cardSection='links'}>Add or change links →</button></section>
      {:else if cardSection==='links'}
      <form class="organization-editor" onsubmit={event=>{event.preventDefault();saveOrganization();}}><p>Only add the structure that helps. External references link to other trackers; they do not sync yet.</p><label>Item role<select bind:value={organizationDraft.role} onchange={()=>organizationDirty=true}>{#each ['task','epic','milestone','sprint','iteration'] as role}<option value={role}>{role}</option>{/each}</select></label>
      {#each [{key:'parentId',label:'Part of epic',roles:['epic']},{key:'milestoneId',label:'Target milestone',roles:['milestone']},{key:'cycleId',label:'Sprint or iteration',roles:['sprint','iteration']}] as field}<label>{field.label}<select bind:value={organizationDraft[field.key as 'parentId'|'milestoneId'|'cycleId']} onchange={()=>organizationDirty=true}><option value={null}>None</option>{#each data?.tasks.filter(t=>t.id!==selected.id&&!t.deleted&&field.roles.includes(t.organization?.role || 'task')) || [] as target}<option value={target.id}>{reference(target.id)} · {target.title}</option>{/each}</select></label>{/each}
      <label>Related items<input bind:value={relatedDraft} oninput={()=>organizationDirty=true} placeholder="TASK-001, TASK-002"/></label>
      {#if ['sprint','iteration','milestone'].includes(organizationDraft.role)}<div class="planning-dates"><label>Starts<input type="date" value={organizationDraft.startsOn || ''} oninput={event=>{organizationDraft.startsOn=event.currentTarget.value||null;organizationDirty=true;}}/></label><label>Ends<input type="date" value={organizationDraft.endsOn || ''} oninput={event=>{organizationDraft.endsOn=event.currentTarget.value||null;organizationDirty=true;}}/></label></div>{/if}
      <h3>External references</h3>{#each organizationDraft.external as link,i}<div class="external-reference"><button type="button" onclick={()=>openReference(selected.id,i)}>{link.provider} · {link.remoteId} ↗</button><button type="button" onclick={()=>{organizationDraft.external=organizationDraft.external.filter((_,index)=>index!==i);organizationDirty=true;}} aria-label="Remove external reference">×</button></div>{/each}
      <label>Tracker<input bind:value={externalProvider} placeholder="GitHub, GitLab, Jira, OpenProject…" /></label><label>Issue reference<input bind:value={externalId} placeholder="owner/repo#42 or TEAM-42" /></label><label>Issue URL<input type="url" bind:value={externalUrl} placeholder="https://…" /></label><button class="primary-button" disabled={busy}>Save links</button></form>
      {:else}
      {#if a}<section class="assessment-summary"><div><strong>{wu(selected.status==='done'?a.totalUnits:a.units)} WU</strong><span>{mins(selected.status==='done'?a.predictedMinutes:a.remainingMinutes)} {selected.status==='done'?'estimated total':'estimated left'}</span></div><p>{a.reason}</p><small>{a.estimate.source==='starter'?'Starter estimate — scope not assessed':a.estimate.source==='user'?'Your assessment':'Assistant assessment'} · rough total range {mins(a.lowMinutes)}–{mins(a.highMinutes)}</small></section>{/if}
      <form class="estimate-editor" onsubmit={event=>{event.preventDefault();saveEstimate();}}><h3>What does this task ask of you?</h3><label>Estimated total focused minutes<input type="number" min="5" max="2400" step="1" required bind:value={estimateDraft.minutes} oninput={()=>estimateDirty=true}/></label>{#each factors as factor}<label class="factor-control"><span><strong>{factor.label}</strong><small>{factor.hint}</small></span><output>{estimateDraft[factor.key]}/5</output><input type="range" min="1" max="5" step="1" bind:value={estimateDraft[factor.key]} oninput={()=>estimateDirty=true}/></label>{/each}<div class="estimate-buttons"><button class="primary-button" disabled={busy||!estimateDirty}>Save estimate</button><button type="button" class="secondary-button" disabled={assessmentBusy||busy||estimateDraft.source==='user'} onclick={assessEstimate}>{assessmentBusy?'Assessing…':'Assess effort'}</button></div><p class="muted">Deadlines never reduce this effort estimate. Timers measure work; ratings teach the personal load weights.</p></form>
      <section class="tracked-detail"><h3>Recorded work</h3><strong>{duration(elapsed(selected))}</strong><span>{selected.timeEntries?.length || 0} completed sessions</span>{#if selected.status==='done'}<p>How demanding did this feel?</p><label class="actual-time-label">Actual focused minutes <small>Optional · use if the timer was inaccurate</small><input type="number" min="5" max="960" step="1" bind:value={actualDraft} placeholder="Use tracked time" /></label><div class="rating-buttons"><button disabled={busy||!data?.workload.profile.learningEnabled} onclick={()=>rateEffort(selected.id,1)}>Lighter</button><button disabled={busy||!data?.workload.profile.learningEnabled} onclick={()=>rateEffort(selected.id,3)}>As expected</button><button disabled={busy||!data?.workload.profile.learningEnabled} onclick={()=>rateEffort(selected.id,5)}>Draining</button></div>{/if}</section>
      {/if}{#if selected.canvas}<section class="canvas-assignment"><strong>{selected.canvas.courseCode}</strong><p>{selected.canvas.dueAt?`Canvas deadline: ${new Date(selected.canvas.dueAt).toLocaleString()}`:'No Canvas deadline'}</p><button onclick={()=>openCanvasTask(selected)}>Open assignment in Canvas ↗</button><small>Local completion never submits coursework.</small></section>{/if}<button class="delete-action" disabled={busy||selected.deleted} onclick={()=>mutate('delete',selected.id)}>Move to Trash</button></div></div></div>{/if}
  <aside class="chat-panel" hidden={!chatOpen}><header class="chat-header" data-tauri-drag-region><div><strong>Conversation</strong><small>Capture, plan, and adjust.</small></div><button onclick={()=>chatOpen=false} aria-label="Close conversation">×</button></header><div class="chat-messages" aria-live="polite" aria-busy={chatBusy}>{#if !data?.messages.length}<div class="chat-welcome"><h2>Plan in your own words.</h2><p>Start with what you want to accomplish. I’ll help clarify the scope, suggest labels, and write a useful description.<br/>Try “Help me create a task for my project.”</p></div>{/if}{#each data?.messages || [] as message (message.id)}<div class={`message ${message.role}`}><div class="message-author">{message.role==='user'?'YOU':'CADENCE'}</div><div class="message-content">{message.content}</div>{#if message.taskIds.length}<div class="message-refs">{#each message.taskIds as id}<button onclick={()=>inspect(id)}>{reference(id)} ↗</button>{/each}</div>{/if}</div>{/each}{#if pending}<div class="message user"><div class="message-author">YOU</div><div class="message-content">{pending}</div></div>{/if}{#if chatBusy}<div class="thinking">{phase} {Math.max(0,Math.floor((clock-chatStarted)/1000))}s{#if clock-chatStarted>15000}<p>The assistant is still working. You can keep using your tasks.</p>{/if}</div>{/if}<div bind:this={chatEnd}></div></div><div class="chat-compose-area">{#if selected}<div class="context-chip">About {reference(selected.id)}<button onclick={()=>selectedId=null} aria-label="Clear task context">×</button></div>{/if}{#if !data?.configured}<button class="connect-banner" onclick={openSettings}>Connect OpenRouter to use chat →</button>{/if}<form class="composer" onsubmit={event=>{event.preventDefault();send();}}><textarea bind:this={composer} bind:value={input} disabled={chatBusy} rows="3" maxlength="20000" aria-label="Chat with Cadence" placeholder="What needs doing?" onkeydown={event=>{if(event.key==='Enter'&&!event.shiftKey&&!event.metaKey&&!event.ctrlKey){event.preventDefault();send();}}}></textarea><div class="composer-bottom"><span>Enter to send</span><button class="send-button" type="submit" disabled={chatBusy||busy||!input.trim()} aria-label="Send message">↑</button></div></form></div></aside>
</div>

{#if settingsOpen}
  <div class="modal-backdrop" role="presentation"><div class="settings-modal" role="dialog" aria-modal="true" aria-labelledby="settings-title" tabindex="-1">
    <div class="modal-title"><span class="assistant-mark">✧</span><button aria-label="Close settings" onclick={() => settingsOpen = false}>×</button></div>
    <h2 id="settings-title">Connections</h2><p>Your assistant, your coursework, one place to focus.</p>
    <div class="connection-tabs"><button class:chosen={settingsTab === 'assistant'} onclick={() => settingsTab = 'assistant'}>Assistant</button><button class:chosen={settingsTab === 'canvas'} onclick={() => settingsTab = 'canvas'}>Canvas</button></div>
    {#if settingsTab === 'assistant'}
      <form onsubmit={(event) => { event.preventDefault(); saveSettings(); }}><label>OpenRouter API key<input type="password" bind:value={apiKey} autocomplete="off" placeholder={settings.configured ? 'Connected · leave blank to keep your key' : 'sk-or-…'} /></label><label>Writing & reasoning model<input bind:value={settings.model} placeholder="google/gemini-2.5-flash" /></label><label>Jev model<input bind:value={settings.jevModel} /></label><label>Reasoning effort<select bind:value={settings.reasoningEffort}><option value="minimal">Minimal · quickest</option><option value="low">Low · fast and thoughtful</option><option value="medium">Medium</option><option value="high">High · more thinking time</option></select></label><p class="settings-note">Your key is stored locally with restricted file permissions. Chat sends task context, including imported assignment descriptions, to OpenRouter.</p>{#if settingsError}<div class="settings-error" role="alert">{settingsError}</div>{/if}<button class="primary-button save-settings" disabled={settingsBusy || browserPreview}>{settingsBusy ? 'Saving…' : 'Save assistant settings'}<span>→</span></button></form>
    {:else}
      <form onsubmit={event => { event.preventDefault(); saveCanvas(); }}>
        <p class="canvas-connect-intro">Bring assignments and their requirements from your active student courses into All tasks. Every imported assignment gets the <strong>canvas</strong> label.</p>
        <label>Canvas address<input type="url" bind:value={canvasFormUrl} placeholder="https://pcci.instructure.com" required /></label>
        <label>Personal access token<input type="password" bind:value={canvasFormToken} autocomplete="off" placeholder={canvas.configured ? 'Connected · leave blank to keep your token' : 'Paste your Canvas token'} /></label>
        <p class="settings-note">Create a token in Canvas → Account → Settings → New Access Token. The token stays in the Rust backend after saving and is never sent to OpenRouter. Canvas sync only reads your courses and assignments.</p>
        <label class="canvas-auto"><input type="checkbox" bind:checked={canvasFormAuto} /><span>Sync on launch and every 15 minutes</span></label>
        {#if canvas.report}<div class="canvas-sync-report"><strong>{canvas.report.assignments} assignments · {canvas.report.courses} courses</strong><small>Last synced {new Date(canvas.report.syncedAt).toLocaleString()}</small><small>{canvas.report.created} added · {canvas.report.updated} updated · {canvas.report.unchanged} unchanged</small><button type="button" class="canvas-view-button" disabled={busy} onclick={() => { settingsOpen = false; showCanvas(); }}>View assignments →</button></div>{/if}
        {#if canvasSettingsError || canvas.error}<div class="settings-error" role="alert">{canvasSettingsError || canvas.error}</div>{/if}
        <p class="settings-note">Sync keeps task references, local edits, timers, and Trash. Newly submitted Canvas work becomes complete locally. Checking off a task here never submits or changes anything in Canvas.</p>
        <button class="primary-button save-settings" disabled={canvasSaving || canvasBusy || browserPreview}>{canvasBusy ? 'Syncing Canvas…' : canvasSaving ? 'Saving…' : 'Save & sync assignments'}<span>↻</span></button>
      </form>
    {/if}
  </div></div>
{/if}
