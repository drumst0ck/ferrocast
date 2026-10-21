import { describe, expect, it } from 'vitest';
import { changelogIndexMeta, releaseMeta } from './meta';

const entry = (category: string, title: string | null, bodyMd = 'something changed') => ({ category, title, bodyMd });
const release = (version: string, releasedOn: string | null, summary: string | null, entries: ReturnType<typeof entry>[]) => ({ version, releasedOn, summary, entries });

describe('releaseMeta', () => {
  const dated = release('0.1.0', '2026-10-15', 'First public build.', [entry('added', 'Search'), entry('added', 'Calculator'), entry('added', null, 'Clipboard history')]);
  const unreleased = release('unreleased', null, null, [...Array.from({ length: 40 }, (_, i) => entry('added', `Tool ${i}`)), ...Array.from({ length: 8 }, () => entry('fixed', null))]);

  it('keeps title <= 60 and description between 120 and 155 characters', () => {
    for (const r of [dated, unreleased, release('1.2.3', '2027-01-01', null, [entry('fixed', null)])]) {
      const m = releaseMeta(r);
      expect(m.title.length, m.title).toBeLessThanOrEqual(60);
      expect(m.description.length, m.description).toBeGreaterThanOrEqual(120);
      expect(m.description.length, m.description).toBeLessThanOrEqual(155);
    }
  });
  it('names the version, the date and the real counts', () => {
    const m = releaseMeta(dated);
    expect(m.title).toContain('0.1.0');
    expect(m.description).toContain('2026-10-15');
    expect(m.description).toContain('3 changes');
    expect(m.description).toContain('3 added');
    expect(m.description).not.toMatch(/fixed/);
  });
  it('describes an unreleased block as unreleased and never prints undefined', () => {
    const m = releaseMeta(unreleased);
    expect(m.title.toLowerCase()).toContain('unreleased');
    expect(m.description).toContain('48 changes');
    expect(m.description).toContain('8 fixed');
    expect(`${m.title} ${m.description}`).not.toMatch(/undefined|null/);
  });
  it('uses different text for different releases', () => {
    expect(releaseMeta(dated).description).not.toBe(releaseMeta(unreleased).description);
  });
});

describe('changelogIndexMeta', () => {
  it('is long enough to be a real title and description', () => {
    const m = changelogIndexMeta();
    expect(m.title.length).toBeGreaterThanOrEqual(30);
    expect(m.title.length).toBeLessThanOrEqual(60);
    expect(m.description.length).toBeGreaterThanOrEqual(120);
    expect(m.description.length).toBeLessThanOrEqual(155);
  });
});
