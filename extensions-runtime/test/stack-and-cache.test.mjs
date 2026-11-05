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
