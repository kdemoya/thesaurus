// Unit harness for recovery rendering and IPC behavior, not browser automation.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const source = fs.readFileSync(path.join(__dirname, '../apps/ui/recovery/recovery.js'), 'utf8');
const conflict = {
  phase: 'other_desktop', headline: 'Another desktop is connected',
  detail: 'Existing recovery detail', can_start: false, can_enable_windows: false,
};
const settle = () => new Promise(resolve => setImmediate(resolve));

function harness(query = '', native = false, invokeOverride) {
  const nodes = new Map();
  const calls = [], timers = [], events = new Map();
  const actionIds = ['retry', 'start', 'enable', 'quit'];
  const get = id => {
    if (!nodes.has(id)) nodes.set(id, {
      hidden: ['other-desktop-help', 'error', 'start', 'enable'].includes(id),
      disabled: false, textContent: '', attributes: {}, listeners: {},
      addEventListener(event, fn) { this.listeners[event] = fn; },
      setAttribute(name, value) { this.attributes[name] = value; },
      querySelectorAll() { return actionIds.map(get); },
    });
    return nodes.get(id);
  };
  const document = {getElementById: get, documentElement: {dataset: {}}, hidden: false};
  const location = new URL(`http://127.0.0.1/recovery/recovery.html${query}`);
  let replacedUrl = null;
  const bridge = {
    core: {invoke: async (command, args) => {
      calls.push([command, args.action]);
      return invokeOverride ? invokeOverride(args.action) : conflict;
    }},
    event: {listen: async (name, fn) => events.set(name, fn)},
  };
  const context = {
    document, location, URL, URLSearchParams,
    history: {replaceState: (_, __, url) => {replacedUrl = String(url);}},
    localStorage: {getItem: () => null},
    matchMedia: () => ({matches: true, addEventListener() {}}),
    window: {addEventListener() {}, ...(native ? {__TAURI__: bridge} : {})},
    setInterval: fn => {timers.push(fn);},
  };
  vm.runInNewContext(source, context, {filename: 'recovery.js'});
  return {get, document, calls, timers, events, click: id => get(id).listeners.click(), url: () => replacedUrl};
}

test('native conflict shows recovery guidance and runs retry', async () => {
  const app = harness('', true); await settle();
  assert.equal(app.get('other-desktop-help').hidden, false);
  assert.match(app.get('detail').textContent, /another desktop connection/);
  assert.deepEqual(app.calls, [['recovery_action', 'status']]);
  await app.click('retry');
  assert.ok(app.calls.some(([, action]) => action === 'retry'));
});

test('plain browser view exposes no native recovery actions', () => {
  const app = harness();
  assert.equal(app.get('recovery-actions').hidden, true);
  assert.equal(app.get('heading').textContent, 'Open the Thesaurus desktop app');
  assert.equal(app.calls.length, 0);
  assert.equal(app.timers.length, 0);
});

test('help follows native status rather than retaining stale conflict advice', async () => {
  const app = harness('', true); await settle();
  assert.equal(app.get('other-desktop-help').hidden, false);
  app.events.get('desktop-recovery')({payload: {
    phase: 'disconnected', headline: 'Service unavailable', detail: 'Start your service',
    can_start: true, can_enable_windows: false,
  }});
  assert.equal(app.get('other-desktop-help').hidden, true);
  assert.equal(app.get('detail').textContent, 'Start your service');
  assert.equal(app.get('start').hidden, false);
});

test('native action failure restores controls and presents the error', async () => {
  const app = harness('', true, action => {
    if (action === 'retry') throw new Error('Service unavailable');
    return conflict;
  });
  await settle(); await app.click('retry');
  assert.equal(app.get('error').hidden, false);
  assert.match(app.get('error').textContent, /Service unavailable/);
  for (const id of ['retry', 'quit']) assert.equal(app.get(id).disabled, false);
  assert.match(app.get('hint').textContent, /continue independently/);
});
