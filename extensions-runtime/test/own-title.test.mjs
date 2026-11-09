// R5-B6: the host translates the titles this runtime writes itself (`ownTitle: true`) and shows an
// extension's own titles as written. Driven over stdio like the app does. Run: `npm test` (after `npm run build`).
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
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fc-rt5-'));
  const entry = path.join(dir, 'cmd.js');
  fs.writeFileSync(entry, source);
  const child = spawn(process.execPath, [runtime], { env: { PATH: process.env.PATH }, stdio: ['pipe', 'pipe', 'pipe'] });
  children.add(child);
  const h = { msgs: [] };
  const send = (m) => child.stdin.write(JSON.stringify(m) + '\n');
  readline.createInterface({ input: child.stdout }).on('line', (line) => {
    const m = JSON.parse(line);
    h.msgs.push(m);
    if (m.type === 'call') send({ type: 'result', id: m.id, value: null });
  });
  h.send = send;
  h.wait = async (pred) => {
    for (let i = 0; i < 100 && !pred(); i++) await sleep(100);
    await sleep(300);
  };
  h.of = (type) => h.msgs.filter((m) => m.type === type);
  send({
    type: 'launch', entry, extension: 'fx', command: 'cmd', title: 'Cmd', mode, arguments: {}, preferences: {},
    environment: { extensionName: 'fx', commandName: 'cmd', supportPath: path.join(dir, 'support'), assetsPath: dir, isDevelopment: false, raycastVersion: '1.0.0', launchType: 'userInitiated' },
  });
  return h;
}

const HEAD = `const api = require('@raycast/api'); const React = require('react'); const h = React.createElement;\n`;

function actions(node, out = []) {
  if (node.t === 'Action' || node.t.startsWith('Action.')) out.push(node);
  for (const c of node.c || []) actions(c, out);
  return out;
}

test('a toast and a HUD written by the extension are not marked as ours', async () => {
  const r = start(HEAD + `module.exports = async function () {
    await api.showToast({ style: api.Toast.Style.Success, title: 'Copied to Clipboard' });
    await api.showHUD('Settings');
  };`, 'no-view');
  await r.wait(() => r.of('exit').length > 0);
  assert.deepEqual(r.of('toast').map((m) => [m.title, !!m.ownTitle]), [['Copied to Clipboard', false]]);
  assert.deepEqual(r.of('hud').map((m) => [m.title, !!m.ownTitle]), [['Settings', false]]);
});

test('the HUD of the built-in copy action is marked as ours', async () => {
  const r = start(HEAD + `module.exports = function () {
    return h(api.List, null, h(api.List.Item, { title: 'A' }, h(api.ActionPanel, null, h(api.Action.CopyToClipboard, { content: 'x' }))));
  };`, 'view');
  await r.wait(() => r.of('render').length > 0);
  r.send({ type: 'action', id: actions(r.of('render')[0].tree)[0].p._id });
  await r.wait(() => r.of('hud').length > 0);
  assert.deepEqual(r.of('hud').map((m) => [m.title, m.ownTitle]), [['Copied to Clipboard', true]]);
});

test('a failure title written by the runtime is ours, one given by the extension is not', async () => {
  const r = start(HEAD + `module.exports = function () {
    return h(api.List, null, h(api.List.Item, { title: 'A' }, h(api.ActionPanel, null,
      h(api.Action, { title: 'Run it', onAction: () => { throw new Error('boom'); } }),
      h(api.Action, { onAction: () => { throw new Error('bang'); } }))));
  };`, 'view');
  await r.wait(() => r.of('render').length > 0);
  const [named, unnamed] = actions(r.of('render')[0].tree);
  r.send({ type: 'action', id: named.p._id });
  r.send({ type: 'action', id: unnamed.p._id });
  await r.wait(() => r.of('toast').length >= 2);
  const failures = r.of('toast').map((m) => [m.title, m.message, !!m.ownTitle]);
  assert.deepEqual(failures, [['Run it', 'boom', false], ['Action failed', 'bang', true]]);
});
