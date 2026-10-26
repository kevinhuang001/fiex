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

## 权限边界

```text
Manifest permissions
  → 用户 Grant（approval + enabled + per-capability）
  → PermissionPolicy::allowed / require / domain_allowed
  → Host RPC / Broker 执行
  → 具体资源 Scope
```

- `plugin_permissions` 负责**保存和读取 Grant**。
- `plugin_permission_policy` 负责**做决定**，是 Host RPC 和网络 Broker 的共同入口。
- 文件操作仍需要 `plugin_fs_access::Scope`；权限通过不等于路径范围通过。
- 连接插件在保存/打开时通过 `PermissionPolicy::require_all` 检查声明权限。
- 终端 Session 采用 owner capability 校验：只有创建该 Session 的插件能读写它。

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
- 后台任务由 `ctx.spawn_internal_task` / `spawn_task` 管理，任务不延长失效授权。
