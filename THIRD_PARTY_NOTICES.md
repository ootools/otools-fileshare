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
