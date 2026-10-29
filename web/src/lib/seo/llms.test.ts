import { describe, expect, it } from 'vitest';
import { GET } from '../../pages/llms.txt';
import { allTools } from './tools';

describe('llms.txt', async () => {
  const text = await (await GET()).text();
  const toolLines = text.split('\n').filter((l) => l.includes('/tools/'));
  it('lists at least 20 /tools/ lines including split-pdf', () => {
    expect(toolLines.length).toBeGreaterThanOrEqual(20);
    expect(text).toContain('https://ferrocast.app/tools/split-pdf/');
  });
  it('has a File tools section with every tool page', () => {
    expect(text).toContain('## File tools');
    for (const t of allTools()) expect(text, t.slug).toContain(`(https://ferrocast.app/tools/${t.slug}/)`);
  });
});
