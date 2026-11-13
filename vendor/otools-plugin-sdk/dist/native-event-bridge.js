const c = /* @__PURE__ */ new Map(), r = /* @__PURE__ */ new Map(), a = /* @__PURE__ */ new Map(), o = /* @__PURE__ */ new Map();
let d = 0, f = 0, i = null;
function b() {
  if (!(typeof window > "u"))
    return window.otools ?? window.utools;
}
function _(e) {
  const t = e && typeof e == "object" && "payload" in e ? e.payload : e;
  if (!t || typeof t != "object")
    return null;
  const n = t.topic;
  return typeof n != "string" || !n ? null : {
    topic: n,
    payload: t.payload ?? null
  };
}
function m(e) {
  const t = o.get(e.topic);
  if (t)
    for (const n of t.values())
      n(e.payload);
}
async function p() {
  if (i)
    return i;
  const e = b();
  return e != null && e.listenNative ? (i = e.listenNative((t) => {
    const n = _(t);
    n && m(n);
  }), i) : (i = Promise.resolve(async () => {
  }), i);
}
async function N() {
  if (o.size > 0 || !i)
    return;
  const e = await i;
  i = null, await e();
}
function v(e) {
  const t = r.get(e) ?? 0;
  r.set(e, t + 1);
}
function g(e) {
  const t = r.get(e);
  if (!t || t <= 1) {
    r.delete(e), c.delete(e);
    return;
  }
  r.set(e, t - 1);
}
function l() {
  if (typeof window > "u")
    return;
  const e = window.__TAURI_EVENT_PLUGIN_INTERNALS__;
  if (typeof (e == null ? void 0 : e.unregisterListener) == "function")
    return;
  const t = {
    unregisterListener(n, s) {
      w(s);
    }
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = t;
}
function T(e, t = !1) {
  l();
  const n = ++d;
  return c.set(n, { callback: e, once: t }), n;
}
async function L(e, t) {
  l();
  const n = ++f, s = o.get(e) ?? /* @__PURE__ */ new Map();
  return s.set(n, (y) => {
    const u = c.get(t);
    u && (u.callback({ payload: y }), u.once && (c.delete(t), r.delete(t)));
  }), o.set(e, s), a.set(n, { topic: e, handlerId: t }), v(t), await p(), n;
}
async function w(e) {
  const t = a.get(e);
  if (!t)
    return;
  a.delete(e);
  const n = o.get(t.topic);
  n == null || n.delete(e), n && n.size === 0 && o.delete(t.topic), typeof t.handlerId == "number" && g(t.handlerId), await N();
}
async function k(e, t) {
  l();
  const n = ++f, s = o.get(e) ?? /* @__PURE__ */ new Map();
  return s.set(n, t), o.set(e, s), a.set(n, { topic: e }), await p(), async () => {
    await w(n);
  };
}
export {
  L as attachTransformListener,
  w as detachTransformListener,
  k as listenNativeTopic,
  T as registerTransformCallback
};
//# sourceMappingURL=native-event-bridge.js.map
