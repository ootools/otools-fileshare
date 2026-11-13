import { invoke as a } from "./runtime.js";
const u = (e) => {
  if (typeof e == "string") {
    const r = e.trim();
    return r || null;
  }
  if (e && typeof e == "object" && "path" in e) {
    const r = e.path;
    if (typeof r == "string") {
      const n = r.trim();
      return n || null;
    }
  }
  return null;
}, f = (e) => {
  if (Array.isArray(e))
    return e.map((n) => u(n)).filter((n) => !!n);
  const r = u(e);
  return r ? [r] : [];
}, c = (e, r) => {
  const n = f(e);
  return r.multiple ? n.length ? n : null : n[0] ?? null;
};
function l() {
  var r;
  if (typeof window > "u")
    return;
  const e = window.otools ?? window.utools;
  return (e == null ? void 0 : e.dialog) ?? ((r = e == null ? void 0 : e.runtime) == null ? void 0 : r.dialog) ?? void 0;
}
async function i(e) {
  try {
    const r = await a("plugin:dialog|open", {
      options: e
    });
    return c(r, e);
  } catch {
    return null;
  }
}
async function d(e) {
  try {
    const r = await a("plugin:dialog|save", { options: e });
    return u(r);
  } catch {
    return null;
  }
}
async function g(e = {}) {
  const r = e ?? {}, n = l();
  if (n != null && n.open)
    return c(
      await n.open(r),
      r
    );
  const t = await i(r);
  return t !== null ? t : null;
}
async function w(e = {}) {
  const r = l();
  if (r != null && r.save)
    return u(await r.save(e));
  const n = await d(e);
  return n !== null ? n : null;
}
async function k(e, r) {
  var t;
  const n = l();
  if (n != null && n.message) {
    await n.message(e, r);
    return;
  }
  try {
    await a("plugin:dialog|message", {
      message: String(e),
      title: typeof r == "string" ? r : r == null ? void 0 : r.title,
      kind: typeof r == "string" || r == null ? void 0 : r.kind,
      okButtonLabel: typeof r == "string" ? void 0 : (r == null ? void 0 : r.okLabel) ?? ((t = r == null ? void 0 : r.buttons) == null ? void 0 : t.ok)
    });
    return;
  } catch {
  }
  typeof window < "u" && window.alert(e);
}
async function m(e, r) {
  const n = l();
  if (n != null && n.confirm)
    return n.confirm(e, r);
  try {
    return !!await a("plugin:dialog|confirm", {
      message: String(e),
      title: typeof r == "string" ? r : r == null ? void 0 : r.title,
      kind: typeof r == "string" || r == null ? void 0 : r.kind,
      okButtonLabel: typeof r == "string" || r == null ? void 0 : r.okLabel,
      cancelButtonLabel: typeof r == "string" || r == null ? void 0 : r.cancelLabel
    });
  } catch {
  }
  return typeof window > "u" ? !1 : window.confirm(e);
}
async function v(e, r) {
  const n = l();
  if (n != null && n.ask)
    return n.ask(e, r);
  try {
    return !!await a("plugin:dialog|ask", {
      message: String(e),
      title: typeof r == "string" ? r : r == null ? void 0 : r.title,
      kind: typeof r == "string" || r == null ? void 0 : r.kind,
      yesButtonLabel: typeof r == "string" || r == null ? void 0 : r.okLabel,
      noButtonLabel: typeof r == "string" || r == null ? void 0 : r.cancelLabel
    });
  } catch {
  }
  return typeof window > "u" ? !1 : window.confirm(e);
}
export {
  v as ask,
  m as confirm,
  k as message,
  g as open,
  w as save
};
//# sourceMappingURL=tauri-plugin-dialog-shim.js.map
