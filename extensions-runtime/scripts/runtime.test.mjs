// Behaviour tests of dist/runtime.js (run `npm run build` first): fixture commands are written to a
// temp dir and driven over stdio like the app does. Run: `npm test`.
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

/** Start a fixture command; `answer(method, params)` returns the value (or throws) for host calls. */
async function start(source, { mode = 'view', answer = () => null, prefs = {}, env = {} } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fc-rt-'));
  const entry = path.join(dir, 'cmd.js');
  fs.writeFileSync(entry, source);
  const child = spawn(process.execPath, [runtime], { env: { PATH: process.env.PATH, ...env }, stdio: ['pipe', 'pipe', 'pipe'] });
  children.add(child);
  const h = { renders: [], toasts: [], calls: [], stderr: [], exited: false, tree: null };
  const send = (m) => child.stdin.write(JSON.stringify(m) + '\n');
  readline.createInterface({ input: child.stderr }).on('line', (l) => h.stderr.push(l));
  readline.createInterface({ input: child.stdout }).on('line', (line) => {
    const m = JSON.parse(line);
    if (m.type === 'render') {
      h.renders.push(m);
      h.tree = m.tree;
    } else if (m.type === 'toast') h.toasts.push(m);
    else if (m.type === 'exit') h.exited = true;
    else if (m.type === 'call') {
      h.calls.push({ method: m.method, params: m.params });
      Promise.resolve()
        .then(() => answer(m.method, m.params))
        .then((value) => send({ type: 'result', id: m.id, value: value ?? null, error: null }))
        .catch((e) => send({ type: 'result', id: m.id, value: null, error: e.message }));
    }
  });
  h.send = send;
  h.stop = () => child.kill('SIGKILL');
  // Wait until something new arrives (up to 10 s, so a slow start under load does not end the
  // wait early), then until output stops changing.
  const count = () => h.renders.length + h.toasts.length + h.calls.length;
  h.settle = async (ms = 400) => {
    const base = count();
    for (let i = 0; i < 100 && count() === base; i++) await sleep(100);
    let n = -1;
    for (let i = 0; i < 40; i++) {
      await sleep(ms);
      const now = count();
      if (now === n) return;
      n = now;
    }
  };
  send({
    type: 'launch', entry, extension: 'fx', command: 'cmd', title: 'Cmd', mode, arguments: {}, preferences: prefs,
    environment: { extensionName: 'fx', commandName: 'cmd', supportPath: path.join(dir, 'support'), assetsPath: dir, isDevelopment: false, raycastVersion: '1.0.0', launchType: 'userInitiated' },
  });
  await h.settle();
  return h;
}
const find = (n, t) => (n.t === t ? n : n.c.map((c) => find(c, t)).find(Boolean));
const all = (n, t, out = []) => (n.t === t && out.push(n), n.c.forEach((c) => all(c, t, out)), out);

const HEAD = `const api = require('@raycast/api'); const React = require('react'); const h = React.createElement;\n`;

test('Cache methods work when detached from the instance (as @raycast/utils uses them)', async () => {
  const r = await start(HEAD + `
    module.exports = function () {
      const cache = new api.Cache({ namespace: 'x' });
      const { get, set, has, remove, subscribe } = cache;
      let seen = 0;
      const un = subscribe(() => { seen++; });
      set('a', '1');
      const out = [get('a'), has('a'), seen, typeof un];
      remove('a');
      return h(api.Detail, { markdown: JSON.stringify(out.concat(has('a'))) });
    };`);
  r.stop();
  assert.deepEqual(r.stderr.filter((l) => /Error/.test(l)), []);
  assert.equal(find(r.tree, 'Detail').p.markdown, JSON.stringify(['1', true, 1, 'function', false]));
});

const SURFACE = [
  'List', 'List.Item', 'List.Item.Detail', 'List.Item.Detail.Metadata', 'List.Item.Detail.Metadata.Label', 'List.Item.Detail.Metadata.Link',
  'List.Item.Detail.Metadata.TagList', 'List.Item.Detail.Metadata.TagList.Item', 'List.Item.Detail.Metadata.Separator', 'List.Section',
  'List.EmptyView', 'List.Dropdown', 'List.Dropdown.Item', 'List.Dropdown.Section',
  'Detail', 'Detail.Metadata', 'Detail.Metadata.Label', 'Detail.Metadata.Link', 'Detail.Metadata.TagList', 'Detail.Metadata.TagList.Item',
  'Detail.Metadata.Separator',
  'Grid', 'Grid.Item', 'Grid.Section', 'Grid.EmptyView', 'Grid.Dropdown', 'Grid.Dropdown.Item', 'Grid.Dropdown.Section', 'Grid.Inset.Small',
  'Grid.Inset.Medium', 'Grid.Inset.Large', 'Grid.Fit.Contain', 'Grid.Fit.Fill', 'Grid.ItemSize.Small', 'Grid.ItemSize.Medium', 'Grid.ItemSize.Large',
  'Form', 'Form.TextField', 'Form.PasswordField', 'Form.TextArea', 'Form.Checkbox', 'Form.DatePicker', 'Form.DatePicker.Type.Date',
  'Form.DatePicker.Type.DateTime', 'Form.DatePicker.isFullDay', 'Form.Dropdown', 'Form.Dropdown.Item', 'Form.Dropdown.Section', 'Form.TagPicker',
  'Form.TagPicker.Item', 'Form.Description', 'Form.Separator', 'Form.FilePicker', 'Form.LinkAccessory',
  'ActionPanel', 'ActionPanel.Section', 'ActionPanel.Submenu', 'Action', 'Action.Style.Regular', 'Action.Style.Destructive',
  'Action.CopyToClipboard', 'Action.OpenInBrowser', 'Action.Open', 'Action.Paste', 'Action.Push', 'Action.ShowInFinder', 'Action.Trash',
  'Action.SubmitForm', 'Action.OpenWith', 'Action.CreateSnippet', 'Action.CreateQuicklink', 'Action.ToggleQuickLook', 'Action.PickDate',
  'Action.PickDate.Type.Date', 'MenuBarExtra', 'MenuBarExtra.Item', 'MenuBarExtra.Section', 'MenuBarExtra.Submenu', 'MenuBarExtra.Separator',
  'Icon', 'Color', 'Color.Red', 'Image', 'Image.Mask.Circle', 'Image.Mask.RoundedRectangle', 'Keyboard.Shortcut.Common.Copy', 'Alert.ActionStyle.Default',
  'Alert.ActionStyle.Destructive', 'Alert.ActionStyle.Cancel', 'Toast', 'Toast.Style.Success', 'Toast.Style.Failure', 'Toast.Style.Animated',
  'Toast.ActionStyle.Primary', 'Toast.ActionStyle.Cancel', 'LaunchType.UserInitiated', 'LaunchType.Background', 'PopToRootType.Default',
  'PopToRootType.Immediate', 'PopToRootType.Suspended', 'Clipboard', 'Clipboard.copy', 'Clipboard.paste', 'Clipboard.read', 'Clipboard.readText',
  'Clipboard.clear', 'LocalStorage.getItem', 'LocalStorage.allItems', 'Cache', 'environment', 'environment.canAccess', 'AI', 'AI.ask', 'AI.Creativity',
  'OAuth', 'OAuth.PKCEClient', 'OAuth.RedirectMethod.Web', 'BrowserExtension', 'BrowserExtension.getTabs', 'WindowManagement', 'WindowManagement.getActiveWindow',
  'getPreferenceValues', 'showToast', 'showHUD', 'closeMainWindow', 'popToRoot', 'clearSearchBar', 'open', 'showInFinder', 'trash', 'getSelectedText',
  'getSelectedFinderItems', 'getFrontmostApplication', 'getApplications', 'getDefaultApplication', 'launchCommand', 'confirmAlert',
  'openExtensionPreferences', 'openCommandPreferences', 'updateCommandMetadata', 'useNavigation', 'captureException',
];

test('the exported @raycast/api surface has every component, enum and function extensions use', async () => {
  const r = await start(HEAD + `
    const SURFACE = ${JSON.stringify(SURFACE)};
    module.exports = function () {
      const missing = SURFACE.filter((p) => p.split('.').reduce((o, k) => (o === undefined || o === null ? undefined : o[k]), api) === undefined);
      return h(api.Detail, { markdown: JSON.stringify(missing) });
    };`);
  r.stop();
  assert.equal(find(r.tree, 'Detail').p.markdown, '[]');
});

test('every exported component renders (no React #130 for undefined element types)', async () => {
  const r = await start(HEAD + `
    const names = ${JSON.stringify(SURFACE.filter((n) => /^(List|Detail|Grid|Form|ActionPanel|Action|MenuBarExtra)(\.[A-Z]\w+)*$/.test(n) && !/Style|Type|Inset|Fit|ItemSize|isFullDay|Common/.test(n)))};
    module.exports = function () {
      const bad = [];
      for (const p of names) {
        const C = p.split('.').reduce((o, k) => o && o[k], api);
        if (typeof C !== 'function' && !(C && C.$$typeof)) bad.push(p);
      }
      return h(api.Detail, { markdown: JSON.stringify(bad) });
    };`);
  r.stop();
  assert.equal(find(r.tree, 'Detail').p.markdown, '[]');
});

test('getApplications, getDefaultApplication and getFrontmostApplication go to the host', async () => {
  const app = { name: 'Safari', path: '/Applications/Safari.app', bundleId: 'com.apple.Safari' };
  const r = await start(
    HEAD + `
    module.exports = function () {
      const [s, set] = React.useState('…');
      React.useEffect(() => {
        (async () => {
          const apps = await api.getApplications();
          const apps2 = await api.getApplications('/tmp/a.txt');
          const def = await api.getDefaultApplication('/tmp/a.txt');
          const front = await api.getFrontmostApplication();
          set(JSON.stringify([apps.length, apps2.length, def.name, front.bundleId]));
        })().catch((e) => set('ERR ' + e.message));
      }, []);
      return h(api.Detail, { markdown: s });
    };`,
    { answer: (m) => (m === 'apps.list' ? [app, app] : app) },
  );
  r.stop();
  assert.equal(find(r.tree, 'Detail').p.markdown, '[2,2,"Safari","com.apple.Safari"]');
  assert.deepEqual(r.calls.map((c) => c.method), ['apps.list', 'apps.list', 'apps.default', 'apps.frontmost']);
  assert.deepEqual(r.calls[1].params, { path: '/tmp/a.txt' });
});

test('getSelectedText asks the host and rejects with its error', async () => {
  const mk = (answer) =>
    start(
      HEAD + `
      module.exports = function () {
        const [s, set] = React.useState('…');
        React.useEffect(() => { api.getSelectedText().then((t) => set('OK ' + t), (e) => set('ERR ' + e.message)); }, []);
        return h(api.Detail, { markdown: s });
      };`,
      { answer },
    );
  const ok = await mk(() => 'hello');
  ok.stop();
  assert.equal(find(ok.tree, 'Detail').p.markdown, 'OK hello');
  assert.equal(ok.calls[0].method, 'selection.text');
  const none = await mk(() => {
    throw new Error('Unable to get selected text');
  });
  none.stop();
  assert.equal(find(none.tree, 'Detail').p.markdown, 'ERR Unable to get selected text');
});

test('launchCommand is a host call', async () => {
  const r = await start(
    HEAD + `
    module.exports = async function () {
      await api.launchCommand({ name: 'open-page', type: api.LaunchType.UserInitiated, arguments: { title: 'x' } });
    };`,
    { mode: 'no-view' },
  );
  r.stop();
  assert.equal(r.calls[0].method, 'launchCommand');
  assert.equal(r.calls[0].params.name, 'open-page');
  assert.equal(r.calls[0].params.arguments.title, 'x');
});

const FORM = HEAD + `
  module.exports = function () {
    const [name, setName] = React.useState('Ann');
    return h(api.Form, { actions: h(api.ActionPanel, null, h(api.Action.SubmitForm, { title: 'Save', onSubmit: (v) => api.showToast({ title: 'submitted', message: JSON.stringify(v) }) })) },
      h(api.Form.Description, { text: 'Hello' }),
      h(api.Form.TextField, { id: 'name', title: 'Name', value: name, onChange: (v) => { setName(v.toUpperCase()); } }),
      h(api.Form.TextArea, { id: 'bio', title: 'Bio', defaultValue: 'b' }),
      h(api.Form.Checkbox, { id: 'ok', label: 'OK', defaultValue: true }),
      h(api.Form.Dropdown, { id: 'color', title: 'Color', defaultValue: 'r' }, h(api.Form.Dropdown.Item, { value: 'r', title: 'Red' }), h(api.Form.Dropdown.Item, { value: 'g', title: 'Green' })),
      h(api.Form.DatePicker, { id: 'when', title: 'When', type: api.Form.DatePicker.Type.Date }),
      h(api.Form.TagPicker, { id: 'tags', title: 'Tags' }, h(api.Form.TagPicker.Item, { value: 'a', title: 'A' })),
      h(api.Form.Separator),
    );
  };`;

test('Form: tree carries fields; field changes call onChange; SubmitForm gets the values', async () => {
  const r = await start(FORM);
  const form = find(r.tree, 'Form');
  assert.ok(form, 'Form node');
  assert.deepEqual(all(form, 'Form.TextField').map((n) => n.p.id), ['name']);
  assert.equal(all(form, 'Form.TextField')[0].p.onChange, true);
  assert.equal(all(form, 'Form.Checkbox')[0].p.defaultValue, true);
  assert.equal(all(form, 'Form.Dropdown.Item').length, 2);
  // typing in the host: the runtime calls onChange, the controlled value comes back changed
  r.send({ type: 'field', id: 'name', value: 'bob' });
  await r.settle();
  assert.equal(all(r.tree, 'Form.TextField')[0].p.value, 'BOB');
  const submit = all(r.tree, 'Action.SubmitForm')[0];
  r.send({ type: 'action', id: submit._id || submit.p._id, values: { name: 'BOB', bio: 'x', ok: false, color: 'g', when: '2024-05-06T00:00:00.000Z', tags: ['a'] } });
  await r.settle();
  r.stop();
  const t = r.toasts.find((x) => x.title === 'submitted');
  assert.ok(t, 'onSubmit ran');
  const v = JSON.parse(t.message);
  assert.deepEqual([v.name, v.bio, v.ok, v.color, v.tags], ['BOB', 'x', false, 'g', ['a']]);
  assert.equal(v.when, '2024-05-06T00:00:00.000Z', 'date values are Dates (JSON gives the ISO string back)');
});

test('a date typed as YYYY-MM-DD is local midnight, like Raycast (not UTC midnight)', async () => {
  const r = await start(HEAD + `
    module.exports = function () {
      const show = (v) => { const d = v.when; api.showToast({ title: 'got', message: d instanceof Date ? [d.getFullYear(), d.getMonth() + 1, d.getDate(), d.getHours(), d.getMinutes()].join(',') : String(d) }); };
      return h(api.Form, { actions: h(api.ActionPanel, null, h(api.Action.SubmitForm, { title: 'Go', onSubmit: show })) },
        h(api.Form.DatePicker, { id: 'when', title: 'When', type: api.Form.DatePicker.Type.Date }));
    };`, { env: { TZ: 'Pacific/Auckland' } });
  const submit = all(r.tree, 'Action.SubmitForm')[0];
  const go = async (when) => { r.send({ type: 'action', id: submit._id || submit.p._id, values: { when } }); await r.settle(); return r.toasts.filter((x) => x.title === 'got').pop().message; };
  assert.equal(await go('2024-05-06'), '2024,5,6,0,0', 'date-only text is midnight in the local zone');
  // full ISO timestamps (what a DateTime field sends) keep their instant
  assert.equal(await go('2024-05-06T10:00:00.000Z'), '2024,5,6,22,0');
  assert.equal(await go('not a date'), 'null');
  r.stop();
});

test('Grid renders Grid.Item nodes with sections', async () => {
  const r = await start(HEAD + `
    module.exports = function () {
      return h(api.Grid, { columns: 4, inset: api.Grid.Inset.Medium, fit: api.Grid.Fit.Fill, itemSize: api.Grid.ItemSize.Small },
        h(api.Grid.Section, { title: 'S' }, h(api.Grid.Item, { id: 'a', title: 'A', content: 'icon:Star' })),
        h(api.Grid.Item, { id: 'b', title: 'B', content: { source: 'x.png' } }));
    };`);
  r.stop();
  assert.equal(all(r.tree, 'Grid.Item').length, 2);
  assert.equal(find(r.tree, 'Grid').p.inset, 'medium');
});

test('Detail.Metadata and List.Item.Detail pass through with their children', async () => {
  const r = await start(HEAD + `
    module.exports = function () {
      return h(api.Detail, { markdown: '# T', metadata: h(api.Detail.Metadata, null, h(api.Detail.Metadata.Label, { title: 'A', text: 'B' }), h(api.Detail.Metadata.Separator), h(api.Detail.Metadata.Link, { title: 'L', text: 'go', target: 'https://x.test' }), h(api.Detail.Metadata.TagList, { title: 'T' }, h(api.Detail.Metadata.TagList.Item, { text: 'one' }))) });
    };`);
  r.stop();
  assert.equal(all(r.tree, 'Detail.Metadata.Label').length, 1);
  assert.equal(all(r.tree, 'Detail.Metadata.TagList.Item')[0].p.text, 'one');
});

test('@raycast/utils style hooks work on top of the shim (usePromise-like effect, Cache subscribe)', async () => {
  const r = await start(HEAD + `
    const cache = new api.Cache();
    module.exports = function () {
      const [data, setData] = React.useState(undefined);
      const sub = React.useSyncExternalStore(cache.subscribe, () => cache.get('k') || 'none');
      React.useEffect(() => { Promise.resolve('v').then((v) => { cache.set('k', v); setData(v); }); }, []);
      return h(api.Detail, { markdown: String(data) + '/' + sub });
    };`);
  r.stop();
  assert.equal(find(r.tree, 'Detail').p.markdown, 'v/v');
});

test('List.Dropdown fires onChange on mount with its default (or first) value and again when the host picks one', async () => {
  const r = await start(HEAD + `
    module.exports = function () {
      const [page, setPage] = React.useState(undefined);
      return h(api.List, { isLoading: page === undefined, searchBarAccessory: h(api.List.Dropdown, { tooltip: 'T', onChange: setPage },
          h(api.List.Dropdown.Section, { title: 'S' }, h(api.List.Dropdown.Item, { value: 'a', title: 'A' }), h(api.List.Dropdown.Item, { value: 'b', title: 'B' })))},
        h(api.List.Item, { id: String(page), title: 'page ' + page }));
    };`);
  assert.equal(all(r.tree, 'List.Item')[0].p.title, 'page a', 'first item value is the initial value');
  const dd = find(r.tree, 'List.Dropdown');
  r.send({ type: 'field', id: dd.p._id, value: 'b' });
  await r.settle();
  assert.equal(all(r.tree, 'List.Item')[0].p.title, 'page b');
  r.stop();
});

test('form items accept refs with focus() and reset() (extensions call them)', async () => {
  const r = await start(HEAD + `
    module.exports = function () {
      const ref = React.useRef(null);
      const [s, set] = React.useState('…');
      React.useEffect(() => {
        try { ref.current.focus(); ref.current.reset(); set('ok'); } catch (e) { set('ERR ' + e.message); }
      }, []);
      return h(api.Form, null, h(api.Form.TextField, { id: 'a', title: s, ref }));
    };`);
  r.stop();
  assert.equal(find(r.tree, 'Form.TextField').p.title, 'ok');
  assert.equal(find(r.tree, 'Form.TextField').p.ref, undefined, 'refs never travel to the host');
});

test('confirmAlert asks the host and resolves to its boolean', async () => {
  const mk = (value) =>
    start(
      HEAD + `
      module.exports = function () {
        const [s, set] = React.useState('…');
        React.useEffect(() => {
          api.confirmAlert({ title: 'Delete?', message: 'Really', primaryAction: { title: 'Delete', style: api.Alert.ActionStyle.Destructive }, dismissAction: { title: 'Keep' } })
            .then((v) => set('got ' + v), (e) => set('ERR ' + e.message));
        }, []);
        return h(api.Detail, { markdown: s });
      };`,
      { answer: () => value },
    );
  const yes = await mk(true);
  yes.stop();
  assert.equal(find(yes.tree, 'Detail').p.markdown, 'got true');
  assert.deepEqual(yes.calls[0], { method: 'alert.confirm', params: { title: 'Delete?', message: 'Really', primary: 'Delete', dismiss: 'Keep', style: 'destructive' } });
  const no = await mk(false);
  no.stop();
  assert.equal(find(no.tree, 'Detail').p.markdown, 'got false');
});
