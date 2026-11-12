import test from 'node:test';
import assert from 'node:assert/strict';
import { sharePaths, handleLaunch } from '../web/src/api/launch.js';

test('file delivery deduplicates paths and starts the stopped server after adding', async () => {
  const calls=[];
  const api={ addFile:async file=>calls.push(['add',file]), getServerStatus:async()=> 'stop', startServer:async()=>calls.push(['start']) };
  await sharePaths(api,['/tmp/a.txt','/tmp/a.txt','C:\\Users\\Public\\b.txt']);
  assert.deepEqual(calls,[['add',{name:'a.txt',path:'/tmp/a.txt'}],['add',{name:'b.txt',path:'C:\\Users\\Public\\b.txt'}],['start']]);
});

test('text launch and autoStart preserve upstream entry semantics', async () => {
  const calls=[];
  const api={ addText:async text=>calls.push(text),getServerStatus:async()=> 'stop',startServer:async()=>calls.push('start'),getSetting:async()=>({autoStart:true}) };
  await handleLaunch(api,{type:'over',payload:'shared text'});
  assert.deepEqual(calls,['shared text','start']);
  calls.length=0;
  await handleLaunch(api,null);
  assert.deepEqual(calls,['start']);
});
