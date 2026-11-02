import fs from 'node:fs';
import assert from 'node:assert/strict';
import manifest from '../../src/data/convert-indexability.json' with {type:'json'};
import {isConversionNoindex} from '../../convert-indexability.mjs';
const dist = new URL('../../dist/client/',import.meta.url);
const sitemap = fs.readdirSync(dist).filter(n=>/^sitemap-\d+\.xml$/.test(n)).map(n=>fs.readFileSync(new URL(n,dist),'utf8')).join('');
const decisions=fs.readFileSync(new URL('./decisions.csv',import.meta.url),'utf8').trim().split('\n').slice(1);
for(const line of decisions){
 const [,url,decision]=line.match(/^"([^"]+)","([^"]+)"/);
 const path=new URL(url).pathname,html=fs.readFileSync(new URL('.'+path+'index.html',dist),'utf8');
 const excluded=manifest.enabled && decision==='noindex';
 assert.equal(/<meta name="robots" content="noindex,follow"/.test(html),excluded,path);
 assert.equal(sitemap.includes(`<loc>${url}</loc>`),!excluded,path);
 assert.ok(html.includes(`rel="canonical" href="${url}"`),path);
 assert.ok(html.includes('<h1'),path);
}
for(const path of ['/','/es/','/files/','/tools/','/docs/','/es/convertir/'])assert.ok(!/name="robots"[^>]*noindex/.test(fs.readFileSync(new URL('.'+path+'index.html',dist),'utf8')),path);
function walk(dir){for(const ent of fs.readdirSync(dir,{withFileTypes:true})){const p=new URL(ent.name+(ent.isDirectory()?'/':''),dir);if(ent.isDirectory())walk(p);else if(ent.name.endsWith('.html'))for(const tag of fs.readFileSync(p,'utf8').matchAll(/<link[^>]*rel="alternate"[^>]*>/g)){const href=tag[0].match(/href="([^"]+)"/);if(href)assert.ok(!isConversionNoindex(href[1]),`${p}: ${href[1]}`);}}}
walk(dist);
console.log(`Verified ${decisions.length} conversion URLs; robots, sitemap, canonical, content, outside-family samples and all hreflang targets.`);
