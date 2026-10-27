import { describe, expect, it } from 'vitest';
import { page } from './vs-raycast';

describe('Ferrocast vs Raycast copy', () => {
  it('uses the brief URL and a description of snippet length', () => {
    expect(page.path).toBe('/ferrocast-vs-raycast/');
    expect(page.description.length).toBeGreaterThanOrEqual(140);
    expect(page.description.length).toBeLessThanOrEqual(160);
  });

  it('has four cells in every table row and keeps Ferrocast gaps in the table', () => {
    for (const row of page.limits.rows) expect(row).toHaveLength(page.limits.cols.length);
    const ferrocast = Object.fromEntries(page.limits.rows.map((r) => [r[0], r[3]]));
    expect(ferrocast['Cloud Sync']).toBe('No');
    expect(ferrocast['AI with screen or selection context']).toBe('No');
    expect(ferrocast['Extensions']).toMatch(/no store/);
  });

  it('quotes the Raycast prices from the pricing page', () => {
    const price = page.limits.rows.find((r) => r[0] === 'Price')!;
    expect(price[2]).toContain('$10/month or $96/year');
  });

  it('never says Raycast "does not have" something and gives no Raycast speed or memory numbers', () => {
    const raycast = JSON.stringify(page.limits.rows.map((r) => r.slice(1, 3)));
    expect(raycast).not.toMatch(/does not have|doesn't have|lacks/i);
    expect(raycast).not.toMatch(/\d+\s?(ms|MB)\b/);
  });
});
