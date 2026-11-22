export type Organization = { role: string; parentId: number | null; milestoneId: number | null; cycleId: number | null; relatedIds: number[]; startsOn: string | null; endsOn: string | null; external: {provider:string;remoteId:string;url:string}[] };
export type Task = {
  organization?: Organization;
  id: number; title: string; description: string; raw: string; type: string; labels: string[];
  priority: number; status: 'open' | 'in_progress' | 'done'; confidence: number;
  createdAt: string; updatedAt: string; dueDate: string | null; deleted: boolean;
  trackedMs: number; timerStartedAt: string | null;
  timeEntries: { startedAt: string; endedAt: string; durationMs: number; manual?: boolean }[];
  estimate?: Estimate | null;
  canvas?: { baseUrl: string; courseId: number; assignmentId: number; courseName: string; courseCode: string; title: string; url: string; dueAt: string | null; submitted: boolean; syncedAt: string } | null;
};
export type View = { status: string; query: string; label: string; taskType: string; sort: string };
export type Stats = {
  total: number; active: number; completed: number; highPriority: number; dueToday: number;
  overdue: number; completionRate: number; completedThisWeek: number;
  labels: Record<string, number>; types: Record<string, number>;
  trackedMs: number; trackedTodayMs: number; runningTaskId: number | null;
};
export type Message = { id: number; role: string; content: string; taskIds: number[]; createdAt: string };
export type Snapshot = {
  tasks: Task[]; visible: Task[]; stats: Stats; messages: Message[]; view: View; focusIds: number[];
  configured: boolean; model: string; demo: boolean;
  nextTaskId: number | null;
  showList: boolean;
  workload: Workload;
};
export type Settings = { configured: boolean; model: string; jevModel: string; reasoningEffort: string; labelThreshold: number };
export type CanvasReport = { courses: number; assignments: number; created: number; updated: number; unchanged: number; warnings: string[]; syncedAt: string };
export type CanvasSettings = { baseUrl: string; configured: boolean; autoSync: boolean; report: CanvasReport | null; error: string | null };
export const defaultView = (): View => ({ status: 'open', query: '', label: '', taskType: '', sort: 'fit' });
export const reference = (id: number) => `TASK-${String(id).padStart(3, '0')}`;
export type Estimate = { minutes: number; technical: number; tedium: number; ambiguity: number; switching: number; source: string };
export type Assessment = { estimate: Estimate; predictedMinutes: number; remainingMinutes: number; units: number; totalUnits: number; lowMinutes: number; highMinutes: number; loadFactor: number; deadlinePressure: number; fitScore: number; exceedsEstimate: boolean; reason: string };
export type PlanBlock = { taskId: number; minutes: number; units: number; partial: boolean };
export type CapacityDay = { date: string; weekday: number; minutes: number; energy: number; capacityUnits: number; spentMinutes: number; spentUnits: number; remainingMinutes: number; remainingUnits: number; blocks: PlanBlock[]; learnedFactor: number };
export type Workload = { profile: { weekdayMinutes: number[]; learningEnabled: boolean }; calibration: { timeSamples: number; effortSamples: number; daySamples: number; timeCoefficients: number[]; loadWeights: number[]; weekdayFactors: number[]; uncertainty: number; stage: string }; assessments: Record<number, Assessment>; days: CapacityDay[]; riskIds: number[]; nextTaskId: number | null };
export type BadgeMode = 'off' | 'overdue' | 'due_today';
export type TrayTitle = 'none' | 'timer' | 'due' | 'both';
export type Repeat = 'none' | 'hourly' | 'daily' | 'weekdays' | 'weekly';
export type AlertSettings = {
  enabled: boolean; dueAlerts: boolean; leadMinutes: number[]; allDayTime: string; overdueDaily: boolean; minPriority: number;
  reminders: boolean; sound: boolean; quietHours: boolean; quietStart: string; quietEnd: string; pausedUntil: string | null;
  badge: BadgeMode; trayTitle: TrayTitle; dockProgress: boolean; keepRunning: boolean;
};
export type Reminder = { id: string; taskId: number | null; title: string; note: string; at: string; repeat: Repeat; enabled: boolean; lastFiredAt: string | null };
export type ReminderInput = { id?: string | null; taskId: number | null; title: string; note: string; at: string; repeat: Repeat; enabled: boolean };
export type AlertsView = { settings: AlertSettings; reminders: Reminder[]; platform: 'macos' | 'windows' | 'linux' };
export const defaultAlerts = (): AlertsView => ({
  platform: 'macos', reminders: [],
  settings: { enabled: true, dueAlerts: true, leadMinutes: [1440, 60, 0], allDayTime: '09:00', overdueDaily: true, minPriority: 0, reminders: true, sound: true, quietHours: false, quietStart: '22:00', quietEnd: '08:00', pausedUntil: null, badge: 'due_today', trayTitle: 'timer', dockProgress: true, keepRunning: true },
});
export const repeatLabels: Record<Repeat, string> = { none: 'Once', hourly: 'Every hour', daily: 'Every day', weekdays: 'Every weekday', weekly: 'Every week' };
/** RFC 3339 with the local offset, from a `datetime-local` value. */
export const localToIso = (value: string) => { const d = new Date(value); return Number.isNaN(d.getTime()) ? '' : d.toISOString(); };
/** A `datetime-local` value for a Date, in local time. */
export const isoToLocal = (d: Date) => new Date(d.getTime() - d.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
export const reminderWhen = (iso: string) => {
  const d = new Date(iso), now = new Date();
  const time = d.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
  if (d.toDateString() === now.toDateString()) return `Today ${time}`;
  const tomorrow = new Date(now); tomorrow.setDate(now.getDate() + 1);
  if (d.toDateString() === tomorrow.toDateString()) return `Tomorrow ${time}`;
  return `${d.toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' })} ${time}`;
};
export type TaskDraft = { title: string; description: string; taskType: string; labels: string[]; priority: number; dueDate: string | null; estimateMinutes: number | null };
/** Mirrors `TYPES` in core/src/model.rs. */
export const taskTypes: [string, string][] = [['chore', 'Chore'], ['feature', 'Feature'], ['bug', 'Bug'], ['research', 'Research'], ['meeting', 'Meeting'], ['personal', 'Personal'], ['assignment', 'Assignment']];
/** Mirrors `LABELS` in core/src/model.rs. */
export const starterLabels = ['work', 'school', 'coding', 'writing', 'planning', 'finance', 'health', 'home', 'urgent', 'blocked', 'quick-win'];
