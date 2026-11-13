function w() {
  var o, e, n, t, i;
  return typeof window > "u" ? "unknown" : ((e = (o = window.otools) == null ? void 0 : o.getAppVersion) == null ? void 0 : e.call(o)) ?? ((t = (n = window.utools) == null ? void 0 : n.getAppVersion) == null ? void 0 : t.call(n)) ?? ((i = window.__OToolsEnv) == null ? void 0 : i.appVersion) ?? "unknown";
}
async function r() {
  return w();
}
async function s() {
  var o, e, n, t, i;
  return typeof window > "u" ? "OTools" : ((e = (o = window.otools) == null ? void 0 : o.getAppName) == null ? void 0 : e.call(o)) ?? ((t = (n = window.utools) == null ? void 0 : n.getAppName) == null ? void 0 : t.call(n)) ?? ((i = window.__OToolsEnv) == null ? void 0 : i.appName) ?? "OTools";
}
export {
  s as getName,
  r as getVersion
};
//# sourceMappingURL=tauri-app-shim.js.map
