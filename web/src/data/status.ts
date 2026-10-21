/*
 * Hand-written status vocabulary for the website.
 *
 * KEEP IN SYNC: this file mirrors the status record in docs/STATUS.es.md (moved out of the README) and the odd/ task log.
 * Refresh it by hand whenever docs/STATUS.es.md changes. conversions.json and tools.json are
 * generated from the Rust code (cargo run -p ferrocast-core --example site_data); this file is not.
 */
import conversions from './conversions.json';
import tools from './tools.json';

export type Level = 'tested' | 'typed' | 'planned';

export const STATUS_DATE = '2026-10-21';

export const levels: Record<Level, { label: string; short: string; text: string }> = {
  tested: { label: 'Tested', short: 'Tested', text: 'Tested' },
  typed: { label: 'Typed', short: 'Typed', text: 'Typed command only' },
  planned: { label: 'Planned', short: 'Planned', text: 'Not included yet' },
};

/** tools.json status -> website level. Window and one-step preset are both "tested". */
export function toolLevel(status: string): Level {
  if (status === 'window' || status === 'preset') return 'tested';
  if (status === 'typed') return 'typed';
  if (status === 'tested') return 'tested';
  return 'planned';
}

export const statusLine = `Status as of ${STATUS_DATE}: every conversion pair has run on real files in tests; the interactive windows, the drag wheel, AirDrop, Terminal and the shelf are tested; Enter on a Finder selection is available as an opt-in setting (Settings, General; off by default, needs Accessibility).`;

/** Counters computed from the generated data. Throws at build time if the data drifts. */
export function counts() {
  if (conversions.conversions !== 275) throw new Error(`expected 275 conversion pairs, got ${conversions.conversions}`);
  if (conversions.inputs !== 46) throw new Error(`expected 46 input formats, got ${conversions.inputs}`);
  const pairs = conversions.families.reduce((n, f) => n + f.inputs.reduce((m, i) => m + i.outputs.length, 0), 0);
  if (pairs !== conversions.conversions) throw new Error(`families hold ${pairs} pairs, header says ${conversions.conversions}`);
  return { inputs: conversions.inputs, pairs: conversions.conversions, tools: tools.tools.length, actions: tools.actions.length };
}
