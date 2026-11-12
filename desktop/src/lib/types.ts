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
