import type { Snapshot, Task, Assessment } from './types';
import { defaultView } from './types';
// Display-only fixtures for reviewing the UI in a browser, never used by the desktop app.
export function preview(): Snapshot {
  const now = new Date().toISOString();
  const today = new Date().toLocaleDateString('en-CA');
  const tasks = [
    { id: 1, title: 'Fix the checkout crash', description: 'Investigate the payment crash and get checkout working before launch.', type: 'bug', labels: ['work', 'coding', 'urgent'], priority: 96, status: 'in_progress', dueDate: today },
    { id: 2, title: 'Finish the database assignment', description: 'Complete the database assignment and review the SQL queries.', type: 'chore', labels: ['school', 'coding'], priority: 82, status: 'open', dueDate: today },
    { id: 3, title: 'Plan the weekend trip', description: 'Choose somewhere to stay and a few things to do.', type: 'personal', labels: ['home', 'planning'], priority: 55, status: 'open', dueDate: null },
    { id: 4, title: 'Explore vector databases', description: 'Compare options for the next project.', type: 'research', labels: ['coding', 'work'], priority: 43, status: 'open', dueDate: null },
    { id: 5, title: 'Book a dentist appointment', description: 'Schedule a routine checkup.', type: 'personal', labels: ['health'], priority: 30, status: 'open', dueDate: null },
    { id: 6, title: 'Send the weekly update', description: 'Share this week’s progress with the team.', type: 'chore', labels: ['work', 'writing'], priority: 50, status: 'done', dueDate: null },
  ].map(t => {
    const trackedMs = t.id === 1 ? 2520000 : t.id === 6 ? 900000 : 0;
    return { ...t, raw: t.description, confidence: 0.96, createdAt: now, updatedAt: now, deleted: false, trackedMs, timerStartedAt: null,
      timeEntries: trackedMs ? [{ startedAt: now, endedAt: now, durationMs: trackedMs, manual: true }] : [] };
  }) as Task[];
  const assessments = Object.fromEntries(tasks.map(t => { const minutes = t.id===1 ? 90 : t.id===2 ? 75 : 30; return [t.id, { estimate: {minutes,technical:t.type==='bug'?4:2,tedium:3,ambiguity:3,switching:2,source:'starter'},predictedMinutes:minutes,remainingMinutes:minutes,units:minutes/25,totalUnits:minutes/25,lowMinutes:minutes/2,highMinutes:minutes*2,loadFactor:1,deadlinePressure:t.dueDate?1:0,fitScore:80,exceedsEstimate:false,reason:'Starter estimate. Review the task scope before relying on it.' }]; })) as Record<number,Assessment>;
  const days = Array.from({length:7}, (_,i) => {const date=new Date();date.setDate(date.getDate()+i); const weekday=(date.getDay()+6)%7;const minutes=weekday<5?120:60; return {date:date.toLocaleDateString('en-CA'),weekday,minutes,energy:3,capacityUnits:minutes/25,spentMinutes:0,spentUnits:0,remainingMinutes:minutes,remainingUnits:minutes/25,blocks:i===0?[{taskId:1,minutes:75,units:3,partial:true}]:[],learnedFactor:1};});
  const workload = {profile:{weekdayMinutes:[120,120,120,120,120,60,60],learningEnabled:true},calibration:{timeSamples:0,effortSamples:0,daySamples:0,timeCoefficients:[],loadWeights:[1,.2,.15,.12,.08],weekdayFactors:[1,1,1,1,1,1,1],uncertainty:.7,stage:'Starter estimates'},assessments,days,riskIds:[2],nextTaskId:1};
  return { workload, showList: false, nextTaskId: 1, tasks, visible: tasks.filter(t => t.status !== 'done'), stats: { total: 6, active: 5, completed: 1, highPriority: 2, dueToday: 2, overdue: 0, completionRate: 17, completedThisWeek: 1, trackedMs: 3420000, trackedTodayMs: 3420000, runningTaskId: null, labels: { work: 3, coding: 3, school: 1, home: 1, planning: 1, health: 1, urgent: 1, writing: 1 }, types: { bug: 1, chore: 2, personal: 2, research: 1 } }, messages: [
    { id: 1, role: 'user', content: 'I need to fix the checkout crash before launch. It’s blocking everything.', taskIds: [], createdAt: now },
    { id: 2, role: 'assistant', content: 'Added it as a high priority bug, with work, coding, and urgent labels. It’s at the top of your list.', taskIds: [1], createdAt: now },
  ], view: defaultView(), focusIds: [], configured: false, model: 'google/gemini-2.5-flash', demo: true };
}
