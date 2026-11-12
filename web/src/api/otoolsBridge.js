export function createOtoolsApi(host = globalThis.window?.otools || globalThis.window?.utools) {
  const listeners = new Map();
  let subscription;
  let unlisten;

  const call = async (method, payload = null) => {
    if (!host?.invokeNative && !host?.invokeNativeRaw) throw new Error('请在 OTools 中打开此插件，浏览器预览不能启动本地文件服务');
    try {
      const raw = typeof host.invokeNativeRaw === 'function';
      const result = await (raw ? host.invokeNativeRaw(method, payload) : host.invokeNative(method, payload));
      if (raw && result?.ok === false) throw new Error(typeof result.error === 'object' ? result.error.message : result.error || '调用失败');
      if (raw && result?.ok === true && Object.hasOwn(result, 'data')) return result.data;
      return result;
    } catch (error) {
      if (error instanceof Error) throw error;
      throw new Error(error?.message || String(error));
    }
  };

  const ensureSubscription = () => {
    if (!subscription && typeof host?.listenNative === 'function') {
      subscription = host.listenNative(event => {
        const message = event?.payload?.topic ? event.payload : event;
        const type = message?.topic?.replace(/^fileshare:/, '');
        if (!type) return;
        for (const listener of listeners.get(type) || []) {
          Promise.resolve(listener({ type, data: message.payload })).catch(console.error);
        }
      }).then(dispose => { unlisten = dispose; });
    }
    return subscription || Promise.resolve();
  };

  const api = {
    ready: ensureSubscription,
    registryEventListener(type, listener) {
      if (!listeners.has(type)) listeners.set(type, new Set());
      listeners.get(type).add(listener);
      return () => listeners.get(type)?.delete(listener);
    },
    async dispose() { await subscription; await unlisten?.(); listeners.clear(); subscription = undefined; unlisten = undefined; },
    addFile: file => call('addFile', file),
    addText: (text, username) => call('addText', { text, username }),
    removeFile: file => call('removeFile', file),
    openFile: filename => call('openFile', { filename }),
    updateSetting: settings => call('updateSetting', settings),
    updateUploadPath: path => call('updateUploadPath', path),
    updateIp: ip => call('updateIp', ip),
    getToken: (permanent = true, timeoutSec = 3600) => call('getToken', { permanent, timeoutSec }),
    getNetInterfaceNames: (family = 'ipv4') => call('getNetInterfaceNames', { family }),
    getIpAddress: (idx = 0, family = 'ipv4') => call('getIpAddress', { idx, family }),
    getIpAddresses: (netInterfaceName, family = 'ipv4') => call('getIpAddresses', { netInterfaceName, family }),
    setIpFamily: ipFamily => call('setIpFamily', { ipFamily }),
    setNetInterface: netInterfaceName => call('setNetInterface', { netInterfaceName }),
    addAccount: account => call('addAccount', account),
    updateAccount: (id, account) => call('updateAccount', { id, account }),
    updateAccountPassword: (id, password) => call('updateAccountPassword', { id, password }),
    removeAccount: id => call('removeAccount', { id }),
    openExternal: url => host?.shellOpenExternal ? Promise.resolve(host.shellOpenExternal(url)) : call('openExternal', { url }),
  };
  for (const method of ['getSetting','getUrl','startServer','stopServer','getServerStatus','listFiles','getIp','getIpFamily','getNetInterface','getPlatform','getVersion','listAccounts','openDevTool','closeDevTool']) {
    api[method] = () => call(method);
  }
  return api;
}

if (typeof window !== 'undefined') window.api = createOtoolsApi();
