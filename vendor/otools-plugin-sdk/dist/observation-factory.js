class c extends Error {
  constructor(s, i) {
    super(i), this.code = s, this.name = "ObservationError";
  }
}
function g(a) {
  if (a instanceof c) return a;
  let s = a;
  if (typeof s == "string")
    try {
      s = JSON.parse(s);
    } catch {
    }
  const i = s && typeof s == "object" ? s : void 0;
  return new c((i == null ? void 0 : i.code) ?? "unsupported", (i == null ? void 0 : i.message) ?? String(a));
}
function u() {
  throw new c("invalidRequest", "Provide an explicit target and a sampling interval of at least 30000 ms");
}
function o(a) {
  (typeof a != "string" || !a || a.length > 128) && u();
}
function d(a, s) {
  if (!a || typeof a.id != "string" || s && a.id !== s || !["pending", "active", "stopped"].includes(a.state))
    throw new c("invalidResponse", "Invalid observation status");
  return a;
}
function f(a) {
  const s = /* @__PURE__ */ new Map(), i = async (t, n) => {
    try {
      return await a.invoke(`otools_host_observation_${t}`, n);
    } catch (e) {
      throw g(e);
    }
  }, l = async ({ id: t }) => {
    o(t);
    const n = d(await i("status", { id: t }), t);
    if (n.state === "stopped") {
      const e = s.get(t);
      e && (e.cancelled = !0);
    }
    return n;
  };
  return {
    targets: () => i("targets"),
    async request(t) {
      const n = t == null ? void 0 : t.target, e = (n == null ? void 0 : n.kind) === "screen" ? n.displayId : (n == null ? void 0 : n.kind) === "window" ? n.windowId : void 0, r = (t == null ? void 0 : t.intervalMs) ?? 6e4;
      return (typeof e != "string" || !/^[1-9]\d{0,9}$/.test(e) || Number(e) > 4294967295 || !Number.isSafeInteger(r) || r < 3e4 || r > 36e5) && u(), d(await i("request", { options: { target: n, intervalMs: r } }));
    },
    status: l,
    async capture({ id: t }) {
      if (o(t), s.has(t) || s.size >= 32) throw new c("busy", "An observation capture is already in progress");
      const n = { cancelled: !1 };
      s.set(t, n);
      let e, r = !1;
      try {
        if (e = await i("capture", { id: t }), !e || typeof e.imageId != "string" || !e.imageId || e.imageId.length > 128 || e.mimeType !== "image/png" || !Number.isSafeInteger(e.width) || e.width <= 0 || !Number.isSafeInteger(e.height) || e.height <= 0 || e.width * e.height > 64e6 || !Number.isFinite(e.expiresAt)) throw new c("invalidResponse", "Invalid observation image");
        if (n.cancelled || (await l({ id: t })).state !== "active" || n.cancelled)
          throw new c("stopped", "Observation stopped before capture delivery");
        return r = !0, e;
      } finally {
        if (s.delete(t), !r && typeof (e == null ? void 0 : e.imageId) == "string" && e.imageId.length <= 128)
          try {
            await a.invoke("otools_host_capture_release", { imageId: e.imageId });
          } catch {
          }
      }
    },
    async stop({ id: t }) {
      o(t);
      const n = s.get(t);
      n && (n.cancelled = !0), await i("stop", { id: t });
    }
  };
}
export {
  c as ObservationError,
  f as createObservationApi
};
//# sourceMappingURL=observation-factory.js.map
