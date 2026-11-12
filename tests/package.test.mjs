import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..',import.meta.url));
test('native plugin package contains receiver assets and exact native metadata', () => {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(),'fileshare-package-test-'));
  try {
    fs.mkdirSync(path.join(temporary,'scripts'));
    for (const name of ['pack-plugin.mjs','verify-plugin.mjs']) fs.copyFileSync(path.join(root,'scripts',name),path.join(temporary,'scripts',name));
    const sourceManifest = JSON.parse(fs.readFileSync(path.join(root, 'plugin.json'), 'utf8'));
    fs.writeFileSync(path.join(temporary,'plugin.json'),JSON.stringify({packid:'otools-fileshare',uuid:'otools-fileshare',version:'0.1.0',entry:'dist/index.html',native:{enabled:true},openMode:sourceManifest.openMode}));
    fs.writeFileSync(path.join(temporary,'logo.svg'),'<svg xmlns="http://www.w3.org/2000/svg"/>');
    for (const directory of ['dist','lib','web-static/page_web']) fs.mkdirSync(path.join(temporary,directory),{recursive:true});
    fs.writeFileSync(path.join(temporary,'dist/index.html'),'window.otools');
    fs.writeFileSync(path.join(temporary,'web-static/page_web/index.html'),'<html>mobile</html>');
    const binary=Buffer.alloc(128); binary.writeUInt32LE(0xfeedfacf,0);
    fs.writeFileSync(path.join(temporary,'lib/macOS.dylib'),binary);
    const packed=spawnSync(process.execPath,[path.join(temporary,'scripts/pack-plugin.mjs')],{encoding:'utf8'});
    assert.equal(packed.status,0,packed.stderr);
    const packedManifest=JSON.parse(fs.readFileSync(path.join(temporary,'dist-pack/plugin.json'),'utf8'));
    assert.equal(packedManifest.openMode,'panel');
    const meta=JSON.parse(fs.readFileSync(path.join(temporary,'dist-pack/meta.json')));
    assert.deepEqual(meta.nativeLibs,['lib/macOS.dylib']);
    assert.equal(meta.entries,5);
    const verified=spawnSync(process.execPath,[path.join(temporary,'scripts/verify-plugin.mjs'),'dist-pack'],{encoding:'utf8'});
    assert.equal(verified.status,0,verified.stderr);
  } finally { fs.rmSync(temporary,{recursive:true,force:true}); }
});

test('per-arch packing keeps only its own library and suffixes the artifact name', () => {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(),'fileshare-package-test-'));
  try {
    fs.mkdirSync(path.join(temporary,'scripts'));
    for (const name of ['pack-plugin.mjs','verify-plugin.mjs']) fs.copyFileSync(path.join(root,'scripts',name),path.join(temporary,'scripts',name));
    fs.writeFileSync(path.join(temporary,'plugin.json'),JSON.stringify({packid:'otools-fileshare',uuid:'otools-fileshare',version:'0.1.3',entry:'dist/index.html',native:{enabled:true},openMode:'panel'}));
    fs.writeFileSync(path.join(temporary,'logo.svg'),'<svg xmlns="http://www.w3.org/2000/svg"/>');
    for (const directory of ['dist','web-static/page_web']) fs.mkdirSync(path.join(temporary,directory),{recursive:true});
    fs.writeFileSync(path.join(temporary,'dist/index.html'),'window.otools');
    fs.writeFileSync(path.join(temporary,'web-static/page_web/index.html'),'<html>mobile</html>');
    fs.mkdirSync(path.join(temporary,'lib'));
    const binary=Buffer.alloc(128); binary.writeUInt32LE(0xfeedfacf,0);
    fs.writeFileSync(path.join(temporary,'lib/macOS-arm64.dylib'),binary);
    const packed=spawnSync(process.execPath,[path.join(temporary,'scripts/pack-plugin.mjs'),'--platform','macos','--arch','arm64'],{encoding:'utf8'});
    assert.equal(packed.status,0,packed.stderr);
    const meta=JSON.parse(fs.readFileSync(path.join(temporary,'dist-pack/macos/meta.json')));
    assert.equal(meta.platform,'macos');
    assert.deepEqual(meta.nativeLibs,['lib/macOS.dylib']);
    const verified=spawnSync(process.execPath,[path.join(temporary,'scripts/verify-plugin.mjs'),'dist-pack/macos'],{encoding:'utf8'});
    assert.equal(verified.status,0,verified.stderr);
  } finally { fs.rmSync(temporary,{recursive:true,force:true}); }
});
