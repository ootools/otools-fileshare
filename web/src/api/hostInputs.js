import { sharePaths, handleLaunch } from './launch.js';

export async function subscribeHostInputs(api, { listenTauri, invokeTauri, pluginUuid, search = '' }) {
  const cleanup=[]; const seen=new Set();
  let disposed=false;
  const consumeFiles=async()=>{
    if(disposed || !pluginUuid)return;
    const paths=await invokeTauri('otools_consume_open_files',{pluginUuid});
    if(Array.isArray(paths))await sharePaths(api,paths);
  };
  const consumeKeywords=async()=>{
    if(disposed || !pluginUuid)return;
    const items=await invokeTauri('otools_consume_keyword_dispatch',{pluginUuid});
    for(const item of Array.isArray(items)?items:[]){
      if(!item?.id || seen.has(item.id))continue;
      seen.add(item.id);
      if(typeof item.payload==='string')await handleLaunch(api,{type:'over',payload:item.payload});
      else if(item.payload?.type)await handleLaunch(api,item.payload);
    }
  };
  try {
    cleanup.push(await listenTauri('otools-open-file',consumeFiles));
    cleanup.push(await listenTauri('otools-keyword-dispatch',consumeKeywords));
    cleanup.push(await listenTauri('tauri://drag-drop',async event=>{
      if(event.payload?.paths?.length)await sharePaths(api,event.payload.paths);
    }));
    const raw=new URLSearchParams(search).get('otoolsOpenFile');
    if(raw){try {const paths=JSON.parse(raw);if(Array.isArray(paths))await sharePaths(api,paths);}catch{}}
    await consumeFiles();
    await consumeKeywords();
  } catch(error) {
    for(const dispose of cleanup)await dispose?.();
    throw error;
  }
  return async()=>{disposed=true;for(const dispose of cleanup)await dispose?.();};
}
