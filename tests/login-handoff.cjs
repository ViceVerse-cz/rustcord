// Offline VM test only. Node is never a messaging runtime dependency.
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const source = fs.readFileSync('crates/platform/src/login-handoff.js', 'utf8').replace('__SEREIN_LOGIN_CAPABILITY__', 'SYNTHETIC_CAPABILITY:');
function setup(origin = 'https://discord.com') {
  const messages = [];
  class XHR { open() {} setRequestHeader() {} }
  const context = { location: {origin, href: origin + '/login'}, XMLHttpRequest: XHR, Headers, Request, URL, Date, WeakMap };
  context.window = context; context.top = context;
  context.ipc = {postMessage: value => messages.push(value)};
  context.fetch = () => Promise.resolve();
  vm.runInNewContext(source, context);
  return {context, messages};
}
{
  const {context, messages} = setup();
  const xhr = new context.XMLHttpRequest();
  xhr.open('GET', 'https://evil.test/api/v10/users/@me');
  xhr.setRequestHeader('Authorization', 'SYNTHETIC_OTHER_ORIGIN');
  assert.equal(messages.length, 0);
  xhr.open('GET', '/api/v10/users/@me');
  xhr.setRequestHeader('Authorization', 'x'.repeat(2049));
  assert.equal(messages.length, 0);
  xhr.setRequestHeader('Authorization', 'SYNTHETIC_SESSION_MARKER');
  xhr.setRequestHeader('Authorization', 'SYNTHETIC_DUPLICATE_MARKER');
  assert.deepEqual(messages, ['SYNTHETIC_CAPABILITY:SYNTHETIC_SESSION_MARKER']);
}
{
  const {context, messages} = setup();
  context.fetch('https://discord.com/api/v10/users/@me', {headers:{authorization:'SYNTHETIC_FETCH_MARKER'}});
  assert.deepEqual(messages, ['SYNTHETIC_CAPABILITY:SYNTHETIC_FETCH_MARKER']);
}
{
  const {context, messages} = setup('https://evil.test');
  context.fetch('https://discord.com/api/v10/users/@me', {headers:{authorization:'SYNTHETIC_FETCH_MARKER'}});
  assert.equal(messages.length, 0);
}
console.log('Authentication handoff: origin, byte cap, one-shot and fetch/XHR checks passed (synthetic only).');


// WebKit6 does not identify the sender frame in its script-message callback.
// Its bridge sends only a boolean; native code queries the bounded main-frame slot.
const linuxCapability = 'a'.repeat(64) + ':';
const linuxBridge = fs.readFileSync('crates/platform/src/login-linux-bridge.js', 'utf8')
  .replace('__SEREIN_LOGIN_CAPABILITY__', linuxCapability);
const linuxHandoff = fs.readFileSync('crates/platform/src/login-handoff.js', 'utf8')
  .replace('__SEREIN_LOGIN_CAPABILITY__', linuxCapability);
function linuxSetup({ origin = 'https://discord.com', frame = false } = {}) {
  const messages = [];
  let now = 0;
  class XHR { open() {} setRequestHeader() {} }
  const context = {
    location: { origin, href: origin + '/login' }, XMLHttpRequest: XHR,
    Headers, Request, URL, WeakMap, Date: { now: () => now },
    webkit: { messageHandlers: { sereinLogin: { postMessage: value => messages.push(value) } } },
    fetch: () => Promise.resolve(),
  };
  context.window = context; context.top = frame ? {} : context;
  vm.createContext(context);
  vm.runInContext(linuxBridge, context);
  vm.runInContext(linuxHandoff, context);
  return { context, messages, expire: () => now = 600001,
    take: () => context['__serein_login_take_' + linuxCapability.slice(0, -1)]() };
}
{
  const { context, messages, take } = linuxSetup();
  for (const value of [null, true, {}, 'wrong:' + 'x'.repeat(16), linuxCapability + 'x'.repeat(2049),
      linuxCapability + 'short', linuxCapability + ' '.repeat(16), linuxCapability + '\u00e9'.repeat(16)]) {
    context.ipc.postMessage(value);
  }
  assert.equal(messages.length, 0);
  assert.equal(take(), null);
  const ipc = context.ipc;
  vm.runInContext('window.ipc = {}; window.ipc.postMessage = () => {}', context);
  assert.equal(context.ipc, ipc);
  assert.equal(Object.getOwnPropertyDescriptor(context, 'ipc').configurable, false);
  assert.equal(Object.getOwnPropertyDescriptor(context, '__serein_login_take_' + linuxCapability.slice(0, -1)).writable, false);
  context.fetch('/api/v10/users/@me', { headers: { authorization: 'SYNTHETIC_LINUX_SESSION' } });
  assert.deepEqual(messages, [true]);
  assert.equal(take(), linuxCapability + 'SYNTHETIC_LINUX_SESSION');
  assert.equal(take(), null);
  context.ipc.postMessage(linuxCapability + 'SYNTHETIC_DUPLICATE_SESSION');
  assert.deepEqual(messages, [true]);
}
for (const options of [{origin: 'https://evil.test'}, {frame: true}]) {
  const { context, messages } = linuxSetup(options);
  assert.equal(context.ipc, undefined);
  assert.equal(messages.length, 0);
}
{
  const { context, messages, expire, take } = linuxSetup();
  context.ipc.postMessage(linuxCapability + 'SYNTHETIC_EXPIRED_SESSION');
  expire();
  assert.equal(take(), null);
  assert.deepEqual(messages, [true]);
}
{
  const { context, messages, take } = linuxSetup();
  context.ipc.postMessage(linuxCapability + 'SYNTHETIC_NAVIGATED_SESSION');
  context.location.origin = 'https://evil.test';
  assert.equal(take(), null);
  context.location.origin = 'https://discord.com';
  assert.equal(take(), null);
  assert.deepEqual(messages, [true]);
}
console.log('Linux bridge: main-frame origin, ASCII byte cap, protected one-shot slot, expiry and token-free wake checks passed (synthetic only).');
