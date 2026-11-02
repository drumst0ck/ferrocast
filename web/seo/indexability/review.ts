import fs from 'node:fs';
import { allPairs, familyHubs, pairFacts } from '../../src/lib/seo/convert';
import { pairGuides } from '../../src/lib/seo/pair-guides';
import { esPairs } from '../../src/lib/seo/convert-es';
const root = new URL('./', import.meta.url);
const csv = fs.readFileSync(new URL('sources/paginas.csv',root),'utf8').trim().split(/\r?\n/).slice(1);
const evidence = new Map<string,string[]>();
function add(url:string, source:string, clicks:number, impressions:number) {
 if(clicks>0 || impressions>0) evidence.set(url,[...(evidence.get(url)||[]),`${source}: clicks=${clicks}; impressions=${impressions}`]);
}
for(const line of csv){const [url,c,i]=line.split(',');add(url,'paginas.csv (2026-10-28 baseline)',+c,+i);}
for(const name of ['gsc-prev-page.json','gsc-cur-page.json']) for(const r of JSON.parse(fs.readFileSync(new URL(`sources/${name}`,root),'utf8')).rows) add(r.keys[0],name,r.clicks,r.impressions);
const rows = allPairs().map(p=>{
 const url=`https://ferrocast.app/convert/${p.slug}/`;
 const signals=evidence.get(url)||[];
 const guide=pairGuides[p.slug];
 const limits=pairFacts(p).limits;
 const es=esPairs.some(e=>e.en===p.slug);
 // Review the actual rendered branches: authored guide, pair-specific limitations,
 // and curated bilingual pairs are conservative quality exceptions.
 const reason = signals.length ? 'Protected: observed search demand (union, not summed)' : guide ? `Keep: authored guide with ${guide.sections.length} specific sections and ${guide.faq.length} FAQs` : limits.length ? `Keep: useful pair-specific limitation: ${limits.join(' ')}` : es ? 'Keep: curated bilingual pair; preserve reciprocal hreflang group' : `Noindex: generic PairPage only; ${p.inExt} to ${p.outExt}, ${p.needs || 'macOS'}; no authored guide or pair-specific limitations, same four wheel steps and generic FAQs`;
 return {url,decision:signals.length||guide||limits.length||es?'keep':'noindex',signal:signals.join(' | ')||'No observed clicks/impressions in supplied exports',source:'PairPage.astro; convert.ts; pair-guides.ts; convert-es.ts',reason};
});
for(const f of familyHubs())rows.push({url:`https://ferrocast.app/convert/${f.id}/`,decision:'keep',signal:'Hub',source:'FamilyHub.astro',reason:'Navigational family hub; not an individual conversion template'});
fs.writeFileSync(new URL('decisions.csv',root),Object.keys(rows[0]).join(',')+'\n'+rows.map(r=>Object.values(r).map(v=>'"'+v.replaceAll('"','""')+'"').join(',')).join('\n')+'\n');
fs.writeFileSync(new URL('../../src/data/convert-indexability.json',root),JSON.stringify({enabled:true,noindexPaths:rows.filter(r=>r.decision==='noindex').map(r=>new URL(r.url).pathname)},null,2)+'\n');
console.log({total:rows.length,pairs:allPairs().length,protected:rows.filter(r=>r.reason.startsWith('Protected')).length,keep:rows.filter(r=>r.decision==='keep').length,noindex:rows.filter(r=>r.decision==='noindex').length});
