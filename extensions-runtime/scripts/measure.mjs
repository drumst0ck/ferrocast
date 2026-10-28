// Measurement harness: drives dist/runtime.js over stdio the way the Ferrocast app does (launch,
// search, selection, action, pop; answers `call` messages with stubs) and records what each `view`
// command renders. It never prints private content: only counts and error text.
//
//   node scripts/measure.mjs --label store --dir <ext folder> [--dir ...] [--runtime dist/runtime.js]
//        [--supported baseline|current] [--timeout 10000] [--skip name,name] [--no-view-allow ext/cmd,..]
//        [--out results.json]
//
// <ext folder> holds package.json and the built commands (`<cmd>.js`, as Raycast installs them) either
// directly or in a `dist/` subfolder (output of `ray build -e dist`). Folders are only read.
// Host calls are stubbed: clipboard/localStorage in memory, open/showInFinder recorded and ignored, no
// real selection. The sidecar runs with HOME, PATH, LANG, TMPDIR only; its support dir is scratch.
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import readline from 'node:readline';

const args = process.argv.slice(2);
const opt = (k, d) => {
  const i = args.indexOf(k);
  return i >= 0 ? args[i + 1] : d;
};
const dirs = args.flatMap((a, i) => (a === '--dir' ? [args[i + 1]] : []));
const runtime = path.resolve(opt('--runtime', 'dist/runtime.js'));
const label = opt('--label', 'set');
const timeout = Number(opt('--timeout', '10000'));
const skip = new Set((opt('--skip', '') || '').split(',').filter(Boolean));
const noViewAllow = new Set((opt('--no-view-allow', '') || '').split(',').filter(Boolean));
const supportedSet = opt('--supported', 'current');
const outFile = opt('--out', '');
// --dump-trees <file>: the final render tree of every command (contains content: keep it in scratch).
const dumpFile = opt('--dump-trees', '');
const dumped = [];

// Component types the launcher draws (keep in sync with crates/core/src/extensions/view.rs).
const BASE = ['List', 'List.Item', 'List.Section', 'List.EmptyView', 'List.Dropdown', 'List.Dropdown.Item', 'List.Dropdown.Section', 'Detail', '#text'];
const NEW = [
  ...BASE,
  'Form', 'Form.TextField', 'Form.PasswordField', 'Form.TextArea', 'Form.Checkbox', 'Form.Dropdown', 'Form.Dropdown.Item',
  'Form.Dropdown.Section', 'Form.DatePicker', 'Form.TagPicker', 'Form.TagPicker.Item', 'Form.Description', 'Form.Separator',
  'Grid', 'Grid.Item', 'Grid.Section', 'Grid.EmptyView', 'Grid.Dropdown', 'Grid.Dropdown.Item', 'Grid.Dropdown.Section',
  'List.Item.Detail', 'List.Item.Detail.Metadata', 'Detail.Metadata',
];
const SUPPORTED = new Set(supportedSet === 'baseline' ? BASE : NEW);
const isAction = (t) => t === 'ActionPanel' || t.startsWith('ActionPanel.') || t === 'Action' || t.startsWith('Action.');
// metadata children are drawn as part of their parent
const isMeta = (t) => /\.Metadata\./.test(t);

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const scratchRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'fc-measure-'));

function prefDefaults(cmd, manifest) {
  const out = {};
  const missing = [];
  for (const p of [...(manifest.preferences || []), ...(cmd.preferences || [])]) {
    let v = p.default;
    if (v === undefined && p.type === 'checkbox') v = false;
    if (v === undefined && p.type === 'dropdown' && p.required && p.data && p.data[0]) v = undefined;
    if (v === undefined) {
      if (p.required) missing.push(p.name);
      continue;
    }
    out[p.name] = v;
  }
  return { values: out, missing };
}

function summarize(tree) {
  const m = { root: null, items: 0, sections: 0, actions: 0, fields: 0, markdown: 0, emptyView: false, unsupported: [], types: {} };
  const root = tree.c.find((n) => n.t !== '#text');
  m.root = root ? root.t : null;
  const walk = (n, inAction) => {
    const act = inAction || isAction(n.t);
    if (n.t === 'List.Item' || n.t === 'Grid.Item') m.items++;
    else if (n.t === 'List.Section' || n.t === 'Grid.Section') m.sections++;
    else if (/^Action\./.test(n.t) || n.t === 'Action') m.actions++;
    else if (/^Form\./.test(n.t) && !['Form.Description', 'Form.Separator', 'Form.Dropdown.Item', 'Form.Dropdown.Section', 'Form.TagPicker.Item'].includes(n.t)) m.fields++;
    if ((n.t === 'List.EmptyView' || n.t === 'Grid.EmptyView') && (n.p.title || n.p.description)) m.emptyView = true;
    if (n.t === 'Detail' && typeof n.p.markdown === 'string') m.markdown += n.p.markdown.length;
    if (!act && !isMeta(n.t) && !SUPPORTED.has(n.t) && !m.unsupported.includes(n.t)) m.unsupported.push(n.t);
    for (const c of n.c) walk(c, act);
  };
  for (const c of tree.c) walk(c, false);
  return m;
}

function firstItemInfo(tree) {
  let found = null;
  const walk = (n) => {
    if (found) return;
    if (n.t === 'List.Item' || n.t === 'Grid.Item') {
      const ap = n.c.find((c) => c.t === 'ActionPanel');
      const first = ap && firstAction(ap);
      found = { id: n.p.id || n.p._id, firstAction: first };
      return;
    }
    n.c.forEach(walk);
  };
  tree.c.forEach(walk);
  return found;
}
function firstAction(n) {
  for (const c of n.c) {
    if (c.t === 'Action' || c.t.startsWith('Action.')) return { id: c.p._id, type: c.t };
    const f = firstAction(c);
    if (f) return f;
  }
  return null;
}
function hasSearchHandler(tree) {
  const root = tree.c.find((n) => n.t !== '#text');
  return !!(root && root.p.onSearchTextChange);
}

async function runCommand(extDir, manifest, cmd, mode) {
  const rec = {
    ext: manifest.name, cmd: cmd.name, mode, status: 'fails', cause: '', renders: 0, first: null, final: null,
    afterSearch: null, pushed: null, toasts: [], stderr: [], calls: {}, exit: null, ms: 0,
  };
  const jsDir = fs.existsSync(path.join(extDir, `${cmd.name}.js`)) ? extDir : path.join(extDir, 'dist');
  const entry = path.join(jsDir, `${cmd.name}.js`);
  if (!fs.existsSync(entry)) {
    rec.status = 'skipped';
    rec.cause = 'command bundle not found (not built)';
    return rec;
  }
  const { values, missing } = prefDefaults(cmd, manifest);
  if (missing.length) {
    rec.status = 'blocked';
    rec.cause = `required preference without default: ${missing.join(', ')}`;
    return rec;
  }
  const support = fs.mkdtempSync(path.join(scratchRoot, 'support-'));
  const env = { HOME: process.env.HOME, PATH: process.env.PATH, LANG: process.env.LANG || 'en_US.UTF-8', TMPDIR: process.env.TMPDIR || os.tmpdir() };
  const t0 = Date.now();
  const child = spawn(process.execPath, [runtime], { env, cwd: support, stdio: ['pipe', 'pipe', 'pipe'], detached: true });
  const kill = () => {
    try {
      process.kill(-child.pid, 'SIGKILL');
    } catch {}
  };
  const send = (m) => {
    try {
      child.stdin.write(JSON.stringify(m) + '\n');
    } catch {}
  };
  let tree = null;
  let depthTrees = [];
  let lastMsg = Date.now();
  let exited = false;
  child.on('exit', (code, sig) => {
    exited = true;
    rec.exit = `code ${code}${sig ? ' ' + sig : ''}`;
  });
  child.stdin.on('error', () => {});
  const err = readline.createInterface({ input: child.stderr });
  err.on('line', (l) => {
    if (rec.stderr.length < 4) rec.stderr.push(l.replace(/\s+/g, ' ').slice(0, 160));
  });
  const store = new Map();
  let clip = '';
  const answer = (id, value, error) => send({ type: 'result', id, value: value ?? null, error: error ?? null });
  const rl = readline.createInterface({ input: child.stdout });
  rl.on('line', (line) => {
    lastMsg = Date.now();
    let m;
    try {
      m = JSON.parse(line);
    } catch {
      return;
    }
    switch (m.type) {
      case 'render':
        rec.renders++;
        depthTrees[m.depth - 1] = m.tree;
        depthTrees.length = m.depth;
        tree = m.tree;
        if (!rec.first) rec.first = { ms: Date.now() - t0, ...summarize(m.tree) };
        break;
      case 'toast':
        rec.toasts.push({ style: m.style, text: `${m.title || ''}${m.message ? ': ' + m.message : ''}`.replace(/\s+/g, ' ').slice(0, 140) });
        break;
      case 'hud':
        rec.toasts.push({ style: 'hud', text: String(m.title).slice(0, 80) });
        break;
      case 'call': {
        rec.calls[m.method] = (rec.calls[m.method] || 0) + 1;
        const p = m.params || {};
        switch (m.method) {
          case 'clipboard.copy':
          case 'clipboard.paste':
            clip = p.text || '';
            return answer(m.id, null);
          case 'clipboard.readText':
            return answer(m.id, clip || null);
          case 'clipboard.clear':
            clip = '';
            return answer(m.id, null);
          case 'open':
          case 'showInFinder':
          case 'launchCommand': // recorded; the app would start the command
            return answer(m.id, null);
          case 'localStorage.getItem':
            return answer(m.id, store.has(p.key) ? store.get(p.key) : null);
          case 'localStorage.setItem':
            store.set(p.key, p.value);
            return answer(m.id, null);
          case 'localStorage.removeItem':
            store.delete(p.key);
            return answer(m.id, null);
          case 'localStorage.allItems':
            return answer(m.id, Object.fromEntries(store));
          case 'localStorage.clear':
            store.clear();
            return answer(m.id, null);
          case 'apps.list': {
            let names = [];
            try {
              names = fs.readdirSync('/Applications').filter((n) => n.endsWith('.app')).slice(0, 50);
            } catch {}
            return answer(m.id, names.map((n) => ({ name: n.replace(/\.app$/, ''), path: `/Applications/${n}`, bundleId: '' })));
          }
          case 'apps.default':
          case 'apps.frontmost':
            return answer(m.id, { name: 'Safari', path: '/Applications/Safari.app', bundleId: 'com.apple.Safari' });
          case 'selection.text':
            return answer(m.id, null, 'Unable to get selected text');
          default:
            return answer(m.id, null, `not supported in Ferrocast yet: ${m.method}`);
        }
      }
      case 'exit':
        rec.exit = rec.exit || 'asked to exit';
        break;
      default:
    }
  });

  send({
    type: 'launch', entry, extension: manifest.name, command: cmd.name, title: cmd.title || cmd.name, mode,
    arguments: {}, preferences: values,
    environment: {
      extensionName: manifest.name, commandName: cmd.name, supportPath: support, assetsPath: path.join(extDir, fs.existsSync(path.join(extDir, 'assets')) ? 'assets' : 'dist/assets'),
      isDevelopment: false, raycastVersion: '1.100.0', launchType: 'userInitiated',
    },
  });

  // settle: wait for the first render, then until nothing arrives for 1.5 s (or the time limit)
  const settle = async (quiet = 1500, limit = timeout) => {
    const start = Date.now();
    while (Date.now() - start < limit && !exited) {
      await sleep(100);
      const root = tree && tree.c.find((n) => n.t !== '#text');
      const loading = !!(root && root.p.isLoading);
      // a view that says it is loading keeps us waiting (up to the limit)
      if (rec.renders > 0 && Date.now() - lastMsg > quiet && !loading) break;
      if (mode === 'no-view' && Date.now() - lastMsg > quiet) break;
    }
  };
  await settle();
  if (mode === 'view') {
    if (tree) {
      rec.final = summarize(tree);
      if (hasSearchHandler(tree)) {
        const before = rec.renders;
        lastMsg = Date.now();
        send({ type: 'search', text: 'react' });
        await settle(3000, 12000);
        if (tree && rec.renders > before) rec.afterSearch = summarize(tree);
      }
      const rootNode = tree.c.find((n) => n.t !== '#text');
      if (rootNode && rootNode.t === 'Form') {
        // submit with the default values: everything goes through (stubbed) host calls
        const sub = firstAction(rootNode);
        if (sub && sub.type === 'Action.SubmitForm') {
          const before = rec.toasts.length;
          lastMsg = Date.now();
          send({ type: 'action', id: sub.id });
          await settle(1500, 6000);
          rec.submit = { toasts: rec.toasts.slice(before).map((x) => `${x.style}`) };
          const bad = rec.toasts.slice(before).find((x) => x.style === 'failure');
          if (bad) rec.submit.failure = bad.text;
        }
      }
      const info = firstItemInfo(tree);
      if (info) {
        send({ type: 'selection', id: info.id });
        const safeKinds = ['Action.CopyToClipboard', 'Action.Paste', 'Action.OpenInBrowser', 'Action.Open', 'Action.ShowInFinder'];
        if (info.firstAction && safeKinds.includes(info.firstAction.type)) {
          // only actions that go through (stubbed) host calls: nothing real happens
          const before = rec.toasts.length;
          send({ type: 'action', id: info.firstAction.id });
          await sleep(500);
          rec.primary = { type: info.firstAction.type, toasts: rec.toasts.length - before };
        }
        if (info.firstAction && info.firstAction.type === 'Action.Push') {
          const before = rec.renders;
          lastMsg = Date.now();
          send({ type: 'action', id: info.firstAction.id });
          await settle(1500, 6000);
          if (rec.renders > before && depthTrees.length > 1) rec.pushed = summarize(depthTrees[depthTrees.length - 1]);
          send({ type: 'pop' });
          await sleep(200);
        }
      }
    }
  } else {
    await settle(1500, 8000);
  }
  rec.ms = Date.now() - t0;
  if (dumpFile && tree) dumped.push({ name: `${manifest.name}/${cmd.name}`, tree });
  kill();
  err.close();
  rl.close();
  classify(rec);
  return rec;
}

// The extension asked for input the harness cannot give (empty clipboard, no selection, empty form).
const NEEDS_INPUT = /No text found in clipboard|Unable to get selected text|cannot be empty|must be a number|No values set|Missing |Please (copy|enter) a valid/i;
// The extension drives a macOS app that is not installed on the measuring Mac.
const MISSING_APP = /application "[^"]+"\. \(-1728|is not installed/i;

function classify(rec) {
  const fin = rec.final;
  const failToasts = rec.toasts.filter((t) => t.style === 'failure');
  if (rec.mode === 'no-view') {
    if (rec.exit && /^code [1-9]/.test(rec.exit)) {
      rec.status = 'fails';
      rec.cause = rec.stderr[0] || rec.exit;
    } else if (failToasts.length) {
      rec.status = 'fails';
      rec.cause = failToasts[0].text;
    } else {
      rec.status = 'works';
      rec.cause = rec.toasts.length ? 'finished with a message' : 'finished';
    }
    return;
  }
  if (!rec.first) {
    rec.status = 'fails';
    rec.cause = failToasts[0] ? failToasts[0].text : rec.stderr[0] || rec.exit || 'no render before the time limit';
    return;
  }
  const f = fin || rec.first;
  const as = rec.afterSearch;
  const content = f.items > 0 || f.fields > 0 || f.markdown > 0 || !!(as && (as.items > 0 || as.markdown > 0));
  if (f.unsupported.length && (f.root === null || !SUPPORTED.has(f.root))) {
    rec.status = 'fails';
    rec.cause = `unsupported root ${f.root}`;
  } else if (f.unsupported.length) {
    rec.status = 'partly';
    rec.cause = `not drawn: ${f.unsupported.join(', ')}`;
  } else if (failToasts.length && MISSING_APP.test(failToasts[0].text)) {
    rec.status = 'missing-app';
    rec.cause = `needs a macOS app that is not installed here: ${failToasts[0].text.slice(0, 80)}`;
  } else if (failToasts.length && NEEDS_INPUT.test(failToasts[0].text)) {
    rec.status = 'input';
    rec.cause = `needs input the harness has none of: ${failToasts[0].text}`;
  } else if (failToasts.length) {
    rec.status = content ? 'partly' : 'fails';
    rec.cause = failToasts[0].text;
  } else if (!failToasts.length && rec.toasts.some((x) => MISSING_APP.test(x.text))) {
    rec.status = 'missing-app';
    rec.cause = 'needs a macOS app that is not installed here';
  } else if (rec.submit && rec.submit.failure && NEEDS_INPUT.test(rec.submit.failure)) {
    rec.status = 'input';
    rec.cause = `form renders; submitting empty values is rejected by the extension: ${rec.submit.failure}`;
  } else if (rec.submit && rec.submit.failure) {
    rec.status = 'partly';
    rec.cause = `submit failed: ${rec.submit.failure}`;
  } else if (!content) {
    if (f.emptyView) {
      rec.status = 'works';
      rec.cause = 'shows the extension\'s own empty state';
    } else {
      rec.status = 'partly';
      rec.cause = rec.afterSearch ? 'renders but empty, also after searching' : 'renders but empty';
    }
  } else {
    rec.status = 'works';
    rec.cause = '';
  }
}

const results = [];
for (const dir of dirs) {
  const abs = path.resolve(dir);
  const manifestPath = fs.existsSync(path.join(abs, 'package.json')) ? path.join(abs, 'package.json') : null;
  if (!manifestPath) continue;
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  if (skip.has(manifest.name)) {
    results.push({ ext: manifest.name, cmd: '*', mode: '-', status: 'skipped', cause: 'excluded on purpose (secrets, private data or account)' });
    continue;
  }
  for (const cmd of manifest.commands || []) {
    const mode = cmd.mode || 'view';
    if (mode === 'menu-bar') {
      results.push({ ext: manifest.name, cmd: cmd.name, mode, status: 'skipped', cause: 'menu-bar command (not judged)' });
      continue;
    }
    if (mode === 'no-view' && !noViewAllow.has(`${manifest.name}/${cmd.name}`)) {
      results.push({ ext: manifest.name, cmd: cmd.name, mode, status: 'skipped', cause: 'no-view command (side effects), not run' });
      continue;
    }
    const r = await runCommand(abs, manifest, cmd, mode);
    results.push(r);
    process.stderr.write(`${r.status.padEnd(8)} ${manifest.name}/${cmd.name} ${r.cause.slice(0, 100)}\n`);
  }
}
fs.rmSync(scratchRoot, { recursive: true, force: true });
const judged = results.filter((r) => ['works', 'input', 'missing-app', 'partly', 'fails'].includes(r.status));
const tally = Object.fromEntries(['works', 'input', 'missing-app', 'partly', 'fails'].map((s) => [s, judged.filter((r) => r.status === s).length]));
const summary = { label, supported: supportedSet, runtime, judged: judged.length, ...tally, blocked: results.filter((r) => r.status === 'blocked').length, skipped: results.filter((r) => r.status === 'skipped').length };
console.log(JSON.stringify(summary));
if (dumpFile) fs.writeFileSync(dumpFile, JSON.stringify(dumped));
if (outFile) fs.writeFileSync(outFile, JSON.stringify({ summary, results }, null, 1));
process.exit(0);
