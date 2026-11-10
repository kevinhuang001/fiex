# 虚拟文件系统 VFS

当插件需要把远程服务、归档或应用私有数据呈现为文件夹和文件时，实现 `VfsProvider`。本地磁盘 provider 不需要插件机制；它属于 Core。

## 身份与路径

Core 拥有挂载 ID、Fiex URI 和 provider 路由。插件处理的是当前挂载内的 `nodeId`，以及 SDK 传入的 `FsRequest` / `ResourceRef`。`nodeId` 是 provider 自己的不透明标识，不是宿主机路径；不要拼造 `fiex://` URI，也不要把用户输入当作本机绝对路径。

同一个 filesystem provider 可以服务多个账户或连接。用当前请求中的 `mountId`/connection context 找到对应远程会话，不要用进程级“当前账户”变量把并发请求混在一起。

## 声明实际支持的操作

`VfsProvider::operations()` 声明实现上限，逻辑操作如下：

| 操作 | 方法 | 说明 |
| --- | --- | --- |
| `list` | `vfs.list` | 列出一个目录的直接子项 |
| `stat` | `vfs.stat` | 获取节点元数据 |
| `read` | `vfs.openRead`、`vfs.readStream`、`vfs.closeRead` | 读取文件流 |
| `write` | `vfs.beginWrite`、`vfs.writeStream`、`vfs.commit`、`vfs.abort` | 暂存并提交文件写入 |
| `mkdir` | `vfs.mkdir` | 创建目录 |
| `rename` | `vfs.rename` | 重命名/移动 provider 内项目 |
| `trash` | `vfs.trash` | 移入 provider 可恢复的废纸篓 |
| `remove` | `vfs.remove` | 永久删除 |
| `search` | `vfs.search` | provider 内搜索 |

只声明真正完成的操作。Core 不会因为你在 Rust 代码中“能发出请求”就假设服务器账号允许该操作。

## 每次连接/资源都要发现能力

初始化握手声明操作上限和 discovery 支持；当前协议要求 VFS 提供 `discovery: true`，并报告已实现的 preflight 操作。随后 Core 会对当前 mount/资源调用 `vfs.discover`。SDK 将 `discover(ctx)` 返回值与 `operations()` 的静态上限取交集，因此运行时结果只能收窄，不能增加代码未实现的能力。

在 `discover` 中结合当前连接身份、服务端权限和节点能力返回可用操作。错误要 fail closed；不得因为登录成功就推断账号有写权限。节点返回的 `allowedOps` 也会用于缩小当前节点可执行的操作。

如果支持搜索，另外实现 `search_capabilities(ctx)`，分别声明名称搜索和内容搜索。`search` 逻辑操作本身不代表两种搜索模式都能用。

## 实现列表与元数据

`list` 返回 `FsListing`，其中每个 `FsEntry` 包含稳定 node ID、显示名和文件/目录类型；可用时补充大小、修改时间、扩展名、MIME、revision 与当前节点允许的操作。`stat` 应对同一 node ID 返回一致身份。

Core 依赖这些信息绘制列表、排序、预览和操作菜单。文件名是显示数据，不要用它当唯一身份；名称重复时 node ID 仍需区分。对远端的权限变化或内容变化，应更新 revision/operation 能力，避免把旧 UI 状态当作当前授权。

## 流式读取

Provider 的 `read` 返回普通 `Read + Send` 实现，SDK 负责 `vfs.openRead` 建立流、生成 stream ID、按 offset 拉取 chunk，并在 `vfs.closeRead` 回收资源。Wire chunk 上限为 4 MiB；通常让 SDK 管理分块，不要把大文件塞进一次 JSON 返回。

确保读取器在取消、关闭和异常时释放远端响应/句柄。若数据源是宿主 HTTP/TCP/file handle，读取流的授权会随 SDK stream task 延长到关闭为止；不要跨用户请求保留全局可复用的宿主句柄。

## 流式写入与提交

Provider 的 `write` 返回 writer 与提交回调。SDK 先创建写流，再接收多个 `vfs.writeStream` chunk；只有收到 `vfs.commit` 才执行提交回调。`vfs.abort`、连接中断或未完成的 writer drop 应丢弃临时写入。

在远端服务适合的地方使用临时对象/分段上传，直到 commit 再发布最终文件。提交前校验权限和冲突；不要在接收第一个 chunk 时就不可逆地覆盖目标。

## Preflight（可选的动态检查）

Core 使用 `vfs.preflight` 检查创建、重命名、删除或传输的目标是否符合 provider 自己的额外动态策略。它不是授权机制，也不能增加权限。只为你负责的操作注册 preflight handler；检查函数必须只读、可重复。最终执行前 Core 会再做一次检查。

Preflight 操作类型及 phase 见 SDK 的 [VFS transfer preflight contract](https://github.com/kevinhuang001/fiex-sdk/blob/24664a69bb411d0b2608461ae08a588f28cbe385/docs/transfer-preflight.md)。远程账号 ACL 仍需在真实写操作时由服务器确认。

## 测试清单

- node ID 稳定、目录项名称相同时不混淆；文件与目录类型准确。
- discovery 随账户、目录和权限变化而正确收窄；错误不授予能力。
- 搜索名称/内容能力分别反映实际实现。
- 大文件可多次读取，关闭/取消能释放句柄。
- 未提交的写入在 abort、错误和进程退出时不会发布；commit 只提交一次。
- 并发账户、过期连接、服务器拒绝和冲突不会串用别的 mount。
- Core UI 中检查 URI/面包屑、菜单能力、复制/移动、跨 provider 导入导出。

可编译实现参考：[Hello VFS](https://github.com/kevinhuang001/fiex-official-plugins/blob/main/example/hello/src/main.rs)。
