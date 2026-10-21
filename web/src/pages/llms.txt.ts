import { getCollection } from 'astro:content';
import { familyHubs } from '../lib/seo/convert';
import { allTools } from '../lib/seo/tools';
import { SITE } from '../lib/seo/jsonld';

const urlOf = (id: string) => `${SITE}/${id.replace(/\/?index$/, '')}/`.replace(/\/\/$/, '/');

export async function GET() {
  const docs = (await getCollection('docs')).filter((d) => d.id.startsWith('docs')).sort((a, b) => a.id.localeCompare(b.id));
  const lines = [
    '# Ferrocast',
    '',
    '> Fast, open-source launcher for macOS written in Rust, with local file conversion (275 conversions across 46 input formats) and 26 file tools. Free, MIT licensed, Apple Silicon only. Nothing is uploaded.',
    '',
    '## Key pages',
    `- [Home](${SITE}/): what Ferrocast is and how to install it with Homebrew`,
    `- [Convert and file tools](${SITE}/files/): interactive explorer of every conversion`,
    `- [Changelog](${SITE}/changelog/): release notes`,
    '',
    '## Conversions by family',
    ...familyHubs().map((f) => `- [${f.name}](${SITE}/convert/${f.id}/)`),
    '',
    '## File tools',
    ...allTools().map((t) => `- [${t.name}](${SITE}/tools/${t.slug}/)`),
    '',
    '## Documentation',
    ...docs.map((d) => `- [${d.data.title}](${urlOf(d.id)}): ${d.data.description}`),
    '',
  ];
  return new Response(lines.join('\n'), { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
}
