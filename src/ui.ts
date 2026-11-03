import { stdout } from 'node:process';
import { stripVTControlCharacters } from 'node:util';
import type { Task } from './types.ts';

export const paint = (code: string | number, text: string) => stdout.isTTY && !process.env.NO_COLOR ? `\x1b[${code}m${text}\x1b[0m` : text;
export const muted = (text: string) => paint(2, text);
const clean = (text: string) => stripVTControlCharacters(text).replace(/[\x00-\x1f\x7f]/g, ' ');
const fit = (text: string, width: number) => {
  const chars = Array.from(clean(text));
  return (chars.length > width ? chars.slice(0, width - 1).join('') + '…' : chars.join('')).padEnd(width);
};
const priority = (n: number) => n >= 75 ? 'HIGH' : n >= 50 ? 'MED' : 'LOW';
const priorityColor = (n: number) => n >= 75 ? '1;31' : n >= 50 ? '1;33' : '32';
const typeColors: Record<string, number> = { bug: 31, feature: 36, chore: 34, research: 35, meeting: 33, personal: 32 };

export function board(tasks: Task[], all: Task[], filter = '', demo = false) {
  const width = Math.max(42, Math.min(stdout.columns || 112, 140) - 4);
  const wide = width >= 90;
  const narrow = width < 65;
  const columns = narrow
    ? [5, 7, width - 5 - 7 - 10]
    : wide ? [5, 7, 9, 10, width - 5 - 7 - 9 - 10 - 24 - 19, 24]
    : [5, 7, 9, width - 5 - 7 - 9 - 13];
  const headers = narrow ? ['ID', 'STATE', 'TASK'] : wide ? ['ID', 'STATE', 'PRIORITY', 'TYPE', 'TASK', 'LABELS'] : ['ID', 'STATE', 'PRIORITY', 'TASK'];
  const line = (left: string, mid: string, right: string) => '  ' + muted(left + columns.map(n => '─'.repeat(n + 2)).join(mid) + right);
  const row = (cells: string[], styles: (string | number)[] = []) => '  ' + muted('│') + cells.map((cell, i) => ' ' + paint(styles[i] || 0, fit(cell, columns[i])) + ' ').join(muted('│')) + muted('│');
  console.log(`\n  ${paint('1;36', '◈  JEV TASKS')}  ${muted(demo ? 'DEMO · sample data · nothing saved' : 'NATURAL LANGUAGE TASK TRACKER')}`);
  console.log(`  ${muted('CAPTURE')} → ${paint(36, 'JEV · classify + rank')} → ${paint(35, 'REASON · title + description')} → ${paint(32, 'SAVED')}`);
  const open = all.filter(t => t.status === 'open');
  console.log(`\n  ${paint('1;37', `${open.length} OPEN`)}     ${paint('1;31', `${open.filter(t => t.priority >= 75).length} HIGH PRIORITY`)}     ${paint('1;32', `${all.length - open.length} DONE`)}\n`);
  console.log(`  ${paint(1, filter ? `FILTER: ${clean(filter)}` : 'TASK BOARD')}  ${muted('↓ priority · oldest first on ties')}`);
  console.log(line('┌', '┬', '┐'));
  console.log(row(headers, headers.map(() => '1;36')));
  console.log(line('├', '┼', '┤'));
  for (const task of tasks) {
    const state = task.status === 'done' ? '✓ DONE' : '○ OPEN';
    const p = `${priority(task.priority)} ${task.priority}`;
    const cells = narrow ? [`#${task.id}`, state, task.title] : wide
      ? [`#${task.id}`, state, p, task.type, task.title, task.labels.map(l => `#${l}`).join(' ')]
      : [`#${task.id}`, state, p, task.title];
    const styles = narrow ? [2, task.status === 'done' ? 32 : 33, 1]
      : wide ? [2, task.status === 'done' ? 32 : 33, priorityColor(task.priority), typeColors[task.type], task.status === 'done' ? 2 : 1, 34]
      : [2, task.status === 'done' ? 32 : 33, priorityColor(task.priority), 1];
    console.log(row(cells, styles));
  }
  if (!tasks.length) console.log(row(headers.map((_, i) => i === headers.length - 1 ? 'No tasks yet. Add one below.' : '')));
  console.log(line('└', '┴', '┘'));
  console.log(`  ${muted(`${tasks.length} shown`)}  ${muted('·')}  ${paint(31, 'HIGH 75+')}  ${paint(33, 'MED 50+')}  ${paint(32, 'LOW <50')}`);
  console.log(`\n  ${paint('1;36', 'ADD TASK')}  Type what needs doing, then press Enter.`);
  console.log(`  ${muted('Try: Fix the checkout crash before Friday — blocking launch')}`);
  console.log(`\n  ${paint(36, '/list all')}  ${paint(36, '/show 1')}  ${paint(32, '/done 1')}  ${paint(36, '/list #coding')}  ${muted('/help  /demo  /quit')}\n`);
}

export function detail(task: Task) {
  const width = Math.max(30, Math.min(stdout.columns || 100, 110) - 8);
  const wrap = (text: string) => {
    const words = clean(text).split(/\s+/);
    let line = '';
    for (const word of words) {
      if (line.length + word.length + 1 > width && line) { console.log('    ' + line); line = ''; }
      for (let offset = 0; offset < word.length; offset += width) {
        const chunk = word.slice(offset, offset + width);
        if (offset || chunk.length === width) { if (line) console.log('    ' + line); console.log('    ' + chunk); line = ''; }
        else line += (line ? ' ' : '') + chunk;
      }
    }
    if (line) console.log('    ' + line);
  };
  console.log(`\n  ${paint('1;36', `TASK #${task.id}`)}  ${paint(task.status === 'done' ? 32 : 33, task.status.toUpperCase())}  ${paint(priorityColor(task.priority), `${priority(task.priority)} ${task.priority}`)}`);
  console.log('  ' + muted('─'.repeat(width + 2)));
  wrap(task.title);
  console.log(`\n    ${paint(typeColors[task.type], task.type)}  ${paint(34, task.labels.map(x => `#${x}`).join(' '))}  ${muted(`${Math.round(task.confidence * 100)}% type confidence`)}`);
  console.log(`\n    ${muted('DESCRIPTION')}`); wrap(task.description);
  console.log(`\n    ${muted('ORIGINAL INPUT')}`); wrap(task.raw);
  console.log();
}

export function demoTasks(): Task[] {
  return [
    { id: 1, title: 'Fix checkout crash before launch', type: 'bug', labels: ['coding', 'work', 'urgent'], priority: 96, status: 'open', raw: 'Checkout crashes on payment. Fix before Friday, blocking launch.', description: 'Investigate the crash during payment and fix checkout before Friday’s launch.', confidence: 0.98 },
    { id: 2, title: 'Finish database assignment', type: 'chore', labels: ['school', 'coding'], priority: 78, status: 'open', raw: 'Finish my database assignment due tomorrow.', description: 'Complete the database assignment before tomorrow’s deadline.', confidence: 0.89 },
    { id: 3, title: 'Add dark mode to settings', type: 'feature', labels: ['coding', 'work'], priority: 58, status: 'open', raw: 'Add a dark mode toggle in settings.', description: 'Implement a settings toggle for switching between light and dark mode.', confidence: 0.96 },
    { id: 4, title: 'Compare vector database options', type: 'research', labels: ['coding', 'planning'], priority: 41, status: 'open', raw: 'Look into vector databases when I have time.', description: 'Explore vector database options and compare their tradeoffs.', confidence: 0.94 },
    { id: 5, title: 'Book a dentist appointment', type: 'personal', labels: ['health'], priority: 25, status: 'open', raw: 'Book a routine dentist appointment.', description: 'Contact the dentist to schedule a routine appointment.', confidence: 0.97 },
    { id: 6, title: 'Send project update', type: 'chore', labels: ['work', 'writing'], priority: 50, status: 'done', raw: 'Send the project update to the team.', description: 'Write and send the team a project progress update.', confidence: 0.92 },
  ].map(t => ({ ...t, createdAt: new Date().toISOString() })) as Task[];
}
