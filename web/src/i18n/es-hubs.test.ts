import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { allTools } from '../lib/seo/tools';
import { galleryEs } from './gallery-es';
import { filesEs, filesEsActions, filesEsFaq, filesEsMeta } from './files-es';
import { toolCardAboutEs, toolCardGroupEs } from './tool-cards-es';
import { toolDescriptionsEs, toolGroupLabelsEs, toolsEsMeta } from './tools-es';

const VOSEO = /\b(vos|tenés|tenés|podés|querés|hacé|andá|sos|decí|fijate)\b/i;
const UNRELEASED = /iCloud|Clear History|Pin\/Delete/i;

function spanishText(value: unknown): string[] {
  if (typeof value === 'string') return [value];
  if (Array.isArray(value)) return value.flatMap(spanishText);
  if (value && typeof value === 'object') return Object.values(value).flatMap(spanishText);
  return [];
}

describe('Spanish files and tools hubs', () => {
  it('keeps meta lengths inside the same limits as the English hubs', () => {
    const files = filesEsMeta();
    expect(files.description.length).toBeGreaterThanOrEqual(140);
    expect(files.description.length).toBeLessThanOrEqual(155);
    expect(files.title.length).toBeLessThanOrEqual(60);
    const tools = toolsEsMeta();
    expect(tools.title.length).toBeLessThanOrEqual(60);
    expect(tools.description.length).toBeGreaterThanOrEqual(120);
    expect(tools.description.length).toBeLessThanOrEqual(155);
  });

  it('translates every tool description and every gallery shot', () => {
    for (const t of allTools()) {
      expect(toolDescriptionsEs[t.docSlug], t.docSlug).toBeTruthy();
    }
    expect(Object.keys(toolGroupLabelsEs).sort()).toEqual(['any', 'archive', 'audio', 'image', 'pdf', 'video']);
    expect(Object.keys(toolCardAboutEs).sort()).toEqual([
      'annotateImage', 'audioChannels', 'audioToVideo', 'changeVideoSpeed', 'compress', 'createCollage', 'createPdf',
      'cropImage', 'cropVideo', 'editImage', 'extractArchive', 'frameImage', 'joinVideos', 'mergePdf', 'muteVideo',
      'normalizeAudio', 'organizePdf', 'redactAudio', 'redactImage', 'redactVideo', 'removeMetadata', 'splitPdf',
      'splitVideo', 'trimAudio', 'trimVideo', 'videoSnapshots',
    ]);
    expect(Object.keys(toolCardGroupEs).sort()).toEqual(['archive', 'audio', 'image', 'pdf', 'video']);
    const galleryIds = [
      'achan', 'annotate', 'anorm', 'atrim', 'avis', 'avis-light', 'bleep', 'collage', 'compress-image', 'compress-video',
      'crop', 'edit', 'frame', 'meta', 'organize', 'redact', 'reorder', 'vcrop', 'vredact', 'vsnapshot', 'vspeed',
      'vsplit', 'vtrim', 'wheel-actions', 'wheel-convert', 'wheel-convert-light', 'wheel-tools', 'wheel-tools-video',
    ];
    expect(Object.keys(galleryEs).sort()).toEqual(galleryIds);
    for (const id of Object.keys(filesEsActions)) expect(filesEsActions[id][0].length).toBeGreaterThan(0);
    expect(filesEsFaq({ inputs: 46, pairs: 275 })).toHaveLength(5);
    expect(filesEs.h1).toMatch(/Arrastra/);
  });

  it('does not use voseo or describe unreleased clipboard and sync controls', () => {
    const blobs = [
      ...spanishText(filesEs),
      ...spanishText(filesEsActions),
      ...filesEsFaq({ inputs: 46, pairs: 275 }).flat(),
      filesEsMeta().description,
      ...spanishText(toolDescriptionsEs),
      ...spanishText(toolCardAboutEs),
      ...spanishText(toolCardGroupEs),
      ...spanishText(galleryEs),
      toolsEsMeta().description,
    ];
    for (const text of blobs) {
      expect(text).not.toMatch(VOSEO);
      expect(text).not.toMatch(UNRELEASED);
    }
    expect(toolCardAboutEs.redactVideo).not.toMatch(/una zona por/i);
    expect(toolDescriptionsEs['redact-video']).toMatch(/varias zonas/);
  });

  it('wires language switcher, nav, footer and docs to the Spanish hubs', () => {
    const read = (path: string) => readFileSync(path, 'utf8');
    expect(read('src/pages/files.astro')).toContain('switchHref="/es/files/"');
    expect(read('src/pages/files.astro')).toContain('https://ferrocast.app/es/files/');
    expect(read('src/pages/tools/index.astro')).toContain('switchHref="/es/tools/"');
    expect(read('src/pages/tools/index.astro')).toContain('${SITE}/es/tools/');
    expect(read('src/pages/es/files/index.astro')).toContain('switchHref="/files/"');
    expect(read('src/pages/es/tools/index.astro')).toContain('switchHref="/tools/"');
    expect(read('src/components/docs/Header.astro')).toContain("'/es/files/'");
    expect(read('src/components/SiteFooter.astro')).toContain("'/es/files/'");
    expect(read('src/components/seo/SeoNav.astro')).toContain("'/es/files/'");
    expect(read('src/components/home/FilesBlock.astro')).toContain("'/es/files/'");
    const linked = [
      'src/pages/es/convertir/index.astro',
      'src/pages/es/convertir/[slug].astro',
      'src/lib/seo/alt-raycast.ts',
      'src/lib/seo/launcher-compare.ts',
      'src/lib/seo/facts.ts',
    ].map(read).join('\n');
    expect(linked).toContain('/es/files/');
    expect(linked).toContain('/es/tools/');
  });
});
