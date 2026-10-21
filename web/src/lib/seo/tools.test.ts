import { existsSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { allTools, toolMeta } from './tools';

describe('tool pages data', () => {
  const tools = allTools();
  it('covers all 26 tools with unique slugs', () => {
    expect(tools).toHaveLength(26);
    expect(new Set(tools.map((t) => t.slug)).size).toBe(26);
  });
  it('points every tool at a real docs page and, when set, a real screenshot', () => {
    for (const t of tools) {
      expect(existsSync(`src/content/docs/docs/files/tools/${t.docSlug}.mdx`) || existsSync(`src/content/docs/docs/files/tools/${t.docSlug}.md`), t.id).toBe(true);
      if (t.screenshot) expect(existsSync(`public${t.screenshot}`), t.screenshot).toBe(true);
    }
  });
  it('has unique titles (<=60) and descriptions (120-155)', () => {
    const titles = new Set<string>();
    const descs = new Set<string>();
    for (const t of tools) {
      const m = toolMeta(t);
      expect(m.title.length, m.title).toBeLessThanOrEqual(60);
      expect(m.description.length, m.description).toBeGreaterThanOrEqual(120);
      expect(m.description.length, m.description).toBeLessThanOrEqual(155);
      titles.add(m.title);
      descs.add(m.description);
    }
    expect(titles.size).toBe(26);
    expect(descs.size).toBe(26);
  });
});
