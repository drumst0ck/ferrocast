import { describe, expect, it } from 'vitest';
import { ogSvg } from './render';

describe('og image layout', () => {
  it('fills exactly 1200×630 and keeps the ferrocast.app footer inside the image', async () => {
    const svg = await ogSvg('Free Clipboard Manager for Mac That Skips Passwords');
    // the dark background covers the whole canvas, padding included
    expect(svg).toContain('width="1200" height="630" fill="#050506"');
    // the footer (the only 0.8-white text) starts above the bottom edge
    const footer = svg.match(/<path fill="rgba\(255,255,255,0\.8\)" d="M[\d.]+ ([\d.]+)/);
    expect(footer).not.toBeNull();
    expect(Number(footer![1])).toBeLessThan(630 - 40);
  });
});
