class c extends Error {
  constructor(t, r = t) {
    super(r), this.code = t, this.name = "AiError";
  }
}
function b(n) {
  if (n instanceof Error) return n;
  if (typeof n == "string") return new c(n);
  const t = n;
  return new c(typeof (t == null ? void 0 : t.code) == "string" ? t.code : "transportFailed", typeof (t == null ? void 0 : t.message) == "string" ? t.message : void 0);
}
function C(n, t, r) {
  const i = () => {
    throw new c("invalidResponse");
  };
  (!n || n.id !== t || !Array.isArray(n.events) || n.events.length > 256 || typeof n.done != "boolean") && i();
  let l = !1, m = 0;
  for (const e of n.events)
    switch ((!e || !Number.isSafeInteger(e.sequence) || e.sequence !== ++r || l) && i(), e.type) {
      case "delta":
        typeof e.text != "string" && i(), m += e.text.length, m > 256 * 1024 && i();
        break;
      case "error":
        typeof e.error != "string" && i(), l = !0;
        break;
      case "done":
      case "cancelled":
        l = !0;
        break;
      default:
        i();
    }
  n.done !== l && i();
}
function T(n) {
  if (!n || typeof n != "object" || Array.isArray(n)) throw new c("invalidSettings");
  const t = (r, i) => {
    if (r === null) return null;
    if (typeof r != "string" || r.length > i || !r.trim()) throw new c("invalidSettings");
    return r.trim();
  };
  return { model: t(n.model, 512), reasoningEffort: t(n.reasoningEffort, 64) };
}
function w(n, t) {
  if (!n || typeof n != "object" || Array.isArray(n)) throw new c("invalidRequest");
  const r = n;
  return Object.fromEntries(t.filter((i) => r[i] !== void 0).map((i) => [i, r[i]]));
}
function F(n) {
  const t = async (e, s) => {
    try {
      return await n.invoke(`otools_host_ai_${e}`, s);
    } catch (f) {
      throw b(f);
    }
  }, r = async (e, s) => {
    try {
      return await n.invoke(e, { request: s });
    } catch (f) {
      throw b(f);
    }
  }, i = /* @__PURE__ */ new Map(), l = async (e, s, f) => {
    for (; ; ) {
      const u = i.get(e);
      await u;
      const a = f.model ?? (await t("media_get_settings", { channel: e })).model;
      if (u === i.get(e)) {
        if (typeof a != "string" || !a.trim()) throw new c("modelNotSelected");
        return r(s, { ...f, model: a });
      }
    }
  }, m = {
    media: {
      models: ({ channel: e }) => t("media_models", { channel: e }),
      getSettings: ({ channel: e }) => t("media_get_settings", { channel: e }),
      saveSettings: async ({ channel: e, model: s }) => {
        const f = T({ model: s, reasoningEffort: null }).model, u = i.get(e), a = (async () => (await (u == null ? void 0 : u.catch(() => {
        })), t("media_save_settings", { channel: e, model: f })))();
        return i.set(e, a), a.catch(() => {
        }), a;
      },
      images: (e) => l("images", "otools_ai_images", w(e, ["prompt", "model", "size", "n"])),
      speech: (e) => l("tts", "otools_ai_speech", w(e, ["input", "model", "voice", "responseFormat", "speed"])),
      transcribe: (e) => l("stt", "otools_ai_transcribe", w(e, ["audioBase64", "filename", "model", "language"])),
      embeddings: (e) => l("embeddings", "otools_ai_embeddings", w(e, ["input", "model"])),
      videoSubmit: (e) => l("video", "otools_ai_video_submit", w(e, ["prompt", "model", "image", "duration", "fps", "width", "height", "seed", "metadata"])),
      videoStatus: (e) => r("otools_ai_video_status", w(e, ["taskId", "refresh"])),
      videoContent: (e) => r("otools_ai_video_content", w(e, ["taskId"]))
    },
    capabilities: () => t("capabilities"),
    models: () => t("models"),
    getSettings: () => t("get_settings"),
    saveSettings: async (e) => t("save_settings", { settings: T(e) }),
    start: (e) => t("start", { request: e }),
    read: (e) => t("read", e),
    cancel: (e) => t("cancel", e),
    consume(e, s = {}) {
      return new Promise((f, u) => {
        var M, I;
        let a = !1, g, p = 0, v = "", E, S;
        const A = (o) => {
          m.cancel({ id: o }).catch(() => {
          });
        }, x = () => {
          var o;
          clearTimeout(E), clearTimeout(S), (o = s.signal) == null || o.removeEventListener("abort", _);
        }, y = (o) => {
          a || (a = !0, x(), g && A(g), u(b(o)));
        }, _ = () => y(new c("cancelled"));
        if ((M = s.signal) == null || M.addEventListener("abort", _, { once: !0 }), (I = s.signal) != null && I.aborted) {
          _();
          return;
        }
        S = setTimeout(() => y(new c("timeout")), 600 * 1e3);
        const z = Number.isFinite(s.pollIntervalMs) ? Math.max(0, Math.min(1e4, s.pollIntervalMs)) : 100, k = async () => {
          var o;
          if (!(a || !g))
            try {
              const h = await m.read({ id: g, after: p });
              if (a) return;
              C(h, g, p);
              for (const d of h.events) {
                if (p = d.sequence, d.type === "delta" && (v += d.text, v.length > 4 * 1024 * 1024))
                  throw new c("responseTooLarge");
                if ((o = s.onEvent) == null || o.call(s, d), a) return;
                if (d.type === "error") throw new c(d.error);
                if (d.type === "cancelled") throw new c("cancelled");
              }
              h.done ? (a = !0, x(), m.read({ id: g, after: p }).catch(() => {
              }), f({ id: g, text: v })) : E = setTimeout(() => {
                k();
              }, z);
            } catch (h) {
              y(h);
            }
        };
        (async () => {
          try {
            const o = await m.capabilities();
            if (a) return;
            if (!(o != null && o.configured)) throw new c("notConfigured");
            if (!o.streaming || !o.cancellation) throw new c("unsupported");
            if ((!e.model || e.model === o.model) && o.vision === "unsupported" && e.messages.some((R) => R.content.some((j) => j.type === "image")))
              throw new c("visionUnsupported");
            const d = await m.start(e);
            if (!d || typeof d.id != "string" || !d.id) throw new c("invalidResponse");
            if (g = d.id, a) {
              A(g);
              return;
            }
            k();
          } catch (o) {
            y(o);
          }
        })();
      });
    }
  };
  return m;
}
export {
  c as AiError,
  F as createAiApi
};
//# sourceMappingURL=ai-factory.js.map
