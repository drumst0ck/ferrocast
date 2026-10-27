import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { COMMANDS, page } from './windows';

describe('window management page', () => {
  it('lists exactly the commands windows.rs defines, in order', () => {
    const src = readFileSync(new URL('../../../../crates/macos/src/windows.rs', import.meta.url), 'utf8');
    const positions = COMMANDS.map((c) => src.indexOf(`"${c}"`));
    for (const [i, p] of positions.entries()) expect(p, COMMANDS[i]).toBeGreaterThan(-1);
    expect([...positions].sort((a, b) => a - b)).toEqual(positions);
    expect(COMMANDS).toHaveLength(40);
  });

  it('has a description of snippet length and a cell for every column', () => {
    expect(page.description.length).toBeGreaterThanOrEqual(140);
    expect(page.description.length).toBeLessThanOrEqual(160);
    for (const row of page.limits.rows) expect(row).toHaveLength(page.limits.cols.length);
  });

  it('does not state a licence for Rectangle', () => {
    expect(JSON.stringify(page.limits.rows.map((r) => r[2]))).not.toMatch(/MIT|open source|GPL/i);
  });
});
