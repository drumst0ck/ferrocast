'use strict';
// Entry point of the sidecar. Protocol: see crates/core/src/extensions/protocol.rs.
const readline = require('readline');
const path = require('path');
const Module = require('module');
const React = require('react');
const jsxRuntime = require('react/jsx-runtime');
const bridge = require('./bridge');
const api = require('./api');
const { createRoot, toTree } = require('./reconciler');

const stack = []; // {root, last, pendingSend}
let mode = 'view';

function toast(style, title, message) {
  bridge.send({ type: 'toast', style, title, message: message || '' });
}
let failures = 0;
function fail(e, title = 'Extension error') {
  if (e && (e.code === 'EPIPE' || e.code === 'ERR_STREAM_DESTROYED')) bridge.die();
  const msg = e && e.message ? e.message : String(e);
  failures++;
  // a failing loop must not flood the host: report the first few, log a few more, then go quiet
  if (failures <= 20) bridge.log(`${title}: ${(e && e.stack) || msg}`);
  if (failures <= 5) toast('failure', title, msg);
  if (failures > 1000) bridge.die();
}

// Raycast reports a dropdown's starting value through onChange as soon as it is shown.
function firstItemValue(inst) {
  for (const c of inst.children) {
    if (/\.Dropdown\.Item$/.test(c.type)) return c.props.value;
    const v = firstItemValue(c);
    if (v !== undefined) return v;
  }
  return undefined;
}
function initDropdowns(container) {
  for (const inst of container.dropdowns) {
    if (inst.initialised) continue;
    inst.initialised = true;
    const f = inst.props.onChange;
    if (typeof f !== 'function') continue;
    const v = inst.props.value !== undefined ? inst.props.value : inst.props.defaultValue !== undefined ? inst.props.defaultValue : firstItemValue(inst);
    if (v !== undefined) Promise.resolve().then(() => f(v)).catch((e) => fail(e));
  }
}

function flush(entry) {
  entry.queued = false;
  const depth = stack.indexOf(entry) + 1;
  if (depth === 0) return;
  const json = JSON.stringify(toTree(entry.root.container));
  initDropdowns(entry.root.container);
  if (json === entry.last) return;
  entry.last = json;
  bridge.send({ type: 'render', depth, tree: JSON.parse(json) });
}

function push(element) {
  const entry = { last: null, queued: false };
  const nav = { push: pushTarget, pop };
  entry.root = createRoot(
    () => {
      if (!entry.queued) {
        entry.queued = true;
        setImmediate(() => flush(entry));
      }
    },
    (e) => fail(e),
  );
  stack.push(entry);
  entry.root.render(React.createElement(NavContext.Provider, { value: nav }, element));
}
function pushTarget(el) {
  if (el) push(el);
}
function pop() {
  if (stack.length <= 1) {
    bridge.send({ type: 'popToRoot' });
    return;
  }
  const e = stack.pop();
  e.root.unmount();
  const top = stack[stack.length - 1];
  top.last = null;
  flush(top);
}
const NavContext = React.createContext(null);
bridge.navigation.push = pushTarget;
bridge.navigation.pop = pop;

// A form value as the extension expects it (the host sends JSON: dates as ISO strings, tags as arrays).
function fieldValue(type, v) {
  if (type === 'Form.DatePicker') {
    // typed as YYYY-MM-DD: local midnight like Raycast (new Date('2024-05-06') would be UTC midnight)
    const day = typeof v === 'string' && /^(\d{4})-(\d{2})-(\d{2})$/.exec(v);
    const d = day ? new Date(+day[1], +day[2] - 1, +day[3]) : v ? new Date(v) : null;
    return d && !Number.isNaN(d.getTime()) ? d : null;
  }
  if (type === 'Form.TagPicker') return Array.isArray(v) ? v : [];
  if (type === 'Form.Checkbox') return !!v;
  return v === undefined || v === null ? '' : v;
}
function formValues(container, given) {
  const out = {};
  for (const [id, inst] of container.fields) {
    if (inst.type.startsWith('Form.') && !(inst.type === 'Form.Description' || inst.type === 'Form.Separator' || inst.type === 'Form.LinkAccessory')) {
      let v = given && id in given ? given[id] : inst.props.value !== undefined ? inst.props.value : inst.props.defaultValue;
      if (v === undefined && inst.type === 'Form.Dropdown') v = firstItemValue(inst);
      out[id] = fieldValue(inst.type, v);
    }
  }
  return out;
}

function runField(msg) {
  const top = stack[stack.length - 1];
  const inst = top && top.root.container.fields.get(msg.id);
  if (!inst) return;
  const f = inst.props.onChange;
  if (typeof f === 'function') Promise.resolve().then(() => f(inst.type.startsWith('Form.') ? fieldValue(inst.type, msg.value) : msg.value)).catch((e) => fail(e));
}

async function runAction(id, values) {
  const top = stack[stack.length - 1];
  const inst = top && top.root.container.actions.get(id);
  if (!inst) return;
  const p = inst.props;
  try {
    switch (inst.type) {
      case 'Action':
        if (p.onAction) await p.onAction();
        break;
      case 'Action.CopyToClipboard':
        await api.Clipboard.copy(p.content, { concealed: p.concealed });
        if (p.onCopy) p.onCopy(p.content);
        await api.showHUD('Copied to Clipboard');
        break;
      case 'Action.OpenInBrowser':
        await api.open(p.url);
        if (p.onOpen) p.onOpen(p.url);
        await api.closeMainWindow();
        break;
      case 'Action.Open':
        await api.open(p.target, p.application);
        if (p.onOpen) p.onOpen(p.target);
        await api.closeMainWindow();
        break;
      case 'Action.Paste':
        await api.Clipboard.paste(p.content);
        if (p.onPaste) p.onPaste(p.content);
        break;
      case 'Action.ShowInFinder':
        await api.showInFinder(p.path);
        break;
      case 'Action.Push':
        if (p.onPush) p.onPush();
        pushTarget(p.target);
        break;
      case 'Action.SubmitForm':
        if (p.onSubmit) await p.onSubmit(formValues(top.root.container, values));
        break;
      default:
        throw new Error(`not supported in Ferrocast yet: ${inst.type}`);
    }
  } catch (e) {
    fail(e, p.title || 'Action failed');
  }
}

function launch(msg) {
  mode = msg.mode || 'view';
  api.__configure(msg.environment, msg.preferences, mode);
  const origLoad = Module._load;
  Module._load = function (request, parent, isMain) {
    if (request === '@raycast/api') return api;
    if (request === 'react') return React;
    if (request === 'react/jsx-runtime' || request === 'react/jsx-dev-runtime') return jsxRuntime;
    return origLoad.call(this, request, parent, isMain);
  };
  let mod;
  try {
    mod = require(path.resolve(msg.entry));
  } catch (e) {
    fail(e, 'Cannot load extension');
    bridge.send({ type: 'exit' });
    return;
  }
  const Command = mod && (mod.default || mod);
  if (typeof Command !== 'function') {
    fail(new Error('the command has no default export'), 'Cannot start extension');
    bridge.send({ type: 'exit' });
    return;
  }
  const props = { arguments: msg.arguments || {}, launchType: 'userInitiated' };
  if (mode === 'no-view') {
    Promise.resolve()
      .then(() => Command(props))
      .catch((e) => fail(e))
      .finally(() => setTimeout(() => bridge.send({ type: 'exit' }), 50));
  } else {
    push(React.createElement(Command, props));
  }
}

function handle(msg) {
  const top = stack[stack.length - 1];
  switch (msg.type) {
    case 'launch':
      launch(msg);
      break;
    case 'search': {
      const f = top && top.root.container.listProps && top.root.container.listProps.onSearchTextChange;
      if (f) Promise.resolve().then(() => f(msg.text)).catch((e) => fail(e));
      break;
    }
    case 'selection': {
      const f = top && top.root.container.listProps && top.root.container.listProps.onSelectionChange;
      if (f) Promise.resolve().then(() => f(msg.id)).catch((e) => fail(e));
      break;
    }
    case 'action':
      runAction(msg.id, msg.values);
      break;
    case 'field':
      runField(msg);
      break;
    case 'pop':
      if (stack.length > 1) pop();
      break;
    case 'result':
      bridge.result(msg);
      break;
  }
}

process.on('uncaughtException', (e) => fail(e));
process.on('unhandledRejection', (e) => fail(e));

const rl = readline.createInterface({ input: process.stdin });
rl.on('line', (line) => {
  if (!line.trim()) return;
  try {
    handle(JSON.parse(line));
  } catch (e) {
    fail(e, 'Runtime error');
  }
});
rl.on('close', () => process.exit(0));
