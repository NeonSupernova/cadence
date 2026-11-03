import { createInterface } from 'node:readline/promises';
import { stdin, stdout } from 'node:process';
import { board, detail, demoTasks } from './ui.ts';
import { classify, writeTask } from './ai.ts';
import { openStore } from './store.ts';
import { TASK_TYPES, LABELS } from './types.ts';
import type { Task } from './types.ts';

const color = (code: number, text: string) => stdout.isTTY && !process.env.NO_COLOR ? `\x1b[${code}m${text}\x1b[0m` : text;
const cyan = (s: string) => color(36, s);
const dim = (s: string) => color(2, s);
const store = openStore();

function list(filter = '') {
  const parts = filter.split(/\s+/).filter(Boolean);
  const tasks = store.list().filter(task => parts.every(part => {
    if (part === 'all') return true;
    if (part === 'done' || part === 'open') return task.status === part;
    if (part.startsWith('#')) return task.labels.includes(part.slice(1) as any);
    if (part.startsWith('type:')) return task.type === part.slice(5);
    return `${task.title} ${task.description} ${task.raw}`.toLowerCase().includes(part.toLowerCase());
  })).filter(task => parts.includes('all') || parts.includes('done') || task.status === 'open');
  if (stdout.isTTY) console.clear();
  board(tasks, store.list(), filter);
}

function help() {
  console.log(`\n  ${cyan('Just type a task.')} Jev sorts it, a reasoning model writes it.\n
  /list [all|done|#label|type:bug|search]  Filter and sort tasks
  /show 1                                Full task details
  /done 1                                Complete a task
  /open 1                                Reopen a task
  /types                                 Show Jev taxonomy
  /demo                                  Preview a populated sample board
  /help                                  Show commands
  /quit                                  Exit\n`);
}

async function handle(input: string) {
  input = input.trim();
  if (!input) return;
  const [command, ...rest] = input.split(/\s+/);
  const arg = rest.join(' ');
  if (command === '/help') return help();
  if (command === '/demo') { board(demoTasks(), demoTasks(), '', true); return; }
  if (command === '/list') return list(arg);
  if (command === '/types') {
    console.log('\n' + Object.entries(TASK_TYPES).map(([k, v]) => `  ${cyan(k)} — ${v}`).join('\n'));
    console.log(`\n  Labels: ${Object.keys(LABELS).map(x => `#${x}`).join(' ')}\n`);
    return;
  }
  if (['/show', '/done', '/open'].includes(command)) {
    if (!/^[1-9]\d*$/.test(arg) || !Number.isSafeInteger(Number(arg))) throw new Error('Supply a task ID, for example /done 1.');
    const id = Number(arg);
    if (command === '/show') {
      const task = store.list().find(t => t.id === id);
      if (!task) throw new Error(`No task #${id}.`);
      return detail(task);
    }
    if (!store.status(id, command === '/done' ? 'done' : 'open')) throw new Error(`No task #${id}.`);
    list('all');
    console.log(color(32, `  ✓ Task #${id} ${command === '/done' ? 'completed' : 'reopened'}.\n`));
    return;
  }
  if (input.startsWith('/')) throw new Error('Unknown command. Use /help.');
  if (input.length > 20_000) throw new Error('Keep tasks under 20,000 characters.');
  const started = performance.now();
  console.log(cyan('\n  [1/3] CLASSIFY  Jev → type, labels, priority…'));
  const classification = await classify(input);
  console.log(`  ${cyan(classification.type)} ${classification.labels.map(x => `#${x}`).join(' ')} ${dim(`· priority ${classification.priority}`)}`);
  console.log(color(35, '  [2/3] WRITE     Reasoning model → title + description…'));
  const writing = await writeTask(input, classification);
  const task = store.add({ raw: input, ...classification, ...writing });
  list();
  console.log(color(32, `  [3/3] SAVED     Task #${task.id} · ${((performance.now() - started) / 1000).toFixed(1)}s`));
  detail(task);
}

try {
  if (process.argv.length > 2) {
    await handle(process.argv.slice(2).join(' '));
  } else {
    list();
    const rl = createInterface({ input: stdin, output: stdout, terminal: Boolean(stdin.isTTY) });
    let closed = false;
    rl.on('close', () => { closed = true; });
    rl.on('SIGINT', () => rl.close());
    try {
      while (!closed) {
        const input = await rl.question(cyan('  TASK OR COMMAND › '));
        if (input.trim() === '/quit' || input.trim() === '/exit') break;
        try { await handle(input); }
        catch (error) { console.error(color(31, `  ✗ ${(error as Error).message}\n`)); }
      }
    } finally { rl.close(); }
  }
} catch (error) {
  if ((error as NodeJS.ErrnoException).code !== 'ERR_USE_AFTER_CLOSE') {
    console.error(color(31, `  ✗ ${(error as Error).message}`));
    process.exitCode = 1;
  }
} finally { store.close(); }
