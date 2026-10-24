# 02 · Core 挂载与虚拟文件系统

[索引](README.md) · [架构](01-ARCHITECTURE.md) · [缓存与同步](03-CACHE-SYNC.md)

## Contribution 与权限

独立虚拟树声明 `contributions.vfs: [{"id":"history","title":"Git 历史","readOnly":true}]`。
Connection Factory 声明 `contributions.connections`，天然具有内部 VFS 能力，无需重复声明 vfs。
工具栏、右键菜单、预览面板只提供 UI，不获得 VFS 或 Connection Provider 资格。顶层 `capabilities` 分类字段已移除，Manifest 不接受该字段；插件目录也不再重复返回该分类。
`vfs`、`connections`、`toolbar` 中的 `scheme`、`schemes`、`uriPrefix`、`mountId`、`mountPath`、`capabilities` 都会被拒绝。

Contribution 不是磁盘权限。普通文件、HTTP、TCP、命令接口仍按操作作用域及用户授权检查。
Connection 专用 `fs.*` 和 `connection.*` 请求只由 Core 发起；普通 VFS 不得到同步、钉选、释放缓存等权力。
Core 的进程调用层检查 Provider 声明；前端通用 `plugin_invoke` 拒绝直接调用 Provider/host 方法。

## 唯一导航 URI

```text
fiex://mount/<Core-generated-id>/
fiex://mount/<Core-generated-id>/Documents/report.docx
```

Core 编码每个路径段；空格、Unicode、`#`、`?` 和字面 `%` 可逆。URI 解析拒绝遍历、分隔符注入、NUL、非法百分号及嵌套危险编码。
地址是定位符，不是授权令牌；未知、撤销和禁用的挂载返回明确错误，不进入本地 OS 路径分支。
插件不得生成 URI，也不得在 URI 中放仓库路径、凭据或令牌。

`mounts::resolve` 是统一的路由入口：持久 Connection 实例，或者显式 VFS 的轻量 View。
每个连接有独立且持久的随机 mount ID。同工厂的多个账号不共享身份。
侧边栏、地址栏、列表、面包屑及标签页使用相同 URI。
`~/.fiex/drives/<connectionId>` 仅是内部缓存物化目录，云端 Entry.path 使用 URI。
开发版旧缓存路径首次列目录会返回规范 URI；旧历史地址显示“请在 Git 面板重新选择提交并保存标签页/书签”，不保留旧执行通道。

## 创建独立 VFS View

插件先验证业务上下文，再在当前请求中调用：

```rust
let uri = ctx.open_vfs_view(
    "history",
    &repo_root,
    serde_json::json!({"repoRoot": repo_root, "commitId": full_commit_id}),
)?;
```

对应 `host.vfs.openView` 参数为 `contributionId`、`resourceRoot`、`viewContext`。
Core 从 IPC 会话取得调用者身份，检查显式 VFS contribution 和选中资源作用域，生成随机 ID。
`resourceRoot` 必须是本次已选中范围内的真实目录。连接本身不能用隐式 VFS 资格创建自定义 View。

上下文最大 16 KiB，只应保存小型、不可变的业务描述，不存凭据或目录树。
相同 owner/contribution/resource/context 可复用现存 View；不同 Commit 有独立上下文。
创建 View 不读取目录，不复制仓库，也不参与 Connection 同步。

## Provider 读取协议

Core 向已注册 owner 发送：

```json
{
  "contributionId": "history",
  "viewContext": { "repoRoot": "/selected/repo", "commitId": "full-sha" },
  "nodeId": "opaque-provider-node-id",
  "cursor": null,
  "limit": 500
}
```

- `vfs.list` 返回 `entries`、`nextCursor`、可选 `warnings`。每个 Entry 提供 `name`、`nodeId` 和现有 Entry 元信息。根 node ID 是空字符串。
- `vfs.stat` 返回相同 Entry 形状。
- `vfs.readSmall` 接受 `maxBytes`，返回 `base64`；Core 限制单次至 1 MiB，不接受超过请求长度的数据。
- `vfs.search` 接受 query/hidden/ignores/limit，结果 Entry 另外包含相对挂载根的 `relativePath`。

Core 根据当前 mount 和已验证名称绑定 Entry.path，忽略插件自称的路径；nodeId 是不透明标识，不是本地文件授权。
地址栏直达深层节点时只按需列祖先目录以解析 opaque ID。目录项上限 100,000；分页上限 1,000，并拒绝不前进的游标。
独立 View 当前为只读，不显示 Connection 同步动作。Connection 继续使用既有 fs.list/stat/openRead/readStream/writeFile 等协议（无 Manifest 同步或写入模式开关）。

## 生命周期与恢复

每个窗口报告当前、后台标签页、前进/回退历史和分栏引用；取消引用的 View 在 60 秒创建宽限后可回收。
最多保留 256 个 View，每个 View 的节点索引为有界共享内存，不在每次调用中复制目录树。
持久化仅保存明确恢复的标签页、收藏、工作区和“上次路径”所引用的小描述。
普通浏览历史不无限写入磁盘。恢复时重新核查插件、授权与目录，Provider 按固定完整 Commit 读取；不可用时不会退回 HEAD。
多个窗口的引用取并集，窗口销毁时移除内存引用。

## 外部应用与限制

Connection 文件由 Core 复用现有流式下载/缓存/同步；离线元数据不创建虚假 0 字节文件。
已下载文件仍需通过当前 Provider 授权检查才可打开。复制和拖拽显式转换为受管缓存路径。
独立 VFS 的外部预览/打开使用只读临时文件；当前 bounded readSmall 合同只支持不超过 1 MiB 的完整导出，超限明确报错，绝不静默截断。
临时导出最多 64 项、随进程结束清理，关闭单个标签页不会删掉其他消费者正在用的文件。小型文本预览无需导出。

本次不提供 WinFsp、Cloud Files、File Provider 或 FUSE；终端不能 `cd fiex://...`。本地文件路径、USBFS、最近文件仍沿用既有适配器。
