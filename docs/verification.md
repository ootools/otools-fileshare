# 验证记录（otools-fileshare）

手动 / 宿主内验证逐项记录在此；每项带命令与预期结果。自动化的 wire 测试结果见各任务 commit。

## Task 2 冒烟（native FFI 骨架）

- [ ] `node scripts/build-native.mjs` 产出 `lib/macOS.dylib`（本机 arm64；CI 阶段换 universal2）。
- [ ] 宿主 dev 模式（quickDev，devUrl `http://127.0.0.1:6546`）加载插件后，宿主日志无 native 装载报错。
- [ ] 占位页 `web/index.html` 控制台执行 `await window.otools.invokeNative('ping')` 返回 `{ok: true, data: "pong"}`（临时页可先 `alert(JSON.stringify(...))` 弹出 pong）。
- [ ] `await window.otools.invokeNative('poll_events')` 返回 `{"ok":true,"data":{"events":[...]}}` 形状（队列可以为空数组）。

## Task 3 冒烟（存储层与 Setting 模块）

- [ ] `await window.otools.invokeNative('getSetting')` 返回 9 键聚合：`{uploadPath, port, ip, url, authEnable, tusEnable, chunkSize, autoStart, language}`；首装默认 `port=5421`、`authEnable=false`、`tusEnable=false`、`chunkSize=20`、`autoStart=false`、`ipFamily` 侧文件为 `ipv4`、`language=null`、`uploadPath`=系统下载目录（url 形如 `http://127.0.0.1:5421`，Task 4 接真实 IpUtil 前 ip 恒为 127.0.0.1）。
- [ ] 面板设置页把端口改成 6000 → `updateSetting` resolve `{success:true,message:'修改成功'}`；随后 `getSetting().url` 变为 `http://…:6000`（端口变更重启服务在 Task 8 接入，本阶段 URL 已跟随端口）。
- [ ] 设置持久化跨重载：改端口 / 开启认证 / 换语言后重启宿主（或重载插件）再 `getSetting`，值保持；落盘位置 `~/.otools/local/otools-fileshare/state.json`。
- [ ] 空值校验 reject 形状：`await window.otools.invokeNative('updateSetting', {})` reject `{success:false,message:'更新上传路径失败，路径为空'}`（Promise.all 语义：完整表单才全过；面板永远发全量 settingForm，日常不触发）。
- [ ] 传文件路径给 `updateUploadPath` → reject `{success:false,message:'上传路径必须为文件夹'}`；不存在的路径 → reject `文件夹不存在`；相同目录重复提交 → resolve `ValueNotChange`。
- [ ] 语言只认 zh/en：`updateSetting` 带其它语言值整体 reject `不支持的语言`，且 zh/en 切换后 `getSetting().language` 生效。
