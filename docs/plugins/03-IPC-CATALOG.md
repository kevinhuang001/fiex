# 03 · Native IPC 协议

## 传输

Core 与 native 插件进程使用双向 JSON-RPC 2.0：

- stdout/stderr 只用于协议帧，日志走 stderr；
- 帧格式为 `Content-Length` header + JSON；
- `id` 关联并发请求与响应；
- 支持 `plugin.initialize`、`plugin.ping`、`plugin.shutdown`；
- 插件在处理请求时可以反向调用 `host.*`；
- 大文件使用有界 buffer/handle，不整文件内联 JSON。

## Core → 插件

| 方法                                    | 参数                                  | 返回                                     |
| --------------------------------------- | ------------------------------------- | ---------------------------------------- |
| `plugin.initialize`                     | `protocolVersion, hostVersion?`       | `pluginId, ready`                        |
| `plugin.ping` / `plugin.shutdown`       | 空                                    | `alive` / `null`                         |
| `connection.validate`                   | `connection`                          | `valid`                                  |
| `connection.open` / `connection.create` | `connectionId/factoryId/connection…`  | `NavigationTarget`                       |
| `info.resolve`                          | `target / selection / options`        | `{ sections: [...] }`                    |
| `preview.open`                          | `target / selection / options`        | `PreviewView`                            |
| `decorations.resolve`                   | `DecorationContext`                   | `{decorations:[...]}`                    |
| `contextMenu.resolve`                   | 菜单 Session、opaque refs             | `{items:[...]}`                          |
| `contextMenu.execute`                   | `menuSessionId,itemId,interactionId`  | 操作结果                                 |
| `search.query`                          | `query, root, hidden, ignores, limit` | `{entries:[...]}`                        |
| `toolbar.invoke`                        | `root, selection`                     | 业务结果                                 |
| `hooks.invoke`                          | `event, resource…`                    | `HookResponse`（生命周期接线未全部完成） |
| `terminal.open`                         | `sessionId, cols, rows, connection`   | `{started:true}`（SSH 插件）             |

## 标准事件上下文

Info / Preview:

Standard resource requests and permission projection are specified in [08](08-UNIFIED-PREVIEW-INFO.md).

## 插件 → Core

| 方法                                                    | 关键参数                  | 许可                              |
| ------------------------------------------------------- | ------------------------- | --------------------------------- |
| `host.fs.stat` / `statBatch`                            | refs                      | `metadata` + Scope                |
| `host.fs.list`                                          | directory ref             | `folders.read` + Scope            |
| `host.fs.openRead` / `readChunk`                        | ref / read handle         | `files.read` + Scope              |
| `host.fs.read/readOpen/closeRead`                       | resource/handle           | 读取所有权 + Scope                |
| `host.fs.beginWrite/writeChunk/commit/abort`            | 目标 ref/缓冲区/句柄      | `files.write` 或 `files.create`   |
| `host.fs.mkdir`                                         | parent,name               | `folders.create`                  |
| `host.fs.remove` / `rename`                             | 目标/源+目标              | 按真实对象类型检查对应权限        |
| `host.net.http.*`                                       | URL/headers/body          | 已批准 Network host               |
| `host.net.tcp.open/readBytes/writeBytes/shutdown/close` | host/port/stream          | 已批准 Network host，读写独立推进 |
| `host.process.run` / `host.exec`                        | 已声明命令                | `commands`                        |
| `host.storage.*` / `host.secret.*`                      | 插件命名空间键            | 插件进程身份                      |
| `host.vfs.openView`                                     | VFS 资源                  | 已注册 `vfs` + MountRegistry      |
| `host.terminal.*`                                       | `sessionId`、base64、尺寸 | 终端 Session owner（仅 SSH 插件） |
| `host.task.*`                                           | 任务/进度/取消            | 当前 operation/task 授权          |

## Connection 导航

只接受两种结果：

```json
{ "kind": "vfs", "mountId": "m1", "nodeRef": { "mountId": "m1", "nodeId": "root" } }
```

```json
{
  "kind": "pluginView",
  "view": { "source": "package", "entry": "ui/home.html" },
  "state": { "connectionId": "c1" },
  "tabTitle": "账号"
}
```

`pluginView` 的 `tabTitle` 最多 80 字符且不能含控制字符；没有时使用连接名称。`kind` 不接受 `dialog`、`panel`、`externalUrl`、`modal` 等其他值。

## 错误

标准错误码：

- `invalid_params`
- `method_not_found`
- `permission_denied`
- `conflict`
- `cancelled`
- `deadline_exceeded`
- `unavailable`

权限失败不能伪装成空目录或空结果；错误必须能让用户定位到插件和操作。
