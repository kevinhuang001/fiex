# 07 · 实现状态

## 已完成

### Manifest / 扩展

- `schemaVersion: 2`；
- 每扩展点每插件最多一个贡献；
- 扩展贡献 id 已删除，身份为插件 id；
- `connections`、`vfs`、`toolbar`、`search`、`hooks`、`info`、`preview`、`themes` 单对象；
- 固定方法：`info.resolve`、`preview.open`、`search.query`、`hooks.invoke`、`toolbar.invoke`、`connection.validate/open/create`、`decorations.resolve`、`contextMenu.resolve/execute`；
- `settings` 保留字段数组，字段 id 是存储 key。

### 权限

- 独立的 `PermissionPolicy` 模块；
- Host RPC 通过 `PermissionPolicy::allowed_name` 判断；
- 网络 Broker 通过 `PermissionPolicy::domain_allowed` 判断；
- 连接通过 `require_all` 检查全部声明权限；
- 文件操作叠加 `plugin_fs_access::Scope`；
- 授权/激活/撤销仍由 `plugin_permissions` 存储管理。

### 连接 / VFS

- Connection 只返回 `vfs` 或 `pluginView`；
- Core 分配 mountId；
- 保存连接时自动写入目标 host；
- `pluginView` 支持 `tabTitle`。

### Info / Preview / 面板

- Info 按优先级汇总多个 Provider，Preview 选择一个可用 Provider；
- Info 与 Preview 共用资源快照及 `EventContext.targets`；内置 Info 不创建插件专用请求；
- 插件预览面板与 toolbar 面板走包内 HTML + CSP；
- Toolbar 固定 `toolbar.invoke`；
- 右键菜单根项目和子菜单按需调用固定的 `contextMenu.resolve`；点击 `contextMenu.execute` 后才绑定被选资源的内容访问 Scope。

### SSH

- 官方 `org.fiex.ssh` 插件；
- TCP 通过 Host Broker；
- `russh` SSH 握手；
- 私钥/密码/agent 认证；
- Core terminal bridge + xterm.js；
- 本地 sshd 集成测试已加入 CI。

### 统一事件调用架构

- Info、Preview、ContextMenu、Decorations、Search 通过共同 EventRouter 完成 Provider 匹配、执行、逐源校验和事件结果策略；
- 统一 targets 投影及引用绑定在 Host 边界执行，作用域独立；ScopedRead 和读取令牌注册表已删除；
- 菜单与搜索由 Core 汇集，UI 不再遍历插件或自行合并搜索来源；懒加载子菜单保持原会话；
- Preview 对失败、非法响应、unsupported 自动回退，尊重用户允许的来源顺序；
- 远程元数据无需下载；文件字节在实际授权 IO 时才 materialize，操作快照复用缓存；
- 详细流程与结果策略见 [08](08-UNIFIED-PREVIEW-INFO.md)。

## 其他独立工作

- `hooks.invoke` 方法固定，但文件生命周期调用点尚未全部接线；
- OS 级插件沙箱未在所有平台强制；
- 异步 SDK 仍以同步 Broker 流为主，README 中未列出的 async API 不存在；
- 主题只支持单主题文件；
- 网络 `all` 有危险提示，但尚未提供“自动收窄到白名单”的迁移。

## 未实现

- ProxyJump / ProxyCommand / `~/.ssh/config`；
- MFA / keyboard-interactive / 加密私钥交互式解锁；
- known_hosts 管理 UI（当前为首次信任）；
- 完整字段级 PermissionProjection；
- 全平台负面权限测试矩阵。

## 验收

CI 覆盖：

- Core Rust 单元测试；
- Manifest / 权限 / 扩展合同测试；
- 官方插件真实进程 `plugin_smoke`；
- SSH 插件对真实本地 sshd 的集成测试；
- 前端 e2e（终端、工作区、插件搜索、连接）。
