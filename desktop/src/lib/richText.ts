// A small, safe renderer for assistant replies: paragraphs, "- " bullets,
// **bold** and TASK-### references. Produces data, never HTML strings.
export type Part = { kind: 'text' | 'bold'; text: string } | { kind: 'ref'; text: string; id: number };
export type Block = { kind: 'p'; lines: Part[][] } | { kind: 'ul' | 'ol'; items: Part[][] };

const INLINE = /\*\*(.+?)\*\*|\bTASK-(\d{1,6})\b/g;

export function inline(text: string): Part[] {
  const parts: Part[] = [];
  let last = 0;
  for (const m of text.matchAll(INLINE)) {
    const at = m.index ?? 0;
    if (at > last) parts.push({ kind: 'text', text: text.slice(last, at) });
    if (m[1] !== undefined) parts.push({ kind: 'bold', text: m[1] });
    else parts.push({ kind: 'ref', text: m[0], id: Number(m[2]) });
    last = at + m[0].length;
  }
  if (last < text.length) parts.push({ kind: 'text', text: text.slice(last) });
  return parts;
}

export function blocks(text: string): Block[] {
  const out: Block[] = [];
  for (const raw of text.replace(/\r\n?/g, '\n').split('\n')) {
    const line = raw.trimEnd();
    const bullet = /^\s*[-*•]\s+(.*)$/.exec(line);
    const numbered = /^\s*\d{1,2}[.)]\s+(.*)$/.exec(line);
    const prev = out[out.length - 1];
    if (!line.trim()) { out.push({ kind: 'p', lines: [] }); continue; }
    if (bullet || numbered) {
      const kind = bullet ? 'ul' : 'ol';
      const item = inline((bullet ?? numbered)![1]);
      if (prev && prev.kind === kind) prev.items.push(item);
      else out.push({ kind, items: [item] });
      continue;
    }
    const content = inline(line.replace(/^#{1,6}\s+/, ''));
    if (prev && prev.kind === 'p') prev.lines.push(content);
    else out.push({ kind: 'p', lines: [content] });
  }
  return out.filter((b) => (b.kind === 'p' ? b.lines.length > 0 : true));
}
