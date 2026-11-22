<script lang="ts">
  import { onMount } from 'svelte';
  import '$lib/canvas-workspace.css';
  import { invoke } from '$lib/bridge';
  import type { CanvasItem, CanvasTarget, CanvasMutationResult } from '$lib/canvas-types';
  import CanvasAssignment from '$lib/CanvasAssignment.svelte';
  import CanvasContent from '$lib/CanvasContent.svelte';
  let { baseUrl, preview=false, initial=null, onsubmitted=()=>{}, onsettings=()=>{} }: {
    baseUrl: string; preview?: boolean; initial?: CanvasTarget | null; onsubmitted?:()=>void; onsettings?:()=>void;
  } = $props();
  const courseTabs = [{id:'assignments',label:'Assignments'},{id:'modules',label:'Modules'},{id:'announcements',label:'Announcements'},{id:'discussions',label:'Discussions'},{id:'grades',label:'Grades'},{id:'pages',label:'Pages'},{id:'files',label:'Files'},{id:'syllabus',label:'Syllabus'},{id:'quizzes',label:'Quizzes'},{id:'tools',label:'Course tools'}];
  const personalTabs = [{id:'calendar',label:'Upcoming'},{id:'todo',label:'To do'},{id:'inbox',label:'Inbox'},{id:'groups',label:'Groups'}];
  let courses=$state<CanvasItem[]>([]);
  let courseId=$state(0);
  let tab=$state('assignments');
  let items=$state<CanvasItem[]>([]);
  let detail=$state<CanvasItem | null>(null);
  let thread=$state<CanvasItem | null>(null);
  let courseInfo=$state<CanvasItem | null>(null);
  let loading=$state(true);
  let loadingDetail=$state(false);
  let sending=$state(false);
  let error=$state('');
  let detailError=$state('');
  let notice=$state('');
  let search=$state('');
  let drafts=$state<Record<string,string>>({});
  let replyTo=$state<number | null>(null);
  let compose=$state(false);
  let recipientSearch=$state('');
  let recipientResults=$state<CanvasItem[]>([]);
  let recipients=$state<CanvasItem[]>([]);
  let lookingUp=$state(false);
  let subject=$state('');
  let newBody=$state('');
  let sequence=0;
  let detailSequence=0;
  let course=$derived(courses.find(c=>c.id===courseId));
  let personal=$derived(personalTabs.some(t=>t.id===tab));
  let visible=$derived(items.filter(item=>`${item.name || ''} ${item.title || ''} ${item.subject || ''} ${item.display_name || ''} ${item.last_message || ''} ${item.assignment?.name || ''}`.toLowerCase().includes(search.toLowerCase())));
  let draftKey=$derived(`${baseUrl}:${courseId}:${tab}:${detail?.id || 0}:${replyTo || 0}`);
  let replyText=$derived(drafts[draftKey] || '');
  const title=(item:CanvasItem)=>item.name || item.title || item.subject || item.display_name || item.filename || item.label || item.assignment?.name || 'Untitled';
  const time=(value?:string | null)=>value ? new Date(value).toLocaleString() : 'No date';
  const submissionState=(item:CanvasItem)=>item.submission?.workflow_state?.replaceAll('_',' ') || item.workflow_state?.replaceAll('_',' ') || '';
  const sampleCourse:CanvasItem={id:40017,name:'Software Engineering',course_code:'CS 351',enrollments:[{type:'student',computed_current_score:94.5}]};
  const sampleAssignment:CanvasItem={id:101,name:'Iteration 4 · Implementation & review',due_at:new Date(Date.now()+86400000).toISOString(),points_possible:100,submission:{id:0,workflow_state:'unsubmitted'}};
  async function query<T>(resource:string, args:Record<string,unknown>={}):Promise<T> {
    return invoke<T>('canvas_workspace_query',{query:{resource,courseId:courseId || null,...args}});
  }
  async function open(path:string) { try { await invoke('open_canvas_link',{path}); } catch(e) { error=String(e); } }
  async function load() {
    const request=++sequence; ++detailSequence;
    loading=true; loadingDetail=false; error=''; detailError=''; detail=null; thread=null; replyTo=null; items=[]; search='';
    try {
      let result:CanvasItem[]=[];
      let info:CanvasItem | null=null;
      if(preview) {
        const examples:Record<string,CanvasItem[]>={assignments:[sampleAssignment],grades:[sampleAssignment],modules:[{id:1,name:'Week 12 · Build, test, reflect',items_count:3}],announcements:[{id:2,title:'Iteration review this Friday',message_text:'Bring your latest build and a short reflection to class.'}],discussions:[{id:3,title:'Implementation questions',message_text:'Share questions and discuss approaches.',permissions:{reply:true}}],pages:[{id:4,title:'Course resources',url:'course-resources'}],files:[{id:5,display_name:'Iteration rubric.pdf',size:24000}],inbox:[{id:6,subject:'Project check-in',last_message:'How is the implementation going?',workflow_state:'unread'}],calendar:[{id:7,title:'Iteration 4 deadline',start_at:sampleAssignment.due_at || undefined}],todo:[{id:8,assignment:sampleAssignment}],groups:[{id:9,name:'Project team'}],quizzes:[{id:10,title:'Week 12 knowledge check',time_limit:20}],tools:[{id:11,label:'Course home',html_url:'/courses/40017'}]};
        result=examples[tab] || [];
        info={...sampleCourse,syllabus_body_text:'Course schedule, expectations, grading, and policies appear here.'};
      } else if(tab==='syllabus') info=await query<CanvasItem>('course');
      else result=await query<CanvasItem[]>(tab==='tools'?'tabs':tab);
      if(request!==sequence)return;
      items=result; if(info)courseInfo=info;
    } catch(e) { if(request===sequence)error=String(e); }
    finally { if(request===sequence)loading=false; }
  }
  async function selectTab(next:string) { if(sending)return; tab=next; compose=false; await load(); }
  async function selectCourse(next:number) { if(sending)return; courseId=next; courseInfo=null; if(personal)tab='assignments'; await load(); }
  async function selectItem(item:CanvasItem) {
    if(sending)return;
    const request=++detailSequence;
    detail=item; thread=null; replyTo=null; loadingDetail=true; detailError='';
    try {
      let result=item;
      let view:CanvasItem | null=null;
      if(preview) {
        if(tab==='modules') view={id:item.id,view:[{id:21,title:'Project requirements',type:'Assignment',content_id:101},{id:22,title:'Reflection checklist',type:'Page',page_url:'reflection-checklist',completion_requirement:{type:'must_mark_done',completed:false}}]};
        if(tab==='pages')result={...item,body_text:'Your course page and its linked resources appear here.'};
        if(tab==='discussions')view={id:item.id,view:[{id:31,user_id:1,message_text:'Post your implementation questions here.'}],participants:[{id:1,display_name:'Instructor'}]};
        if(tab==='inbox')result={...item,messages:[{id:61,author_id:1,body_text:'How is the implementation going?',created_at:new Date().toISOString()}],participants:[{id:1,name:'Instructor'}]};
      } else if(tab==='modules')view={id:item.id,view:await query<CanvasItem[]>('moduleItems',{id:item.id})};
      else if(tab==='pages')result=await query<CanvasItem>('page',{key:item.url});
      else if(tab==='discussions' || tab==='announcements') {
        result=await query<CanvasItem>('discussion',{id:item.id});
        try { view=await query<CanvasItem>('discussionView',{id:item.id}); }
        catch(e) { if(request===detailSequence)detailError=String(e); }
      } else if(tab==='inbox')result=await query<CanvasItem>('conversation',{id:item.id});
      if(request!==detailSequence)return;
      detail=result; thread=view;
    } catch(e) { if(request===detailSequence)detailError=String(e); }
    finally { if(request===detailSequence)loadingDetail=false; }
  }
  async function mutate(action:Record<string,unknown>, success:string):Promise<boolean> {
    if(sending || preview)return false;
    sending=true; error=''; notice='';
    try { const response=await invoke<CanvasMutationResult>('canvas_workspace_action',{action}); notice=success+(response.syncWarning?` ${response.syncWarning}`:''); return true; }
    catch(e) { error=String(e); return false; }
    finally { sending=false; }
  }
  async function postReply() {
    if(!detail || !replyText.trim())return;
    const item=detail, key=draftKey;
    const action=tab==='inbox'?{kind:'message',conversationId:detail.id,text:replyText}:{kind:'discussionPost',courseId,topicId:detail.id,entryId:replyTo,text:replyText};
    if(await mutate(action,tab==='inbox'?'Message sent to Canvas.':'Reply posted to Canvas.')) { drafts[key]=''; await selectItem(item); }
  }
  async function toggleSubscription() {
    if(!detail)return;
    const subscribed=!detail.subscribed;
    if(await mutate({kind:'discussionSubscribe',courseId,topicId:detail.id,subscribed},subscribed?'Subscribed to discussion.':'Unsubscribed from discussion.'))detail={...detail,subscribed};
  }
  async function moduleAction(item:CanvasItem) {
    if(!detail)return;
    const module=detail;
    if(await mutate({kind:'moduleDone',courseId,moduleId:module.id,itemId:item.id,done:!item.completion_requirement?.completed},'Module progress updated in Canvas.'))await selectItem(module);
  }
  async function viewModuleItem(item:CanvasItem) {
    if(item.type==='Assignment' && item.content_id) { tab='assignments'; const assignment={id:item.content_id,name:item.title}; await load(); detail=assignment; return; }
    if(item.type==='Page' && item.page_url) { tab='pages'; await load(); await selectItem({id:item.content_id || item.id,title:item.title,url:item.page_url}); return; }
    if(item.type==='Discussion' && item.content_id) { tab='discussions'; await load(); await selectItem({id:item.content_id,title:item.title}); return; }
    if(item.html_url)await open(item.html_url);
  }
  async function markRead(item:CanvasItem) {
    if(!detail)return;
    const module=detail;
    if(await mutate({kind:'moduleRead',courseId,moduleId:module.id,itemId:item.id},'Canvas recorded this module item as viewed.'))await selectItem(module);
  }
  async function conversationState(next:string) {
    if(!detail)return;
    if(await mutate({kind:'conversationState',conversationId:detail.id,state:next},`Conversation marked ${next}.`))await load();
  }
  async function findRecipients() {
    if(preview || recipientSearch.trim().length<2 || lookingUp)return;
    lookingUp=true; error='';
    try { recipientResults=await query<CanvasItem[]>('recipients',{key:recipientSearch}); }
    catch(e) { error=String(e); }
    finally { lookingUp=false; }
  }
  async function sendNewMessage() {
    if(await mutate({kind:'newMessage',recipients:recipients.map(r=>Number(r.id)),subject,text:newBody},'Message sent to Canvas.')) { compose=false; newBody=''; subject=''; recipients=[]; recipientResults=[]; await load(); }
  }
  onMount(() => {
    void (async()=>{
      try {
        courses=preview?[sampleCourse]:await query<CanvasItem[]>('courses');
        courseId=courses.find(c=>c.id===initial?.courseId)?.id || courses[0]?.id || 0;
        if(!courseId)tab='calendar';
        await load();
        if(initial) {
          if(initial.baseUrl.replace(/\/$/,'')!==baseUrl.replace(/\/$/,''))error='This task belongs to a different Canvas connection. Change connections to open its submission form.';
          else if(courseId===initial.courseId)detail={id:initial.assignmentId};
          else error='This assignment’s course is not in your active student courses.';
        }
      } catch(e) { error=String(e); loading=false; }
    })();
  });
</script>

<section class="canvas-workspace">
  <div class="cw-connection"><div><span class="canvas-glyph">◉</span><strong>Canvas workspace</strong><small>{baseUrl.replace('https://','')}{preview?' · Display-only preview':''}</small></div><button class="secondary-button" disabled={sending} onclick={onsettings}>Connection settings</button></div>
  <div class="cw-navigation"><label>Course<select value={courseId} disabled={sending || !courses.length} onchange={event=>selectCourse(Number(event.currentTarget.value))}>{#each courses as item}<option value={item.id}>{item.course_code || item.name}</option>{/each}</select></label><div class="cw-personal" aria-label="Your Canvas">{#each personalTabs as item}<button class:chosen={tab===item.id} disabled={sending} onclick={()=>selectTab(item.id)}>{item.label}</button>{/each}</div></div>
  <nav class="cw-tabs" aria-label="Canvas course">{#each courseTabs as item}<button class:chosen={tab===item.id} disabled={sending || !courseId} onclick={()=>selectTab(item.id)}>{item.label}</button>{/each}</nav>
  {#if error}<div class="cw-error" role="alert">{error}<button disabled={loading || sending} onclick={load}>Refresh</button></div>{/if}
  {#if notice}<div class="cw-success" role="status">{notice}</div>{/if}
  <div class="cw-toolbar"><div><span class="section-kicker">{personal?'YOUR CANVAS':course?.course_code || 'COURSE'}</span><h2>{[...courseTabs,...personalTabs].find(t=>t.id===tab)?.label}</h2></div><div><input aria-label="Search this Canvas view" type="search" placeholder="Filter this view…" bind:value={search}/>{#if tab==='inbox'}<button class="secondary-button" disabled={sending} onclick={()=>compose=!compose}>{compose?'Close composer':'New message'}</button>{/if}<button class="secondary-button" disabled={loading || sending} onclick={load}>↻ Refresh</button></div></div>
  {#if compose && tab==='inbox'}<form class="cw-compose" onsubmit={event=>{event.preventDefault();sendNewMessage();}}><h3>New Canvas message</h3><label>Find recipients{courseId?' in this course':''}<div class="cw-actions"><input bind:value={recipientSearch} placeholder="Name or email" disabled={sending}/><button type="button" class="secondary-button" disabled={lookingUp || preview || sending || recipientSearch.trim().length<2} onclick={findRecipients}>{lookingUp?'Searching…':'Find'}</button></div></label><div class="cw-recipient-results">{#each recipientResults.filter(r=>!recipients.some(p=>p.id===r.id)) as person}<button type="button" onclick={()=>{recipients=[...recipients,person];recipientResults=[];recipientSearch='';}}>{person.name} ＋</button>{/each}</div><div class="cw-actions">{#each recipients as person}<button type="button" class="cw-chip" disabled={sending} onclick={()=>recipients=recipients.filter(p=>p.id!==person.id)}>{person.name} ×</button>{/each}</div><label>Subject<input bind:value={subject} maxlength="255" required disabled={sending}/></label><label>Message<textarea rows="5" bind:value={newBody} maxlength="200000" required disabled={sending}></textarea></label><button class="primary-button" disabled={preview || sending || !recipients.length || !newBody.trim()}>{sending?'Sending…':'Send message to Canvas'}</button></form>{/if}
  {#if loading}<p class="cw-empty" role="status">Loading from Canvas…</p>
  {:else if tab==='syllabus'}<section class="cw-section"><h2>{course?.name}</h2><CanvasContent text={courseInfo?.syllabus_body_text || 'No syllabus has been published in Canvas.'} html={courseInfo?.syllabus_body}/></section>
  {:else}
    {#if tab==='grades'}<div class="cw-grade-summary"><strong>{course?.enrollments?.find(e=>e.type==='student')?.computed_current_score ?? '—'}<small>% current course score</small></strong><p>Canvas calculates your grade. Hidden or unposted grades remain hidden.</p></div>{/if}
    {#if tab==='quizzes'}<p class="cw-warning">Quiz details appear here. Take quizzes in Canvas to preserve timers, question behavior, and proctoring requirements.</p>{/if}
    {#if tab==='tools'}<p class="cw-muted">Your school’s enabled course navigation, including external tools and features that require Canvas.</p>{/if}
    <div class="cw-split" class:has-detail={Boolean(detail)}><div class="cw-list">
      {#each visible as item, index (`${item.id || item.assignment?.id || index}:${index}`)}
        {#if tab==='files'}<button class="cw-list-item" disabled={preview || sending} onclick={()=>open(`/courses/${courseId}/files/${item.id}/download`)}><strong>{title(item)} ↗</strong><small>{((item.size || 0)/1024).toFixed(1)} KB</small></button>
        {:else if tab==='tools'}<button class="cw-list-item" disabled={preview || sending} onclick={()=>open(item.html_url || `/courses/${courseId}`)}><strong>{item.label || title(item)} ↗</strong><small>{item.type}</small></button>
        {:else if tab==='groups'}<button class="cw-list-item" disabled={preview || sending} onclick={()=>open(`/groups/${item.id}`)}><strong>{title(item)} ↗</strong><small>Open group workspace in Canvas</small></button>
        {:else if tab==='quizzes'}<button class="cw-list-item" disabled={preview || sending} onclick={()=>open(item.html_url || `/courses/${courseId}/quizzes/${item.id}`)}><strong>{title(item)} ↗</strong><small>{item.time_limit?`${item.time_limit} minute limit · `:''}{time(item.due_at)}</small></button>
        {:else if tab==='calendar' || tab==='todo'}<button class="cw-list-item" disabled={preview || sending} onclick={()=>open(item.html_url || item.assignment?.html_url || '/calendar')}><strong>{title(item)} ↗</strong><small>{time(item.start_at || item.assignment?.due_at || item.due_at)}</small></button>
        {:else}<button class="cw-list-item" class:chosen={detail?.id===item.id} disabled={sending} onclick={()=>selectItem(item)}><strong>{title(item)}</strong><small>{tab==='assignments'||tab==='grades'?`${time(item.due_at)} · ${submissionState(item)}`:tab==='modules'?`${item.items_count || 0} items`:tab==='inbox'?`${item.workflow_state || ''} · ${item.last_message || ''}`:time(item.updated_at || item.created_at)}</small>{#if tab==='grades'}<span class="cw-score">{item.submission?.grade ?? '—'} / {item.points_possible ?? '—'}{item.submission?.late?' · Late':''}{item.submission?.missing?' · Missing':''}</span>{/if}</button>{/if}
      {/each}
      {#if !visible.length}<div class="cw-empty">{error?'This view could not be loaded.':search?'No matching items.':'No items in this view.'}</div>{/if}
    </div>
    {#if detail}<div class="cw-detail"><button class="cw-back" disabled={sending} onclick={()=>{detail=null;++detailSequence;}}>← Back to list</button>
      {#if tab==='assignments' || tab==='grades'}{#key `${courseId}:${detail.id}`}<CanvasAssignment {courseId} assignmentId={detail.id} {baseUrl} {preview} {onsubmitted} onbusy={value=>sending=value}/>{/key}
      {:else if loadingDetail}<p class="cw-empty" role="status">Loading details…</p>
      {:else}
        {#if detailError}<p class="cw-warning" role="alert">{detailError}</p>{/if}
        <div class="cw-detail-heading"><h2>{title(detail)}</h2>{#if detail.html_url}<button class="secondary-button" disabled={preview || sending} onclick={()=>open(detail!.html_url!)}>Open in Canvas ↗</button>{/if}</div>
        {#if tab==='pages'}<CanvasContent text={detail.body_text} html={detail.body}/>
        {:else if tab==='modules'}{#each thread?.view || [] as item}<div class="cw-module-item"><button class="cw-resource-link" disabled={sending} onclick={()=>viewModuleItem(item)}>{item.title}<small>{item.type}{item.content_details?.locked_for_user?' · Locked':''}</small></button>{#if item.completion_requirement?.type==='must_mark_done'}<button class="secondary-button" disabled={preview || sending} onclick={()=>moduleAction(item)}>{item.completion_requirement.completed?'✓ Done · Undo':'Mark as done'}</button>{:else if item.completion_requirement?.type==='must_view'}<button class="secondary-button" disabled={preview || sending || item.completion_requirement.completed} onclick={()=>markRead(item)}>{item.completion_requirement.completed?'✓ Viewed':'Mark viewed'}</button>{/if}</div>{/each}
        {:else if tab==='inbox'}<div class="cw-actions"><button class="secondary-button" disabled={preview || sending} onclick={()=>conversationState(detail?.workflow_state==='unread'?'read':'unread')}>Mark {detail.workflow_state==='unread'?'read':'unread'}</button><button class="secondary-button" disabled={preview || sending} onclick={()=>conversationState('archived')}>Archive</button></div><p class="cw-muted">With {detail.participants?.map(p=>p.name).join(', ')}</p>{#each detail.messages || [] as message}<article class="cw-comment"><strong>{detail.participants?.find(p=>p.id===message.author_id)?.name || 'Participant'}</strong><small>{time(message.created_at)}</small><CanvasContent text={message.body}/>{#each message.attachments || [] as file}{#if file.url}<button class="cw-resource-link" disabled={preview || sending} onclick={()=>open(file.url!)}>{file.display_name || file.filename} ↗</button>{/if}{/each}</article>{/each}
        {:else if tab==='discussions' || tab==='announcements'}<CanvasContent text={detail.message_text} html={detail.message}/><div class="cw-actions"><button class="secondary-button" disabled={preview || sending} onclick={toggleSubscription}>{detail.subscribed?'Unsubscribe':'Subscribe'}</button></div>{#each thread?.view || [] as entry}<article class="cw-comment"><strong>{thread?.participants?.find(p=>p.id===entry.user_id)?.display_name || 'Participant'}</strong><small>{time(entry.created_at)}</small><CanvasContent text={entry.message_text} html={entry.message}/>{#if !detail.locked && detail.permissions?.reply!==false}<button class="cw-resource-link" disabled={sending} onclick={()=>replyTo=entry.id}>Reply to this post</button>{/if}{#each entry.replies || [] as reply}<article class="cw-comment cw-nested"><strong>{thread?.participants?.find(p=>p.id===reply.user_id)?.display_name || 'Participant'}</strong><CanvasContent text={reply.message_text} html={reply.message}/></article>{/each}</article>{/each}
        {/if}
        {#if tab==='inbox' || ((tab==='discussions'||tab==='announcements') && !detail.locked && detail.permissions?.reply!==false)}<form class="cw-reply" onsubmit={event=>{event.preventDefault();postReply();}}><label>{replyTo?'Reply to selected post':tab==='inbox'?'Reply to conversation':'Add a discussion post'}<textarea rows="4" value={replyText} oninput={event=>drafts[draftKey]=event.currentTarget.value} disabled={sending || preview} required maxlength="200000" placeholder="Write your message…"></textarea></label><div class="cw-actions"><button class="primary-button" disabled={preview || sending || !replyText.trim()}>{sending?'Sending…':tab==='inbox'?'Send reply to Canvas':'Post to Canvas'}</button>{#if replyTo}<button type="button" class="secondary-button" onclick={()=>replyTo=null}>Reply to topic instead</button>{/if}</div></form>{/if}
      {/if}
    </div>{/if}</div>
  {/if}
  <p class="cw-footnote">Task checkboxes track your own progress. Only the explicit Canvas submission, posting, and progress controls change Canvas.</p>
</section>
