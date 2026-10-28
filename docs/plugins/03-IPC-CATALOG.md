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

| 方法                                                              | 参数                                                  | 返回                                                      |
| ----------------------------------------------------------------- | ----------------------------------------------------- | --------------------------------------------------------- |
| `plugin.initialize`                                               | `protocolVersion, hostVersion?`                       | `pluginId, ready, vfs.operations[]`                       |
| `plugin.ping` / `plugin.shutdown`                                 | 空                                                    | `alive` / `null`                                          |
| `connection.validate`                                             | `connection`                                          | `valid`                                                   |
| `connection.open` / `connection.create`                           | `connectionId/factoryId/connection…`                  | `NavigationTarget`                                        |
| `info.resolve`                                                    | `EventContext.targets`                                | `{ sections: [...] }`                                     |
| `preview.open`                                                    | `EventContext.targets`                                | `PreviewView`                                             |
| `decorations.resolve`                                             | `DecorationContext`                                   | `{decorations:[...]}`                                     |
| `contextMenu.resolve`                                             | `menuSessionId,targets,parentId?`                     | `{items:[...]}`                                           |
| `contextMenu.execute`                                             | `menuSessionId,itemId,interactionId`                  | 操作结果                                                  |
| `search.query`                                                    | `query, root(ref), hidden, ignores, limit`            | `{entries:[...]}`                                         |
| `vfs.list`                                                        | `mountId,nodeId,cursor?,limit,hidden?,ignores?`       | `{entries:[FsEntry],nextCursor?,warnings?}`               |
| `vfs.stat`                                                        | `mountId,nodeId`                                      | `FsEntry`                                                 |
| `vfs.search`                                                      | `mountId,nodeId,query,limit,hidden?,ignores?`         | `{entries:[FsEntry],truncated?}`                          |
| `vfs.openRead` / `vfs.readStream` / `vfs.closeRead`               | `nodeId` / `streamId,offset,maxBytes` / `streamId`    | `{streamId,size?}` / `{base64,eof}` / `null`              |
| `vfs.beginWrite` / `vfs.writeStream` / `vfs.commit` / `vfs.abort` | `nodeId                                               | parentId+name,size?,ifRevision?`/`streamId,offset,base64` | `{streamId}` / `{written}` / `FsEntry` / `null` |
| `vfs.mkdir` / `vfs.rename` / `vfs.remove`                         | `parentId,name` / `nodeId,name?,parentId?` / `nodeId` | `FsEntry` / `FsEntry` / `null`                            |
| `toolbar.invoke`                                                  | `EventContext.targets`                                | 业务结果                                                  |
| `hooks.invoke`                                                    | `event, targets`                                      | `HookResponse`（beforeOpen 已接线）                       |
| `terminal.open`                                                   | `sessionId, cols, rows, connection`                   | `{started:true}`（SSH 插件）                              |

Root `contextMenu.resolve` 返回的 `items` 只包含顶层 action、submenu、separator，不能提供子菜单静态项。用户打开 submenu 时，Core 使用原来的菜单 Session 再次调用 `contextMenu.resolve`，附带 `parentId`；插件只返回该 submenu 的直接子项（不得指定 `parentId`）。Core 校验 ID、层级和会话归属后补充 `parentId` 并展示。菜单解析仅投影允许的元数据，不授予文件内容访问；`contextMenu.execute` 才会附上本次选中资源的操作 Scope。

连接与独立视图共用上表的 `vfs.*`，差别只在请求携带 `connection` 还是
`contributionId`/`viewContext`。完整字段见 [10-UNIFIED-FS](10-UNIFIED-FS.md)。

## 标准事件上下文

Info / Preview 使用相同的目标集合；无需重复的 `target` 和 `selection` 字段，RPC `method` 代表事件类型：

```json
{
  "sessionId": "opaque",
  "targets": [
    {
      "ref": { "mountId": "m1", "nodeId": "n9" },
      "metadata": {
        "name": "song.mp3",
        "kind": "file",
        "size": 2048,
        "mime": "audio/mpeg"
      }
    }
  ]
}
```

资源引用绑定当前 Operation Scope；文件读取仍需 FilesRead 许可，使用 `ctx.files().open_ref(&target.reference)`。

Decorations 返回直接成员名：

```json
{
  "decorations": [
    {
      "name": "main.ts",
      "badge": { "text": "M", "color": "orange" },
      "style": { "color": "#c2710c" }
    }
  ]
}
```

## 插件 → Core

| 方法                                                        | 关键参数             | 许可                              |
| ----------------------------------------------------------- | -------------------- | --------------------------------- |
| `host.fs.stat` / `statBatch`                                | refs                 | `metadata` + Scope                |
| `host.fs.list`                                              | directory ref        | `folders.read` + Scope            |
| `host.fs.openRead` / `readChunk` / `readOpen` / `closeRead` | ref / read handle    | `files.read` + Scope              |
| `host.fs.beginWrite/writeChunk/commit/abort`                | 目标 ref/缓冲区/句柄 | `files.write` 或 `files.create`   |
| `host.fs.mkdir`                                             | parent,name          | `folders.create`                  |
| `host.fs.remove` / `rename`                                 | 目标/源+目标         | 按真实对象类型检查对应权限        |
| `host.net.http.*`                                           | URL/headers/body     | 已批准 Network host               |
| `host.net.tcp.open/readBytes/writeBytes/shutdown/close`     | host/port/stream     | 已批准 Network host，读写独立推进 |
| `host.process.run`                                          | 已声明命令           | `commands`                        |
| `host.storage.*` / `host.secret.*`                          | 插件命名空间键       | 插件进程身份                      |
| `host.vfs.openView`                                         | VFS 资源             | 已注册 `vfs` + MountRegistry      |
| `host.task.*`                                               | 任务/进度/取消       | 当前 operation/task 授权          |

权限被用户关闭时，`host.*` 调用会等待 Core 统一弹出的授权请求（同一插件/权限/目标合并为一个弹窗）。允许后 Core 先持久化授权再继续；拒绝、超时或插件停止会返回 `permission_denied`，调用方不会收到部分结果。Manifest 未声明或超出已批准声明快照的权限不会触发弹窗，直接拒绝。

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
