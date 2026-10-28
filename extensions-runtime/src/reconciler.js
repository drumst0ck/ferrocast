'use strict';
// react-reconciler host config: the "DOM" is a plain tree of {type, props, children}; after each
// commit the tree is turned into a JSON render tree (functions and React elements never travel).
const React = require('react');
const Reconciler = require('react-reconciler');
const { DefaultEventPriority, NoEventPriority } = require('react-reconciler/constants');

let priority = NoEventPriority;
const noop = () => {};

function detach(parent, child) {
  const i = parent.children.indexOf(child);
  if (i >= 0) parent.children.splice(i, 1);
}

const host = {
  supportsMutation: true,
  supportsPersistence: false,
  supportsHydration: false,
  supportsMicrotasks: true,
  isPrimaryRenderer: true,
  noTimeout: -1,
  scheduleTimeout: setTimeout,
  cancelTimeout: clearTimeout,
  scheduleMicrotask: queueMicrotask,
  // form items are given to `ref`s: extensions call focus() / reset() on them (the host owns the real focus)
  createInstance: (type, props) => ({ type, props, children: [], focus: noop, reset: noop, show: noop, hide: noop }),
  createTextInstance: (text) => ({ type: '#text', props: { text }, children: [] }),
  appendInitialChild: (parent, child) => parent.children.push(child),
  finalizeInitialChildren: () => false,
  shouldSetTextContent: () => false,
  getRootHostContext: () => ({}),
  getChildHostContext: (ctx) => ctx,
  getPublicInstance: (i) => i,
  prepareForCommit: () => null,
  resetAfterCommit: (container) => container.onCommit(),
  preparePortalMount: noop,
  appendChild(parent, child) {
    detach(parent, child);
    parent.children.push(child);
  },
  insertBefore(parent, child, before) {
    detach(parent, child);
    const i = parent.children.indexOf(before);
    if (i < 0) parent.children.push(child);
    else parent.children.splice(i, 0, child);
  },
  removeChild: detach,
  commitUpdate(instance, type, prev, next) {
    instance.props = next;
  },
  commitTextUpdate(instance, _old, text) {
    instance.props = { text };
  },
  clearContainer(container) {
    container.children = [];
  },
  resolveUpdatePriority: () => priority || DefaultEventPriority,
  setCurrentUpdatePriority: (p) => {
    priority = p;
  },
  getCurrentUpdatePriority: () => priority,
  NotPendingTransition: null,
  HostTransitionContext: React.createContext(null),
  resetFormInstance: noop,
  requestPostPaintCallback: noop,
  shouldAttemptEagerTransition: () => false,
  trackSchedulerEvent: noop,
  resolveEventType: () => null,
  resolveEventTimeStamp: () => -1.1,
  maySuspendCommit: () => false,
  preloadInstance: () => true,
  startSuspendingCommit: noop,
  suspendInstance: noop,
  waitForCommitToBeReady: () => null,
  detachDeletedInstance: noop,
  getInstanceFromNode: () => null,
  beforeActiveInstanceBlur: noop,
  afterActiveInstanceBlur: noop,
  prepareScopeUpdate: noop,
  getInstanceFromScope: () => null,
};
host.appendChildToContainer = host.appendChild;
host.insertInBeforeContainer = host.insertBefore;
host.removeChildFromContainer = host.removeChild;

const reconciler = Reconciler(host);

function createRoot(onCommit, onError) {
  const container = { children: [], onCommit, actions: new Map(), fields: new Map(), dropdowns: [], listProps: null };
  const fail = (e) => onError(e);
  const root = reconciler.createContainer(container, 1, null, false, null, '', fail, fail, fail, noop, null);
  return {
    container,
    render(element) {
      reconciler.updateContainer(element, root, null, noop);
    },
    unmount() {
      reconciler.updateContainer(null, root, null, noop);
    },
  };
}

// ---- JSON render tree
function clean(v, depth) {
  if (v === null || typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean') return v;
  if (depth > 6 || v === undefined) return undefined;
  if (v instanceof Date) return v.toISOString();
  if (typeof v !== 'object' || React.isValidElement(v)) return undefined;
  if (Array.isArray(v)) return v.map((x) => clean(x, depth + 1)).filter((x) => x !== undefined);
  const out = {};
  for (const [k, x] of Object.entries(v)) {
    const c = clean(x, depth + 1);
    if (c !== undefined) out[k] = c;
  }
  return out;
}

function serialize(inst, path, container) {
  if (inst.type === '#text') return { t: '#text', p: { text: String(inst.props.text) }, c: [] };
  const p = { _id: path };
  for (const [k, v] of Object.entries(inst.props)) {
    if (k === 'children' || k === 'key' || k === 'ref') continue;
    if (typeof v === 'function') p[k] = true;
    else {
      const c = clean(v, 0);
      if (c !== undefined) p[k] = c;
    }
  }
  if ((inst.type === 'List' || inst.type === 'Grid') && !container.listProps) container.listProps = inst.props;
  if (inst.type.startsWith('Form.') && typeof inst.props.id === 'string') container.fields.set(inst.props.id, inst);
  if (/^(List|Grid)\.Dropdown$/.test(inst.type)) {
    container.fields.set(path, inst);
    container.dropdowns.push(inst);
  }
  if (inst.type === 'Action' || inst.type.startsWith('Action.')) container.actions.set(path, inst);
  const c = inst.children.map((ch, i) => serialize(ch, `${path}.${i}`, container));
  return { t: inst.type, p, c };
}

function toTree(container) {
  container.actions = new Map();
  container.fields = new Map();
  container.dropdowns = [];
  container.listProps = null;
  return { t: '#root', p: {}, c: container.children.map((ch, i) => serialize(ch, String(i), container)) };
}

module.exports = { createRoot, toTree };
