import { describe, expect, it } from 'vitest';
import { allPairs } from './convert';
import { pairGuides } from './pair-guides';

describe('pair guides', () => {
  it('only exist for real conversion pairs', () => {
    const slugs = new Set(allPairs().map((p) => p.slug));
    for (const slug of Object.keys(pairGuides)) expect(slugs.has(slug), slug).toBe(true);
  });

  it('quote the engine arguments that convert.rs uses', () => {
    expect(JSON.stringify(pairGuides['mov-to-gif'])).toContain('fps=12,scale=640:-1:flags=lanczos');
    expect(JSON.stringify(pairGuides['heic-to-jpg'])).toContain('sips -s format jpeg');
  });

  it('never promise kept metadata or no quality loss', () => {
    expect(JSON.stringify(pairGuides)).not.toMatch(/keeps? (all )?(the )?metadata|without (losing|quality loss)|lossless/i);
  });
});
