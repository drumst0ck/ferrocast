import { existsSync, readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { plain } from './markup';
import { allTools } from './tools';
import { toolGuides, type ToolGuide } from './tool-guides';

const SLUGS = ['split-pdf', 'create-pdf', 'crop-image', 'crop-video', 'join-videos', 'split-video', 'trim-audio', 'edit-metadata', 'edit-photos', 'add-background'];
const FFMPEG = ['crop-video', 'join-videos', 'split-video', 'trim-audio'];
const guides = SLUGS.map((s) => toolGuides[s]);

const text = (g: ToolGuide) =>
  [g.lead, g.stepsH, ...g.steps.flat(), ...g.sections.flatMap((s) => [s.h, ...(s.paras ?? []), ...(s.list ?? [])]), ...g.faq.flat()].map(plain).join(' ');
const words = (s: string) => s.split(/\s+/).filter(Boolean).length;
const hrefs = (g: ToolGuide) => [...(text(g) + JSON.stringify(g)).matchAll(/\]\((\/[^)\s]+)\)/g)].map((m) => m[1]).concat(g.related.map(([, h]) => h));

describe('tool guide pages', () => {
  it('cover exactly the ten pages of the brief, each with a real tool', () => {
    expect(Object.keys(toolGuides).sort()).toEqual([...SLUGS].sort());
    const tools = new Map(allTools().map((t) => [t.slug, t]));
    for (const g of guides) expect(tools.has(g.slug), g.slug).toBe(true);
  });

  it('give each page its own task-style title, meta and H1', () => {
    const titles = new Set<string>();
    for (const g of guides) {
      expect(g.title, g.slug).toMatch(/ \| Ferrocast$/);
      expect(g.title, g.slug).not.toMatch(/Docs/);
      expect(g.title.length, g.slug).toBeLessThanOrEqual(60);
      expect(g.description.length, g.slug).toBeGreaterThanOrEqual(120);
      expect(g.description.length, g.slug).toBeLessThanOrEqual(155);
      expect(g.h1, g.slug).toMatch(/^How to .* on a Mac without uploading (it|them)$/);
      titles.add(g.title);
    }
    expect(titles.size).toBe(SLUGS.length);
  });

  it('have at least 500 words of their own copy', () => {
    for (const g of guides) expect(words(text(g)), g.slug).toBeGreaterThanOrEqual(500);
  });

  it('have three or four FAQ entries and numbered steps', () => {
    for (const g of guides) {
      expect(g.faq.length, g.slug).toBeGreaterThanOrEqual(3);
      expect(g.faq.length, g.slug).toBeLessThanOrEqual(4);
      expect(g.steps.length, g.slug).toBeGreaterThanOrEqual(5);
    }
  });

  it('say ffmpeg is needed only where the docs do', () => {
    for (const g of guides) {
      const docs = readFileSync(`src/content/docs/docs/files/tools/${g.slug}.mdx`, 'utf8');
      const needs = /Requires ffmpeg|Needs ffmpeg/i.test(docs);
      expect(needs, g.slug).toBe(FFMPEG.includes(g.slug));
      expect(/ffmpeg/i.test(g.description), g.slug).toBe(needs);
      if (!needs) expect(/ffmpeg/i.test(text(g)) && /needs the free ffmpeg|Needs the free ffmpeg/.test(text(g)), g.slug).toBe(false);
    }
  });

  it('link to /free-file-converter-mac/, to the docs reference page and only to pages that exist', () => {
    const slugs = new Set(allTools().map((t) => t.slug));
    for (const g of guides) {
      expect(g.related.map(([, h]) => h), g.slug).toContain('/free-file-converter-mac/');
      for (const h of hrefs(g)) {
        const tool = h.match(/^\/tools\/([^/]+)\/$/);
        if (tool) expect(slugs.has(tool[1]), `${g.slug} -> ${h}`).toBe(true);
        else if (h.startsWith('/docs/')) expect(existsSync(`src/content/docs${h.replace(/\/$/, '')}.md`) || existsSync(`src/content/docs${h.replace(/\/$/, '')}.mdx`), `${g.slug} -> ${h}`).toBe(true);
        else expect(['/free-file-converter-mac/', '/remove-metadata-mac/', '/tools/'], `${g.slug} -> ${h}`).toContain(h);
      }
    }
  });

  it('are linked from the matching docs page, in both directions', () => {
    for (const g of guides) {
      const docs = readFileSync(`src/content/docs/docs/files/tools/${g.slug}.mdx`, 'utf8');
      expect(docs, g.slug).toContain(`Step-by-step guide: [`);
      expect(docs, g.slug).toContain(`](/tools/${g.slug}/)`);
    }
  });

  it('quote native macOS routes only with an Apple source', () => {
    for (const g of guides) {
      const t = JSON.stringify(g);
      if (/Preview can|Finder has|Photos app/.test(t)) expect(t, g.slug).toMatch(/support\.apple\.com/);
    }
    for (const slug of ['join-videos', 'split-video', 'crop-video', 'trim-audio', 'edit-metadata', 'add-background']) expect(JSON.stringify(toolGuides[slug]), slug).not.toMatch(/QuickTime|support\.apple\.com/);
  });
});
