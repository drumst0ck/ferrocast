'use strict';
// Our implementation of the `@raycast/api` surface that Ferrocast supports. Everything it cannot do
// fails with "not supported in Ferrocast yet: <name>" instead of silently doing nothing.
const React = require('react');
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');
const bridge = require('./bridge');

const unsupportedMsg = (name) => `not supported in Ferrocast yet: ${name}`;
const unsupportedFn = (name) => () => {
  throw new Error(unsupportedMsg(name));
};

// Component factory: `actions={<ActionPanel/>}` becomes a child so the host sees one tree.
// Props that hold a whole element tree (not data) travel as children so the host sees one tree:
// `actions` (ActionPanel), `metadata` (Detail.Metadata), `detail` (List.Item.Detail), `searchBarAccessory`.
const ELEMENT_PROPS = ['actions', 'metadata', 'detail', 'searchBarAccessory'];
function mk(name) {
  const C = (props) => {
    const { children, ...rest } = props || {};
    const extra = [];
    for (const k of ELEMENT_PROPS) {
      if (rest[k] !== undefined && React.isValidElement(rest[k])) extra.push(rest[k]);
      if (React.isValidElement(rest[k])) delete rest[k];
    }
    return React.createElement(name, rest, children, ...extra);
  };
  C.displayName = name;
  return C;
}
function group(name, subs) {
  const C = mk(name);
  for (const s of subs) C[s] = mk(`${name}.${s}`);
  return C;
}

const metadata = (name) => {
  const M = group(name, ['Label', 'Link', 'TagList', 'Separator']);
  M.TagList.Item = mk(`${name}.TagList.Item`);
  return M;
};
const dropdown = (name) => {
  const D = mk(name);
  D.Item = mk(`${name}.Item`);
  D.Section = mk(`${name}.Section`);
  return D;
};
const List = group('List', ['Item', 'Section', 'EmptyView']);
List.Dropdown = dropdown('List.Dropdown');
List.Item.Detail = mk('List.Item.Detail');
List.Item.Detail.Metadata = metadata('List.Item.Detail.Metadata');
const Detail = group('Detail', []);
Detail.Metadata = metadata('Detail.Metadata');
const ActionPanel = group('ActionPanel', ['Section', 'Submenu']);
const Action = group('Action', [
  'CopyToClipboard', 'OpenInBrowser', 'Open', 'Paste', 'Push', 'ShowInFinder', 'Trash', 'SubmitForm',
  'OpenWith', 'CreateSnippet', 'CreateQuicklink', 'ToggleQuickLook', 'PickDate',
]);
const Form = group('Form', ['TextField', 'PasswordField', 'TextArea', 'Checkbox', 'DatePicker', 'Description', 'Separator', 'FilePicker', 'LinkAccessory']);
Form.Dropdown = dropdown('Form.Dropdown');
Form.TagPicker = mk('Form.TagPicker');
Form.TagPicker.Item = mk('Form.TagPicker.Item');
Form.DatePicker.Type = { Date: 'date', DateTime: 'dateTime' };
Form.DatePicker.isFullDay = (d) => d instanceof Date && d.getHours() === 0 && d.getMinutes() === 0 && d.getSeconds() === 0 && d.getMilliseconds() === 0;
Action.Style = { Regular: 'regular', Destructive: 'destructive' };
Action.PickDate.Type = Form.DatePicker.Type;
Action.PickDate.isFullDay = Form.DatePicker.isFullDay;
const Grid = group('Grid', ['Item', 'Section', 'EmptyView']);
Grid.Dropdown = dropdown('Grid.Dropdown');
Grid.Inset = { Small: 'small', Medium: 'medium', Large: 'large' };
Grid.Fit = { Contain: 'contain', Fill: 'fill' };
Grid.ItemSize = { Small: 'small', Medium: 'medium', Large: 'large' };
const MenuBarExtra = group('MenuBarExtra', ['Item', 'Section', 'Submenu', 'Separator']);

const iconNames = new Proxy({}, { get: (_t, k) => (typeof k === 'string' ? `icon:${k}` : undefined) });
const Color = {
  Blue: '#0A84FF', Green: '#30D158', Magenta: '#BF5AF2', Orange: '#FF9F0A', Purple: '#5E5CE6',
  Red: '#FF453A', Yellow: '#FFD60A', PrimaryText: '#FFFFFF', SecondaryText: '#98989D',
};
const common = (key, modifiers = ['cmd']) => ({ modifiers, key });
const Keyboard = {
  Shortcut: {
    Common: {
      Copy: common('c', ['cmd', 'shift']), CopyName: common('.', ['cmd', 'shift']), CopyPath: common(',', ['cmd', 'shift']),
      CopyDeeplink: common('c', ['cmd', 'shift']), ToggleQuickLook: common('y'),
      Save: common('s'), Duplicate: common('d'), Edit: common('e'), MoveDown: common('arrowDown', ['cmd', 'shift']),
      MoveUp: common('arrowUp', ['cmd', 'shift']), New: common('n'), Open: common('o'), OpenWith: common('o', ['cmd', 'shift']),
      Pin: common('p', ['cmd', 'shift']), Refresh: common('r'), Remove: common('x', ['ctrl']), RemoveAll: common('x', ['ctrl', 'shift']),
    },
  },
};

// ---- environment / preferences (filled by the launch message)
const environment = {
  extensionName: '', commandName: '', supportPath: '', assetsPath: '', isDevelopment: false,
  raycastVersion: '', launchType: 'userInitiated', appearance: 'dark', theme: 'dark',
  textSize: 'medium', commandMode: 'view', ownerOrAuthorName: '',
  canAccess: (api) => !(api && (api.name === 'AI' || api === AI)),
};
let preferences = {};
function configure(env, prefs, mode) {
  Object.assign(environment, env, { commandMode: mode });
  preferences = prefs || {};
}
const getPreferenceValues = () => ({ ...preferences });

const LaunchType = { UserInitiated: 'userInitiated', Background: 'background' };

// ---- toast / HUD / window
const ToastStyle = { Animated: 'animated', Success: 'success', Failure: 'failure' };
class Toast {
  constructor(opts = {}) {
    this._style = opts.style || 'animated';
    this._title = opts.title || '';
    this._message = opts.message || '';
    this.primaryAction = opts.primaryAction;
    this.secondaryAction = opts.secondaryAction;
  }
  _send() {
    bridge.send({ type: 'toast', style: this._style, title: this._title, message: this._message });
  }
  // Changes after show() are coalesced into one message per tick (the host may turn each into a
  // notification); a toast that was never shown stays silent.
  _changed() {
    if (!this._shown || this._queued) return;
    this._queued = true;
    queueMicrotask(() => {
      this._queued = false;
      this._send();
    });
  }
  get style() { return this._style; }
  set style(v) { this._style = v; this._changed(); }
  get title() { return this._title; }
  set title(v) { this._title = v; this._changed(); }
  get message() { return this._message; }
  set message(v) { this._message = v; this._changed(); }
  async show() { this._shown = true; this._send(); }
  async hide() {}
}
Toast.Style = ToastStyle;
Toast.ActionStyle = { Primary: 'primary', Destructive: 'destructive', Cancel: 'cancel' };
async function showToast(a, b, c) {
  const opts = a && typeof a === 'object' ? a : { style: a, title: b, message: c };
  const t = new Toast(opts);
  await t.show();
  return t;
}
async function closeMainWindow() { bridge.send({ type: 'closeMainWindow' }); }
async function popToRoot() { bridge.send({ type: 'popToRoot' }); }
async function showHUD(title) {
  bridge.send({ type: 'hud', title: String(title) });
}
async function clearSearchBar() {}

// ---- clipboard
const htmlToText = (html) => String(html)
  .replace(/<br\s*\/?>|<\/(p|div|li|h[1-6])>/gi, '\n')
  .replace(/<[^>]*>/g, '')
  .replace(/&nbsp;/g, ' ').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, '&')
  .trim();
const asText = (c) => (c !== null && typeof c === 'object' ? c.text ?? (c.html !== undefined ? htmlToText(c.html) : c.file ?? '') : String(c));
const Clipboard = {
  copy: (content, options) => bridge.call('clipboard.copy', { text: asText(content), concealed: !!(options && options.concealed) }),
  paste: (content) => bridge.call('clipboard.paste', { text: asText(content) }),
  readText: async () => (await bridge.call('clipboard.readText', {})) || undefined,
  read: async () => ({ text: (await bridge.call('clipboard.readText', {})) || '' }),
  clear: () => bridge.call('clipboard.clear', {}),
};

// ---- storage
const LocalStorage = {
  getItem: async (key) => (await bridge.call('localStorage.getItem', { key })) ?? undefined,
  setItem: (key, value) => bridge.call('localStorage.setItem', { key, value }),
  removeItem: (key) => bridge.call('localStorage.removeItem', { key }),
  allItems: () => bridge.call('localStorage.allItems', {}),
  clear: () => bridge.call('localStorage.clear', {}),
};

// Cache is synchronous in Raycast's API, so it lives in the runtime (a JSON file in supportPath).
// One store per namespace, shared by every instance: @raycast/utils creates a Cache per hook, and each
// instance writing its own snapshot would erase the keys of the others.
const cacheStores = new Map(); // namespace -> {data, subs}
class Cache {
  constructor(opts = {}) {
    const raw = String(opts.namespace || 'default');
    this._ns = raw.replace(/[^\w.-]/g, '_');
    // "a/b" and "a_b" must not share a file: a namespace the file name had to rewrite gets a hash of the original
    if (this._ns !== raw) this._ns += `-${crypto.createHash('sha1').update(raw).digest('hex').slice(0, 8)}`;
    // @raycast/utils hands these around detached (`useSyncExternalStore(cache.subscribe, ...)`)
    for (const m of ['has', 'get', 'set', 'remove', 'clear', 'subscribe']) this[m] = this[m].bind(this);
  }
  _file() { return path.join(environment.supportPath, `cache-${this._ns}.json`); }
  _store() {
    let st = cacheStores.get(this._ns);
    if (!st) {
      // null prototype: keys like "constructor" or "__proto__" are ordinary entries, not Object.prototype members
      st = { data: Object.create(null), subs: new Set() };
      try {
        const parsed = JSON.parse(fs.readFileSync(this._file(), 'utf8'));
        if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
          for (const k of Object.keys(parsed)) Object.defineProperty(st.data, k, { value: parsed[k], writable: true, enumerable: true, configurable: true });
        }
      } catch { /* none yet, or unreadable: start empty */ }
      cacheStores.set(this._ns, st);
    }
    return st;
  }
  _save(key, value) {
    const st = this._store();
    const file = this._file();
    const tmp = `${file}.${process.pid}.tmp`;
    try {
      fs.mkdirSync(environment.supportPath, { recursive: true });
      fs.writeFileSync(tmp, JSON.stringify(st.data));
      fs.renameSync(tmp, file);
    } catch (e) {
      try { fs.unlinkSync(tmp); } catch { /* not written */ }
      bridge.log(`cache write failed: ${e.message}`);
    }
    // a subscriber that throws must neither break the write nor starve the other subscribers
    for (const f of [...st.subs]) {
      try { f(key, value); } catch (e) { bridge.log(`cache subscriber failed: ${e && e.message}`); }
    }
  }
  get isEmpty() { return Object.keys(this._store().data).length === 0; }
  has(key) { return key in this._store().data; }
  get(key) { return this._store().data[key]; }
  set(key, value) { this._set(this._store(), key, value); this._save(key, value); }
  _set(st, key, value) { Object.defineProperty(st.data, key, { value, writable: true, enumerable: true, configurable: true }); }
  remove(key) {
    const st = this._store();
    if (!(key in st.data)) return false;
    delete st.data[key];
    this._save(key, undefined);
    return true;
  }
  clear() { this._store().data = Object.create(null); this._save(undefined, undefined); }
  subscribe(f) { const subs = this._store().subs; subs.add(f); return () => subs.delete(f); }
}

// ---- open / finder
const open = (target, application) => bridge.call('open', { target: String(target), application: application && (application.path || String(application)) });
const showInFinder = (p) => bridge.call('showInFinder', { path: String(p) });
const trash = unsupportedFn('trash');
const getSelectedText = () => bridge.call('selection.text', {});
const getSelectedFinderItems = unsupportedFn('getSelectedFinderItems');
const getFrontmostApplication = () => bridge.call('apps.frontmost', {});
const getApplications = (p) => bridge.call('apps.list', p ? { path: String(p.path || p) } : {});
const getDefaultApplication = (p) => bridge.call('apps.default', { path: String(p) });
const isPlainObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
// launchCommand({ name, type, extensionName?, ownerOrAuthorName?, arguments?, context?, fallbackText? }):
// the host starts the command with props.launchContext / props.arguments / props.fallbackText and
// environment.launchType (`background` runs it without showing the launcher).
const launchCommand = async (o) => {
  const bad = (m) => Promise.reject(new Error(`launchCommand: ${m}`));
  if (!isPlainObject(o)) return bad('options must be an object with the command name');
  if (typeof o.name !== 'string' || !o.name) return bad('name is required');
  const type = o.type === undefined || o.type === null ? LaunchType.UserInitiated : o.type;
  if (type !== LaunchType.UserInitiated && type !== LaunchType.Background) return bad(`invalid type ${JSON.stringify(type)}, use LaunchType.UserInitiated or LaunchType.Background`);
  if (o.context !== undefined && o.context !== null && !isPlainObject(o.context)) return bad('context must be an object');
  if (o.arguments !== undefined && o.arguments !== null && !isPlainObject(o.arguments)) return bad('arguments must be an object');
  if (o.fallbackText !== undefined && o.fallbackText !== null && typeof o.fallbackText !== 'string') return bad('fallbackText must be a string');
  const params = { name: o.name, extensionName: o.extensionName, ownerOrAuthorName: o.ownerOrAuthorName, arguments: o.arguments || {}, type };
  if (o.context) params.context = o.context;
  if (typeof o.fallbackText === 'string') params.fallbackText = o.fallbackText;
  return bridge.call('launchCommand', params);
};
const captureException = (e) => bridge.log(`captureException: ${e && e.message ? e.message : e}`);
const PopToRootType = { Default: 'default', Immediate: 'immediate', Suspended: 'suspended' };
const BrowserExtension = { getContent: unsupportedFn('BrowserExtension.getContent'), getTabs: unsupportedFn('BrowserExtension.getTabs') };
const WindowManagement = {
  getActiveWindow: unsupportedFn('WindowManagement.getActiveWindow'), getWindowsOnActiveDesktop: unsupportedFn('WindowManagement.getWindowsOnActiveDesktop'),
  getDesktops: unsupportedFn('WindowManagement.getDesktops'), setWindowBounds: unsupportedFn('WindowManagement.setWindowBounds'),
};
const confirmAlert = async (o = {}) =>
  !!(await bridge.call('alert.confirm', {
    title: String(o.title || ''), message: String(o.message || ''),
    primary: String((o.primaryAction && o.primaryAction.title) || 'OK'), dismiss: String((o.dismissAction && o.dismissAction.title) || 'Cancel'),
    style: (o.primaryAction && o.primaryAction.style) || 'default',
  }));
const openExtensionPreferences = unsupportedFn('openExtensionPreferences');
const openCommandPreferences = unsupportedFn('openCommandPreferences');
const updateCommandMetadata = async () => {};
const AI = { ask: unsupportedFn('AI.ask'), Model: {}, Creativity: { None: 0, Low: 0.25, Medium: 0.5, High: 0.75, Maximum: 1 } };
class OAuthStub { constructor() { throw new Error(unsupportedMsg('OAuth')); } }
const OAuth = { PKCEClient: OAuthStub, RedirectMethod: { Web: 'web', App: 'app', AppURI: 'appURI' } };
const Alert = { ActionStyle: { Default: 'default', Cancel: 'cancel', Destructive: 'destructive' } };
const Image = { Mask: { Circle: 'circle', RoundedRectangle: 'roundedRectangle' } };

// ---- navigation (the runtime owns the stack; see main.js)
function useNavigation() {
  return bridge.navigation;
}

module.exports = {
  List, Detail, ActionPanel, Action, Form, Grid, MenuBarExtra,
  Icon: iconNames, Color, Keyboard, Image, Alert, AI, OAuth, LaunchType, Toast,
  Clipboard, LocalStorage, Cache, environment,
  getPreferenceValues, showToast, showHUD, closeMainWindow, popToRoot, clearSearchBar,
  open, showInFinder, trash, getSelectedText, getSelectedFinderItems, getFrontmostApplication,
  getApplications, getDefaultApplication, launchCommand, PopToRootType, BrowserExtension, WindowManagement, captureException, confirmAlert, openExtensionPreferences, openCommandPreferences,
  updateCommandMetadata, useNavigation,
  __configure: configure,
};
Object.defineProperty(module.exports, '__configure', { enumerable: false });
