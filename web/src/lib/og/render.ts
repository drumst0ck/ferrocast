import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import satori from 'satori';
import sharp from 'sharp';
import { OG_HEIGHT, OG_WIDTH } from './paths';

/*
 * 1200×630 PNG on the brand template of og-welcome.jpg (black field, red glow at the bottom, app icon, spaced wordmark),
 * with the page title as the headline. satori lays the text out as vector paths (so the build host needs no system fonts)
 * and sharp, already in the lockfile through astro, rasterises it.
 */
const require = createRequire(import.meta.url);
const font = (w: 400 | 700) => readFileSync(require.resolve(`@fontsource/inter/files/inter-latin-${w}-normal.woff`));
const iconUri = `data:image/png;base64,${readFileSync(fileURLToPath(new URL('../../../public/icon.png', import.meta.url))).toString('base64')}`;

const h = (type: string, style: Record<string, unknown>, children?: unknown, extra: Record<string, unknown> = {}) => ({ type, props: { style, children, ...extra } });

function fontSizeFor(text: string): number {
  const n = text.length;
  return n <= 28 ? 84 : n <= 44 ? 72 : n <= 62 ? 62 : 54;
}

export async function renderOgImage(text: string): Promise<Buffer> {
  const tree = h('div', {
    display: 'flex', flexDirection: 'column', justifyContent: 'space-between', width: OG_WIDTH, height: OG_HEIGHT, padding: '64px 72px',
    color: '#fff', fontFamily: 'Inter', backgroundColor: '#050506',
    backgroundImage: 'radial-gradient(circle 400px at 50% 112%, rgba(255,130,118,1) 0%, rgba(232,70,86,0.8) 30%, rgba(120,20,40,0.35) 65%, rgba(5,5,6,0) 100%)',
  }, [
    h('div', { display: 'flex', alignItems: 'center' }, [
      h('img', { width: 64, height: 64, borderRadius: 16 }, undefined, { src: iconUri, width: 64, height: 64 }),
      h('div', { marginLeft: 22, fontSize: 30, fontWeight: 400, letterSpacing: 10 }, 'FERROCAST'),
    ]),
    h('div', { display: 'flex', fontSize: fontSizeFor(text), fontWeight: 700, lineHeight: 1.1, letterSpacing: -1.5, maxWidth: 1040, marginBottom: 40 }, text),
    h('div', { display: 'flex', fontSize: 26, fontWeight: 400, color: 'rgba(255,255,255,0.8)' }, 'ferrocast.app'),
  ]);
  const svg = await satori(tree as never, {
    width: OG_WIDTH,
    height: OG_HEIGHT,
    fonts: [
      { name: 'Inter', data: font(400), weight: 400, style: 'normal' },
      { name: 'Inter', data: font(700), weight: 700, style: 'normal' },
    ],
  });
  return sharp(Buffer.from(svg)).png({ compressionLevel: 9, palette: true, quality: 90, dither: 1 }).toBuffer();
}
