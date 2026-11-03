export const TASK_TYPES = {
  bug: 'Fix broken or incorrect behavior.',
  feature: 'Build a new capability or add functionality.',
  chore: 'Maintenance, cleanup, setup, or routine administrative work.',
  research: 'Investigate, learn, compare, or explore something.',
  meeting: 'Schedule or attend a conversation, call, or meeting.',
  personal: 'Personal errands, home, health, or life tasks.',
} as const;

export const LABELS = {
  work: 'Professional or job related work.',
  school: 'School, coursework, studying, or assignments.',
  coding: 'Programming, software, or technical implementation.',
  writing: 'Writing, editing, or documentation.',
  planning: 'Organizing, scheduling, or making a plan.',
  finance: 'Money, bills, budgets, or purchases.',
  health: 'Health, exercise, or medical appointments.',
  home: 'Household, family, or errands.',
  urgent: 'An explicit immediate deadline or time sensitive blocker.',
  blocked: 'Cannot proceed until a dependency is resolved.',
  'quick-win': 'Explicitly a small, quick, low effort task.',
} as const;

export type TaskType = keyof typeof TASK_TYPES;
export type Label = keyof typeof LABELS;
export type ChoiceQuestion<T extends string = string> = {
  type: 'choice'; instructions: string; criteria: Record<T, string>;
};
export type NoulQuestion = {
  type: 'noul'; instructions: string; criteria: { true: string; false: string };
};
export type ScoreQuestion = {
  type: 'score'; instructions: string; criteria: string[];
};
export type JevQuestion = ChoiceQuestion | NoulQuestion | ScoreQuestion;
export type ChoiceAnswer<T extends string = string> = {
  type: 'choice'; choice: T; confidence: number; probabilities: Record<T, number>;
};
export type NoulAnswer = { type: 'noul'; noul: number };
export type ScoreAnswer = {
  type: 'score'; score: number; confidence: number; probabilities: Record<string, number>;
};
export type JevRequest = {
  model: string; state: { task: string; today: string }; questions: Record<string, JevQuestion>;
};
export type JevResponse = {
  answers: { task_type: ChoiceAnswer<TaskType>; priority: ScoreAnswer } & Record<string, ChoiceAnswer | NoulAnswer | ScoreAnswer>;
};
export type Classification = {
  type: TaskType; labels: Label[]; priority: number; confidence: number;
};
export type Task = Classification & {
  id: number; title: string; description: string; raw: string;
  status: 'open' | 'done'; createdAt: string;
};
