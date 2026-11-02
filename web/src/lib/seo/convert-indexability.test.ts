import {it,expect} from 'vitest';
import {isConversionNoindex} from '../../../convert-indexability.mjs';
import manifest from '../../data/convert-indexability.json';
import {allPairs} from './convert';
import {pairGuides} from './pair-guides';
import {esPairs} from './convert-es';
it('only excludes exact existing English pair paths',()=>{
 const paths=new Set(allPairs().map(p=>`/convert/${p.slug}/`));
 expect(new Set(manifest.noindexPaths).size).toBe(manifest.noindexPaths.length);
 for(const path of manifest.noindexPaths)expect(paths.has(path)).toBe(true);
});
it('preserves authored guides, bilingual groups and other routes',()=>{
 for(const slug of [...Object.keys(pairGuides),...esPairs.map(e=>e.en)])expect(isConversionNoindex(`/convert/${slug}/`)).toBe(false);
 for(const path of ['/','/es/','/files/','/tools/','/docs/','/es/convertir/','/convert/images/','/convert/unknown/'])expect(isConversionNoindex(path)).toBe(false);
});
