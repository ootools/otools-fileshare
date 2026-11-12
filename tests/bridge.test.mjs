import test from 'node:test';
import assert from 'node:assert/strict';
import { createOtoolsApi } from '../web/src/api/otoolsBridge.js';

test('bridge preserves upstream arguments and unwraps raw and host-normalized results', async () => {
  const calls = [];
  const host = { async invokeNativeRaw(method, payload) { calls.push({ method, payload }); return { ok: true, data: method === 'getSetting' ? { port: 5421 } : { success: true } }; } };
  const api = createOtoolsApi(host);
  assert.deepEqual(await api.getSetting(), { port: 5421 });
  await api.updateAccount('id1', { username: 'alice', password: '' });
  assert.deepEqual(calls[1], { method: 'updateAccount', payload: { id: 'id1', account: { username: 'alice', password: '' } } });
  await api.addText('你好', 'local');
  assert.deepEqual(calls[2], { method: 'addText', payload: { text: '你好', username: 'local' } });
  const normalized = createOtoolsApi({ async invokeNative() { return ['file']; } });
  assert.deepEqual(await normalized.listFiles(), ['file']);
});

test('bridge normalizes object native failures into usable message errors', async () => {
  const api = createOtoolsApi({ async invokeNativeRaw() { return { ok: false, error: { success: false, message: '账号不存在' } }; } });
  await assert.rejects(api.removeAccount('id'), /账号不存在/);
});

test('single native event subscription fans out and can be disposed', async () => {
  let handler;
  let subscriptions = 0;
  let disposed = 0;
  const host = { async invokeNativeRaw() { return { ok: true, data: null }; }, async listenNative(callback) { handler = callback; subscriptions++; return () => disposed++; } };
  const api = createOtoolsApi(host);
  const received = [];
  const off = api.registryEventListener('fileDb.listChange', event => received.push(event));
  api.registryEventListener('server.statusChange', event => received.push(event));
  await api.ready();
  assert.equal(subscriptions, 1);
  handler({ payload: { topic: 'fileshare:server.statusChange', payload: { status: 'start' } } });
  handler({ payload: { topic: 'fileshare:fileDb.listChange', payload: null } });
  assert.deepEqual(received, [{ type: 'server.statusChange', data: { status: 'start' } }, { type: 'fileDb.listChange', data: null }]);
  off();
  await api.dispose();
  assert.equal(disposed, 1);
});
