// Tests of the navigation stack protocol (S5) and the shared Cache (S6), driven over stdio like the app does.
// Run: `npm test` (after `npm run build`).
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import readline from 'node:readline';
import { fileURLToPath } from 'node:url';

const runtime = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../dist/runtime.js');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const children = new Set();
test.afterEach(() => {
  for (const c of children) c.kill('SIGKILL');
  children.clear();
});

async function start(source, support = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fc-rt2-'));
  for (const [name, body] of Object.entries(support)) {
    fs.mkdirSync(path.join(dir, 'support'), { recursive: true });
    fs.writeFileSync(path.join(dir, 'support', name), body);
  }
  const entry = path.join(dir, 'cmd.js');
  fs.writeFileSync(entry, source);
  const child = spawn(process.execPath, [runtime], { env: { PATH: process.env.PATH }, stdio: ['pipe', 'pipe', 'pipe'] });
  children.add(child);
  const h = { msgs: [], stderr: [], dir };
  const send = (m) => child.stdin.write(JSON.stringify(m) + '\n');
  readline.createInterface({ input: child.stderr }).on('line', (l) => h.stderr.push(l));
  readline.createInterface({ input: child.stdout }).on('line', (line) => h.msgs.push(JSON.parse(line)));
  h.send = send;
  // wait (up to 15 s) for a condition, then a short grace so extra, unwanted messages would show
  h.until = async (pred) => {
    for (let i = 0; i < 150 && !pred(); i++) await sleep(100);
    await sleep(500);
  };
  h.stop = () => child.kill('SIGKILL');
  h.seq = () => h.msgs.filter((m) => m.type === 'render' || m.type === 'pop').map((m) => `${m.type} ${m.depth}`);
  send({
    type: 'launch', entry, extension: 'fx', command: 'cmd', title: 'Cmd', mode: 'view', arguments: {}, preferences: {},
    environment: { extensionName: 'fx', commandName: 'cmd', supportPath: path.join(dir, 'support'), assetsPath: dir, isDevelopment: false, raycastVersion: '1.0.0', launchType: 'userInitiated' },
  });
  return h;
}

const HEAD = `const api = require('@raycast/api'); const React = require('react'); const h = React.createElement;\n`;

test('S5: a lower screen re-rendering while a screen is pushed sends render for its own depth, nothing pops', async () => {
  const r = await start(HEAD + `
    function Child() { return h(api.Detail, { markdown: 'child' }); }
    module.exports = function () {
      const nav = api.useNavigation();
      const [n, setN] = React.useState(0);
      React.useEffect(() => {
        const a = setTimeout(() => nav.push(h(Child)), 100);
        const b = setTimeout(() => setN(1), 400);
        return () => { clearTimeout(a); clearTimeout(b); };
      }, []);
      return h(api.List, { navigationTitle: 'n' + n });
    };`);
  await r.until(() => r.seq().length >= 3);
  r.stop();
  assert.deepEqual(r.stderr.filter((l) => /Error/.test(l)), []);
  assert.deepEqual(r.seq(), ['render 1', 'render 2', 'render 1']);
});

test('S5: pop() sends an explicit pop with the remaining depth, then re-renders the screen below', async () => {
  const r = await start(HEAD + `
    function Child() {
      const nav = api.useNavigation();
      React.useEffect(() => { const t = setTimeout(() => nav.pop(), 300); return () => clearTimeout(t); }, []);
      return h(api.Detail, { markdown: 'child' });
    }
    module.exports = function () {
      const nav = api.useNavigation();
      React.useEffect(() => { const a = setTimeout(() => nav.push(h(Child)), 100); return () => clearTimeout(a); }, []);
      return h(api.List, {});
    };`);
  await r.until(() => r.seq().length >= 4);
  r.stop();
  assert.deepEqual(r.seq(), ['render 1', 'render 2', 'pop 1', 'render 1']);
});

const CACHE = (body) => HEAD + `
  const fs = require('fs'); const path = require('path');
  module.exports = function () {
    ${body}
  };`;

test('S6: two Cache instances of one namespace keep each other\'s keys and notify each other', async () => {
  const r = await start(CACHE(`
    const a = new api.Cache(); const b = new api.Cache();
    b.get('x');
    let fired = 0; b.subscribe(() => { fired++; });
    a.set('one', '1'); b.set('two', '2');
    const file = JSON.parse(fs.readFileSync(path.join(api.environment.supportPath, 'cache-default.json'), 'utf8'));
    return h(api.Detail, { markdown: JSON.stringify({ file, fired, a2: a.get('two'), b1: b.get('one') }) });`));
  await r.until(() => r.msgs.some((m) => m.type === 'render'));
  r.stop();
  const md = JSON.parse(r.msgs.find((m) => m.type === 'render').tree.c[0].p.markdown);
  assert.deepEqual(md.file, { one: '1', two: '2' });
  assert.equal(md.fired, 2);
  assert.equal(md.a2, '2');
  assert.equal(md.b1, '1');
});

test('S6: caches of different namespaces stay apart, and a save leaves no temp file behind', async () => {
  const r = await start(CACHE(`
    const a = new api.Cache({ namespace: 'a' }); const b = new api.Cache({ namespace: 'b' });
    let fired = 0; b.subscribe(() => { fired++; });
    a.set('k', 'A');
    const files = fs.readdirSync(api.environment.supportPath).sort();
    return h(api.Detail, { markdown: JSON.stringify({ fired, has: b.has('k'), files }) });`));
  await r.until(() => r.msgs.some((m) => m.type === 'render'));
  r.stop();
  const md = JSON.parse(r.msgs.find((m) => m.type === 'render').tree.c[0].p.markdown);
  assert.equal(md.fired, 0);
  assert.equal(md.has, false);
  assert.deepEqual(md.files, ['cache-a.json']);
});

test('S6: keys already on disk from an earlier run are still read, and a new key is added beside them', async () => {
  const r = await start(CACHE(`
    const c = new api.Cache();
    const before = c.get('old');
    c.set('new', 'n');
    const file = JSON.parse(fs.readFileSync(path.join(api.environment.supportPath, 'cache-default.json'), 'utf8'));
    return h(api.Detail, { markdown: JSON.stringify({ before, file }) });`), { 'cache-default.json': '{"old":"o"}' });
  await r.until(() => r.msgs.some((m) => m.type === 'render'));
  r.stop();
  const md = JSON.parse(r.msgs.find((m) => m.type === 'render').tree.c[0].p.markdown);
  assert.equal(md.before, 'o');
  assert.deepEqual(md.file, { old: 'o', new: 'n' });
});

async function runCache(body, support) {
  const r = await start(CACHE(body), support);
  await r.until(() => r.msgs.some((m) => m.type === 'render'));
  r.stop();
  return JSON.parse(r.msgs.find((m) => m.type === 'render').tree.c[0].p.markdown);
}

test('R5 B2: keys named like Object.prototype members are not reported as cached', async () => {
  const md = await runCache(`
    const c = new api.Cache();
    const before = { has: c.has('constructor'), get: c.get('toString'), proto: c.has('__proto__') };
    c.set('__proto__', 'p');
    return h(api.Detail, { markdown: JSON.stringify({ before, after: c.get('__proto__'), empty: c.isEmpty }) });`);
  assert.deepEqual(md.before, { has: false, proto: false });
  assert.equal(md.after, 'p');
  assert.equal(md.empty, false);
});

test('R5 B3: a cache file that is valid JSON but not an object is treated as empty', async () => {
  for (const body of ['[1,2]', 'null', '5', '"x"']) {
    const md = await runCache(`
      const c = new api.Cache();
      const empty = c.isEmpty; c.set('k', 'v');
      const file = JSON.parse(fs.readFileSync(path.join(api.environment.supportPath, 'cache-default.json'), 'utf8'));
      return h(api.Detail, { markdown: JSON.stringify({ empty, got: c.get('k'), file }) });`, { 'cache-default.json': body });
    assert.equal(md.empty, true, body);
    assert.equal(md.got, 'v', body);
    assert.deepEqual(md.file, { k: 'v' }, body);
  }
});

test('R5 B4: Cache.remove returns whether the key existed and only notifies then', async () => {
  const md = await runCache(`
    const c = new api.Cache(); let fired = 0; c.subscribe(() => { fired++; });
    c.set('a', '1'); fired = 0;
    const missing = c.remove('nope'); const missingFired = fired;
    const present = c.remove('a');
    return h(api.Detail, { markdown: JSON.stringify({ missing, missingFired, present, fired }) });`);
  assert.deepEqual(md, { missing: false, missingFired: 0, present: true, fired: 1 });
});

test('R5 B5: namespaces that differ only in characters the file name replaces do not share a file', async () => {
  const md = await runCache(`
    const a = new api.Cache({ namespace: 'a/b' }); const b = new api.Cache({ namespace: 'a_b' });
    a.set('k', 'slash'); b.set('k', 'underscore');
    return h(api.Detail, { markdown: JSON.stringify({ a: a.get('k'), b: b.get('k'), files: fs.readdirSync(api.environment.supportPath).length }) });`);
  assert.deepEqual(md, { a: 'slash', b: 'underscore', files: 2 });
});

test('R5 B5: a plain namespace keeps its existing file name', async () => {
  const md = await runCache(`
    const c = new api.Cache({ namespace: 'my-ns.v1' });
    return h(api.Detail, { markdown: JSON.stringify({ v: c.get('x') }) });`, { 'cache-my-ns.v1.json': '{"x":"y"}' });
  assert.equal(md.v, 'y');
});

test('R5 I7: a throwing subscriber neither breaks set() nor starves the other subscribers', async () => {
  const md = await runCache(`
    const c = new api.Cache(); let ok = 0;
    c.subscribe(() => { throw new Error('boom'); }); c.subscribe(() => { ok++; });
    let threw = false;
    try { c.set('a', '1'); } catch { threw = true; }
    return h(api.Detail, { markdown: JSON.stringify({ threw, ok, got: c.get('a') }) });`);
  assert.deepEqual(md, { threw: false, ok: 1, got: '1' });
});
