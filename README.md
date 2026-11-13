# OTools File Share · 文件共享

将 [cky-thinker/file-share](https://github.com/cky-thinker/file-share) 移植为 **OTools 独立插件**。原版 Vue 3 / Element Plus 面板与手机页面保留，Node preload 后端改为插件自己的 Rust native 动态库，网络服务不依赖宿主额外功能。

## 功能

- 文件、文件夹和文本共享，分享链接与二维码。
- 局域网 HTTP 直传；单文件支持 Range / 多线程 / 断点下载。
- 文件夹以 ZIP 下载，流式写临时文件，不把目录整体读入内存。
- 手机浏览器上传文件/文本，SSE 列表自动刷新。
- 可选 tus 分片续传（查询服务器偏移后从断点继续）。
- 网卡与 IPv4 / IPv6 切换、端口、上传目录、服务自启。
- 用户名/密码认证，下载、文件上传、文本上传和路径允许/禁止规则。
- 中文 / English；通过 OTools 文件选择与文件投递集成。

## 本地安装

在 OTools 市场更新，或从 [GitHub Release](https://github.com/ootools/otools-fileshare/releases) 下载对应平台/架构的包本地安装。默认以 **panel** 打开（`openMode: "panel"`）。从 v0.1.4 起仅发布六种分包，`packageMode=1`，每包只包含自己的动态库：

- `-macos.oplg`：macOS ARM64；`-macos-intel.oplg`：macOS Intel。
- `-windows.oplg`：Windows x64；`-windows-arm.oplg`：Windows ARM64。
- `-linux.oplg`：Linux x64；`-linux-arm.oplg`：Linux ARM64。

v0.1.5 修复页面内文件树的宿主请求契约，加入多选、高亮勾选、目录提示与错误重试。磁盘快捷入口是在 OTools 宿主中实现的，宿主须包含 `8f59c100` 或更新代码并重建，不能只靠插件升级获得新磁盘枚举能力。

插件装包后的入口为 `dist/index.html`，原生库为 `lib/macOS.dylib` / `Windows.dll` / `Linux.so`。手机页面随包放在 `web-static/page_web/`，由 native 服务器托管。

普通浏览器打开面板只用于界面预览，会显示需要 OTools 宿主的提示，**不会伪造可用的文件服务**。接收方浏览器无需 OTools。

## 开发和构建

要求 Node.js 22、pnpm 10.15.1、稳定版 Rust（本机验证 rustc 1.95.0）。

```bash
pnpm install --frozen-lockfile
pnpm test
pnpm test:native
node scripts/build-web.mjs
node scripts/build-native.mjs
node scripts/pack-plugin.mjs --platform macos --arch arm64 --out dist-pack
node scripts/verify-plugin.mjs dist-pack/macos
```

- 面板开发：`pnpm dev`，`http://127.0.0.1:6546`（宿主 quickDev 指向该地址）。
- `build-web.mjs` 统一构建两端：`web/ → dist/`，`web-mobile/ → web-static/page_web/`。
- Rust crate：`native/`，独立 workspace，`panic = "unwind"`，只导出 `otools_plugin_invoke` / `otools_plugin_free`。
- 小型宿主依赖快照在 `vendor/host-crates/`，版本见 `source.json`；无需克隆相邻的 OTools 宿主仓库。
- API、事件和 wire contract：[`docs/API.md`](docs/API.md)。

本机原生端到端测试（仅 macOS；临时 HOME 与 loopback 端口，自动清理，不触碰真实用户配置）：

```bash
python3 -I scripts/verify-native-e2e.py \
  --package dist-pack/macos/otools-fileshare-0.1.5-macos.oplg \
  --output /tmp/otools-fileshare-native-e2e.json
```

## 安全与兼容差异

- **匿名上传默认关闭。** 未开启认证时仍可匿名浏览/下载；需要在本机设置明确开启匿名上传，或开启认证并配置账号权限，才能通过局域网写入。
- 上传同名文件会生成新名称（例如 `name (1).txt`），不覆盖现有文件。移除分享只撤销列表条目，不删除原文件。
- 请求路径被限制在明确共享的文件/目录内；`..`、非法路径和逃逸符号链接被拒绝。ZIP 逐项应用路径权限，不包含禁止访问文件。
- 账号密码保留上游的本地存储形式；当前 HTTP 为明文局域网协议，**只在可信局域网使用，勿做公网端口映射**。上线前可进一步改进密码存储与 TLS。
- token 密钥仅驻留插件进程，宿主重启或原生库重新加载后旧 token 失效。
- 因 OTools native 调用异步，面板调用处改为 `await`；桥接层保留原版 `window.api` 方法名并正常解包。
- 上游未实际实现的 tus 开关，在本移植版接为真正的 POST / HEAD / PATCH 续传服务。

## 发布与验证状态

CI 使用 Windows x64/ARM64、Linux x64/ARM64、macOS ARM64/Intel 六个 runner。只打各自架构分包，不再发布 universal2 或三平台合包；市场使用六个包地址并设 `packageMode=1`。

验收实测与仍需补测事项见 [`docs/verification.md`](docs/verification.md)。页面内文件选择器保持在 panel 的 webview 内，不弹出会夺焦的原生对话框。实际宿主面板、外置盘热拔插和第二台设备 LAN 吞吐仍需实机验证；不以普通浏览器模拟结果代替宿主验收。

来源/许可：见 [`LICENSE`](LICENSE)、[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)。宿主快照与字体的公共再分发许可需在正式发布前确认。
