import test from 'node:test';
import assert from 'node:assert/strict';
import { uploadWithResume } from '../web-mobile/src/utils/resumableUpload.js';

test('mobile upload queries previous offset and sends only remaining chunks', async () => {
  const storage = new Map();
  const persistence = { getItem: key => storage.get(key), setItem: (key, value) => storage.set(key, value), removeItem: key => storage.delete(key) };
  const calls=[];
  const file = new Blob(['0123456789']);
  file.name='resume.txt'; file.lastModified=10;
  let creations=0;
  let offset=0;
  const fetcher=async (url,options={}) => {
    if (options.method==='POST') { creations++; return new Response(null,{status:201,headers:{location:'/api/uploads/id1'}}); }
    if (options.method==='HEAD') return new Response(null,{status:200,headers:{'upload-offset':String(offset),'upload-length':'10'}});
    if (options.method==='PATCH') { const text=await options.body.text(); calls.push({offset:options.headers['Upload-Offset'],text}); offset+=text.length; return new Response(null,{status:204,headers:{'upload-offset':String(offset)}}); }
    throw new Error(`unexpected ${url}`);
  };
  await uploadWithResume(file,{fetcher,storage:persistence,chunkSize:4,token:'token',onProgress:()=>{}});
  assert.equal(creations,1);
  assert.deepEqual(calls,[{offset:'0',text:'0123'},{offset:'4',text:'4567'},{offset:'8',text:'89'}]);
  assert.equal(storage.size,0);
});

test('reselecting a partially uploaded file resumes from persisted offset', async () => {
  const file = new Blob(['0123456789']); file.name='resume.txt'; file.lastModified=10;
  const map=new Map([[`fileshare-upload:resume.txt:10:10`,'/api/uploads/existing']]);
  const storage={getItem:key=>map.get(key),setItem:(key,value)=>map.set(key,value),removeItem:key=>map.delete(key)};
  const sent=[];
  let offset=5;
  const fetcher=async (url,options) => {
    assert.equal(url,'/api/uploads/existing');
    if(options.method==='HEAD')return new Response(null,{status:200,headers:{'Upload-Length':'10','Upload-Offset':String(offset)}});
    assert.equal(options.method,'PATCH');
    assert.equal(options.headers['Upload-Offset'],String(offset));
    const chunk=await options.body.text(); sent.push(chunk); offset+=chunk.length;
    return new Response(null,{status:204,headers:{'Upload-Offset':String(offset)}});
  };
  await uploadWithResume(file,{fetcher,storage,chunkSize:4,token:''});
  assert.deepEqual(sent,['5678','9']);
  assert.equal(map.size,0);
});

test('completed persisted session clears without entering an endless 409 loop', async () => {
  const file=new Blob(['0123456789']); file.name='complete.txt'; file.lastModified=10;
  const map=new Map([[`fileshare-upload:complete.txt:10:10`,'/api/uploads/complete']]);
  const storage={getItem:key=>map.get(key),setItem:(key,value)=>map.set(key,value),removeItem:key=>map.delete(key)};
  let patches=0;
  const fetcher=async (url,options) => {
    if(options.method==='HEAD')return new Response(null,{status:200,headers:{'Upload-Length':'10','Upload-Offset':'10'}});
    patches++;
    return new Response(null,{status:patches>1?500:409,headers:{'Upload-Offset':'10'}});
  };
  await uploadWithResume(file,{fetcher,storage,chunkSize:4,token:''});
  assert.equal(patches,0);
  assert.equal(map.size,0);
});
