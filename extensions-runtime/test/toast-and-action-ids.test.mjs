// R2-S13 (toast updates are not one message each) and R2-S14 (action ids go stale with the screen),
// driven over stdio like the app does. Run: `npm test` (after `npm run build`).
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

function start(source, mode) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fc-rt3-'));
  const entry = path.join(dir, 'cmd.js');
  fs.writeFileSync(entry, source);
  const child = spawn(process.execPath, [runtime], { env: { PATH: process.env.PATH }, stdio: ['pipe', 'pipe', 'pipe'] });
  children.add(child);
  const h = { msgs: [], stderr: [], dir };
  const send = (m) => child.stdin.write(JSON.stringify(m) + '\n');
  readline.createInterface({ input: child.stderr }).on('line', (l) => h.stderr.push(l));
  readline.createInterface({ input: child.stdout }).on('line', (line) => h.msgs.push(JSON.parse(line)));
  h.send = send;
  h.wait = async (pred) => {
    for (let i = 0; i < 100 && !pred(); i++) await sleep(100);
    await sleep(400);
  };
  h.of = (type) => h.msgs.filter((m) => m.type === type);
  h.log = () => (fs.existsSync(path.join(dir, 'log')) ? fs.readFileSync(path.join(dir, 'log'), 'utf8') : '');
  send({
    type: 'launch', entry, extension: 'fx', command: 'cmd', title: 'Cmd', mode, arguments: {}, preferences: {},
    environment: { extensionName: 'fx', commandName: 'cmd', supportPath: path.join(dir, 'support'), assetsPath: dir, isDevelopment: false, raycastVersion: '1.0.0', launchType: 'userInitiated' },
  });
  return h;
}

const HEAD = `const api = require('@raycast/api'); const React = require('react'); const h = React.createElement; const fs = require('fs');\n`;

function actions(node, out = []) {
  if (node.t === 'Action' || node.t.startsWith('Action.')) out.push(node);
  for (const c of node.c || []) actions(c, out);
  return out;
}

test('S13: a toast changed after show() is sent as one update, not one message per property', async () => {
  const r = start(HEAD + `module.exports = async function () {
    const t = await api.showToast({ style: api.Toast.Style.Animated, title: 'Loading' });
    t.style = api.Toast.Style.Success;
    t.title = 'Done';
    t.message = 'all fine';
  };`, 'no-view');
  await r.wait(() => r.of('exit').length > 0);
  const toasts = r.of('toast').map((m) => `${m.style}|${m.title}|${m.message}`);
  assert.deepEqual(toasts, ['animated|Loading|', 'success|Done|all fine']);
});

test('S13: changing a toast that was never shown sends nothing', async () => {
  const r = start(HEAD + `module.exports = async function () {
    const t = new api.Toast({ title: 'x' });
    t.title = 'y';
    t.style = api.Toast.Style.Failure;
  };`, 'no-view');
  await r.wait(() => r.of('exit').length > 0);
  assert.equal(r.of('toast').length, 0);
});

const LIST = HEAD + `module.exports = function () {
  const [items, setItems] = React.useState(['A', 'B']);
  React.useEffect(() => { const t = setTimeout(() => setItems(['B']), 300); return () => clearTimeout(t); }, []);
  return h(api.List, null, items.map((n) => h(api.List.Item, { key: n, title: n },
    h(api.ActionPanel, null, h(api.Action, { title: 'Delete ' + n, onAction: () => fs.appendFileSync(__dirname + '/log', n) })))));
};`;

test('S14: an action id from a screen that has since changed is refused, the current id runs', async () => {
  const r = start(LIST, 'view');
  await r.wait(() => r.of('render').length >= 2);
  const renders = r.of('render');
  assert.equal(renders.length, 2, 'initial render and the one after the refetch');
  const [oldA, oldB] = actions(renders[0].tree).map((n) => n.p._id);
  const [nowB] = actions(renders[1].tree).map((n) => n.p._id);
  assert.notEqual(oldA, nowB, 'position of the old item A is no longer a valid id');
  assert.notEqual(oldB, nowB);
  r.send({ type: 'action', id: oldA });
  await sleep(1200);
  assert.equal(r.log(), '', 'the stale id ran nothing (before: B Delete)');
  assert.ok(r.stderr.some((l) => l.includes('stale action')), 'the refusal is logged');
  r.send({ type: 'action', id: nowB });
  await r.wait(() => r.log() !== '');
  assert.equal(r.log(), 'B');
});

test('S14: a re-render with the same content keeps the ids valid (no spurious render either)', async () => {
  const r = start(HEAD + `module.exports = function () {
    const [n, setN] = React.useState(0);
    React.useEffect(() => { const t = setTimeout(() => setN(1), 300); return () => clearTimeout(t); }, []);
    return h(api.List, null, h(api.List.Item, { title: 'A' }, h(api.ActionPanel, null, h(api.Action, { title: 'Go', onAction: () => fs.appendFileSync(__dirname + '/log', 'go') }))));
  };`, 'view');
  await sleep(1200);
  assert.equal(r.of('render').length, 1, 'identical content is not sent again');
  r.send({ type: 'action', id: actions(r.of('render')[0].tree)[0].p._id });
  await r.wait(() => r.log() !== '');
  assert.equal(r.log(), 'go');
});
