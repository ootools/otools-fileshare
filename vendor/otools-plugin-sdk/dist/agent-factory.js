class u extends Error {
  constructor(t, i = t) {
    super(i), this.code = t, this.name = "AgentError";
  }
}
const V = 4 * 1024 * 1024, $ = 256 * 1024, D = 1e5, J = /* @__PURE__ */ new Set(["done", "cancelled", "error"]);
function G(e) {
  if (e instanceof u) return e;
  const t = e, i = typeof e == "string" ? e : typeof (t == null ? void 0 : t.message) == "string" ? t.message : "Agent transport failed";
  return typeof (t == null ? void 0 : t.code) == "string" ? new u(t.code, i) : /(unknown command|command.*not found|not implemented|agent.*unavailable)/i.test(i) ? new u("unavailable", i) : e instanceof Error ? e : new u(typeof e == "string" ? e : "transportFailed", i);
}
function y() {
  throw new u("invalidRequest");
}
function m(e) {
  return (!e || typeof e != "object" || Array.isArray(e)) && y(), e;
}
function d(e, t = 512, i = !1) {
  return (typeof e != "string" || e.length > t || !i && !e.trim() || e.includes("\0")) && y(), e;
}
function L(e, t, i) {
  return (typeof e != "number" || !Number.isSafeInteger(e) || e < t || e > i) && y(), e;
}
function j(e) {
  return typeof e != "boolean" && y(), e;
}
function I(e, t, i, o = d) {
  t[i] !== void 0 && (e[i] = o(t[i]));
}
function x(e, t) {
  return { [t]: d(m(e)[t]) };
}
function H(e) {
  const t = m(e);
  return { id: d(t.id), after: t.after === void 0 ? 0 : L(t.after, 0, Number.MAX_SAFE_INTEGER) };
}
function W(e, t = !1) {
  const i = d(e, 4096, t);
  return t && (i === "" || i === ".") || (/^[\\/]|[\\:]|[\x00-\x1f]/.test(i) || i.split("/").some((o) => !o || o === "." || o === "..")) && y(), i;
}
function K(e) {
  const t = m(e), i = { providerId: d(t.providerId) };
  for (const o of ["clientSessionId", "workspaceId"]) I(i, t, o);
  I(i, t, "instructions", (o) => d(o, 1024 * 1024, !0)), I(i, t, "sandbox", (o) => (o !== "read-only" && o !== "workspace-write" && y(), o));
  for (const o of ["network", "background"]) I(i, t, o, j);
  return I(i, t, "tools", (o) => ((!Array.isArray(o) || o.length > 128) && y(), o.map((n) => d(n)))), i;
}
function Q(e) {
  const t = m(e), i = { sessionId: d(t.sessionId), prompt: d(t.prompt, 1024 * 1024) };
  for (const o of ["model", "requestId"]) I(i, t, o);
  if (I(i, t, "reasoningEffort", (o) => o === null ? null : d(o, 64)), I(i, t, "timeoutSecs", (o) => L(o, 1, 86400)), t.mediaModels !== void 0) {
    const o = m(t.mediaModels), n = Object.entries(o);
    (n.length > 5 || n.some(([r]) => !["images", "video", "tts", "stt", "embeddings"].includes(r))) && y(), i.mediaModels = Object.fromEntries(n.map(([r, c]) => [r, d(c, 512)]));
  }
  return i;
}
function U(e, t, i) {
  const o = () => {
    throw new u("invalidResponse");
  }, n = () => {
    throw new u("responseTooLarge");
  };
  (!e || e.id !== t || !Array.isArray(e.events) || typeof e.done != "boolean" || !Number.isSafeInteger(e.after) || e.after < i) && o(), e.events.length > 256 && n(), ["running", "completed", "cancelled", "failed", "interrupted"].includes(e.status) || o(), e.text !== void 0 && (typeof e.text != "string" && o(), e.text.length > V && n());
  let r = $, c = 0;
  const h = (s, g = 0) => {
    if ((++c > 16384 || g > 32) && n(), s === null || typeof s == "boolean") r -= 5;
    else if (typeof s == "number")
      Number.isFinite(s) || o(), r -= 24;
    else if (typeof s == "string") r -= s.length;
    else if (Array.isArray(s)) {
      r -= s.length;
      for (const _ of s) h(_, g + 1);
    } else if (typeof s == "object") {
      const _ = Object.getPrototypeOf(s);
      _ !== Object.prototype && _ !== null && o();
      for (const [k, M] of Object.entries(s))
        r -= k.length, h(M, g + 1);
    } else o();
    r < 0 && n();
  };
  let A = 0, l = i, f;
  const p = [];
  for (const s of e.events) {
    (!s || !Number.isSafeInteger(s.sequence) || s.sequence <= A || s.sequence < 1 || f) && o(), A = s.sequence, ["text", "reasoning", "tool", "permission", "question", "status", "done", "cancelled", "error"].includes(s.type) || o();
    for (const g of ["text", "itemId", "error"])
      s[g] !== void 0 && (typeof s[g] != "string" && o(), r -= s[g].length);
    if (s.type === "text" && typeof s.text != "string" && o(), s.type === "error" && (typeof s.error != "string" || !s.error) && o(), s.data !== void 0 && h(s.data), r < 0 && n(), J.has(s.type) && (f = s), !(s.sequence <= i)) {
      if (s.sequence !== l + 1) throw new u("cursorGap");
      l = s.sequence, p.push(s);
    }
  }
  return (e.after !== (e.events.length ? A : i) || e.after !== l) && o(), e.done !== (e.status !== "running") && o(), f && !e.done && o(), e.done && !f && e.events.length > 0 && o(), (f == null ? void 0 : f.type) === "done" && e.status !== "completed" && o(), (f == null ? void 0 : f.type) === "cancelled" && e.status !== "cancelled" && o(), (f == null ? void 0 : f.type) === "error" && e.status !== "failed" && e.status !== "interrupted" && o(), p;
}
function Y(e) {
  const t = async (n, r) => {
    try {
      if (!e || typeof e.invoke != "function") throw new u("unavailable", "Host agent API is unavailable");
      return await e.invoke(`otools_host_agent_${n}`, r);
    } catch (c) {
      throw G(c);
    }
  }, i = {
    providers: () => t("providers"),
    tools: () => t("tools"),
    capabilities: async (n) => t("capabilities", x(n, "providerId")),
    health: async (n) => {
      const r = m(n), c = x(r, "providerId");
      return I(c, r, "refresh", j), t("health", c);
    },
    openManager: async (n = {}) => {
      const r = {};
      return I(r, m(n), "providerId"), t("open_manager", r);
    },
    openSession: async (n) => t("open_session", { request: K(n) }),
    configOptions: async (n) => t("config_options", x(n, "sessionId")),
    setConfigOption: async (n) => {
      const r = m(n);
      return t("set_config_option", { sessionId: d(r.sessionId), configId: d(r.configId), value: d(r.value, 4096) });
    },
    start: async (n) => {
      const { mediaModels: r, ...c } = Q(n);
      return t("start", { request: c, ...r ? { mediaModels: r } : {} });
    },
    read: async (n) => t("read", H(n)),
    cancel: async (n) => t("cancel", x(n, "id")),
    closeSession: async (n) => t("close_session", x(n, "sessionId")),
    respondQuestion: async (n) => {
      const r = m(n), c = Object.entries(m(r.answers));
      c.length > 128 && y();
      const h = /* @__PURE__ */ Object.create(null);
      let A = 0;
      for (const [l, f] of c)
        d(l), (["__proto__", "constructor", "prototype"].includes(l) || !Array.isArray(f) || f.length > 128) && y(), h[l] = f.map((p) => {
          const s = d(p, 65536, !0);
          return A += s.length, A > 1048576 && y(), s;
        });
      return t("respond_question", { id: d(r.id), answers: h });
    },
    openPermission: async (n) => t("open_permission", x(n, "id")),
    workspace: async (n = {}) => {
      const r = {};
      return I(r, m(n), "workspaceId"), t("workspace", r);
    },
    syncAssets: async (n) => {
      const r = m(n);
      (!Array.isArray(r.files) || r.files.length > 256) && y();
      let c = 0;
      const h = r.files.map((A) => {
        const l = m(A), f = W(l.path), p = d(l.contentBase64, 16 * 1024 * 1024, !0);
        c += p.length;
        const s = p.endsWith("==") ? 2 : p.endsWith("=") ? 1 : 0, g = s ? p.slice(0, -s) : p;
        return (c > 32 * 1024 * 1024 || p.length % 4 !== 0 || /[^A-Za-z0-9+/]/.test(g) || s > 0 && g.length < 2) && y(), { path: f, contentBase64: p };
      });
      return t("sync_assets", { workspaceId: d(r.workspaceId), files: h });
    },
    workspaceFiles: async (n) => {
      const r = m(n), c = x(r, "workspaceId");
      return I(c, r, "path", (h) => W(h, !0)), t("workspace_files", c);
    },
    consume: (n, r = {}) => o(n, void 0, r),
    resume: (n, r = {}) => o(void 0, n, r)
  };
  function o(n, r, c) {
    return new Promise((h, A) => {
      let l = !1, f, p, s = 0, g = "", _ = 0, k = !1, M = !1, N = !1, R, z, w;
      const F = () => {
        !f || M || k || (M = !0, i.cancel({ id: f }).catch(() => {
        }));
      }, P = () => {
        clearTimeout(R), clearTimeout(z), w == null || w.removeEventListener("abort", O);
      }, E = (a, v = n !== void 0) => {
        l || (l = !0, N = v, P(), v && F(), A(G(a)));
      }, O = () => E(new u("cancelled"), n !== void 0 || c.cancelOnAbort === !0);
      let X, B, q;
      try {
        if (m(c), c.pollIntervalMs !== void 0 && (typeof c.pollIntervalMs != "number" || !Number.isFinite(c.pollIntervalMs)) && y(), X = c.pollIntervalMs === void 0 ? 100 : Math.max(0, Math.min(1e4, c.pollIntervalMs)), B = c.timeoutMs === void 0 ? 6e5 : L(c.timeoutMs, 1, 864e5), c.onEvent !== void 0 && typeof c.onEvent != "function" && y(), "cancelOnAbort" in c && c.cancelOnAbort !== void 0 && j(c.cancelOnAbort), w = c.signal, w && (typeof w.addEventListener != "function" || typeof w.removeEventListener != "function" || typeof w.aborted != "boolean") && y(), n !== void 0) q = Q(n);
        else {
          const a = H(r);
          f = a.id, s = a.after;
        }
        if (w == null || w.addEventListener("abort", O, { once: !0 }), w != null && w.aborted) {
          O();
          return;
        }
      } catch (a) {
        E(a, !1);
        return;
      }
      z = setTimeout(() => E(new u("timeout"), n !== void 0 || c.cancelOnAbort === !0), B);
      const T = async () => {
        var a;
        if (!(l || !f))
          try {
            const v = await i.read({ id: f, after: s });
            if (l) return;
            const S = U(v, f, s);
            if (_ += S.length, _ > D) throw new u("responseTooLarge");
            const Z = S.filter((b) => b.type === "text").map((b) => b.text).join(""), C = v.text === void 0 ? g + Z : v.text;
            if (C.length > V) throw new u("responseTooLarge");
            g = C, k = v.done;
            for (const b of S) {
              if (s = b.sequence, (a = c.onEvent) == null || a.call(c, b), l) return;
              if (b.type === "error") throw new u(b.error);
              if (b.type === "cancelled") throw new u("cancelled");
            }
            if (s = v.after, v.done) {
              if (v.status !== "completed") throw new u(v.status === "failed" ? "failed" : v.status);
              l = !0, P(), h({ id: f, ...p === void 0 ? {} : { sessionId: p }, text: g });
            } else R = setTimeout(() => {
              T();
            }, X);
          } catch (v) {
            E(v);
          }
      };
      if (!q) {
        T();
        return;
      }
      (async () => {
        try {
          const a = await i.start(q);
          if (!a || typeof a.id != "string" || !a.id.trim() || a.id.length > 512) throw new u("invalidResponse");
          if (f = a.id, l) {
            N && F();
            return;
          }
          if (a.sessionId !== q.sessionId) throw new u("invalidResponse");
          p = a.sessionId, T();
        } catch (a) {
          E(a);
        }
      })();
    });
  }
  return i;
}
export {
  u as AgentError,
  Y as createAgentApi
};
//# sourceMappingURL=agent-factory.js.map
