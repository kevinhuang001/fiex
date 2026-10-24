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
| `preview`      | 文件预览器；声明扩展名与优先级，可覆盖内置预览（设计见 [23](23-PREVIEW-EXTENSIONS.md)）。  |
| `hooks`        | 打开文件前接管操作；按优先级执行，可放行或消费（见 [25](25-BEFORE-OPEN-HOOKS.md)）。       |
| `info`         | 向“显示简介”右列提供元数据；按扩展名匹配（见 [26](26-INFO-EXTENSIONS.md)）。               |
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

**动手做插件，先看这两篇：**

- [21 面板 UI 开发指南](21-PANEL-UI-GUIDE.md) —— iframe 面板、SDK、主题继承、本地 dev server
- [22 主题包开发指南](22-THEME-AUTHORING.md) —— 主题包形态、token 参考、校验规则、验收
- [23 预览扩展点设计](23-PREVIEW-EXTENSIONS.md) —— 预览器注册、优先级、office-preview
- [24 插件诊断](24-PLUGIN-DIAGNOSTICS.md) —— 命令可用性、错误来源与设置页
- [25 打开前钩子](25-BEFORE-OPEN-HOOKS.md)
- [26 显示简介扩展点](26-INFO-EXTENSIONS.md)

**协议与设计**

- [01 架构](01-ARCHITECTURE.md)
- [02 VFS 与目录树](02-VFS.md)
- [03 缓存与选择性同步](03-CACHE-SYNC.md)
- [04 Native IPC 协议](04-IPC-PROTOCOL.md)
- [05 iframe UI 扩展点](05-UI-EXTENSIONS.md)
- [06 任务与活动中心](06-TASKS.md)
- [07 安全、权限与打包](07-SECURITY-PACKAGING.md)
- [08 SDK 与第三方开发](08-SDK.md)
- [09 迁移与 CI 验收](09-MIGRATION-TESTING.md)
- [10 完整能力场景](10-EXAMPLES.md)
- [11 UI SDK 与 Host Bridge](11-UI-SDK.md)
- [12 实现状态](12-IMPLEMENTATION-STATUS.md)
- [13 Storage / Secrets / OAuth](13-PERSISTENCE-OAUTH.md)
- [14 GitHub 安装流程](14-GITHUB-INSTALLATION.md)
- [15 外部仓库与 Release 契约](15-EXTERNAL-REPOSITORIES.md)
- [16 搜索来源插件](16-SEARCH.md)
- [16 插件签名](16-SIGNING.md)（历史编号与搜索重复）
- [17 文件装饰](17-DECORATIONS.md)
- [18 激活与代理资源](18-ACTIVATION-AND-BROKER.md)
- [19 插件静态资源与文件图标](19-FILE-ICONS.md)
- [20 主题与面板生命周期](20-THEMES-AND-PERFORMANCE.md)
- [21 面板 UI 开发指南](21-PANEL-UI-GUIDE.md)
- [22 主题包开发指南](22-THEME-AUTHORING.md)
- [23 预览扩展点设计](23-PREVIEW-EXTENSIONS.md)
- [24 插件诊断](24-PLUGIN-DIAGNOSTICS.md)
- [25 打开前钩子](25-BEFORE-OPEN-HOOKS.md)
- [26 显示简介扩展点](26-INFO-EXTENSIONS.md)
