import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';

const require = createRequire(new URL('../vendor/otools-plugin-sdk/package.json', import.meta.url));
const { JSDOM } = require('jsdom');
const { pickFromHostTree } = await import('../vendor/otools-plugin-sdk/dist/index.js');

const home = '/home/test';
const docs = `${home}/docs`;
const file = { name: 'one.txt', path: `${home}/one.txt`, kind: 'file', size: 3, mime: 'text/plain' };
const directory = { name: 'docs', path: docs, kind: 'directory', size: 0, mime: 'inode/directory' };

async function waitFor(check) {
  for (let attempt = 0; attempt < 100; attempt++) {
    if (check()) return;
    await delay(5);
  }
  assert.fail('File chooser did not reach the expected visible state');
}

function setup() {
  const dom = new JSDOM('<!doctype html><html><body></body></html>', { url: 'http://localhost/' });
  const original = new Map(['window', 'document', 'Element'].map(key => [key, Object.getOwnPropertyDescriptor(globalThis, key)]));
  Object.assign(globalThis, { window: dom.window, document: dom.window.document, Element: dom.window.Element });
  let nativeCalls = 0;
  const calls = [];
  window.otools = { invokeNative: async () => { nativeCalls++; throw new Error('Filesystem request reached plugin native'); } };
  window.__TAURI_INTERNALS__ = { invoke: async (command, payload) => {
    calls.push([command, payload]);
    assert.equal(command, 'tools_webview_browse_dialog');
    const requested = payload?.request?.path;
    assert.equal(typeof requested, 'string');
    const path = requested || home;
    assert.ok([home, docs].includes(path));
    return { currentPath: path, parentPath: path === docs ? home : null, roots: [], entries: path === home ? [file, directory] : [] };
  } };
  const row = path => [...document.querySelectorAll('[data-host-tree-entry]')].find(entry => entry.getAttribute('data-path') === path);
  return { row, calls, get nativeCalls() { return nativeCalls; }, cleanup() {
    document.querySelector('[data-host-tree-cancel]')?.click();
    dom.window.close();
    for (const [key, descriptor] of original) {
      if (descriptor) Object.defineProperty(globalThis, key, descriptor);
      else delete globalThis[key];
    }
  } };
}

test('packaged SDK opens files through panel-safe host commands and can reopen for folders', async () => {
  const fixture = setup();
  try {
    const picked = pickFromHostTree({ title: '添加文件' });
    await waitFor(() => fixture.row(file.path));
    const fileRow = fixture.row(file.path);
    const unselectedBackground = window.getComputedStyle(fileRow).backgroundColor;
    fileRow.click();
    assert.equal(fileRow.getAttribute('aria-selected'), 'true');
    assert.notEqual(window.getComputedStyle(fileRow).backgroundColor, unselectedBackground);
    assert.equal(fileRow.querySelector('[data-host-tree-check]').hidden, false);
    assert.equal(document.querySelector('[data-host-tree-selection]').textContent, '已选 1 个文件');
    document.querySelector('[data-host-tree-confirm]').click();
    assert.deepEqual(await picked, { kind: 'picked', source: 'host-tree', paths: [file.path], names: ['one.txt'] });
    assert.equal(document.querySelector('[data-host-tree]'), null);
    assert.equal(fixture.nativeCalls, 0);

    const folder = pickFromHostTree({ title: '添加文件夹', directory: true });
    await waitFor(() => fixture.row(docs));
    fixture.row(docs).click();
    await waitFor(() => document.querySelector('.otools-host-tree-path')?.textContent === docs);
    document.querySelector('[data-host-tree-confirm]').click();
    assert.deepEqual(await folder, { kind: 'picked', source: 'host-tree', paths: [docs], names: ['docs'] });
    assert.deepEqual(fixture.calls.at(-1), ['tools_webview_browse_dialog', { request: { path: docs } }]);
  } finally { fixture.cleanup(); }
});
