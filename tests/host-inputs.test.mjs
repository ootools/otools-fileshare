import test from 'node:test';
import assert from 'node:assert/strict';
import { subscribeHostInputs } from '../web/src/api/hostInputs.js';
import { encodeSharePath, decodeSharePath } from '../web-mobile/src/utils/sharePath.js';

test('host Tauri delivery knocks and drops reach file sharing without native-topic routing', async () => {
  const listeners=new Map(); const calls=[]; const pending=['/tmp/first.txt'];
  const api={addFile:async file=>calls.push(['file',file.path]),addText:async text=>calls.push(['text',text]),getServerStatus:async()=> 'start',startServer:async()=>{},getSetting:async()=>({autoStart:false})};
  let keywords=[];
  const invokeTauri=async (method,payload)=>{
    assert.equal(payload.pluginUuid,'otools-fileshare');
    if(method==='otools_consume_open_files')return pending.splice(0);
    if(method==='otools_consume_keyword_dispatch'){ const value=keywords; keywords=[]; return value; }
    throw new Error(method);
  };
  const cleanup=await subscribeHostInputs(api,{pluginUuid:'otools-fileshare',search:'',invokeTauri,listenTauri:async(topic,handler)=>{listeners.set(topic,handler);return()=>listeners.delete(topic);}});
  assert.deepEqual(calls,[['file','/tmp/first.txt']]);
  pending.push('/tmp/later.txt');
  await listeners.get('otools-open-file')({});
  await listeners.get('tauri://drag-drop')({payload:{paths:['/tmp/drop.txt']}});
  keywords=[{id:'one',keyword:'share',payload:'hello from host'}];
  await listeners.get('otools-keyword-dispatch')({});
  assert.deepEqual(calls,[['file','/tmp/first.txt'],['file','/tmp/later.txt'],['file','/tmp/drop.txt'],['text','hello from host']]);
  await cleanup();
  assert.equal(listeners.size,0);
});

test('breadcrumb navigation preserves hash question and percent characters in names',()=>{
  const path=encodeSharePath(['a#b','c?d','50%','中文']);
  assert.equal(path,'/a%23b/c%3Fd/50%25/%E4%B8%AD%E6%96%87');
  assert.deepEqual(decodeSharePath(path),['a#b','c?d','50%','中文']);
});
