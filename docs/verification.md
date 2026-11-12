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

默认 `panel` 打开已在源码及生产清单验证。源码在 main 工作区，当前新增实现未提交，不覆盖其他仓库已有改动。
