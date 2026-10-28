import { execSync } from 'node:child_process';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { translatedEsDocSlugs } from '../docs/es-translations';

const webRoot = process.cwd();

/** Committed git dates for Docker builds without .git (see scripts/sync-lastmod.mjs). */
let lastmodManifest: Record<string, string> | undefined;

function getManifest(): Record<string, string> {
  if (lastmodManifest !== undefined) return lastmodManifest;
  const path = join(webRoot, 'src/data/lastmod.json');
  if (!existsSync(path)) {
    lastmodManifest = {};
    return lastmodManifest;
  }
  try {
    lastmodManifest = JSON.parse(readFileSync(path, 'utf8')) as Record<string, string>;
  } catch {
    lastmodManifest = {};
  }
  return lastmodManifest;
}

function manifestLastCommit(rel: string): Date | null {
  const iso = getManifest()[rel];
  return iso ? new Date(iso) : null;
}
const BUILD_DATE = new Date(
  process.env.SOURCE_DATE_EPOCH ? Number(process.env.SOURCE_DATE_EPOCH) * 1000 : Date.now(),
);

let gitUsable: boolean | undefined;

function hasGit(): boolean {
  if (gitUsable !== undefined) return gitUsable;
  try {
    execSync('git rev-parse --is-inside-work-tree', { cwd: webRoot, stdio: 'ignore' });
    gitUsable = true;
  } catch {
    gitUsable = false;
  }
  return gitUsable;
}

function gitLastCommit(file: string): Date | null {
  if (!hasGit() || !existsSync(file)) return null;
  try {
    const iso = execSync(`git log -1 --format=%cI -- "${file}"`, {
      cwd: webRoot,
      stdio: ['pipe', 'pipe', 'ignore'],
    })
      .toString()
      .trim();
    return iso ? new Date(iso) : null;
  } catch {
    return null;
  }
}

function fileLastmod(...paths: string[]): Date {
  let best: Date | null = null;
  for (const rel of paths) {
    const abs = join(webRoot, rel);
    if (!existsSync(abs)) continue;
    const d = manifestLastCommit(rel) ?? gitLastCommit(abs) ?? statSync(abs).mtime;
    if (best === null || d > best) best = d;
  }
  return best ?? BUILD_DATE;
}

function docContentPath(segments: string, locale: 'en' | 'es'): string | null {
  const base = locale === 'es' ? 'src/content/docs/es/docs' : 'src/content/docs/docs';
  const candidates = [
    join(base, segments, 'index.mdx'),
    join(base, segments, 'index.md'),
    join(base, `${segments}.mdx`),
    join(base, `${segments}.md`),
  ];
  if (locale === 'es' && segments && !translatedEsDocSlugs.has(segments)) {
    return docContentPath(segments, 'en');
  }
  return candidates.find((p) => existsSync(join(webRoot, p))) ?? null;
}

function pathnameOf(url: string): string {
  try {
    return new URL(url).pathname.replace(/\/$/, '') || '/';
  } catch {
    return url;
  }
}

/** Resolve last modification time for a sitemap URL (never throws). */
export function lastmodForPageUrl(url: string): Date {
  try {
    const path = pathnameOf(url);
    if (path === '/' || path === '') return fileLastmod('src/pages/index.astro', 'src/components/home/HomePage.astro');
    if (path === '/es') return fileLastmod('src/pages/es/index.astro', 'src/components/home/HomePage.astro');
    if (path === '/files') return fileLastmod('src/pages/files.astro');
    if (path === '/es/files') return fileLastmod('src/pages/es/files/index.astro', 'src/i18n/files-es.ts', 'src/i18n/gallery-es.ts', 'src/i18n/tool-cards-es.ts');
    if (path === '/facts' || path === '/es/facts') return fileLastmod('src/lib/seo/facts.ts', 'src/components/seo/FactsPage.astro');
    if (path === '/es/convertir' || path.startsWith('/es/convertir/')) {
      return fileLastmod('src/lib/seo/convert-es.ts', 'src/pages/es/convertir/[slug].astro', 'src/pages/es/convertir/index.astro');
    }
    if (path === '/es/raycast-vs-alfred-vs-spotlight') {
      return fileLastmod('src/lib/seo/launcher-compare.ts', 'src/pages/es/raycast-vs-alfred-vs-spotlight.astro');
    }
    if (path === '/mac-window-management') {
      return fileLastmod('src/lib/seo/windows.ts', 'src/components/seo/GuidePage.astro');
    }
    if (path === '/es/historial-portapapeles-mac') {
      return fileLastmod('src/lib/seo/clipboard-es.ts', 'src/components/seo/GuidePage.astro');
    }
    if (path === '/clipboard-manager-mac') {
      return fileLastmod('src/lib/seo/clipboard-manager.ts', 'src/components/seo/GuidePage.astro');
    }
    if (path === '/text-expander-mac' || path === '/es/expansor-de-texto-mac') {
      return fileLastmod('src/lib/seo/text-expander.ts', 'src/components/seo/GuidePage.astro');
    }
    if (path === '/free-file-converter-mac') {
      return fileLastmod('src/lib/seo/file-converter.ts', 'src/components/seo/GuidePage.astro', 'src/data/conversions.json');
    }
    if (path === '/es/convertidor-de-archivos-gratis-mac') {
      return fileLastmod('src/lib/seo/file-converter-es.ts', 'src/components/seo/FileConverterEsPage.astro');
    }
    if (path === '/local-ai-launcher') {
      return fileLastmod('src/lib/seo/local-ai.ts', 'src/components/seo/GuidePage.astro');
    }
    if (path === '/screen-studio-alternative' || path === '/es/grabar-pantalla-mac-zoom-automatico') {
      return fileLastmod('src/lib/seo/screen-recorder.ts', 'src/components/seo/GuidePage.astro');
    }
    if (path === '/alfred-alternative') {
      return fileLastmod('src/lib/seo/alfred-alt.ts', 'src/components/seo/GuidePage.astro');
    }
    if (path === '/ferrocast-vs-raycast') {
      return fileLastmod('src/lib/seo/vs-raycast.ts', 'src/components/seo/GuidePage.astro');
    }
    if (path === '/raycast-script-commands') {
      return fileLastmod('src/lib/seo/script-commands.ts', 'src/components/seo/GuidePage.astro');
    }
    if (path === '/compress-video-to-size' || path === '/es/comprimir-video-a-un-tamano') {
      return fileLastmod('src/lib/seo/compress-video.ts', 'src/components/seo/GuidePage.astro');
    }
    if (path === '/blur-video-mac' || path === '/es/difuminar-video-mac') {
      return fileLastmod('src/lib/seo/blur-video.ts', 'src/components/seo/BlurVideoPage.astro');
    }
    if (path === '/raycast-alternative' || path === '/es/alternativa-a-raycast') {
      return fileLastmod('src/lib/seo/alt-raycast.ts', 'src/components/seo/AlternativePage.astro');
    }
    if (path === '/tools/organize-pdf') {
      return fileLastmod('src/lib/seo/organize-pdf.ts', 'src/components/seo/OrganizePdfPage.astro', 'src/content/docs/docs/files/tools/organize-pdf.mdx');
    }
    if (path === '/tools') return fileLastmod('src/pages/tools/index.astro', 'src/data/tools.json');
    if (path === '/es/tools') return fileLastmod('src/pages/es/tools/index.astro', 'src/i18n/tools-es.ts', 'src/data/tools.json');
    if (path.startsWith('/tools/')) {
      return fileLastmod('src/pages/tools/[slug].astro', 'src/data/tools.json');
    }
    if (path.startsWith('/convert/')) {
      return fileLastmod('src/pages/convert/[slug].astro', 'src/data/conversions.json');
    }
    if (path === '/changelog') return fileLastmod('src/pages/changelog/index.astro', 'CHANGELOG.md');
    if (path.startsWith('/changelog/')) return fileLastmod('CHANGELOG.md', 'src/pages/changelog/[version].astro');
    if (path.startsWith('/es/docs')) {
      const seg = path.replace(/^\/es\/docs\/?/, '');
      const rel = docContentPath(seg, 'es');
      return rel ? fileLastmod(rel) : BUILD_DATE;
    }
    if (path.startsWith('/docs')) {
      const seg = path.replace(/^\/docs\/?/, '');
      const rel = docContentPath(seg, 'en');
      return rel ? fileLastmod(rel) : BUILD_DATE;
    }
    return BUILD_DATE;
  } catch {
    return BUILD_DATE;
  }
}
