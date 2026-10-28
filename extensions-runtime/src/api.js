'use strict';
// Our implementation of the `@raycast/api` surface that Ferrocast supports. Everything it cannot do
// fails with "not supported in Ferrocast yet: <name>" instead of silently doing nothing.
const React = require('react');
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
      Copy: common('c', ['cmd', 'shift']), CopyName: common('.', ['cmd', 'shift']), CopyPath: common('c', ['cmd', 'opt']),
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
  get style() { return this._style; }
  set style(v) { this._style = v; this._send(); }
  get title() { return this._title; }
  set title(v) { this._title = v; this._send(); }
  get message() { return this._message; }
  set message(v) { this._message = v; this._send(); }
  async show() { this._send(); }
  async hide() {}
}
Toast.Style = ToastStyle;
Toast.ActionStyle = { Primary: 'primary', Destructive: 'destructive', Cancel: 'cancel' };
async function showToast(a, b, c) {
  const opts = typeof a === 'object' ? a : { style: a, title: b, message: c };
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
const asText = (c) => (c !== null && typeof c === 'object' ? c.text ?? c.file ?? '' : String(c));
const Clipboard = {
  copy: (content, options) => bridge.call('clipboard.copy', { text: asText(content), concealed: !!(options && options.concealed) }),
  paste: (content) => bridge.call('clipboard.paste', { text: asText(content) }),
  readText: async () => (await bridge.call('clipboard.readText', {})) || undefined,
  read: async () => ({ text: (await bridge.call('clipboard.readText', {})) || '' }),
  clear: () => bridge.call('clipboard.clear', {}),
};

// ---- storage
const LocalStorage = {
  getItem: (key) => bridge.call('localStorage.getItem', { key }),
  setItem: (key, value) => bridge.call('localStorage.setItem', { key, value }),
  removeItem: (key) => bridge.call('localStorage.removeItem', { key }),
  allItems: () => bridge.call('localStorage.allItems', {}),
  clear: () => bridge.call('localStorage.clear', {}),
};

// Cache is synchronous in Raycast's API, so it lives in the runtime (a JSON file in supportPath).
class Cache {
  constructor(opts = {}) {
    this._ns = (opts.namespace || 'default').replace(/[^\w.-]/g, '_');
    this._subs = new Set();
    this._data = null;
    // @raycast/utils hands these around detached (`useSyncExternalStore(cache.subscribe, ...)`)
    for (const m of ['has', 'get', 'set', 'remove', 'clear', 'subscribe']) this[m] = this[m].bind(this);
  }
  _file() { return path.join(environment.supportPath, `cache-${this._ns}.json`); }
  _load() {
    if (this._data) return this._data;
    try { this._data = JSON.parse(fs.readFileSync(this._file(), 'utf8')); } catch { this._data = {}; }
    return this._data;
  }
  _save(key, value) {
    try {
      fs.mkdirSync(environment.supportPath, { recursive: true });
      fs.writeFileSync(this._file(), JSON.stringify(this._data));
    } catch (e) { bridge.log(`cache write failed: ${e.message}`); }
    for (const f of this._subs) f(key, value);
  }
  get isEmpty() { return Object.keys(this._load()).length === 0; }
  has(key) { return key in this._load(); }
  get(key) { return this._load()[key]; }
  set(key, value) { this._load()[key] = value; this._save(key, value); }
  remove(key) { const had = delete this._load()[key]; this._save(key, undefined); return had; }
  clear() { this._data = {}; this._save(undefined, undefined); }
  subscribe(f) { this._subs.add(f); return () => this._subs.delete(f); }
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
const launchCommand = (o) =>
  bridge.call('launchCommand', {
    name: o && o.name, extensionName: o && o.extensionName, ownerOrAuthorName: o && o.ownerOrAuthorName,
    arguments: (o && o.arguments) || {}, type: (o && o.type) || 'userInitiated',
  });
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
