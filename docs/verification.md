# 验证记录（otools-fileshare）

手动 / 宿主内验证逐项记录在此；每项带命令与预期结果。自动化的 wire 测试结果见各任务 commit。

## Task 2 冒烟（native FFI 骨架）

- [ ] `node scripts/build-native.mjs` 产出 `lib/macOS.dylib`（本机 arm64；CI 阶段换 universal2）。
- [ ] 宿主 dev 模式（quickDev，devUrl `http://127.0.0.1:6546`）加载插件后，宿主日志无 native 装载报错。
- [ ] 占位页 `web/index.html` 控制台执行 `await window.otools.invokeNative('ping')` 返回 `{ok: true, data: "pong"}`（临时页可先 `alert(JSON.stringify(...))` 弹出 pong）。
- [ ] `await window.otools.invokeNative('poll_events')` 返回 `{"ok":true,"data":{"events":[...]}}` 形状（队列可以为空数组）。
