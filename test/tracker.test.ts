import { test } from 'node:test';
import assert from 'node:assert/strict';
import { classify, parseClassification, parseWriting, questions, writeTask } from '../src/ai.ts';
import { openStore } from '../src/store.ts';
import { LABELS } from '../src/types.ts';
import type { JevResponse } from '../src/types.ts';

function fixture(): JevResponse {
  return { answers: {
    task_type: { type: 'choice', choice: 'bug', confidence: 0.91, probabilities: { bug: 0.91 } as any },
    priority: { type: 'score', score: 3.6, confidence: 0.9, probabilities: {} },
    ...Object.fromEntries(Object.keys(LABELS).map(label => [`label_${label}`, { type: 'noul', noul: ['coding', 'urgent'].includes(label) ? 0.9 : 0.2 }])),
  } } as JevResponse;
}

test('Jev questions and validated decisions preserve multilabels and score ordering', () => {
  assert.equal(Object.keys(questions()).length, Object.keys(LABELS).length + 2);
  const result = parseClassification(fixture());
  assert.deepEqual(result, { type: 'bug', labels: ['coding', 'urgent'], priority: 90, confidence: 0.91 });
  const invalid = fixture();
  invalid.answers.priority.score = NaN;
  assert.throws(() => parseClassification(invalid), /Invalid Jev/);
  delete (invalid.answers as any).label_coding;
  assert.throws(() => parseClassification(invalid), /omitted label/);
  assert.throws(() => parseWriting('{"title":"","description":"abc"}'), /invalid/);
});

test('two-stage API pipeline uses decisions then reasoning, persists and completes tasks', async () => {
  const oldFetch = globalThis.fetch;
  const oldKey = process.env.OPENROUTER_API_KEY;
  process.env.OPENROUTER_API_KEY = 'test-key';
  const requests: { url: string; body: any }[] = [];
  globalThis.fetch = async (url, init) => {
    const body = JSON.parse(init!.body as string);
    requests.push({ url: String(url), body });
    return Response.json(String(url).endsWith('decisions') ? fixture() : {
      choices: [{ finish_reason: 'stop', message: { content: JSON.stringify({ title: 'Fix checkout crash', description: 'Resolve the checkout crash before launch.' }) } }],
    });
  };
  const store = openStore(':memory:');
  try {
    const raw = 'Fix checkout crash before launch';
    const classification = await classify(raw);
    const writing = await writeTask(raw, classification);
    const task = store.add({ raw, ...classification, ...writing });
    store.add({ raw: 'later', ...classification, ...writing, priority: 10 });
    assert.equal(requests[0].url, 'https://openrouter.ai/api/alpha/decisions');
    assert.equal(requests[0].body.state.task, raw);
    assert.equal(requests[1].body.reasoning.exclude, true);
    assert.equal(JSON.parse(requests[1].body.messages[1].content).classification.type, 'bug');
    assert.equal(store.list()[0].id, task.id);
    assert.equal(store.status(task.id, 'done'), true);
    assert.equal(store.list()[0].priority, 10);
    assert.equal(store.list()[1].status, 'done');
    assert.equal(store.status(999, 'done'), false);
  } finally {
    store.close(); globalThis.fetch = oldFetch;
    if (oldKey === undefined) delete process.env.OPENROUTER_API_KEY;
    else process.env.OPENROUTER_API_KEY = oldKey;
  }
});

test('API failures surface actionable errors', async () => {
  const original = globalThis.fetch;
  const oldKey = process.env.OPENROUTER_API_KEY;
  process.env.OPENROUTER_API_KEY = 'test-key';
  globalThis.fetch = async () => new Response('', { status: 402 });
  try { await assert.rejects(classify('test'), /add credits/); }
  finally {
    globalThis.fetch = original;
    if (oldKey === undefined) delete process.env.OPENROUTER_API_KEY;
    else process.env.OPENROUTER_API_KEY = oldKey;
  }
});
