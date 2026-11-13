function o() {
  if (!(typeof window > "u"))
    return window.otools ?? window.utools;
}
async function t(e) {
  const n = o();
  if (n != null && n.shellOpenExternal) {
    n.shellOpenExternal(e);
    return;
  }
  typeof window < "u" && window.open(e, "_blank", "noopener,noreferrer");
}
async function r(e) {
  const n = o();
  if (n != null && n.shellOpenPath) {
    n.shellOpenPath(e);
    return;
  }
}
async function i(e) {
  const n = o();
  if (n != null && n.shellShowItemInFolder) {
    n.shellShowItemInFolder(e);
    return;
  }
  await r(e);
}
export {
  r as openPath,
  t as openUrl,
  i as revealItemInDir
};
//# sourceMappingURL=tauri-plugin-opener-shim.js.map
