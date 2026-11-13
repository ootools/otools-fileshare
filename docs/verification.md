# otools-fileshare 验证记录

## 已实测（2026-11-12，本机 macOS arm64）

| 验证 | 命令 / 操作 | 结果 |
|---|---|---|
| Rust 单元及 wire 测试 | `cargo test --locked --manifest-path native/Cargo.toml` | 54 passed，0 failed |
| Rust lint | `cargo clippy --manifest-path native/Cargo.toml -- -D warnings` | 通过；本机采用已有 Homebrew cargo-clippy |
| 前端及打包测试 | `node --test tests/*.test.mjs` | 11 passed，0 failed |
| 两端构建 | `node scripts/build-web.mjs` | `dist/index.html`、`web-static/page_web/index.html` 产出 |
| Native release | `node scripts/build-native.mjs` | `lib/macOS.dylib` 约 4.8MB |
| Native ABI 导出 | `nm -gU lib/macOS.dylib` | 仅 `_otools_plugin_free`、`_otools_plugin_invoke` |
| .oplg 打包及 CRC/SHA 校验 | `pack-plugin.mjs` → `verify-plugin.mjs dist-pack` | `otools-fileshare-0.1.0.oplg`，13 条目，校验通过 |
| 真实安装包 ABI + HTTP 端到端 | `python3 -I scripts/verify-native-e2e.py --package dist-pack/otools-fileshare-0.1.0.oplg --output /tmp/otools-fileshare-native-e2e.json` | 9 项均通过，详见下方 |
| 手机页浏览器实测 | loopback 真实 Rust 服务 + 内置浏览器 | 共享文件/文本正常显示；上传文本后实际列表新增；390px 窄屏已检查并修复按钮挤压 |
| 普通浏览器面板预览 | `http://127.0.0.1:6546` | 显示“请在 OTools 中打开此插件”，未伪造可用宿主 |

构建存在 Vite 大 chunk 提示（Element Plus 主包约 1.2MB minified）；属于体积警告，非构建失败。Rustfmt 初次默认调用因 rustup 组件未安装失败，改用本机已存在的 Homebrew cargo-fmt 后通过；未偷偷安装工具链。

## 实际 .oplg → 动态库 → HTTP 链路

端到端脚本在新的空临时目录中解包，设置隔离 HOME，按宿主 ABI 调用 dynamic library，使用 loopback 临时端口。实测结果：

- [x] FFI `ping`、服务启动返回成功。
- [x] 手机端静态首页及 SPA 深链接 200。
- [x] 中文/空格文件名 Range 下载：206、Content-Range 与字节内容准确。
- [x] 目录 ZIP 响应可解压，内容正确。
- [x] multipart 上传实际落盘并进入共享列表。
- [x] tus 创建 → PATCH 5 字节 → HEAD offset=5 → PATCH 完成，文件内容准确。
- [x] 开启认证后无 token 列表返回 code=401，账号登录返回 token。
- [x] 登录后的列表可读；uploadText=false 的账号无法上传文本。
- [x] 停止服务与 `__otools_shutdown__` 后 listener 不再接受连接。

## 覆盖的边界

- 网卡过滤、无网卡 fallback、IPv6 URL、大小写归一化。
- 文件夹名称 `_1` 去重与同目录复用、并发添加 12 文件不丢条目。
- 中文文本截取、不删除用户文件、局部状态隔离。
- 账号修改权限即时生效、账号删除后旧 token 不再有效、JWT 篡改/过期拒绝。
- 目录 traversal 和逃逸符号链接拒绝，ZIP 不包含 denyAccess 文件。
- 匿名上传默认拒绝、同名上传不覆盖、50MB multipart 流式落盘。
- tus offset 冲突 409、恢复偏移继续发送、客户端完成残留记录不会循环 PATCH。
- SSE 首帧 registry、listChange 广播、停服关闭流。
- 服务失败 bind 不冒充 start，端口更改后旧端口释放/新端口可连接。

## 仍需实际环境验收（未宣称通过）

- [ ] OTools 桌面宿主本地安装 .oplg，确认插件面板和 native 载入。
- [ ] OTools 原生多文件/文件夹对话框、拖放事件、打开方式投递、文本关键词投递。
- [ ] 宿主主题/语言跟随与各窗口尺寸；当前中文/英文切换保留上游设置。
- [ ] 第二台物理设备同网段下载/上传，实际高带宽、多线程、500MB+ 吞吐与长时间断点续传。
- [ ] Windows / Linux native 库与 macOS universal2 的 CI 实际构建。
- [ ] 远程 CI、GitHub Release 和市场发布；本次未推送/发布。
- [ ] 公开再分发前确认 OTools support crates 与字体许可。

独立只读代码审查已完成；所列 P1/P2 问题已通过一次回归修复处理：匿名 token 升权、暂存文件泄露、共享根符号链接替换、tus no-follow 与崩溃恢复、停服并发/迟到写入、错误字符串契约、宿主 listenTauri 事件、SDK 快照跟踪、ZIP64 与危险归档路径、移动端特殊字符路径、后台 opener 等待线程。新增 8 个 Rust 回归和 2 个前端回归，均先失败再通过；修复后全套 54 Rust / 11 JS、lint、构建、包校验与真实 ABI E2E 通过。未再次派审查者复审，不把测试回归称为二次独立审查。

## 文件选择器修复回归（2026-11-13）

- 已复现并修复：SDK 浏览 IPC 指向 `tools_webview_browse_dialog` 且参数为 `{request:{path}}`；宿主 panel 注入脚本和主 API 同步该请求形状，旧宿主注入的错误包装在 Tauri IPC 可用时被 SDK 绕开。
- 宿主列表与读取返回原始元数据，不再误转换为编辑器树节点/纯文本。
- 自绘弹层保持在 webview 内：多文件选择、导航清除旧选项、加载失败禁用确认并允许重试、取消重开、Escape 不向页面的面板关闭处理器转发。
- 文件选中反馈：蓝色背景、左侧选中条、✓ 勾选及已选数量；目录模式突出当前待确认路径，保留单击进入目录的导航规则。新增高亮、勾选取消与目录提示回归测试均先失败再通过。
- SDK 完整测试：221 passed；SDK typecheck 与 build 通过。宿主注入脚本/主 API：6 passed；`vue-tsc --noEmit --pretty false` 通过。
- 插件实际内嵌 SDK 的 `tests/file-chooser.test.mjs` 已先失败、同步快照后通过，验证选择文件以及重开导航选择目录。插件全部测试：13 passed；面板构建通过。
- 浏览器使用实际插件 SDK 的固定目录夹具验证：自绘弹层显示、读取请求嵌套参数、同目录两文件多选返回及弹层关闭。浏览器定位点击工具多次超时，最终使用页面事件调用验证状态；不将此结果称为真实 OTools panel 实机验收。
- 独立只读审查：本次文件树、宿主接口及回归测试未发现新增可操作问题；未修改 native、市场分包或权限配置。
- 本轮提交 SDK `d8f4537` 与宿主 `8f59c100`，插件 v0.1.5 快照从已提交 SDK 源码独立构建，来源见 `vendor/sdk-source.json`。没有夹带另一会话未完成的事件订阅变更。市场发布与六架构 CI 在本轮发版继续验收。

## 磁盘快捷入口增强（2026-11-13）

- 宿主 `webview_fs::dialog_roots` 在原有 `roots` 列表中增加已挂载卷/磁盘入口，不改变插件协议。
- Windows：继续枚举可访问的 A–Z 盘符；macOS：根目录及 `/Volumes` 下的卷；Linux：读取 `/proc/self/mountinfo`，回退 `/proc/mounts`，过滤 proc/sysfs/tmpfs/cgroup 等虚拟文件系统。
- 自动跳过消失的目录，按真实路径去重；支持中文、空格及 Linux 挂载路径八进制转义。没有执行挂载、卸载、格式化或提权操作。
- 新模块独立编译/单测：`rustc --edition 2021 --test crates/ot-platform-core/src/webview_fs/dialog_roots.rs`，10 passed。只对修改的 Rust 文件运行 rustfmt 检查；未运行整套宿主 Cargo 构建。独立审查指出 Unicode 空白及非 UTF-8 挂载记录边界；已补回归并修复，按 ASCII 分隔、逐行处理，单条无法表示的记录不会丢掉其他磁盘。
- SDK 回归：224 passed，包含 Windows/macOS/Linux 磁盘快捷入口点击、当前目录提示与确认；插件前端 13 passed。
- 本机直接运行实际枚举得到 `/` 与 `/Volumes/Recovery`；只读 `mount`/`stat` 核对 Recovery 为已挂载卷，`Macintosh HD -> /` 不重复列出。未连接外置盘做实机热拔插测试；Windows/Linux 的系统挂载枚举用确定性 fixtures 验证，不冒充各平台物理设备测试。
- 新磁盘入口属于宿主原生代码：需要重建/更新 OTools 宿主才能显示，仅更新插件 SDK 不会让旧宿主自动具备枚举能力。

默认 `panel` 打开已在源码及生产清单验证。
