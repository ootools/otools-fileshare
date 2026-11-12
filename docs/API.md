# Native host API

OTools envelope: `{method, payload, pluginDir}` → `{ok:true,data}` or `{ok:false,error:message}`（字符串错误，宿主原生 IPC 只保留该形式）。
面板优先调用 `invokeNativeRaw` 在一处解包；宿主失败时已拒绝 IPC，因此 native 错误用字符串，并由面板统一转换成 `Error(message)`。拖放和文件/关键词投递使用 `listenTauri` + `invokeTauri`，不走插件 native topic。

| Methods | Payload |
|---|---|
| getSetting / getUrl / getServerStatus / startServer / stopServer | null |
| updateSetting | 完整表单：uploadPath, port, authEnable, tusEnable, chunkSize, autoStart, language；可选 allowAnonymousUpload |
| getIp / getIpFamily / getNetInterface | null |
| getNetInterfaceNames | `{family}`，默认 ipv4 |
| getIpAddress | `{idx,family}` |
| getIpAddresses | `{netInterfaceName,family}` |
| setIpFamily | `{ipFamily}` 或字符串 |
| setNetInterface | `{netInterfaceName}` 或字符串 |
| addFile | `{name,path,username?}`，宿主本地路径 |
| addText | `{text,username?}` |
| removeFile | `{name,...}`，只撤销分享，不删除用户文件 |
| listFiles / listAccounts | null |
| addAccount | `{username,password,permissions}` |
| updateAccount | `{id,account:{username?,password?,permissions?}}` |
| updateAccountPassword | `{id,password}` |
| removeAccount | `{id}` 或 id 字符串 |
| getToken | `{permanent?,timeoutSec?}`，默认永久 admin token |
| openFile | `{filename}` 共享条目名称 |
| openExternal | `{url}` 或字符串，仅 HTTP/HTTPS；面板优先宿主 shellOpenExternal |
| getPlatform / getVersion | null |
| openDevTool / closeDevTool | null，返回 not supported |
| ping / poll_events | null |
| __otools_shutdown__ | null，先停止服务与流，再等待 runtime worker 退出 |

Events: `poll_events` → `{ok:true,data:{events:[{topic,payload}]}}`；topic：`fileshare:server.statusChange`、`fileshare:fileDb.listChange`、`fileshare:transfer.progress`。

HTTP：`GET /api/files?path`、`GET /api/download?filename&token&timestamp`、`POST /api/login`、`GET /api/getSetting`、`GET /api/tusConfig`、`GET /api/logout`、`POST /api/addFile`（multipart file）、`POST /api/addText`（JSON message）、`GET /api/registrySSE`。

Tus：`POST /api/uploads`（Upload-Length / Upload-Metadata）、`HEAD /api/uploads/:id`、`PATCH /api/uploads/:id`（Upload-Offset / application/offset+octet-stream）。所有 tus 请求须 `Tus-Resumable: 1.0.0`。断点偏移取实际临时文件长度。匿名写入默认关闭；开启本地 allowAnonymousUpload 或配置账号权限后才可写入。
