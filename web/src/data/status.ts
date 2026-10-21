/*
 * Counters for the website, computed from the generated data.
 * conversions.json and tools.json are generated from the Rust code (cargo run -p ferrocast-core --example site_data); this file is not.
 */
import conversions from './conversions.json';
import tools from './tools.json';

/** Counters computed from the generated data. Throws at build time if the data drifts. */
export function counts() {
  if (conversions.conversions !== 275) throw new Error(`expected 275 conversion pairs, got ${conversions.conversions}`);
  if (conversions.inputs !== 46) throw new Error(`expected 46 input formats, got ${conversions.inputs}`);
  const pairs = conversions.families.reduce((n, f) => n + f.inputs.reduce((m, i) => m + i.outputs.length, 0), 0);
  if (pairs !== conversions.conversions) throw new Error(`families hold ${pairs} pairs, header says ${conversions.conversions}`);
  return { inputs: conversions.inputs, pairs: conversions.conversions, tools: tools.tools.length, actions: tools.actions.length };
}
