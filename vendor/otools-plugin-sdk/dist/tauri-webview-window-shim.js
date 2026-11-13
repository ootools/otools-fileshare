import { Window as b, hasTauriWindowApi as s, resolveWindowLabel as p } from "./tauri-window-shim.js";
import { invokeTauri as w } from "./runtime.js";
import { ensurePopupManager as d, resolvePopupWindowIdentity as g } from "./popup-manager.js";
const c = () => typeof window > "u" ? void 0 : window.__OTOOLS_CUSTOM_WINDOW__, y = Symbol("existing window");
class m extends b {
  constructor(e, i = {}, n) {
    super(e), this.lifecycle = /* @__PURE__ */ new Map(), this.windowId = e, n !== y && (this.ready = Promise.resolve().then(async () => {
      var r;
      if (s())
        if (c()) {
          const { title: l, url: o, ...h } = i, u = await w("open_custom_plugin_window", {
            windowId: e,
            ...l === void 0 ? {} : { title: l },
            ...o === void 0 ? {} : { url: o },
            options: h
          });
          this.label = u.label, this.windowId = u.windowId;
        } else
          await w("plugin:webview|create_webview_window", {
            options: { ...i, parent: typeof i.parent == "string" ? i.parent : (r = i.parent) == null ? void 0 : r.label, label: e }
          });
      else {
        if (typeof window > "u") throw new Error("Window creation is unavailable outside a browser or Tauri runtime");
        const l = d();
        l.getAll().some((o) => o.label === e) || l.open(i.url ?? window.location.href, "webview-window", void 0, e);
      }
    }), this.ready.then(
      () => this.dispatchLifecycle("tauri://created", null),
      (r) => this.dispatchLifecycle("tauri://error", r)
    ));
  }
  dispatchLifecycle(e, i) {
    const n = this.lifecycle.get(e);
    for (const r of [...n ?? []])
      r.once && (n == null || n.delete(r)), r.handler({ event: e, id: -1, payload: i });
  }
  lifecycleListener(e, i, n) {
    const r = this.lifecycle.get(e) ?? /* @__PURE__ */ new Set();
    this.lifecycle.set(e, r);
    const l = { handler: i, once: n };
    return r.add(l), Promise.resolve(() => {
      r.delete(l);
    });
  }
  listen(e, i) {
    return e === "tauri://created" || e === "tauri://error" ? this.lifecycleListener(e, i, !1) : super.listen(e, i);
  }
  once(e, i) {
    return e === "tauri://created" || e === "tauri://error" ? this.lifecycleListener(e, i, !0) : super.once(e, i);
  }
  async close() {
    await this.ready, s() || !this.label ? await super.close() : d().closeLabel(this.label);
  }
  async destroy() {
    s() ? await super.destroy() : await this.close();
  }
  static getCurrent() {
    return _();
  }
  static getAll() {
    return f();
  }
  static async getByLabel(e) {
    const i = await f();
    return i.find((n) => n.label === e) ?? i.find((n) => n.windowId === e) ?? null;
  }
}
function a(t) {
  const e = new m(t.label, {}, y);
  return e.windowId = t.windowId, e;
}
function _() {
  var i;
  const t = g(), e = (t == null ? void 0 : t.label) ?? p();
  return a({ label: e, windowId: (t == null ? void 0 : t.windowId) ?? ((i = c()) == null ? void 0 : i.windowId) ?? e });
}
async function f() {
  return s() ? c() ? (await w("list_custom_plugin_windows", {})).map(a) : (await w("plugin:window|get_all_windows")).map((t) => a({ label: t, windowId: t })) : typeof window > "u" ? [] : d().getAll().map((t) => a({ label: t.label, windowId: t.label }));
}
export {
  m as WebviewWindow,
  f as getAllWebviewWindows,
  _ as getCurrentWebviewWindow
};
//# sourceMappingURL=tauri-webview-window-shim.js.map
