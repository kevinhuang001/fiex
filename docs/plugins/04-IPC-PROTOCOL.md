# 04 · 插件 IPC 协议 v1（拟议）

[索引](README.md) · [上一篇：缓存与同步](03-CACHE-SYNC.md) · [下一篇：UI 扩展](05-UI-EXTENSIONS.md)

> **这是一份待实现的线协议约定**，并非已经上线的 Fiex API。最终定版前应以契约测试覆盖所有消息和边界条件。

## 1. 控制通道与帧格式

插件是独立可执行进程。Core 启动后占用插件 `stdin/stdout` 作为双向 JSON-RPC 2.0 控制通道，`stderr` 专用于诊断日志。帧采用类似 LSP 的 **Content-Length**：

```text
Content-Length: 96\r\n
\r\n
{"jsonrpc":"2.0","id":"req-1","method":"plugin.initialize","params":{...}}
```

`Content-Length` 是后续 UTF-8 JSON **字节长度**，不是字符数；示意的 96 不是上面内容的实际长度。必须有最大帧限制（建议默认 4 MiB）、头部上限、字段类型校验；禁止按换行拆 JSON。消息既可以由 Core 调插件，也可以由插件调 `host.*` 方法，因此两侧均实现 request/response/notification 和相关 ID 空间。响应携带 `result` 或 `error`，不得两者同时存在。

## 2. 握手与生命周期

1. Core 验证插件包后创建进程，启动计时器。
2. Core 请求 `plugin.initialize`，携带 `protocolVersion="1.0"`、宿主版本、允许的 capability/权限、`processInstanceId`、transport 限额及 locale。
3. 插件返回实际 `protocolVersion`、`pluginId`、`capabilities`、`contributions` 与 `ready=true`；不得声明不在已授权 manifest 中的功能。
4. Core 登记贡献，启用 `plugin.ping` / `plugin.shutdown`，保持重启时挂载 ID/配置不变。退出前收敛任务/流。
5. 超时、异常退出时所有 in-flight 请求标记失败或可恢复中断，不得自动视为成功。重复初始化必须幂等。

`pluginId` 在运行时由**已验证安装记录**绑定，不能仅相信插件握手自称的 ID。

## 3. 方法命名空间（v1 草案）

| 方向 | 方法 | 语义 |
| --- | --- | --- |
| Core→插件 | `plugin.initialize / ping / shutdown` | 协商与生命周期 |
| Core→插件 | `plugin.connection.validate / open / close` | 可选连接工厂 |
| Core→插件 | `plugin.fs.list / stat / openRead` | VFS 目录、元数据和内容流 |
| Core→插件 | `plugin.fs.beginWrite / commitWrite / abortWrite` | 分阶段写入 |
| Core→插件 | `plugin.fs.createDir / rename / delete / move / copy` | 按声明能力调用 |
| Core→插件 | `plugin.fs.getChanges / watch / search` | 可选变化与搜索 |
| Core→插件 | `plugin.command.execute` | UI 菜单/工具栏动作 |
| Core→插件 | `plugin.task.cancel` | 取消长任务 |
| 插件→Core | `host.mount.create / remove` | 注册 VFS 挂载 |
| 插件→Core | `host.task.create / progress / complete / fail / cancelled` | 统一任务中心 |
| 插件→Core | `host.secret.request` | 按授权获取凭据 |
| 插件→Core | `host.context.get` | 受限访问当前选择上下文 |

**iframe ↔ UI Host 使用独立版本的 [UI Bridge](11-UI-SDK.md)**（MessageChannel），而不是本文件的 Native stdio JSON-RPC。UI Host 经过鉴权可将 `plugin.invoke` 路由为 `plugin.command.execute`。宿主可额外提供 `host.vfs.*` 以使功能插件通过授权访问别的挂载，但不能绕过本地路径/其他插件的数据授权。对于插件提供的缓存、同步诉求，不应出现 `plugin.cache.*` 或 `plugin.sync.*`：Core 通过通用 VFS 原语执行它们。

## 4. 目录列表请求与响应

```json
{
  "jsonrpc": "2.0",
  "id": "req-42",
  "method": "plugin.fs.list",
  "params": {
    "mountId": "dav-001",
    "directoryId": "root",
    "cursor": null,
    "limit": 200
  }
}
```

```json
{
  "jsonrpc": "2.0",
  "id": "req-42",
  "result": {
    "entries": [
      {
        "nodeId": "doc-1",
        "parentId": "root",
        "name": "Documents",
        "kind": "directory",
        "size": null,
        "version": null
      }
    ],
    "nextCursor": null,
    "snapshotToken": "etag-directory-1"
  }
}
```

Core 对挂载归属、页大小、节点类型、名称长度和合法 parent 进行验证；一页中的节点 ID 不能重复。未知字段按协议的可扩展字段策略安全忽略，不允许插件通过数据 JSON 嵌入可执行指令。

## 5. 数据通道与背压

大文件不能 base64 塞进控制 JSON。`fs.openRead` 和 `fs.beginWrite` 返回一次性 `streamTicket`（包含 `streamId`、过期时间、允许方向、最大范围），Core 在仅本机可连接的 Unix Domain Socket（macOS/Linux）或 Windows Named Pipe 打开二进制分块通道。

- 票据只属于当前进程实例和任务，单次/限时使用；绑定权限与 `mountId/nodeId`，不得连接任意 TCP 地址。
- 数据块采用 `length + sequence + flags + payload` 帧格式；双方处理读写背压，缓存不无限累积，支持取消和超时。
- 完整度依赖长度、校验和（可选但推荐）、写入完成确认。连接中断时不得提交半成品内容。
- 重试/断点续传能力由插件声明 `rangeRead` / `resumableWrite`；不能假定所有协议都支持。

初版可实现宿主创建临时受限管道并在 IPC 传票据，后续可改内部传输实现，但不能破坏上层 FS 原语。

## 6. 请求取消与任务取消

短 RPC 可使用 `$/cancelRequest`（通知，携带请求 ID）；长操作必须映射到 Core 拥有的 `taskId` 并通过 `plugin.task.cancel` 协商终止。Core 先标记 `cancel_requested`，插件清理后发送 `host.task.cancelled`，然后才能在 UI 展示终止完成；无响应时可超时终止进程，并将任务状态设为 `interrupted`，而不能谎报“安全取消”。

## 7. 错误契约

JSON-RPC `error` 采用标准 `code/message/data`，`data` 包含稳定的 `reason`、`retryable`、`taskId` 与可选 `retryAfterMs`。推荐 `reason`：

```text
not_found, permission_denied, auth_required, offline,
unsupported, version_conflict, cancelled, timeout,
rate_limited, invalid_cursor, partial_commit, internal
```

`message` 是面向开发者的诊断，不得包含明文 token/密码。UI 用本地化 reason 提示。可重试错误需有上限和退避，不能对提交、删除等非幂等操作盲重试。

## 8. 版本兼容策略

`protocolVersion` 的 major 不兼容；minor 只增加可忽略字段/可选方法。插件 manifest 声明支持范围，协商取共同版本。Core 不得把未知方法当成成功响应；SDK/CI 使用 golden fixtures 和模拟独立进程互测，覆盖超长帧、错误 JSON、乱序响应、超时、进程崩溃以及网络中断。

下一篇 [UI 扩展](05-UI-EXTENSIONS.md) 规定插件预编译 iframe 资源；[11 UI SDK](11-UI-SDK.md) 独立定义 iframe ↔ Host Bridge 的版本与消息安全。
