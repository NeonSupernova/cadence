import { DatabaseSync } from 'node:sqlite';
import { mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import type { Task } from './types.ts';

export function openStore(path = process.env.TASK_DB || 'data/tasks.sqlite') {
  mkdirSync(dirname(path), { recursive: true });
  const db = new DatabaseSync(path);
  db.exec(`CREATE TABLE IF NOT EXISTS tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    payload TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    priority INTEGER NOT NULL
  )`);
  return {
    add(task: Omit<Task, 'id' | 'status' | 'createdAt'>): Task {
      const record = { ...task, status: 'open' as const, createdAt: new Date().toISOString() };
      const result = db.prepare('INSERT INTO tasks (payload, priority) VALUES (?, ?)').run(JSON.stringify(record), task.priority);
      return { ...record, id: Number(result.lastInsertRowid) };
    },
    list(): Task[] {
      return db.prepare("SELECT * FROM tasks ORDER BY CASE status WHEN 'open' THEN 0 ELSE 1 END, priority DESC, id ASC").all().map(row => ({
        ...JSON.parse(row.payload as string), id: Number(row.id), status: row.status,
      }));
    },
    status(id: number, status: Task['status']): boolean {
      return db.prepare('UPDATE tasks SET status = ? WHERE id = ?').run(status, id).changes > 0;
    },
    close() { db.close(); },
  };
}
