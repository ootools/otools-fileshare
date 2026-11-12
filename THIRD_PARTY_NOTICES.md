# Third-Party Notices

## cky-thinker/file-share

This plugin (`otools-fileshare`) is a **port** of
[cky-thinker/file-share](https://github.com/cky-thinker/file-share)
(uTools 插件 + Electron 独立应用的局域网文件共享工具) to the OTools plugin platform.

- Upstream project: <https://github.com/cky-thinker/file-share>
- Upstream license: **Apache-2.0**
- The exact upstream source snapshot this port is based on is recorded in
  [`upstream.pin`](./upstream.pin) (field `commit`, a full 40-digit SHA-1 of the
  upstream repository at porting time).

The complete Apache License 2.0 text, as shipped by the upstream project, is
reproduced in [`LICENSE`](./LICENSE) and applies to this plugin's code as well.
Ported code may deviate from the upstream implementation where the OTools
host requires it (e.g. the Node.js backend is rewritten in Rust as a native
cdylib, and the `window.api` bridge is remapped onto `window.otools.invokeNative`);
the upstream project remains the origin of all ported design and UI.

版权与许可说明：本插件移植自 Apache-2.0 项目 cky-thinker/file-share，来源 commit
见 `upstream.pin`；Apache-2.0 完整许可文本随本仓库 `LICENSE` 分发。

## OTools host support crates and SDK

- `vendor/host-crates/`: `ot-plugin-ffi` and `ot-plugin-state` unmodified; `ot-storage` carries one local patch (OTOOLS_HOME env override in `home_dir()`, needed for cross-platform test isolation — see `source.json`), from `ootools/OTools` commit `6a529eac75967d2b84fb69ec3901ceff98ef8a3c`. These remain OTools host code, not newly relicensed by this repository's Apache-2.0 LICENSE. The host snapshot does not declare a root license: confirm public redistribution terms before publishing.
- `vendor/otools-plugin-sdk/`: existing built SDK snapshot reused from this workspace; its own LICENSE applies (included).
- Vue, Element Plus, Vite, qrcode.vue and other dependencies retain their original licenses; see the pnpm lockfile and each installed package's license.
- Upstream DancingScript font is retained as supplied with file-share; confirm its font attribution and redistribution terms before a public release.
