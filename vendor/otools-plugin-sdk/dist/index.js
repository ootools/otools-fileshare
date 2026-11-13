import { capture as de } from "./capture.js";
import { AgentError as ue, createAgentApi as fe } from "./agent-factory.js";
import { createAiApi as me } from "./ai-factory.js";
import { createPluginDataApi as we } from "./plugin-data-factory.js";
import { ObservationError as be, createObservationApi as xe } from "./observation-factory.js";
import { createReminderApi as ke } from "./reminder-factory.js";
import { createOtoolsAliasMap as ve } from "./aliases.js";
import { otoolsTauriShimPlugin as Oe } from "./vite.js";
import { listen as q, invoke as L, listenTauri as $, invokeTauri as M } from "./runtime.js";
import { emit as Se, getOtoolsApi as Pe, isOtoolsPluginRuntime as Ne, openExternal as Le, openPath as Te } from "./runtime.js";
import { pickDirectory as Ue, pickFile as ze, pickFiles as De, pickZipFile as Ie, saveFile as He } from "./dialog.js";
import { CaptureError as Be, createCaptureApi as qe } from "./capture-factory.js";
function U() {
  if (!(typeof window > "u"))
    return window.otools ?? window.utools;
}
async function te(t, e) {
  const o = String(t ?? "").trim();
  if (!o)
    return;
  const n = U();
  if (n != null && n.shellOpen) {
    await n.shellOpen(o, e);
    return;
  }
  if (n != null && n.shellOpenExternal && /^https?:\/\//i.test(o)) {
    n.shellOpenExternal(o);
    return;
  }
  if (n != null && n.shellOpenPath) {
    n.shellOpenPath(o);
    return;
  }
  typeof window < "u" && window.open(o, "_blank", "noopener,noreferrer");
}
async function oe(t) {
  const e = String(t ?? "").trim();
  if (!e)
    return;
  const o = U();
  if (o != null && o.shellOpenPath) {
    o.shellOpenPath(e);
    return;
  }
}
async function ne(t) {
  const e = String(t ?? "").trim();
  if (!e)
    return;
  const o = U();
  if (o != null && o.shellOpenExternal) {
    o.shellOpenExternal(e);
    return;
  }
  typeof window < "u" && window.open(e, "_blank", "noopener,noreferrer");
}
const j = "otools-open-file", I = 3e3;
function W(t) {
  if (!t)
    return [];
  try {
    const o = new URLSearchParams(t).get("otoolsOpenFile");
    if (!o)
      return [];
    const n = JSON.parse(o);
    return Array.isArray(n) ? n.map((s) => String(s || "").trim()).filter(Boolean) : [];
  } catch {
    return [];
  }
}
function K(t) {
  return t.map((e) => {
    const o = String(e || "").trim(), n = o.split(/[\\/]/), s = n[n.length - 1] || o, m = s.lastIndexOf("."), g = m > 0 ? s.slice(m + 1).toLowerCase() : "";
    return { path: o, name: s, ext: g };
  });
}
function V() {
  var t;
  if (typeof window < "u")
    try {
      const e = window.__OToolsEnv, o = typeof (e == null ? void 0 : e.pluginUuid) == "string" ? e.pluginUuid.trim() : "";
      if (o)
        return o;
      const n = new URLSearchParams(((t = window.location) == null ? void 0 : t.search) || ""), s = (n.get("plugin") || n.get("pluginUuid") || n.get("plugin_uuid") || "").trim();
      if (s)
        return s;
    } catch {
    }
  return "";
}
async function re(t) {
  var g;
  const e = /* @__PURE__ */ new Map(), o = (c) => {
    const u = Date.now(), f = [];
    for (const h of c) {
      const d = String(h || "").trim();
      if (!d)
        continue;
      const w = e.get(d);
      typeof w == "number" && u - w < I || (e.set(d, u), f.push(d));
    }
    if (e.size > 256)
      for (const [h, d] of e)
        u - d >= I && e.delete(h);
    f.length && t(K(f));
  }, n = async () => {
    const c = V();
    if (!c)
      return;
    let u = [];
    try {
      const f = await L("otools_consume_open_files", { pluginUuid: c });
      u = Array.isArray(f) ? f.map(String) : [];
    } catch {
      return;
    }
    o(u);
  }, s = await q(j, () => {
    n();
  }), m = W(
    typeof window < "u" && ((g = window.location) == null ? void 0 : g.search) || ""
  );
  return m.length && o(m), n(), s;
}
const J = "otools-keyword-dispatch";
function H() {
  var t;
  if (typeof window < "u")
    try {
      const e = window.__OToolsEnv, o = typeof (e == null ? void 0 : e.pluginUuid) == "string" ? e.pluginUuid.trim() : "";
      if (o)
        return o;
      const n = new URLSearchParams(((t = window.location) == null ? void 0 : t.search) || ""), s = (n.get("plugin") || n.get("pluginUuid") || n.get("plugin_uuid") || "").trim();
      if (s)
        return s;
    } catch {
    }
  return "";
}
const R = /* @__PURE__ */ new Map();
async function se(t) {
  var v, _, C;
  const e = H(), o = typeof window > "u" || (C = (_ = (v = window.__TAURI_INTERNALS__) == null ? void 0 : v.metadata) == null ? void 0 : _.currentWebview) == null ? void 0 : C.label, n = JSON.stringify([e.toLowerCase(), o || ""]), s = R.get(n) || { entries: [], consuming: null };
  R.set(n, s);
  const m = s.consuming, g = /* @__PURE__ */ new Set();
  let c = !1, u = !1, f = null;
  const h = (p) => {
    const y = [];
    for (const x of p) {
      if (!x || typeof x != "object")
        continue;
      const l = x;
      typeof l.id != "string" || !l.id.trim() || typeof l.keyword != "string" || !l.keyword.trim() || g.has(l.id) || (g.add(l.id), y.push(l));
    }
    if (g.size > 512) {
      const x = Array.from(g).slice(-256);
      g.clear();
      for (const l of x)
        g.add(l);
    }
    y.length && !c && t(y);
  }, d = () => (u = !0, f || (f = (async () => {
    for (; u && !c; ) {
      if (u = !1, !e)
        return;
      if (!s.consuming) {
        const p = o ? { pluginUuid: e, targetLabel: o } : { pluginUuid: e };
        s.consuming = M("otools_consume_keyword_dispatch", p).then((y) => {
          Array.isArray(y) && s.entries.push(...y);
        }).finally(() => {
          s.consuming = null;
        });
      }
      try {
        await s.consuming;
      } catch {
        continue;
      }
      if (!c) {
        const p = s.entries;
        s.entries = [], h(p);
      }
    }
  })().finally(() => {
    f = null;
  }), f)), w = await $(J, (p) => {
    const y = p == null ? void 0 : p.payload;
    !c && (typeof y != "string" || y.toLowerCase() === H().toLowerCase()) && d();
  });
  return m ? m.catch(() => {
  }).then(() => {
    c || d();
  }) : d(), async () => {
    c = !0, u = !1, await w();
  };
}
async function Y(t) {
  return L("tools_webview_browse_dialog", { request: { path: t ?? "" } });
}
async function ie(t) {
  const e = String(t ?? "").trim();
  if (!e)
    throw new Error("listHostDir requires path");
  return L("tools_webview_list_dir", { path: e });
}
async function ae(t) {
  const e = String(t ?? "").trim();
  if (!e)
    throw new Error("readHostFile requires path");
  return L("tools_webview_read_file", { path: e });
}
const Z = (t, e) => {
  if (!(e != null && e.length) || t.kind === "directory")
    return !0;
  const o = t.name.includes(".") ? t.name.split(".").pop().toLowerCase() : "";
  return e.some(
    (n) => n.extensions.some((s) => s.trim().toLowerCase().replace(/^\./, "") === o)
  );
};
function G() {
  if (document.getElementById("otools-host-tree-style"))
    return;
  const t = document.createElement("style");
  t.id = "otools-host-tree-style", t.textContent = X, document.head.appendChild(t);
}
async function Q(t) {
  if (typeof document > "u" || !document.body)
    throw new Error("pickFromHostTree requires a DOM document");
  const e = !!t.directory, o = !!t.multiple && !e, n = t.title ?? (e ? "选择目录" : "选择文件");
  return new Promise((s) => {
    let m = "", g = t.startPath ?? "", c = !1, u = !1, f = 0;
    const h = /* @__PURE__ */ new Set();
    G();
    const d = document.createElement("div");
    d.setAttribute("data-host-tree", ""), d.className = "otools-host-tree-overlay";
    const w = document.createElement("div");
    w.className = "otools-host-tree-panel", w.setAttribute("role", "dialog"), w.setAttribute("aria-modal", "true"), w.setAttribute("aria-label", n), d.appendChild(w);
    const v = document.createElement("div");
    v.className = "otools-host-tree-header";
    const _ = document.createElement("div");
    _.className = "otools-host-tree-title", _.textContent = n, v.appendChild(_);
    const C = document.createElement("div");
    C.className = "otools-host-tree-path", v.appendChild(C), w.appendChild(v);
    const p = document.createElement("div");
    p.className = "otools-host-tree-status", p.setAttribute("role", "status"), p.hidden = !0;
    const y = document.createElement("span");
    y.setAttribute("data-host-tree-error", "");
    const x = document.createElement("button");
    x.type = "button", x.className = "otools-host-tree-btn", x.textContent = "重试", x.setAttribute("data-host-tree-retry", ""), p.appendChild(y), p.appendChild(x), w.appendChild(p);
    const l = document.createElement("div");
    l.className = "otools-host-tree-body", w.appendChild(l);
    const P = document.createElement("div");
    P.className = "otools-host-tree-footer";
    const O = document.createElement("div");
    O.className = "otools-host-tree-selection", O.setAttribute("data-host-tree-selection", ""), O.setAttribute("role", "status"), O.textContent = e ? "请选择目录" : "请选择文件", P.appendChild(O);
    const S = document.createElement("button");
    S.type = "button", S.textContent = "取消", S.className = "otools-host-tree-btn", S.setAttribute("data-host-tree-cancel", "");
    const k = document.createElement("button");
    k.type = "button", k.textContent = e ? "选择此目录" : "选择", k.className = "otools-host-tree-btn otools-host-tree-btn-primary", k.setAttribute("data-host-tree-confirm", ""), k.disabled = !0, P.appendChild(S), P.appendChild(k), w.appendChild(P);
    const T = () => {
      k.disabled = !c || (e ? !m : !h.size), C.classList.toggle("otools-host-tree-current", e && c && !!m), O.textContent = e && c ? `已选目录：${m}` : h.size ? `已选 ${h.size} 个文件` : e ? "请选择目录" : "请选择文件", O.title = e && c ? m : "";
    }, N = (i) => {
      u || (u = !0, f += 1, window.removeEventListener("keydown", z, !0), d.remove(), s(i));
    }, z = (i) => {
      i.key === "Escape" && (i.preventDefault(), i.stopImmediatePropagation(), N({ kind: "cancelled", source: "host-tree", paths: [], names: [] }));
    };
    window.addEventListener("keydown", z, !0), d.addEventListener("mousedown", (i) => {
      i.target === d && N({ kind: "cancelled", source: "host-tree", paths: [], names: [] });
    }), S.addEventListener(
      "click",
      () => N({ kind: "cancelled", source: "host-tree", paths: [], names: [] })
    ), k.addEventListener("click", () => {
      if (k.disabled) return;
      const i = e ? [m] : Array.from(h);
      N({
        kind: "picked",
        source: "host-tree",
        paths: i,
        names: i.map((b) => b.split(/[\\/]/).filter(Boolean).pop() || b)
      });
    });
    const B = (i) => {
      if (!i || typeof i.currentPath != "string" || !i.currentPath || !Array.isArray(i.roots) || !Array.isArray(i.entries))
        throw new Error("宿主返回的目录数据无效");
      if (m = i.currentPath, C.textContent = m, l.replaceChildren(), i.parentPath) {
        const r = document.createElement("div");
        r.className = "otools-host-tree-entry otools-host-tree-up", r.setAttribute("data-host-tree-up", ""), r.setAttribute("data-path", i.parentPath), r.setAttribute("data-kind", "directory"), r.textContent = "↑ 上一级", l.appendChild(r);
      }
      for (const r of i.roots) {
        const a = document.createElement("div");
        a.className = "otools-host-tree-entry otools-host-tree-root", a.setAttribute("data-host-tree-root", ""), a.setAttribute("data-path", r.path), a.setAttribute("data-kind", "directory"), a.textContent = `📁 ${r.name}  (${r.path})`, l.appendChild(a);
      }
      const b = i.entries.filter(
        (r) => (r.kind === "file" || r.kind === "directory") && Z(r, t.filters)
      );
      for (const r of b) {
        const a = document.createElement("div");
        a.className = "otools-host-tree-entry", a.setAttribute("data-host-tree-entry", ""), a.setAttribute("data-path", r.path), a.setAttribute("data-kind", r.kind);
        const A = document.createElement("span");
        if (A.className = "otools-host-tree-entry-label", A.textContent = r.kind === "directory" ? `📁 ${r.name}` : `📄 ${r.name}`, a.appendChild(A), !e && r.kind === "file") {
          a.setAttribute("aria-selected", "false");
          const E = document.createElement("span");
          E.className = "otools-host-tree-check", E.setAttribute("data-host-tree-check", ""), E.setAttribute("aria-hidden", "true"), E.textContent = "✓", E.hidden = !0, a.appendChild(E);
        }
        l.appendChild(a);
      }
      if (!b.length) {
        const r = document.createElement("div");
        r.className = "otools-host-tree-empty", r.textContent = "该目录为空", l.appendChild(r);
      }
    }, F = async (i) => {
      if (u) return;
      const b = ++f;
      g = i, c = !1, h.clear(), T(), p.hidden = !0, l.replaceChildren();
      const r = document.createElement("div");
      r.className = "otools-host-tree-empty", r.textContent = "正在读取目录…", l.appendChild(r);
      try {
        const a = await Y(i);
        if (u || b !== f) return;
        B(a), c = !0, T();
      } catch (a) {
        if (u || b !== f) return;
        l.replaceChildren(), y.textContent = `无法读取宿主目录：${a instanceof Error ? a.message : String(a)}`, p.hidden = !1;
      }
    };
    x.addEventListener("click", () => void F(g)), l.addEventListener("click", (i) => {
      if (!c || !(i.target instanceof Element)) return;
      const b = i.target.closest(
        "[data-host-tree-entry],[data-host-tree-root],[data-host-tree-up]"
      );
      if (!b || !l.contains(b)) return;
      const r = b.getAttribute("data-path") || "", a = b.getAttribute("data-kind");
      r && (a === "directory" ? F(r) : !e && a === "file" && (o ? h.has(r) ? h.delete(r) : h.add(r) : (h.clear(), h.add(r)), l.querySelectorAll("[data-host-tree-entry]").forEach((A) => {
        const E = h.has(A.getAttribute("data-path") || "");
        A.classList.toggle("otools-host-tree-selected", E), A.getAttribute("data-kind") === "file" && A.setAttribute("aria-selected", String(E));
        const D = A.querySelector("[data-host-tree-check]");
        D && (D.hidden = !E);
      }), T()));
    }), document.body.appendChild(d), F(g);
  });
}
typeof window < "u" && (window.__otoolsHostTree = Q);
const X = `
.otools-host-tree-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2147483000;
  font-family: -apple-system, "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif;
}
.otools-host-tree-panel {
  width: min(560px, 90vw);
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  background: #ffffff;
  border-radius: 12px;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.25);
  overflow: hidden;
  color: #1d1d1f;
}
.otools-host-tree-header {
  padding: 14px 18px 10px;
  border-bottom: 1px solid #ececec;
}
.otools-host-tree-title {
  font-size: 15px;
  font-weight: 600;
}
.otools-host-tree-path {
  margin-top: 4px;
  font-size: 12px;
  color: #6b6b70;
  word-break: break-all;
}
.otools-host-tree-status {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 12px 16px;
  font-size: 13px;
  color: #b42318;
}
.otools-host-tree-status[hidden] { display: none; }
.otools-host-tree-body {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: 6px 8px;
  min-height: 220px;
}
.otools-host-tree-entry {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-radius: 7px;
  font-size: 13px;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.otools-host-tree-entry:hover {
  background: #f2f2f4;
}
.otools-host-tree-up,
.otools-host-tree-root {
  color: #2b6fff;
}
.otools-host-tree-selected {
  background: #dbeafe !important;
  color: #174ea6 !important;
  box-shadow: inset 3px 0 #2563eb;
  font-weight: 600;
}
.otools-host-tree-entry-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}
.otools-host-tree-check { color: #174ea6; font-weight: 700; }
.otools-host-tree-check[hidden] { display: none; }
.otools-host-tree-current {
  background: #dbeafe;
  color: #174ea6;
  border-radius: 6px;
  padding: 6px 8px;
}
.otools-host-tree-selection {
  flex: 1;
  min-width: 0;
  color: #174ea6;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.otools-host-tree-empty {
  padding: 18px 10px;
  text-align: center;
  color: #9a9aa0;
  font-size: 13px;
}
.otools-host-tree-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
  padding: 12px 16px;
  border-top: 1px solid #ececec;
}
.otools-host-tree-btn {
  padding: 7px 16px;
  border: 1px solid #d6d6da;
  border-radius: 8px;
  background: #f7f7f9;
  font-size: 13px;
  cursor: pointer;
}
.otools-host-tree-btn-primary {
  background: #2b6fff;
  border-color: #2b6fff;
  color: #fff;
}
.otools-host-tree-btn-primary:disabled {
  background: #b9c8ee;
  border-color: #b9c8ee;
  cursor: not-allowed;
}
`;
export {
  ue as AgentError,
  Be as CaptureError,
  j as OTOOLS_FILE_DELIVER_EVENT,
  J as OTOOLS_KEYWORD_DISPATCH_EVENT,
  be as ObservationError,
  Y as browseHostDir,
  de as capture,
  fe as createAgentApi,
  me as createAiApi,
  qe as createCaptureApi,
  xe as createObservationApi,
  ve as createOtoolsAliasMap,
  we as createPluginDataApi,
  ke as createReminderApi,
  Se as emit,
  Pe as getOtoolsApi,
  L as invoke,
  Ne as isOtoolsPluginRuntime,
  ie as listHostDir,
  q as listen,
  se as onOtoolsKeyword,
  re as onOtoolsOpenFile,
  Le as openExternal,
  Te as openPath,
  Oe as otoolsTauriShimPlugin,
  W as parseLaunchOpenFiles,
  Ue as pickDirectory,
  ze as pickFile,
  De as pickFiles,
  Q as pickFromHostTree,
  Ie as pickZipFile,
  ae as readHostFile,
  He as saveFile,
  te as shellOpen,
  ne as shellOpenExternal,
  oe as shellOpenPath,
  K as toOpenFileEntries
};
//# sourceMappingURL=index.js.map
