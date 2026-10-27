import { describe, expect, it } from 'vitest';
import { page } from './script-commands';

describe('Raycast Script Commands page copy', () => {
  it('uses the brief URL and a description of snippet length', () => {
    expect(page.path).toBe('/raycast-script-commands/');
    expect(page.description.length).toBeGreaterThanOrEqual(140);
    expect(page.description.length).toBeLessThanOrEqual(160);
  });

  it('lists the three required metadata keys', () => {
    const required = page.limits.rows.filter((r) => r[2] === 'Yes').map((r) => r[0]);
    expect(required).toEqual(['`schemaVersion`', '`title`', '`mode`']);
  });

  it('never claims full Raycast compatibility', () => {
    expect(JSON.stringify(page)).not.toMatch(/100 ?%|fully compatible|full compatibility|every Raycast/i);
  });

  it('keeps the example script valid for the parser rules (schemaVersion 1, title, mode)', () => {
    expect(page.steps.code).toMatch(/@raycast\.schemaVersion 1/);
    expect(page.steps.code).toMatch(/@raycast\.title \S/);
    expect(page.steps.code).toMatch(/@raycast\.mode (fullOutput|compact|silent|inline)/);
  });
});
