# 02 · 虚拟文件系统与目录树

[索引](README.md) · [上一篇：架构](01-ARCHITECTURE.md) · [下一篇：缓存与同步](03-CACHE-SYNC.md)

## 1. 挂载与路径模型

内部统一使用 `fiex://mount/<mountId>/<encoded-segments>` 作为**导航 URI**，真实文件系统路径由 LocalFsAdapter 映射。URI 不代表插件直接控制的 OS 路径；权威资源引用始终是 `(mountId, nodeId)`。用户界面的路径由父子关联与显示名推导。

例如：

```text
fiex://mount/local-001/Documents/
fiex://mount/dav-001/Projects/
fiex://mount/gcal-001/Personal/2026-10/
fiex://mount/git-history-001/abc123/
```

URI 段进行百分号编码和严格规范化；拒绝 `..`、混用分隔符、空字节及其他可能的路径穿越。`mountId` 必须来自宿主挂载表，不允许插件声明另一个插件的挂载。`nodeId` 为不透明 UTF-8 ID，不通过拼接文件名生成资源权限。

## 2. 标准节点数据

```json
{
  "mountId": "dav-001",
  "nodeId": "opaque-124",
  "parentId": "opaque-parent",
  "name": "report.pdf",
  "kind": "file",
  "size": 102400,
  "modifiedAt": "2026-10-21T06:20:00Z",
  "contentType": "application/pdf",
  "version": { "token": "\"etag-xyz\"", "kind": "etag" },
  "capabilities": ["read", "delete", "rename"],
  "attributes": {
    "hidden": false,
    "readonly": false
  }
}
```

- `kind` 可以是 `directory`、`file`、`link`、`virtual`；`virtual` 可带有显示/可打开语义。符号链接必须避免目录环。
- `size`、时间、哈希、MIME 等未知字段使用 `null`/省略，不能伪造为 0。
- `version.token` 必须由提供者原样保留，结合 mount 与 node 解释；内容是否可缓存及版本强度须单独声明。
- 显示名称不是身份；一个远程对象在两个虚拟视图出现时可通过 `objectKey` 关联同一对象，展示节点自身仍具有独立 `nodeId`，避免多父目录冲突。
- Core 必须对名称、总项数、单页大小、递归深度及返回负载加限制；显示不会触发不受控的递归枚举。

## 3. 文件系统能力

每个挂载宣告 `read`、`write`、`create`、`delete`、`rename`、`move`、`serverCopy`、`search`、`watch`、`changeFeed`、`conditionalWrite`、`atomicCommit`、`stableIds` 等能力，且节点可以进一步缩小权限。能力不是自动授权；UI 菜单只有在能力和用户权限均允许时才启用。

**协议中必需的基础读取能力**：`fs.list`（分页）、`fs.stat`；支持读取文件的挂载还实现 `fs.openRead`。所有写入/监听/搜索均按能力启用。只用于展示结构的虚拟目录可没有内容流。Git 历史 VFS 可只读。

## 4. 列表与导航

```text
Explorer navigate → Core VFS.resolve(uri) → NodeIndex cache lookup
                 → (stale/absent) plugin.fs.list(mountId,nodeId,cursor,limit)
                 → Core validates + stores page → UI renders
                 → optional async refresh → diff UI update
```

- 一个目录按页加载（建议默认 200 项；限额由 Core 控制），响应携带 `entries`、`nextCursor`、`snapshotToken`（可空）。
- 并发导航采用请求 ID 与代次号，较早响应不得覆盖新目录；不支持连续一致性游标的插件要容忍目录内容变化导致的重复/遗漏，Core 使用 node ID 去重。
- 离线时呈现已有**元数据快照**并清晰标记快照时间；没有缓存时显示“离线、无法列出”，不创建空目录的假象。
- `watch` 和变化通知只用于使缓存失效或提前刷新；真正的同步基线由 SyncEngine 维护。

## 5. 内容访问和写入原语

`fs.openRead` 返回受限的 `streamId`、已知字节长度、版本；大内容经二进制通道分块流式传送而不塞进 JSON。Core 对读取按需写 CacheManager（缓存校验成功后提交），或直接流向 TransferEngine。支持按偏移读取与断点续传者声明 `rangeRead`。

写入使用两阶段原语：

1. `fs.beginWrite(parentId,name,expectedVersion,mode)` → 暂存流票据。
2. Core 通过二进制通道写入分块，带背压；取消时 `fs.abortWrite` 清理。
3. `fs.commitWrite(ticket,expectedVersion)` → 新节点与版本；能提供强前置条件的插件在远端原子验证版本（如适用 `If-Match`）；否则必须声明弱语义，不允许静默覆盖并冒充安全双向同步。

`fs.delete`、`fs.rename`、`fs.move` 也应支持版本前置条件；不支持的插件可退化为手动确认模式。跨挂载的操作绝不依赖插件私有文件路径。

## 6. 跨挂载传输

```text
Source VFS openRead → Core TransferEngine stream → Destination beginWrite
                              ↓ taskId/progress/checksum
                    Destination commitWrite
                     ↓ verify committed bytes/version
                 If MOVE: source conditional delete
```

- 同挂载优先使用可选原生 `serverMove/serverCopy`；不支持则走通用流。
- 跨挂载移动必须在目标确认成功且持久化后再请求删除源。若删除源失败，报告“目标存在、源未删除”，不可声称移动成功。
- 文件夹操作以逐节点计划执行，允许查看冲突清单。保留现有 `replace/skip/keepBoth` 语义，不把远端默认覆盖当成功。
- 对外拖拽到 Finder/Explorer/浏览器使用 Core 的**按需物化**临时文件/目录，保证系统可访问；跨应用仅复制，不承诺源删除。Core 负责安全清理和任务进度。
- 不允许将未下载的远程节点映射为零字节真实文件占位。否则外部程序可能错误读取或覆盖。

## 7. 具有虚拟目录的功能插件

功能插件也可以注册一个或多个 VFS 挂载。典型 Git：

```text
Git panel → choose commit → plugin asks host.mount.create(...)
      → Git history mount → read-only dirs and blobs
      → Core caches metadata + selected blob content
      → Explorer navigates normal mount URI
```

Git 当前 `history://` 的解析应迁入 Git 插件，Core 只看 `mountId/nodeId`。Google Calendar 可以将日历、月份和事件组成目录，事件内容以 `text/calendar` 导出；导出为文件不代表插件天然拥有双向同步语义。详见 [示例](10-EXAMPLES.md)。

## 8. 本地路径兼容

在迁移期提供 `PathResolver` 将旧本地路径与旧 `~/.fiex/drives/<id>`、`history://` 书签解析为新 VFS 位置；迁移完成后不再让内核中的导航、传输、索引使用“是否 remote / history”判断。保留外部系统文件操作时需要明确物化行为。
