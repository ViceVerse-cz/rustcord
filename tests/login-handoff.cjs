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
