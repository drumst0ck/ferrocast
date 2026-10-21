/**
 * Generates the docs reference pages that must not drift from the app data:
 *   - src/content/docs/docs/files/conversions.md and its Spanish twin (from src/data/conversions.json)
 *   - src/content/docs/docs/files/tools/_generated/<toolId>.json, one sidecar per file tool (from src/data/tools.json),
 *     read by the <ToolFacts id="..." /> component.
 *
 * Deterministic: no dates, sorted iteration, trailing newline. The output is committed, so the Docker build does not
 * need to run it. CI checks `npm run gen:docs && git diff --exit-code -- src/content/docs`.
 *
 * Run: npm run gen:docs   (from web/)
 */
import { mkdirSync, readdirSync, unlinkSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import conversions from '../src/data/conversions.json' with { type: 'json' };
import tools from '../src/data/tools.json' with { type: 'json' };
import { counts } from '../src/data/status.ts';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const docs = join(root, 'src/content/docs');
const n = counts(); // throws if the data drifts from 275 pairs / 46 inputs

/** tools.json id -> docs page slug (files/tools/<slug>). */
export const toolSlugs: Record<string, string> = {
  compress: 'compress',
  removeMetadata: 'edit-metadata',
  editImage: 'edit-photos',
  annotateImage: 'annotate',
  frameImage: 'add-background',
  cropImage: 'crop-image',
  redactImage: 'redact-image',
  createCollage: 'collage',
  trimVideo: 'trim-video',
  cropVideo: 'crop-video',
  changeVideoSpeed: 'video-speed',
  joinVideos: 'join-videos',
  videoSnapshots: 'video-snapshots',
  splitVideo: 'split-video',
  redactVideo: 'redact-video',
  muteVideo: 'mute-video',
  normalizeAudio: 'normalize-audio',
  audioToVideo: 'audio-to-video',
  trimAudio: 'trim-audio',
  audioChannels: 'audio-channels',
  redactAudio: 'bleep-audio',
  mergePdf: 'merge-pdf',
  organizePdf: 'organize-pdf',
  splitPdf: 'split-pdf',
  createPdf: 'create-pdf',
  extractArchive: 'extract-archive',
};

function write(path: string, text: string) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text.endsWith('\n') ? text : `${text}\n`);
}

// ---- per-tool sidecars -------------------------------------------------------------------------------------------
const sidecarDir = join(docs, 'docs/files/tools/_generated');
mkdirSync(sidecarDir, { recursive: true });
for (const f of readdirSync(sidecarDir)) if (f.endsWith('.json')) unlinkSync(join(sidecarDir, f));
for (const tool of [...tools.tools].sort((a, b) => a.id.localeCompare(b.id))) {
  const slug = toolSlugs[tool.id];
  if (!slug) throw new Error(`gen-docs: no docs slug for tool ${tool.id}; add it to toolSlugs`);
  // Owner decision: everything is tested, so the docs sidecars always show "Tested" (web/src/data/status.ts stays unchanged).
  const docsToolLevel = 'tested';
  write(
    join(sidecarDir, `${tool.id}.json`),
    JSON.stringify({ id: tool.id, slug, name: tool.name, status: tool.status, level: docsToolLevel, shortcut: tool.shortcut, formats: tool.formats }, null, 2),
  );
}
if (Object.keys(toolSlugs).length !== tools.tools.length) throw new Error('gen-docs: toolSlugs and tools.json disagree');

// ---- conversions reference ---------------------------------------------------------------------------------------
type Output = { ext: string; label: string; needs: string | null };
const families = conversions.families as { id: string; name: string; inputs: { ext: string; label: string; outputs: Output[] }[] }[];

const needCounts = new Map<string, number>();
for (const f of families) for (const i of f.inputs) for (const o of i.outputs) if (o.needs) needCounts.set(o.needs, (needCounts.get(o.needs) ?? 0) + 1);
const needsList = [...needCounts.entries()].sort(([a], [b]) => a.localeCompare(b));

const copy = {
  en: {
    title: 'Conversion reference',
    description: `Every file conversion Ferrocast supports: ${n.pairs} pairs across ${n.inputs} input formats in nine families, and which pairs need an optional tool.`,
    intro: (p: number, i: number) => `Ferrocast converts between ${p} format pairs across ${i} input formats in nine families. All conversions run locally; macOS tools come first, and ffmpeg, LibreOffice and cwebp are optional and used only when installed.`,
    how: 'Drag a file and tap <kbd>⇧</kbd>, press <kbd>⌘</kbd> <kbd>T</kbd> on a file result, run Convert Files, or press <kbd>⌥</kbd> <kbd>↵</kbd> on a file result. Pick the target format; the result is written next to the original and never overwrites it.',
    key: 'In the tables below, a target followed by a tool name needs that optional tool installed; every other pair uses tools that ship with macOS.',
    from: 'From',
    to: 'To',
    needsHeading: 'Pairs that need an optional tool',
    needsLine: (tool: string, c: number) => `**${tool}**: ${c} pairs`,
    limitsTitle: 'Limits',
    limits: '`.pages` is written in the iWork \'09 format.',
    statusHeading: 'Status',
    status: '**Tested.** The real-file test runs every pair and reads the result back (text inside documents and sheets, image width, media duration).',
    relatedHeading: 'Related',
    related: ['[Optional tools](/docs/files/optional-tools/)', '[Drag wheel](/docs/files/drag-wheel/)', '[File tools overview](/docs/files/)'],
    sidebarOrder: 3,
  },
  es: {
    title: 'Referencia de conversiones',
    description: `Todas las conversiones de archivos de Ferrocast: ${n.pairs} pares en ${n.inputs} formatos de entrada y nueve familias, y qué pares necesitan una herramienta opcional.`,
    intro: (p: number, i: number) => `Ferrocast convierte entre ${p} pares de formatos en ${i} formatos de entrada y nueve familias. Todo se ejecuta en local; primero se usan las herramientas de macOS, y ffmpeg, LibreOffice y cwebp son opcionales y solo se usan si están instaladas.`,
    how: 'Arrastra un archivo y pulsa <kbd>⇧</kbd>, pulsa <kbd>⌘</kbd> <kbd>T</kbd> sobre un resultado de archivo, ejecuta Convert Files o pulsa <kbd>⌥</kbd> <kbd>↵</kbd> sobre un resultado de archivo. Elige el formato de destino; el resultado se escribe junto al original y nunca lo sobrescribe.',
    key: 'En las tablas, un destino seguido del nombre de una herramienta necesita esa herramienta opcional instalada; el resto de pares usa herramientas que incluye macOS.',
    from: 'Desde',
    to: 'A',
    needsHeading: 'Pares que necesitan una herramienta opcional',
    needsLine: (tool: string, c: number) => `**${tool}**: ${c} pares`,
    limitsTitle: 'Límites',
    limits: '`.pages` se escribe en formato iWork \'09.',
    statusHeading: 'Estado',
    status: '**Tested** (probado). Los 275 pares se ejecutan sobre archivos reales generados y se lee el resultado (texto dentro del documento, ancho de imagen, duración de audio o vídeo).',
    relatedHeading: 'Relacionado',
    related: ['[Herramientas opcionales](/es/docs/files/optional-tools/)', '[Rueda al arrastrar](/es/docs/files/drag-wheel/)', '[Resumen de herramientas de archivos](/es/docs/files/)'],
    sidebarOrder: 3,
  },
} as const;

function conversionsPage(lang: 'en' | 'es'): string {
  const c = copy[lang];
  if (c.description.length < 120 || c.description.length > 155) throw new Error(`gen-docs: ${lang} description is ${c.description.length} characters`);
  const lines: string[] = [];
  lines.push('---', `title: "${c.title}"`, `description: "${c.description}"`, 'status: tested', 'sidebar:', `  order: ${c.sidebarOrder}`, '---');
  lines.push('<!-- Generated by web/scripts/gen-docs.ts from src/data/conversions.json. Do not edit by hand; run npm run gen:docs. -->', '');
  lines.push(c.intro(n.pairs, n.inputs), '', c.how, '', c.key, '');
  for (const f of families) {
    const pairs = f.inputs.reduce((m, i) => m + i.outputs.length, 0);
    lines.push(`## ${f.name}`, '', `${f.inputs.length} ${lang === 'en' ? 'inputs' : 'entradas'}, ${pairs} ${lang === 'en' ? 'pairs' : 'pares'}.`, '');
    lines.push(`| ${c.from} | ${c.to} |`, '|---|---|');
    for (const i of f.inputs) {
      const outs = i.outputs.map((o) => (o.needs ? `\`${o.ext}\` *(${o.needs})*` : `\`${o.ext}\``)).join(', ');
      lines.push(`| ${i.label} (\`${i.ext}\`) | ${outs} |`);
    }
    lines.push('');
  }
  lines.push(`## ${c.needsHeading}`, '');
  for (const [tool, count] of needsList) lines.push(`- ${c.needsLine(tool, count)}`);
  lines.push('', `:::caution[${c.limitsTitle}]`, c.limits, ':::', '', `## ${c.statusHeading}`, '', c.status, '', `## ${c.relatedHeading}`, '');
  for (const r of c.related) lines.push(`- ${r}`);
  return lines.join('\n');
}

write(join(docs, 'docs/files/conversions.md'), conversionsPage('en'));
write(join(docs, 'es/docs/files/conversions.md'), conversionsPage('es'));
console.log(`gen-docs: wrote conversions (en, es) and ${tools.tools.length} tool sidecars`);
