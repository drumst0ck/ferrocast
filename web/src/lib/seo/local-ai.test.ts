import { describe, expect, it } from 'vitest';
import { page } from './local-ai';

describe('local AI launcher copy', () => {
  it('uses its URL and a description of snippet length', () => {
    expect(page.path).toBe('/local-ai-launcher/');
    expect(page.description.length).toBeGreaterThanOrEqual(140);
    expect(page.description.length).toBeLessThanOrEqual(160);
  });

  it('says for every provider whether text leaves the Mac, and only local servers keep it', () => {
    const local = page.limits.rows.filter((r) => r[2] === 'No').map((r) => r[0]);
    expect(local).toEqual(['Ollama (default)', 'LM Studio']);
    for (const r of page.limits.rows) expect(r[2]).toMatch(/^(No|Yes, to )/);
  });

  it('keeps the ChatGPT sign-in caveat and never claims screen or selection context', () => {
    const text = JSON.stringify(page);
    expect(text).toMatch(/not approved third-party use|grey area/);
    expect(text).toMatch(/does not read your screen/);
  });
});
