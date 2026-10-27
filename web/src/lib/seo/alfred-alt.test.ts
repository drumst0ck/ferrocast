import { describe, expect, it } from 'vitest';
import { page } from './alfred-alt';

describe('Alfred alternative copy', () => {
  it('uses its URL and a description of snippet length', () => {
    expect(page.path).toBe('/alfred-alternative/');
    expect(page.description.length).toBeGreaterThanOrEqual(140);
    expect(page.description.length).toBeLessThanOrEqual(160);
  });

  it('has a cell for every column and keeps Ferrocast gaps in the table', () => {
    for (const row of page.limits.rows) expect(row).toHaveLength(page.limits.cols.length);
    const ferrocast = Object.fromEntries(page.limits.rows.map((r) => [r[0], r[2]]));
    expect(ferrocast['Settings sync between Macs']).toMatch(/^No sync/);
    expect(ferrocast['Automation']).toMatch(/no visual workflow editor/);
  });

  it('never says Alfred "does not have" something and gives no Alfred speed or memory numbers', () => {
    const alfred = JSON.stringify(page.limits.rows.map((r) => r[1]));
    expect(alfred).not.toMatch(/does not have|doesn't have|lacks/i);
    expect(alfred).not.toMatch(/\d+\s?(ms|MB)\b/);
  });
});
