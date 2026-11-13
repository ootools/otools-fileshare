const i = "__OTOOLS_POPUP_MANAGER__", f = "data-otools-popup-id", b = "data-otools-popup-target", h = "data-otools-popup-label", w = "data-otools-popup-window-id";
function x(o, t) {
  try {
    return new URL(o).toString();
  } catch {
    return new URL(o, t.location.href).toString();
  }
}
function g() {
  if (typeof window > "u") return;
  const o = window.frameElement, t = o == null ? void 0 : o.dataset.otoolsPopupLabel, e = o == null ? void 0 : o.dataset.otoolsPopupWindowId;
  return t && e ? { label: t, windowId: e } : void 0;
}
class P {
  constructor(t) {
    this.owner = t, this.sequence = 0, this.popups = /* @__PURE__ */ new Map();
  }
  open(t, e, p, s, r) {
    if (typeof window > "u")
      return;
    const d = `otools-popup-${++this.sequence}`, l = this.owner.document, c = l.createElement("div");
    c.setAttribute(f, d), c.style.cssText = [
      "position:fixed",
      "inset:0",
      "z-index:2147483647",
      "display:flex",
      "align-items:center",
      "justify-content:center",
      "padding:24px",
      "background:rgba(15,23,42,0.45)",
      "backdrop-filter:blur(6px)"
    ].join(";");
    const a = l.createElement("div");
    a.style.cssText = [
      "position:relative",
      "display:flex",
      "flex-direction:column",
      "width:min(1120px,100%)",
      "height:min(760px,100%)",
      "overflow:hidden",
      "border-radius:16px",
      "border:1px solid rgba(148,163,184,0.28)",
      "background:#ffffff",
      "box-shadow:0 24px 80px rgba(15,23,42,0.28)"
    ].join(";");
    const u = l.createElement("button");
    u.type = "button", u.textContent = "Close", u.style.cssText = [
      "position:absolute",
      "top:12px",
      "right:12px",
      "z-index:2",
      "border:0",
      "border-radius:999px",
      "padding:8px 12px",
      "background:rgba(15,23,42,0.72)",
      "color:#ffffff",
      "cursor:pointer"
    ].join(";"), u.addEventListener("click", () => {
      this.close(d);
    });
    const n = l.createElement("iframe");
    return n[i] = this, n.setAttribute(b, e ?? "popup"), n.dataset.otoolsPopupId = d, s !== void 0 && (n.setAttribute(h, s), n.setAttribute(w, r ?? s)), n.src = x(t, this.owner), n.style.cssText = [
      "width:100%",
      "height:100%",
      "border:0",
      "background:#ffffff"
    ].join(";"), a.append(u, n), c.append(a), l.body.append(c), this.popups.set(d, { root: c, label: s, windowId: r }), d;
  }
  getAll() {
    return Array.from(this.popups, ([t, e]) => ({ id: t, label: e.label, windowId: e.windowId ?? e.label })).filter((t) => t.label !== void 0);
  }
  closeLabel(t) {
    for (const [e, p] of this.popups)
      p.label === t && this.close(e);
  }
  close(t) {
    const e = this.popups.get(t);
    e && (e.root.remove(), this.popups.delete(t));
  }
  closeCurrent(t = this.owner) {
    const e = t.frameElement, p = e == null ? void 0 : e.dataset.otoolsPopupId;
    if (p) {
      this.close(p);
      return;
    }
    const s = Array.from(this.popups.keys()), r = s[s.length - 1];
    r && this.close(r);
  }
}
function y() {
  const o = window;
  if (o[i]) return o[i];
  const t = o.frameElement;
  return t != null && t[i] ? t[i] : (o[i] = new P(o), o[i]);
}
export {
  y as ensurePopupManager,
  g as resolvePopupWindowIdentity
};
//# sourceMappingURL=popup-manager.js.map
