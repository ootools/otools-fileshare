import { attachTransformListener as G, detachTransformListener as V, listenNativeTopic as z } from "./native-event-bridge.js";
import { ensurePopupManager as X } from "./popup-manager.js";
const q = "otools:", Q = "otools-tauri:", J = { kind: "Any" }, y = /* @__PURE__ */ new Set(["crop-images-updated", "otools:custom-open"]), K = /* @__PURE__ */ new Set([
  "set_window_theme",
  "update_tray_menu"
]), Y = /* @__PURE__ */ new Set([
  "open_commit_window",
  "open_merge_window",
  "open_settings_window",
  "open_stash_window",
  "open_push_window",
  "open_project_boot_window"
]), E = Symbol(
  "shared-host-command-not-handled"
), W = /^(get|save|patch)_otools_plugin_(localstate|syncstate)(_value)?(_with_scheme)?$/, v = () => typeof window < "u", I = () => {
  try {
    return typeof __OTOOLS_PLUGIN_UUID__ == "string" ? S(__OTOOLS_PLUGIN_UUID__) : "";
  } catch {
    return "";
  }
}, O = (t) => t.startsWith("plugin:"), D = (t) => W.test(t), Z = "otools_host_", tt = /* @__PURE__ */ new Set([
  "otools_copy_text",
  "otools_ai_switch_status",
  "otools_ai_switch_models",
  "otools_ai_embeddings",
  "otools_ai_speech",
  "otools_ai_transcribe",
  "otools_ai_images",
  "otools_ai_video_submit",
  "otools_ai_video_status",
  "otools_ai_video_content",
  "otools_ai_generate_text",
  "otools_ai_load_chat_history",
  "otools_ai_save_chat_history",
  "hide_panel_window",
  "detach_tools_tab_to_window",
  "open_custom_plugin_window",
  "list_custom_plugin_windows",
  "close_custom_plugin_windows",
  "custom_plugin_window_ready",
  "tools_webview_browse_dialog",
  "tools_webview_list_dir",
  "tools_webview_read_file"
]), b = (t) => t.startsWith(Z) || tt.has(t), S = (t) => {
  const e = String(t ?? "").trim();
  if (!e)
    return "";
  const n = e.lastIndexOf(":");
  if (n <= 0 || n >= e.length - 1)
    return e;
  const i = e.slice(0, n).trim().toLowerCase(), o = e.slice(n + 1).trim();
  return o && (i === "builtin" || i === "market" || i === "dev-debug" || i === "dev-workspace") ? o : e;
}, et = (t) => {
  const e = [], n = String(t ?? "");
  if (n && e.push(n), t && typeof t == "object") {
    const i = t, o = String(i.message ?? ""), s = String(i.code ?? ""), r = String(i.detail ?? "");
    o && e.push(o), s && e.push(s), r && e.push(r);
  }
  return e.join(" | ");
}, A = (t) => {
  const e = et(t).toLowerCase();
  return e ? e.includes("not allowed") || e.includes("command not found") || e.includes("unknown method") || e.includes("unknown command") : !1;
}, nt = (t) => A(t), d = () => {
  if (v())
    for (const t of j()) {
      const e = t.otools ?? t.utools;
      if (e)
        return e;
    }
}, $ = () => {
  var t;
  return typeof ((t = d()) == null ? void 0 : t.invokeNative) == "function";
}, j = () => {
  if (!v())
    return [];
  const t = [
    window
  ];
  for (const e of [() => window.parent, () => window.top])
    try {
      const n = e();
      if (!n || n === window || t.includes(n))
        continue;
      t.push(n);
    } catch {
    }
  return t;
}, B = () => {
  if (!v())
    return !1;
  try {
    const t = window.__OToolsEnv;
    if (typeof (t == null ? void 0 : t.pluginUuid) == "string" && t.pluginUuid.trim())
      return !0;
    const e = new URLSearchParams(window.location.search || "");
    return !!(e.get("plugin") || e.get("pluginUuid") || e.get("plugin_uuid"));
  } catch {
    return !1;
  }
}, C = async (t = 1200) => {
  if (!v())
    return;
  const e = Date.now() + t;
  for (; Date.now() <= e; ) {
    for (const n of j()) {
      const i = n.otools ?? n.utools;
      if (typeof (i == null ? void 0 : i.invokeNative) == "function" || typeof (i == null ? void 0 : i.invokeNativePlugin) == "function")
        return i;
    }
    await new Promise((n) => {
      window.setTimeout(n, 20);
    });
  }
  return d();
}, L = (t) => {
  var n;
  const e = S((n = t == null ? void 0 : t.getPluginUuid) == null ? void 0 : n.call(t));
  if (e)
    return e;
  if (!v())
    return I();
  try {
    const i = window.__OToolsEnv, o = S(i == null ? void 0 : i.pluginUuid);
    if (o)
      return o;
    const s = new URLSearchParams(window.location.search || ""), r = S(
      s.get("plugin") || s.get("pluginUuid") || s.get("plugin_uuid") || ""
    );
    if (r)
      return r;
  } catch {
  }
  return I();
}, _ = () => {
  if (v())
    return window.__TAURI_INTERNALS__;
}, it = (t) => `${q}${t}`, P = (t) => `${Q}${t}`, R = (t, e) => {
  v() && window.dispatchEvent(
    new CustomEvent(it(t), {
      detail: { payload: e }
    })
  );
}, rt = (t, e) => {
  v() && window.dispatchEvent(
    new CustomEvent(P(t), {
      detail: { payload: e }
    })
  );
}, h = (t) => !t || typeof t != "object" || Array.isArray(t) ? null : t, g = (t, e, n, i = !1) => {
  var s;
  const o = (s = h(e)) == null ? void 0 : s[n];
  if (typeof o != "string" || !i && !o.trim())
    throw new Error(`${t} requires ${n}`);
  return o;
}, ot = (t) => {
  const e = atob(t), n = new Uint8Array(e.length);
  for (let i = 0; i < e.length; i += 1)
    n[i] = e.charCodeAt(i);
  return new TextDecoder().decode(n);
}, st = (t) => {
  const e = new TextEncoder().encode(t), n = [], i = 32768;
  for (let o = 0; o < e.length; o += i)
    n.push(
      String.fromCharCode(...e.subarray(o, o + i))
    );
  return btoa(n.join(""));
}, ut = async (t, e, n) => {
  var i, o;
  if (e === "tools_webview_list_dir" && typeof t.listHostDir == "function")
    return t.listHostDir(g(e, n, "path"));
  if (e === "tools_webview_read_file" && typeof t.readHostFile == "function")
    return t.readHostFile(g(e, n, "path"));
  if (e === "tools_webview_browse_dialog" && typeof t.browseHostDir == "function" && !((i = _()) != null && i.invoke)) {
    const s = h((o = h(n)) == null ? void 0 : o.request);
    return t.browseHostDir(String((s == null ? void 0 : s.path) ?? "").trim());
  }
  if (e === "read_directory_recursive" && typeof t.listHostDir == "function") {
    const s = g(e, n, "path");
    return (await t.listHostDir(s)).filter((a) => !a.name.startsWith(".")).map((a) => {
      const l = a.kind === "directory";
      return {
        id: a.path,
        label: a.name,
        path: a.path,
        is_directory: l,
        isLeaf: !l
      };
    });
  }
  if (e === "read_file_content" && typeof t.readHostFile == "function") {
    const s = g(e, n, "filePath"), r = await t.readHostFile(s);
    return ot(r.dataBase64);
  }
  if (e === "write_file_content" && typeof t.writeHostFile == "function") {
    const s = g(e, n, "filePath"), r = g(e, n, "content", !0);
    return t.writeHostFile({
      path: s,
      dataBase64: st(r)
    });
  }
  if (e === "browse_dialog" && typeof t.browseHostDir == "function") {
    const s = String(
      (n == null ? void 0 : n.path) ?? ""
    ).trim();
    return t.browseHostDir(s);
  }
  return E;
}, at = async (t, e, n) => {
  const i = W.exec(e);
  if (!i)
    return E;
  const [, o, s, r, a] = i, l = `${o}Plugin${s === "localstate" ? "Local" : "Sync"}State${r ? "Value" : ""}`, c = t[l];
  if (typeof c != "function")
    return E;
  const u = h(n), f = typeof (u == null ? void 0 : u.plugin) == "string" ? u.plugin : void 0, T = a && typeof (u == null ? void 0 : u.scheme) == "string" ? u.scheme : null, p = [f];
  return o === "get" ? r && p.push(u == null ? void 0 : u.key) : o === "patch" ? p.push((u == null ? void 0 : u.patch) ?? {}) : r ? p.push(u == null ? void 0 : u.key, (u == null ? void 0 : u.value) ?? null) : p.push((u == null ? void 0 : u.state) ?? null), p.push(T), c.apply(
    t,
    p
  );
}, U = (t, e, n) => {
  const i = h(e);
  t === "switch_provider" && (i != null && i.app) && (i != null && i.id) && R("provider-switched", {
    appType: i.app,
    providerId: i.id,
    source: "native-plugin",
    result: n
  }), t === "sync_universal_provider" && (i != null && i.id) && R("universal-provider-synced", {
    id: i.id,
    result: n
  });
}, w = async (t, e) => {
  const n = _();
  if (!(n != null && n.invoke))
    throw new Error("Tauri runtime is unavailable");
  return n.invoke(
    t,
    e
  );
}, N = async (t, e, n) => {
  var i;
  try {
    return await t();
  } catch (o) {
    if (!nt(o) || !((i = _()) != null && i.invoke))
      throw o;
    return w(e, n);
  }
}, ct = (t) => t && typeof t == "object" && "payload" in t ? t.payload ?? null : t;
async function H(t, e, n = {}) {
  var l;
  const i = _();
  if (!(i != null && i.invoke) || !i.transformCallback)
    throw new Error("Tauri event runtime is unavailable");
  let o = !0;
  const s = i.transformCallback((c) => {
    o && e({
      event: t,
      id: c && typeof c == "object" && "id" in c ? Number(c.id) : -1,
      payload: ct(c)
    });
  }, !1);
  let r;
  try {
    r = await i.invoke("plugin:event|listen", {
      event: t,
      target: typeof n.target == "string" ? { kind: "AnyLabel", label: n.target } : n.target ?? J,
      handler: s
    });
  } catch (c) {
    throw (l = i.unregisterCallback) == null || l.call(i, s), c;
  }
  return async () => {
    var c, u, f;
    if (o) {
      o = !1;
      try {
        await ((u = (c = window.__TAURI_EVENT_PLUGIN_INTERNALS__) == null ? void 0 : c.unregisterListener) == null ? void 0 : u.call(
          c,
          t,
          r
        ));
      } catch {
      }
      (f = i.unregisterCallback) == null || f.call(i, s), await i.invoke("plugin:event|unlisten", {
        event: t,
        eventId: r
      });
    }
  };
}
let k = 0, x = Promise.resolve(), F = Promise.resolve();
const M = (t) => {
  const e = x.then(() => w("custom_plugin_window_ready", { ready: t }));
  return x = e.catch(() => {
  }), e;
};
async function lt(t) {
  var i, o, s, r, a, l;
  const e = await H("otools:custom-open", t, {
    target: { kind: "Webview", label: ((s = (o = (i = _()) == null ? void 0 : i.metadata) == null ? void 0 : o.currentWebview) == null ? void 0 : s.label) ?? ((l = (a = (r = _()) == null ? void 0 : r.metadata) == null ? void 0 : a.currentWindow) == null ? void 0 : l.label) ?? "" }
  });
  k += 1;
  try {
    k === 1 && (F = M(!0)), await F;
  } catch (c) {
    throw k -= 1, await e(), c;
  }
  let n = !0;
  return async () => {
    if (n) {
      n = !1, k -= 1;
      try {
        k === 0 && await M(!1);
      } finally {
        await e();
      }
    }
  };
}
async function wt(t, e) {
  let n = d();
  const i = B();
  if (!$() && i && (n = await C()), n) {
    const r = await ut(
      n,
      t,
      e
    );
    if (r !== E)
      return r;
    const a = await at(
      n,
      t,
      e
    );
    if (a !== E)
      return a;
  }
  if (typeof (n == null ? void 0 : n.invokeNative) != "function") {
    if (O(t))
      return w(t, e);
    const r = L(n);
    if (r && !D(t) && !b(t) && typeof (n == null ? void 0 : n.invokeNativePlugin) == "function")
      return n.invokeNativePlugin(r, t, e ?? null);
    try {
      return await w(t, e);
    } catch (a) {
      if (O(t) || b(t) || !A(a))
        throw a;
      const l = i || I() ? 1200 : 200, c = await C(l), u = L(c);
      if (u && typeof (c == null ? void 0 : c.invokeNativePlugin) == "function")
        return c.invokeNativePlugin(u, t, e ?? null);
      if (typeof (c == null ? void 0 : c.invokeNative) != "function")
        throw a;
      return c.invokeNative(t, e ?? null);
    }
  }
  if (t === "get_init_error")
    return null;
  if (K.has(t))
    return !0;
  if (t === "plugin:event|listen") {
    const r = h(e), a = typeof (r == null ? void 0 : r.event) == "string" ? r.event : "", l = typeof (r == null ? void 0 : r.handler) == "number" ? r.handler : NaN;
    if (!a || Number.isNaN(l))
      throw new Error("plugin:event|listen requires event and handler");
    return await G(a, l);
  }
  if (t === "plugin:event|unlisten") {
    const r = h(e), a = typeof (r == null ? void 0 : r.eventId) == "number" ? r.eventId : NaN;
    Number.isNaN(a) || await V(a);
    return;
  }
  if (O(t) || D(t) || b(t))
    return w(t, e);
  const o = L(n), s = o && typeof n.invokeNativePlugin == "function" ? n.invokeNativePlugin.bind(n) : null;
  if (Y.has(t)) {
    const r = s ? await N(
      () => s(o, t, e ?? null),
      t,
      e
    ) : await N(
      () => n.invokeNative(t, e ?? null),
      t,
      e
    ), a = typeof (r == null ? void 0 : r.path) == "string" ? r.path : null;
    if (a) {
      X().open(a, t, e);
      return;
    }
    return r;
  }
  if (s) {
    const r = await N(
      () => s(o, t, e ?? null),
      t,
      e
    );
    return U(t, e, r), r;
  }
  try {
    const r = await N(
      () => n.invokeNative(t, e ?? null),
      t,
      e
    );
    return U(t, e, r), r;
  } catch (r) {
    if (!o || typeof n.invokeNativePlugin != "function" || !A(r))
      throw r;
    const a = n.invokeNativePlugin.bind(n), l = await N(
      () => a(o, t, e ?? null),
      t,
      e
    );
    return U(t, e, l), l;
  }
}
async function vt(t, e, n = {}) {
  var a, l, c;
  if (t === "otools:custom-open" && ((a = _()) != null && a.invoke) && window.__OTOOLS_CUSTOM_WINDOW__)
    return lt(e);
  if ((l = _()) != null && l.invoke && (y.has(t) || n.target))
    return H(t, e, n);
  if (!((c = _()) != null && c.invoke) && y.has(t)) {
    const u = (f) => {
      var T;
      e({ event: t, id: -1, payload: (T = f.detail) == null ? void 0 : T.payload });
    };
    return window.addEventListener(P(t), u), () => window.removeEventListener(P(t), u);
  }
  let i = d();
  const o = B();
  if (!$() && o && (i = await C()), typeof (i == null ? void 0 : i.invokeNative) != "function")
    return H(t, e);
  const s = (u) => {
    const f = u.detail;
    e({
      event: t,
      id: -1,
      payload: (f == null ? void 0 : f.payload) ?? null
    });
  };
  window.addEventListener(P(t), s);
  const r = await z(t, async (u) => {
    await e({
      event: t,
      id: -1,
      payload: u
    });
  });
  return async () => {
    await r(), window.removeEventListener(
      P(t),
      s
    );
  };
}
async function ht(t, e) {
  if (y.has(t)) {
    await w("plugin:event|emit", { event: t, payload: e ?? null });
    return;
  }
  rt(t, e ?? null);
}
function m(t) {
  var e;
  return t.shell ?? ((e = t.runtime) == null ? void 0 : e.shell) ?? void 0;
}
function pt(t) {
  const e = d();
  if (!e)
    return !1;
  const n = m(e);
  return typeof (n == null ? void 0 : n.openExternal) == "function" ? (n.openExternal(t), !0) : (e.shellOpenExternal(t), !0);
}
function gt(t) {
  const e = d();
  if (!e)
    return !1;
  const n = m(e);
  return typeof (n == null ? void 0 : n.openPath) == "function" ? (n.openPath(t), !0) : (e.shellOpenPath(t), !0);
}
export {
  ht as emit,
  d as getOtoolsApi,
  wt as invoke,
  w as invokeTauri,
  $ as isOtoolsPluginRuntime,
  vt as listen,
  H as listenTauri,
  pt as openExternal,
  gt as openPath
};
//# sourceMappingURL=runtime.js.map
