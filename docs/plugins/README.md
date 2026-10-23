# Fiex 插件系统（实验性 1.0）

> **状态：开发分支已实现，协议与 manifest 仍可破坏性修改。** Native IPC 版本 **2.0**，manifest `schemaVersion: 1`。

## 权限与注册分离

- **权限**回答“插件能碰什么数据”，激活时逐项授权目录操作、文件读写、网络及外部命令。
- **注册**回答“插件向 Fiex 提供什么”，只影响 UI 和路由，不授予任何数据权限。

`manifest.json` 只有三块：身份、`permissions`、`contributions`。

## 注册类别

| 类别           | 用途                                                                                       |
| -------------- | ------------------------------------------------------------------------------------------ |
| `connections`  | 远程连接类型；声明字段并实现 `connection.validate` / `fs.*`。注册连接即拥有远程 VFS 能力。 |
| `toolbar`      | 工具栏动作；`command` 执行命令，`panel` 打开 iframe。                                      |
| `contextMenus` | 右键菜单动作；同样支持 `command` / `panel`。                                               |
| `search`       | 搜索来源；Core 查询所有启用来源并合并结果。                                                |
| `settings`     | 插件配置字段，由 Core 渲染。                                                               |

不再有独立的 `panels` 或 `vfs` 注册：

- iframe 面板直接内联在 `toolbar` / `contextMenus` 条目里；
- 虚拟路径 scheme 直接内联在 `connections` / `toolbar` 条目里（`schemes: ["history"]`）。

## 官方插件

Git、WebDAV、SMB、Archive、Checksum、Everything 搜索。全部为独立进程、独立 crate、独立 Release，不预装。

## 平台沙箱

| 平台    | 机制                               | 状态                        |
| ------- | ---------------------------------- | --------------------------- |
| Linux   | Arapuca：Landlock + seccomp        | 已实现并集成测试            |
| macOS   | Arapuca：Seatbelt                  | 已实现，待真机 CI           |
| Windows | Arapuca：AppContainer + Job Object | 已接入启动器，待 Windows CI |

沙箱失败时拒绝启动。域名白名单由插件进程校验，不等同于 OS 级网络过滤。

## 文档

- [01 架构](01-ARCHITECTURE.md)
- [02 VFS 与连接](02-VFS.md)
- [04 IPC](04-IPC-PROTOCOL.md)
- [05 UI 扩展](05-UI-EXTENSIONS.md)
- [07 安全](07-SECURITY-PACKAGING.md)
- [12 状态](12-IMPLEMENTATION-STATUS.md)
- [14 GitHub 安装](14-GITHUB-INSTALLATION.md)
- [15 仓库边界](15-EXTERNAL-REPOSITORIES.md)
- [16 搜索](16-SEARCH.md)
- [17 文件装饰](17-DECORATIONS.md)
- [19 插件资源与文件图标](19-FILE-ICONS.md)

See [activation and brokered resources](18-ACTIVATION-AND-BROKER.md) for the current authorization contract.
