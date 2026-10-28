import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { page as en } from './local-ai';
import { page } from './local-ai-es';

const ask = readFileSync(new URL('../../content/docs/docs/launcher/ask-ai.md', import.meta.url), 'utf8');

describe('local AI page (ES)', () => {
  it('has a description of snippet length and mirrors the English structure', () => {
    expect(page.description.length).toBeGreaterThanOrEqual(140);
    expect(page.description.length).toBeLessThanOrEqual(160);
    expect(page.limits.rows).toHaveLength(en.limits.rows.length);
    expect(page.steps.items).toHaveLength(en.steps.items.length);
    expect(page.faq.items).toHaveLength(en.faq.items.length);
  });

  it('says for every provider whether text leaves the Mac, and only local servers keep it', () => {
    const local = page.limits.rows.filter((r) => r[2] === 'No').map((r) => r[0]);
    expect(local).toEqual(['Ollama (por defecto)', 'LM Studio']);
    for (const r of page.limits.rows) expect(r[2]).toMatch(/^(No|Sí, a)/);
    // the same providers, in the same order, as the English table and the docs
    expect(page.limits.rows.map((r) => r[2] === 'No')).toEqual(en.limits.rows.map((r) => r[2] === 'No'));
    for (const s of ['Ollama by default', 'Keychain', 'Test connection', 'Claude Code', 'grey area']) expect(ask).toContain(s);
  });

  it('keeps the ChatGPT sign-in caveat and never claims screen or selection context', () => {
    const text = JSON.stringify(page);
    expect(text).toMatch(/no ha aprobado|zona gris/);
    expect(text).toMatch(/no lee tu pantalla/);
    expect(text).not.toMatch(/lee tu pantalla y|contexto de (la )?pantalla/i);
  });

  it('dates the Raycast line and never compares AI quality or speed', () => {
    expect(JSON.stringify(page)).toContain('comprobado el 28 de octubre de 2026');
    expect(JSON.stringify(page)).not.toMatch(/mejor que Raycast|más rápid|\bRAM\b/);
    expect(JSON.stringify(page)).not.toMatch(/\b(vos|tenés|podés|querés|pulsá|arrastrá|hacé|mirá|elegí)\b/i);
  });
});
