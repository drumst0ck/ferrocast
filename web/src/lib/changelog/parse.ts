import { createHash } from 'node:crypto';

export type Category = 'added' | 'changed' | 'fixed' | 'removed' | 'security' | 'limitation';

export interface ParsedEntry {
  category: Category;
  title: string | null;
  bodyMd: string;
  docsUrl: string | null;
  tags: string[];
  position: number;
  slug: string;
}

export interface ParsedRelease {
  version: string;
  releasedOn: string | null;
  summary: string | null;
  entries: ParsedEntry[];
}

const CATEGORIES: Record<string, Category> = {
  added: 'added',
  changed: 'changed',
  fixed: 'fixed',
  removed: 'removed',
  security: 'security',
  'known limitations': 'limitation',
};

const fail = (line: number, msg: string): never => {
  throw new Error(`line ${line}: ${msg}`);
};

function finishEntry(raw: string, category: Category, version: string, position: number, seen: Map<string, number>): ParsedEntry {
  let text = raw.trim();
  let docsUrl: string | null = null;
  const docs = text.match(/\s*\(docs:\s*(\/[^)\s]*)\)\s*(?=(?:#[a-z0-9-]+\s*)*$)/i);
  if (docs) {
    docsUrl = docs[1];
    text = text.replace(docs[0], ' ').trim();
  }
  const tagMatch = text.match(/(?:^|\s)((?:#[a-z0-9-]+\s*)+)$/i);
  const tags: string[] = [];
  if (tagMatch) {
    for (const t of tagMatch[1].trim().split(/\s+/)) tags.push(t.slice(1).toLowerCase());
    text = text.slice(0, text.length - tagMatch[0].length).trim();
  }
  // Tags may also precede the docs link; try the docs link again once tags are gone.
  if (!docsUrl) {
    const d2 = text.match(/\s*\(docs:\s*(\/[^)\s]*)\)\s*$/i);
    if (d2) {
      docsUrl = d2[1];
      text = text.replace(d2[0], '').trim();
    }
  }
  let title: string | null = null;
  const t = text.match(/^\*\*(.+?)\*\*:\s*(.*)$/s);
  if (t) {
    title = t[1].trim();
    text = t[2].trim();
  }
  const base = createHash('sha1').update(`${version}|${category}|${title ?? text.slice(0, 80)}`).digest('hex').slice(0, 12);
  const n = seen.get(base) ?? 0;
  seen.set(base, n + 1);
  return { category, title, bodyMd: text, docsUrl, tags, position, slug: n === 0 ? base : `${base}-${n + 1}` };
}

export function parseChangelog(source: string): ParsedRelease[] {
  const releases: ParsedRelease[] = [];
  const versions = new Set<string>();
  let current: ParsedRelease | null = null;
  let category: Category | null = null;
  let buffer: string | null = null;
  let bufferLine = 0;
  let summary: string[] = [];
  let seen = new Map<string, number>();

  const flushEntry = () => {
    if (buffer !== null && current && category) {
      current.entries.push(finishEntry(buffer, category, current.version, current.entries.length, seen));
    }
    buffer = null;
  };
  const flushSummary = () => {
    if (current && summary.length) current.summary = summary.join(' ').trim() || null;
    summary = [];
  };

  const lines = source.split('\n');
  lines.forEach((line, i) => {
    const no = i + 1;
    if (line.startsWith('## ')) {
      flushEntry();
      flushSummary();
      const m = line.match(/^## \[([^\]]+)\](?:\s+-\s+(\d{4}-\d{2}-\d{2}|unreleased))?\s*$/i);
      if (!m) return fail(no, 'malformed release header');
      const version = m[1].trim().toLowerCase();
      if (versions.has(version)) return fail(no, `duplicate version ${version}`);
      versions.add(version);
      const date = m[2] && m[2].toLowerCase() !== 'unreleased' ? m[2] : null;
      current = { version, releasedOn: date, summary: null, entries: [] };
      releases.push(current);
      category = null;
      seen = new Map();
      return;
    }
    if (!current) return; // preamble before the first release
    if (line.startsWith('### ')) {
      flushEntry();
      flushSummary();
      const name = line.slice(4).trim();
      const c = CATEGORIES[name.toLowerCase()];
      if (!c) return fail(no, `unknown category "${name}"`);
      category = c;
      return;
    }
    if (line.startsWith('- ')) {
      flushEntry();
      if (!category) return fail(no, 'entry outside a ### category');
      buffer = line.slice(2);
      bufferLine = no;
      return;
    }
    if (buffer !== null) {
      if (line.trim() === '') flushEntry();
      else buffer += ' ' + line.trim();
      return;
    }
    if (!category && line.trim()) summary.push(line.trim());
  });
  flushEntry();
  flushSummary();
  void bufferLine;
  return releases;
}
