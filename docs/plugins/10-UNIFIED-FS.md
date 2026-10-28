# 10 · 统一文件系统协议（protocol 5）

Core 与所有文件系统插件之间只有一套 `fs.*` 协议。连接（SMB、WebDAV、Drive、OpenList）
与独立虚拟视图（Git 等）使用相同的方法、相同的 DTO 和相同的流语义；差别只在
请求携带的身份块（`connection` 或 `contributionId`/`viewContext`）。

## 1. 身份与寻址

- Core 分配 `mountId`，插件从不生成或改写它。
- `nodeId` 由插件定义，对 Core 不透明，在同一 mount 内稳定；空字符串表示 mount 根。
- Core 维护 `nodeId <-> 相对路径` 绑定（用于显示和缓存），Response 中的 `nodeId` 是权威身份。
- 请求统一是 `ResourceRef { mountId, nodeId }` 的投影；宿主路径（`path`/`from`/`to`）永远不会出现在
  Core → 插件的 `fs.*` 请求中。

每个请求都带：

```json
{
  "mountId": "…",
  "nodeId": "…",
  "connection": { "settings": {}, "secrets": {}, "networkDomains": [] }
}
```

或（独立视图）：

```json
{ "mountId": "…", "nodeId": "…", "contributionId": "…", "viewContext": {} }
```

## 2. 方法

| 方法             | 参数                                                | 返回                                             |
| ---------------- | --------------------------------------------------- | ------------------------------------------------ |
| `fs.list`        | `mountId,nodeId,cursor?,limit,hidden?,ignores?`     | `{entries:[FsEntry],nextCursor?,warnings?}`      |
| `fs.stat`        | `mountId,nodeId`                                    | `FsEntry`                                        |
| `fs.search`      | `mountId,nodeId,query,limit,hidden?,ignores?`       | `{entries:[FsEntry 含 relativePath],truncated?}` |
| `fs.openRead`    | `mountId,nodeId`                                    | `{streamId,size?}`                               |
| `fs.readStream`  | `streamId,offset,maxBytes`                          | `{base64,eof}`                                   |
| `fs.closeRead`   | `streamId`                                          | `null`                                           |
| `fs.beginWrite`  | `mountId,nodeId?,parentId?,name?,size?,ifRevision?` | `{streamId}`                                     |
| `fs.writeStream` | `streamId,offset,base64`                            | `{written}`                                      |
| `fs.commit`      | `streamId`                                          | `FsEntry`（可为 `{}`）                           |
| `fs.abort`       | `streamId`                                          | `null`                                           |
| `fs.mkdir`       | `mountId,parentId,name`                             | `FsEntry`                                        |
| `fs.rename`      | `mountId,nodeId,name?,parentId?`                    | `FsEntry`                                        |
| `fs.remove`      | `mountId,nodeId`                                    | `null`                                           |

`fs.list`/`fs.search` 每页最多 500 项、每次调用最多 1000 页；Core 超出即拒绝。

### FsEntry

```json
{
  "nodeId": "opaque",
  "name": "report.pdf",
  "kind": "file",
  "size": 2048,
  "modified": 1730000000,
  "extension": "pdf",
  "mime": "application/pdf",
  "revision": "etag-or-version",
  "readonly": false,
  "linkTarget": null,
  "allowedOps": ["read", "rename", "remove"]
}
```

- `kind`: `file` | `directory` | `symlink`。
- `revision`: 内容变化时必须变化，用于 `ifRevision` 条件写；不支持的协议留空。
- `allowedOps`: 该节点的可用操作子集，不能超出插件声明的能力。
- 名字必须通过 Core 的路径段校验（不含 `/ \ : NUL`，不是 `.`/`..`）。

## 3. 能力声明

`plugin.initialize` 的回复带上能力：

```json
{
  "pluginId": "org.fiex.gdrive",
  "ready": true,
  "fs": { "operations": ["list", "stat", "read", "write", "rename"] }
}
```

操作名固定为 `list|stat|read|write|mkdir|rename|remove|search`。
`read` 覆盖 `openRead/readStream/closeRead`，`write` 覆盖 `beginWrite/writeStream/commit/abort`。
SDK 的 `FSProvider::operations()` 显式报告实现上限，`discover(ctx)` 按当前连接与资源的服务器权限进一步收窄。
Core 保存该集合；未声明的操作用 `method_not_found` 拒绝，不会到达插件。

三层能力求交：

1. Provider 能力（初始化声明）；
2. Mount/Node 能力（`allowedOps`、`readonly`）；
3. Core 授权（PermissionPolicy 的 Grant 与 Operation Scope）。

## 4. 流

- 读流由 `openRead` 创建，`readStream` 按绝对 `offset` 前进，禁止后退；`eof` 在读到 0 字节时为真。
- 单块上限 4 MiB，Core 默认按 256 KiB（普通 IPC）或 4 MiB（共享内存窗口）请求。
- 写流只能顺序前进（`offset` 必须等于已写字节数），`commit` 后文件才对其他读者可见；
  `abort`、进程退出或句柄失效都会丢弃未提交内容。
- 插件在 `write` 中返回标准 `Write`；协议特有的收尾（SMB 暂存改名、Drive resumable 完成）
  放在 commit 回调里，`commit` 返回最终 `FsEntry`。
- Core 只用发起调用的那次请求授权 host 资源，并在请求返回时撤销。读流通常由 HTTP 响应体、
  TCP 连接或文件句柄支撑，而 `readStream`/`writeStream`/`commit` 都是后续请求，因此 SDK 把
  每个流放在一个内部后台任务会话里创建：句柄在流关闭前始终有效，关闭时任务结束、句柄回收。
  插件不需要自己延长授权，也不要跨请求保存 `RequestContext`。

## 5. 权限与执行

- 菜单是否显示只是 UI 决策；执行时必须重新校验能力、Scope 和节点状态。
- 本地插件只能通过 `ctx.files()` 访问 Core 授权过的 `ResourceRef`；不存在宿主路径 API。
- 插件之间不直接调用；所有 `fs.*` 都由 Core 路由并记录到 Operation Scope。

## 6. 迁移状态

protocol 4 的 `vfs.list/stat/readSmall/search` 与 `fs.writeFile` 均已删除。
`vfs.openView` 仍保留，它创建的是导航视图，不是文件操作。

Core 只提供**新建文件夹**（`fs.mkdir`）。按名称新建空文件没有协议操作：
`fs.beginWrite` 只针对已存在的 node，因此快捷键/菜单不提供“新建文件”，
本地目录也同样只新建文件夹。

## 显式发现权限

SDK 使用普通 `FSProvider` trait，不再使用过程宏。`operations()` 必须显式声明实现的
操作上限，`discover(ctx)` 按当前连接与资源返回服务器允许的子集。
初始化回复中的 `fs.discovery: true` 启用 `fs.discover`；请求携带与目标操作相同的
mount、connection/view 和 node/parent 参数，返回 `{operations: [...]}`。
Core 与 SDK 均检查此集合；未报告 discovery 的旧协议 5 插件保留静态能力行为。
完整接口规范见 SDK 的 `docs/fs-provider-discovery.md`。
