# Fiex 插件平台

本目录描述**当前实现**的插件平台，而不是尚未落地的路线图。插件通过 Manifest 注册扩展，通过 Core 的 Host Broker 访问文件、网络、命令、终端和存储；Core 负责授权、资源身份和 UI。

## 当前规则

1. 顶层身份只有 `manifest.id`；插件由独立进程运行。
2. Manifest 只有一个 `extensions` 对象，每个扩展点每插件最多一个贡献。
3. 扩展点身份就是插件 id，不再配置贡献 id。
4. 扩展方法名由 Core 固定，Manifest 不写 `method`/`command`。
5. `permissions` 只表达申请；用户批准和运行范围由 `PermissionPolicy` 决定。
6. 插件不能直接访问宿主文件、Socket、Tauri API、Keyring 或任意 DOM；必须走 SDK/Host Broker。
7. Connection 导航只有两种目标：`vfs` 与 `pluginView`。
8. SSH 是官方连接插件；终端 UI 由 Core 的 xterm.js 提供，TCP 由 Host Broker 建立。
9. 所有文件操作只有一套 `vfs.*`；插件在 `plugin.initialize` 上报能力，Core 据此决定可下发的操作和可显示的菜单项。

## 文档索引

- [01 架构](01-ARCHITECTURE.md)
- [02 Manifest 与扩展](02-MANIFEST-EXTENSIONS.md)
- [03 IPC 协议](03-IPC-CATALOG.md)
- [04 SDK](04-SDK-CATALOG.md)
- [05 Connection、VFS 与同步](05-VFS-CONNECTION-SYNC.md)
- [06 权限、安全与生命周期](06-SECURITY-LIFECYCLE.md)
- [07 实现状态](07-IMPLEMENTATION-PLAN.md)
- [08 Info / Preview](08-UNIFIED-PREVIEW-INFO.md)
- [09 面板与运行时 UI](09-RUNTIME-UI-PANELS.md)
- [10 统一文件系统协议](10-UNIFIED-FS.md)

官方插件和示例：

- [fiex-official-plugins](https://github.com/kevinhuang001/fiex-official-plugins)
- 官方 SSH 插件：[official/ssh](https://github.com/kevinhuang001/fiex-official-plugins/tree/refactor/plugin-platform-contract/official/ssh)
