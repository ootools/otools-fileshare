class l extends Error {
  constructor(n, t) {
    super(t), this.code = n, this.name = "CaptureError";
  }
}
const S = ["unsupported", "permissionDenied", "busy", "cancelled", "timeout", "notFound", "invalidRequest", "forbidden", "captureFailed", "outputFailed"];
function x(o) {
  if (o instanceof l) return o;
  let n = o;
  if (typeof n == "string")
    try {
      n = JSON.parse(n);
    } catch {
    }
  const t = n && typeof n == "object" ? n : void 0, r = (t == null ? void 0 : t.message) || String(o), p = t != null && t.code && S.includes(t.code) ? t.code : /runtime.*unavailable|bridge.*unavailable|not supported|unknown command|command .*not found/i.test(r) ? "unsupported" : "captureFailed";
  return new l(p, r);
}
function k(o) {
  const n = async (t, r) => {
    try {
      return await o.invoke(`otools_host_capture_${t}`, r);
    } catch (p) {
      throw x(p);
    }
  };
  return {
    capabilities: () => n("capabilities"),
    permissionStatus: () => n("permission_status"),
    requestPermission: () => n("request_permission"),
    repairPermission: () => n("repair_permission"),
    screen: (t = {}) => n("screen", { options: t }),
    region: (t) => n("region", { options: t }),
    window: (t = {}) => n("window", { options: t }),
    pin: (t) => n("pin", { options: t }),
    retain: (t) => n("retain", { imageId: t }),
    release: (t) => n("release", { imageId: t }),
    select(t = {}, r) {
      return new Promise((p, c) => {
        let i = !1, a, d, s, u = !1;
        const w = /* @__PURE__ */ new Set(), v = (e) => {
          (e == null ? void 0 : e.status) === "completed" && e.captureId === a && !w.has(e.image.imageId) && (w.add(e.image.imageId), n("release", { imageId: e.image.imageId }).catch(() => {
          }));
        }, b = (e) => {
          n("cancel", { captureId: e }).then(() => n("result", { captureId: e })).then(v).catch(() => {
          });
        }, I = () => {
          const e = d;
          if (d = void 0, e)
            try {
              Promise.resolve(e()).catch(() => {
              });
            } catch {
            }
        }, f = (e, m) => {
          i || (i = !0, clearTimeout(B), clearTimeout(s), r == null || r.removeEventListener("abort", h), I(), m ? (a && b(a), c(x(m))) : p(e));
        }, h = () => f(void 0, new l("cancelled", "Capture aborted")), B = setTimeout(() => f(void 0, new l("timeout", "Capture timed out")), 600 * 1e3), y = async () => {
          if (!(i || !a || u)) {
            clearTimeout(s), u = !0;
            try {
              const e = await n("result", { captureId: a });
              if (i) {
                v(e);
                return;
              }
              if (e) {
                if (e.captureId !== a || !["completed", "cancelled", "error"].includes(e.status))
                  throw new l("captureFailed", "Invalid capture result");
                f(e);
              }
            } catch (e) {
              f(void 0, e);
            } finally {
              u = !1, i || (s = setTimeout(() => {
                y();
              }, 250));
            }
          }
        };
        if (r == null || r.addEventListener("abort", h, { once: !0 }), r != null && r.aborted) {
          h();
          return;
        }
        (async () => {
          try {
            if (o.listen)
              try {
                d = await o.listen("otools-capture-result", (m) => {
                  var g;
                  ((g = m.payload) == null ? void 0 : g.captureId) === a && y();
                });
              } catch {
              }
            if (i) {
              I();
              return;
            }
            const e = await n("begin", { options: t });
            if (!(e != null && e.captureId)) throw new l("captureFailed", "Missing capture id");
            if (a = e.captureId, i) {
              b(a);
              return;
            }
            y();
          } catch (e) {
            f(void 0, e);
          }
        })();
      });
    },
    async readImage(t) {
      let c = 0, i;
      do {
        const a = await n("read_chunk", { imageId: t, offset: c, length: 1048576 }), d = () => new l("captureFailed", "Invalid capture image chunk");
        if (!a || !Number.isSafeInteger(a.totalBytes) || a.totalBytes <= 0 || a.totalBytes > 67108864 || !Number.isSafeInteger(a.nextOffset) || a.nextOffset <= c || a.nextOffset > a.totalBytes || a.nextOffset - c > 1048576 || typeof a.dataBase64 != "string" || a.dataBase64.length > Math.ceil(1048576 / 3) * 4 || i && i.length !== a.totalBytes) throw d();
        let s;
        try {
          s = atob(a.dataBase64);
        } catch {
          throw d();
        }
        if (s.length !== a.nextOffset - c) throw d();
        i ?? (i = new Uint8Array(a.totalBytes));
        for (let u = 0; u < s.length; u++) i[c + u] = s.charCodeAt(u);
        c = a.nextOffset;
      } while (c < i.length);
      return i;
    }
  };
}
export {
  l as CaptureError,
  k as createCaptureApi
};
//# sourceMappingURL=capture-factory.js.map
