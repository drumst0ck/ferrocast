import manifest from './src/data/convert-indexability.json' with { type: 'json' };
const excluded = new Set(manifest.noindexPaths);
export function isConversionNoindex(url) {
  return manifest.enabled && excluded.has(new URL(url, 'https://ferrocast.app').pathname);
}
