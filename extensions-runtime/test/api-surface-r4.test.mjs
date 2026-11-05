// Round 4 (S15, S21, S23): Clipboard/showToast edge cases, Keyboard shortcut constants and
// LocalStorage.getItem for a missing key, driven over stdio like the app does. Run: `npm test`.
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

// `answers` maps a host-call method to the value the stub host replies with.
function start(source, answers = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fc-rt4-'));
  const entry = path.join(dir, 'cmd.js');
  fs.writeFileSync(entry, source);
  const child = spawn(process.execPath, [runtime], { env: { PATH: process.env.PATH }, stdio: ['pipe', 'pipe', 'pipe'] });
  children.add(child);
  const h = { msgs: [], stderr: [] };
  const send = (m) => child.stdin.write(JSON.stringify(m) + '\n');
  readline.createInterface({ input: child.stderr }).on('line', (l) => h.stderr.push(l));
  readline.createInterface({ input: child.stdout }).on('line', (line) => {
    const m = JSON.parse(line);
    h.msgs.push(m);
    if (m.type === 'call') send({ type: 'result', id: m.id, value: answers[m.method] === undefined ? null : answers[m.method] });
  });
  h.wait = async (pred) => {
    for (let i = 0; i < 100 && !pred(); i++) await sleep(100);
    await sleep(300);
  };
  h.of = (type) => h.msgs.filter((m) => m.type === type);
  send({
    type: 'launch', entry, extension: 'fx', command: 'cmd', title: 'Cmd', mode: 'no-view', arguments: {}, preferences: {},
    environment: { extensionName: 'fx', commandName: 'cmd', supportPath: path.join(dir, 'support'), assetsPath: dir, isDevelopment: false, raycastVersion: '1.0.0', launchType: 'userInitiated' },
  });
  return h;
}

const HEAD = `const api = require('@raycast/api');\n`;
const exited = (r) => r.of('exit').length > 0;

test('S15: Clipboard.copy({html}) copies the html as plain text, not an empty string', async () => {
  const r = start(HEAD + `module.exports = async function () {
    await api.Clipboard.copy({ html: '<p>Hello <b>big</b> &amp; small</p>' });
    await api.Clipboard.copy({ html: '<b>ignored</b>', text: 'plain wins' });
  };`);
  await r.wait(() => exited(r));
  const copies = r.msgs.filter((m) => m.type === 'call' && m.method === 'clipboard.copy').map((m) => m.params.text);
  assert.deepEqual(copies, ['Hello big & small', 'plain wins']);
});

test('S15: showToast(null) does not throw and still shows a toast', async () => {
  const r = start(HEAD + `module.exports = async function () {
    await api.showToast(null);
    await api.showToast(undefined);
    await api.showToast(api.Toast.Style.Failure, 'T', 'M');
  };`);
  await r.wait(() => exited(r));
  assert.deepEqual(r.stderr.filter((l) => /TypeError|Error/.test(l)), []);
  assert.equal(r.of('toast').length, 3);
  assert.equal(r.of('toast')[2].title, 'T');
});

test('S21: Keyboard.Shortcut.Common matches Raycast and has CopyDeeplink and ToggleQuickLook', async () => {
  const r = start(HEAD + `module.exports = async function () {
    const c = api.Keyboard.Shortcut.Common;
    await api.showHUD(JSON.stringify({ p: c.CopyPath, d: c.CopyDeeplink, q: c.ToggleQuickLook }));
  };`);
  await r.wait(() => exited(r));
  const hud = JSON.parse(r.of('hud')[0].title);
  assert.deepEqual(hud.p, { modifiers: ['cmd', 'shift'], key: ',' });
  assert.deepEqual(hud.d, { modifiers: ['cmd', 'shift'], key: 'c' });
  assert.deepEqual(hud.q, { modifiers: ['cmd'], key: 'y' });
});

test('S23: LocalStorage.getItem resolves undefined for a missing key and keeps stored values', async () => {
  const r = start(HEAD + `module.exports = async function () {
    const missing = await api.LocalStorage.getItem('nope');
    await api.showHUD(JSON.stringify({ missing: missing === undefined, type: typeof missing }));
  };`);
  await r.wait(() => exited(r));
  assert.deepEqual(JSON.parse(r.of('hud')[0].title), { missing: true, type: 'undefined' });
});

test('S23: LocalStorage.getItem returns a stored value unchanged', async () => {
  const r = start(HEAD + `module.exports = async function () {
    await api.showHUD(String(await api.LocalStorage.getItem('k')));
  };`, { 'localStorage.getItem': 'v' });
  await r.wait(() => exited(r));
  assert.equal(r.of('hud')[0].title, 'v');
});
