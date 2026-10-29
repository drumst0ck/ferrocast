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

  it('keeps the rewritten titles, metas and H1s of the five converter pages inside the limits', () => {
    const slugs = ['rtf-to-txt', 'md-to-pages', 'rtf-to-html', 'flac-to-wav', 'flac-to-mp3'];
    for (const slug of slugs) {
      const g = pairGuides[slug];
      expect(g.title, slug).toMatch(/ \| Ferrocast$/);
      expect(g.title!.length, slug).toBeLessThanOrEqual(70);
      expect(g.description!.length, slug).toBeGreaterThanOrEqual(120);
      expect(g.description!.length, slug).toBeLessThanOrEqual(155);
      expect(g.h1, slug).toBeTruthy();
      expect(g.question, slug).toMatch(/\?$/);
    }
  });

  it('only cites macOS routes that Apple documents (TextEdit Make Plain Text, textutil)', () => {
    expect(JSON.stringify(pairGuides['rtf-to-txt'])).toContain('Format → Make Plain Text');
    expect(JSON.stringify(pairGuides['rtf-to-txt'])).toContain('textutil -convert txt file.rtf');
    expect(JSON.stringify(pairGuides['rtf-to-html'])).toContain('textutil -convert html file.rtf');
    expect(JSON.stringify(pairGuides['rtf-to-html'])).not.toMatch(/TextEdit/);
    for (const slug of ['md-to-pages', 'flac-to-wav', 'flac-to-mp3']) expect(JSON.stringify(pairGuides[slug]), slug).not.toMatch(/TextEdit|Preview|Music app|afconvert|textutil/);
  });

  it('says that FLAC to MP3 needs ffmpeg and FLAC to WAV does not', () => {
    expect(pairGuides['flac-to-mp3'].lead).toContain('ffmpeg');
    expect(pairGuides['flac-to-mp3'].description).toContain('ffmpeg');
    expect(pairGuides['flac-to-wav'].lead).not.toContain('ffmpeg');
  });

  it('never promise kept metadata or no quality loss', () => {
    expect(JSON.stringify(pairGuides)).not.toMatch(/keeps? (all )?(the )?metadata|without (losing|quality loss)|lossless/i);
  });
});
