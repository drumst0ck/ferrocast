import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { factsCopy, MIN_MACOS } from './facts';

describe('key facts', () => {
  it('has the same number of facts in English and Spanish', () => {
    expect(factsCopy('es').facts).toHaveLength(factsCopy('en').facts.length);
  });

  it('states the minimum macOS that the app bundle declares', () => {
    const bundle = readFileSync(new URL('../../../../scripts/bundle.sh', import.meta.url), 'utf8');
    expect(bundle).toContain(`<key>LSMinimumSystemVersion</key><string>${MIN_MACOS.version}.0</string>`);
    for (const lang of ['en', 'es'] as const) expect(factsCopy(lang).facts.join(' ')).toContain(`macOS ${MIN_MACOS.version}`);
  });

  it('keeps meta descriptions within 140-160 characters', () => {
    for (const lang of ['en', 'es'] as const) {
      const d = factsCopy(lang).description.length;
      expect(d).toBeGreaterThanOrEqual(140);
      expect(d).toBeLessThanOrEqual(160);
    }
  });
});
