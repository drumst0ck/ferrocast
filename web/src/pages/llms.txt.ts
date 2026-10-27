import { familyHubs } from '../lib/seo/convert';
import { SITE } from '../lib/seo/jsonld';
import { counts } from '../data/status';

export async function GET() {
  const { pairs, inputs, tools } = counts();
  const lines = [
    '# Ferrocast',
    '',
    '> Fast, open-source launcher for macOS (Rust, MIT). Search, calculate, clipboard, snippets, Ask AI (Ollama by default), Raycast-compatible Script Commands, ~40 window commands, screen recording with auto zoom, floating Markdown notes, and local file work: {pairs} conversions across {inputs} input formats and {tools} file tools. Apple Silicon only; no account.'.replace(
      '{pairs}',
      String(pairs),
    ).replace('{inputs}', String(inputs)).replace('{tools}', String(tools)),
    '',
    '## Key facts (quotable)',
    `- ${SITE}/facts/`,
    `- Source code and README: https://github.com/drumst0ck/ferrocast`,
    '',
    '## Primary pages',
    `- [Home](${SITE}/): overview, measured footprint, install (DMG and Homebrew)`,
    `- [Key facts and FAQ](${SITE}/facts/): plain facts for citations`,
    `- [Convert and file tools](${SITE}/files/): interactive explorer of every conversion pair`,
    `- [File tools index](${SITE}/tools/): all local file tools`,
    `- [Documentation](${SITE}/docs/): install, launcher features, file tools reference`,
    `- [Changelog](${SITE}/changelog/): release notes`,
    '',
    '## Documentation highlights',
    `- [Install](${SITE}/docs/getting-started/install/)`,
    `- [Conversion reference](${SITE}/docs/files/conversions/)`,
    `- [Optional tools (ffmpeg, LibreOffice, …)](${SITE}/docs/files/optional-tools/)`,
    `- [Migrating from Raycast](${SITE}/docs/getting-started/migrating-from-raycast/)`,
    `- [Data and privacy](${SITE}/docs/reference/data-and-privacy/)`,
    `- [FAQ in docs](${SITE}/docs/reference/faq/)`,
    '',
    '## Conversions by family',
    ...familyHubs().map((f) => `- [${f.name}](${SITE}/convert/${f.id}/)`),
    '',
  ];
  return new Response(lines.join('\n'), { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
}
