// S10 (owner-decisions-1): launchCommand passes type / context / fallbackText / arguments to the
// launched command (props.launchContext, props.fallbackText, environment.launchType), like Raycast.
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

function start(source, { mode = 'no-view', launchType = 'userInitiated', args = {}, extra = {} } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fc-lc-'));
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
    if (m.type === 'call') send({ type: 'result', id: m.id, value: null });
  });
  h.wait = async (pred) => {
    for (let i = 0; i < 100 && !pred(); i++) await sleep(100);
    await sleep(300);
  };
  h.of = (type) => h.msgs.filter((m) => m.type === type);
  send({
    type: 'launch', entry, extension: 'fx', command: 'cmd', title: 'Cmd', mode, arguments: args, preferences: {},
    environment: { extensionName: 'fx', commandName: 'cmd', supportPath: path.join(dir, 'support'), assetsPath: dir, isDevelopment: false, raycastVersion: '1.0.0', launchType },
    ...extra,
  });
  return h;
}

const HEAD = `const api = require('@raycast/api');\n`;
const exited = (r) => r.of('exit').length > 0;
const hudOf = (r) => JSON.parse(r.of('hud')[0].title);

test('S10: a background launch hands launchContext, fallbackText, arguments and launchType to the command', async () => {
  const r = start(
    HEAD + `module.exports = async function (props) {
      await api.showHUD(JSON.stringify({ p: props, e: api.environment.launchType }));
    };`,
    { launchType: 'background', args: { q: '1' }, extra: { launchContext: { a: [1, 2] }, fallbackText: 'fb' } },
  );
  await r.wait(() => exited(r));
  const { p, e } = hudOf(r);
  assert.equal(e, 'background');
  assert.equal(p.launchType, 'background');
  assert.deepEqual(p.launchContext, { a: [1, 2] });
  assert.equal(p.fallbackText, 'fb');
  assert.deepEqual(p.arguments, { q: '1' }, 'launchContext and fallbackText are not mixed into the arguments');
});

test('S10: a plain launch has launchType userInitiated and no launchContext', async () => {
  const r = start(HEAD + `module.exports = async function (props) {
    await api.showHUD(JSON.stringify({ t: props.launchType, c: 'launchContext' in props, f: 'fallbackText' in props }));
  };`);
  await r.wait(() => exited(r));
  assert.deepEqual(hudOf(r), { t: 'userInitiated', c: false, f: false });
});

test('S10: launchCommand sends type, context, fallbackText and arguments to the host', async () => {
  const r = start(HEAD + `module.exports = async function () {
    await api.launchCommand({ name: 'other', type: api.LaunchType.Background, context: { k: 'v' }, fallbackText: 'f', arguments: { a: '1' } });
    await api.launchCommand({ name: 'plain' });
  };`);
  await r.wait(() => r.msgs.filter((m) => m.type === 'call' && m.method === 'launchCommand').length >= 2);
  const calls = r.msgs.filter((m) => m.type === 'call' && m.method === 'launchCommand').map((m) => m.params);
  assert.equal(calls[0].type, 'background');
  assert.deepEqual(calls[0].context, { k: 'v' });
  assert.equal(calls[0].fallbackText, 'f');
  assert.deepEqual(calls[0].arguments, { a: '1' });
  assert.equal(calls[1].type, 'userInitiated');
  assert.equal(calls[1].context, undefined);
});

test('S10: launchCommand rejects bad input without calling the host', async () => {
  const r = start(HEAD + `module.exports = async function () {
    const bad = [undefined, {}, { name: '' }, { name: 'x', type: 'sometimes' }, { name: 'x', context: 'str' }, { name: 'x', arguments: 3 }, { name: 'x', fallbackText: 4 }];
    const out = [];
    for (const o of bad) { try { await api.launchCommand(o); out.push('ok'); } catch (e) { out.push(e.message); } }
    await api.showHUD(JSON.stringify(out));
  };`);
  await r.wait(() => exited(r));
  assert.equal(r.msgs.filter((m) => m.type === 'call' && m.method === 'launchCommand').length, 0);
  const out = hudOf(r);
  assert.equal(out.length, 7);
  for (const m of out) assert.match(m, /^launchCommand: /);
  assert.match(out[3], /type/);
  assert.match(out[4], /context/);
});

test('S10: an old host message without launchContext / fallbackText still works, and a reserved-looking argument is the command\'s own', async () => {
  const r = start(
    HEAD + `module.exports = async function (props) {
      await api.showHUD(JSON.stringify({ a: props.arguments, c: 'launchContext' in props, f: 'fallbackText' in props }));
    };`,
    { args: { __ferrocastLaunch: { context: { x: 1 }, fallbackText: 'spoof' } } },
  );
  await r.wait(() => exited(r));
  assert.deepEqual(hudOf(r), { a: { __ferrocastLaunch: { context: { x: 1 }, fallbackText: 'spoof' } }, c: false, f: false });
});
