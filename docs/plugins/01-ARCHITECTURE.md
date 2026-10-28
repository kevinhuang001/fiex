# 01 · 架构

## 分层

```text
Fiex Desktop / Sidebar / TabHost
    |
    +-- plugin_host       Manifest、catalog、进程、RPC、面板、权限入口
    +-- plugin_permission_policy
    |      Capability + Grant + Scope 的单一判断入口
    +-- Resource Broker
    |      fs_access / plugin_net / process / resources / ipc
    +-- FsResolver         Core 侧统一 vfs.* 客户端：信封、校验、流
    +-- ProviderRegistry   插件 initialize 上报的能力集合
    +-- VFS / mounts       Core 分配 mountId，处理资源身份与视图
    +-- EventRouter        统一匹配、执行、校验和事件结果策略
    +-- ResourceSnapshot   targets 元数据、操作引用与按需内容
    +-- Info / Preview     事件专用业务处理与渲染
    +-- ContentHost        Explorer、VFS 视图、PluginView iframe
    |
    +-- Native Plugin Process
           Rust SDK / HTTP / TCP / files / tasks
           Plugin-owned sandboxed HTML + MessagePort
```

## 命名

Core 负责 Resolve，SDK 负责 Provide：

| 功能        | Core                                              | SDK                             |
| ----------- | ------------------------------------------------- | ------------------------------- |
| Info        | `InfoResolver`（`plugin_info`）                   | `InfoProvider`                  |
| Preview     | `PreviewResolver`（`plugin_preview`）             | `PreviewProvider`               |
| 右键菜单    | `ContextMenuResolver`（`plugin_context_menu`）    | `ContextMenuProvider`           |
| 连接菜单    | `ConnectionMenuResolver`（`connection_menu`）     | `ConnectionContextMenuProvider` |
| Decorations | `DecorationResolver`（`plugin_decorations`）      | `DecorationProvider`            |
| Search      | `SearchResolver`（`plugin_search`）               | `SearchProvider`                |
| Toolbar     | `ToolbarResolver`                                 | `ToolbarProvider`               |
| 文件系统    | `FsResolver`（`fs_resolver`）+ `ProviderRegistry` | `VfsProvider`                    |

- Resolver 负责匹配、协调、调用、校验和结果聚合；Provider 是插件实现。
- `ResourceBroker`（`fs_access` / `net` / `process` / `resources`）
  负责资源解析、权限、Operation Scope 和访问路由。
- `EventRouter` 保留，负责通用事件调度；`ProviderRegistry` 保存
  `RegisteredProvider`（已声明的能力与元数据）。

## 权限边界

权限由三层共同决定，任何一层不满足都不会执行：

```text
Manifest 声明（这个包最多能申请什么）
  → 用户状态（approved + enabled + per-capability grant）
  → PermissionPolicy::check / domain_check / command_check
  → 具体资源 Scope（本次操作能访问哪些路径 / 主机 / 命令）
  → Host RPC / Broker 执行
```

- `plugin_permissions` 负责**保存和读取 Grant**，包括安装时批准的声明快照。
- `plugin_permission_policy` 负责**做三层决定**，返回 `Allowed`、`Prompt` 或 `Denied(reason)`。
- 文件操作先由 `plugin_fs_access::Scope` 校验资源，再检查权限；范围外直接拒绝，不弹权限框。
- 用户手动关闭某个权限时插件仍保持运行；下一次真实操作由 Core 统一弹窗重新申请。
- 未声明或未包含在已批准声明快照中的权限永远只返回拒绝，运行时申请不能扩大 Manifest。
- 连接插件在保存/打开时通过 `PermissionPolicy::require_all` 检查声明权限。
- 终端 Session 采用 owner capability 校验：只有创建该 Session 的插件能读写它。

## 模块规模

- 新模块和正在重构的模块原则上不超过 300 行，按职责拆分到子模块。
- `npm run check:size` 检查 `src` 与 `src-tauri/src`；`scripts/file-size-baseline.json` 记录尚未拆分的存量文件。
- 存量文件只能缩小，不能继续膨胀；拆分到 300 行以内后从基线删除。
- 权限链路已按 `policy / prompt / permissions / rpc dispatch` 拆分，每个文件保持在限制内。

## 事件合同

- 标准事件：`info.resolve`、`preview.open`、`decorations.resolve`。
- 系统交互事件：`contextMenu.resolve/execute`。
- 自定义插件业务 RPC：通过单独授权的 `plugin.invoke` 保留，不与系统事件混用。
- 导航：`connection.open/create` 只返回 `vfs` 或 `pluginView`。
- 固定扩展方法：`search.query`、`hooks.invoke`、`toolbar.invoke`。
- Core 内置实现与插件实现使用同一个 DTO/Registry，不区分“官方特权路径”。

## 线程与任务

- 插件进程是独立 native 进程；RPC 允许并发 in-flight request。
- Core 对插件响应、路径、HTML 和导航结果一律按不可信输入校验。
- 大文件通过有界句柄/分块传输，不把完整内容塞进 JSON。
- 连接与独立视图共用 `vfs.*`：读取用 `openRead/readStream/closeRead`，写入用
  `beginWrite/writeStream/commit/abort`，详细合同见 [10-UNIFIED-FS](10-UNIFIED-FS.md)。
- 后台任务由 `ctx.spawn_internal_task` / `spawn_task` 管理，任务不延长失效授权。
