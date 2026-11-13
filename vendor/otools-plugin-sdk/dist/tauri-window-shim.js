import { Position as u, Size as h, PhysicalPosition as r, PhysicalSize as a } from "@tauri-apps/api/dpi";
import { LogicalPosition as S, LogicalSize as A, PhysicalPosition as x, PhysicalSize as T, Position as O, Size as R } from "@tauri-apps/api/dpi";
import { ensurePopupManager as d, resolvePopupWindowIdentity as f } from "./popup-manager.js";
import { invokeTauri as c, listen as y, listenTauri as _ } from "./runtime.js";
const o = () => {
  var t;
  return typeof window < "u" && typeof ((t = window.__TAURI_INTERNALS__) == null ? void 0 : t.invoke) == "function";
}, p = () => {
  var t, i, e, s;
  return typeof window > "u" ? "" : ((t = f()) == null ? void 0 : t.label) ?? ((s = (e = (i = window.__TAURI_INTERNALS__) == null ? void 0 : i.metadata) == null ? void 0 : e.currentWindow) == null ? void 0 : s.label) ?? "";
};
class g {
  constructor(i) {
    this.ready = Promise.resolve(), this.label = i;
  }
  async invokeWindow(i, e = {}) {
    return await this.ready, c(`plugin:window|${i}`, { label: this.label, ...e });
  }
  async control(i, e = {}) {
    await this.ready, o() && await this.invokeWindow(i, e);
  }
  async close() {
    await this.ready, o() ? await this.invokeWindow("close") : typeof window < "u" && (this.label ? d().closeLabel(this.label) : d().closeCurrent(window));
  }
  async destroy() {
    o() ? await this.invokeWindow("destroy") : await this.close();
  }
  show() {
    return this.control("show");
  }
  hide() {
    return this.control("hide");
  }
  setFocus() {
    return this.control("set_focus");
  }
  setAlwaysOnTop(i) {
    return this.control("set_always_on_top", { value: i });
  }
  setVisibleOnAllWorkspaces(i) {
    return this.control("set_visible_on_all_workspaces", { value: i });
  }
  setIgnoreCursorEvents(i) {
    return this.control("set_ignore_cursor_events", { value: i });
  }
  setPosition(i) {
    const e = i instanceof u ? i.toJSON() : {
      [i.type ?? "Physical"]: { x: i.x, y: i.y }
    };
    return this.control("set_position", { value: e });
  }
  setSize(i) {
    const e = i instanceof h ? i.toJSON() : {
      [i.type ?? "Physical"]: { width: i.width, height: i.height }
    };
    return this.control("set_size", { value: e });
  }
  async outerPosition() {
    if (o()) return new r(await this.invokeWindow("outer_position"));
    const i = await this.scaleFactor();
    return new r(typeof window > "u" ? 0 : window.screenX * i, typeof window > "u" ? 0 : window.screenY * i);
  }
  async innerSize() {
    if (o()) return new a(await this.invokeWindow("inner_size"));
    const i = await this.scaleFactor();
    return new a(typeof window > "u" ? 0 : window.innerWidth * i, typeof window > "u" ? 0 : window.innerHeight * i);
  }
  async scaleFactor() {
    return o() ? this.invokeWindow("scale_factor") : typeof window > "u" ? 1 : window.devicePixelRatio || 1;
  }
  minimize() {
    return this.control("minimize");
  }
  toggleMaximize() {
    return this.control("toggle_maximize");
  }
  async isMaximized() {
    return o() ? this.invokeWindow("is_maximized") : !1;
  }
  center() {
    return this.control("center");
  }
  startDragging() {
    return this.control("start_dragging");
  }
  startResizeDragging(i) {
    return this.control("start_resize_dragging", { value: i });
  }
  async listen(i, e) {
    return await this.ready, o() ? i === "otools:custom-open" && window.__OTOOLS_CUSTOM_WINDOW__ ? y(i, e) : _(i, e, { target: { kind: "Window", label: this.label } }) : () => {
    };
  }
  async once(i, e) {
    let s = !0, n;
    const w = await this.listen(i, (l) => {
      s && (s = !1, n == null || n(), e(l));
    });
    return n = w, s || await n(), async () => {
      s = !1, await w();
    };
  }
  onMoved(i) {
    return this.listen("tauri://move", (e) => i({ ...e, payload: new r(e.payload) }));
  }
  onResized(i) {
    return this.listen("tauri://resize", (e) => i({ ...e, payload: new a(e.payload) }));
  }
}
function W() {
  return new g(p());
}
async function P() {
  if (o())
    return (await c("plugin:window|available_monitors")).map((n) => {
      var w, l;
      return {
        ...n,
        size: new a(n.size),
        position: new r(n.position),
        workArea: {
          position: new r(((w = n.workArea) == null ? void 0 : w.position) ?? n.position),
          size: new a(((l = n.workArea) == null ? void 0 : l.size) ?? n.size)
        }
      };
    });
  if (typeof window > "u") return [];
  const t = window.devicePixelRatio || 1, i = new a((window.screen.width || window.innerWidth) * t, (window.screen.height || window.innerHeight) * t), e = new r(0, 0);
  return [{ name: null, size: i, position: e, workArea: { size: i, position: e }, scaleFactor: t }];
}
export {
  S as LogicalPosition,
  A as LogicalSize,
  x as PhysicalPosition,
  T as PhysicalSize,
  O as Position,
  R as Size,
  g as Window,
  P as availableMonitors,
  W as getCurrentWindow,
  o as hasTauriWindowApi,
  p as resolveWindowLabel
};
//# sourceMappingURL=tauri-window-shim.js.map
