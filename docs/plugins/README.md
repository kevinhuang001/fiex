# Fiex 插件平台重设计：实施总纲

> **规范性质：待实施的目标架构与 Coding Agent 工作计划**，不用于描述历史状态或宣称功能已经完成。
> **设计依据**：Fiex PR #12、fiex-sdk PR #3、fiex-official-plugins PR #9 对应代码及现有 Manifest。版本规划：Manifest schemaVersion 2、Native IPC 3.0、UI Bridge 2.0，作为一次性破坏性重设计目标；各阶段可以先保留内部临时适配，但最终版本不留 legacy 路径。禁止直接用这些目标版本号误判目前二进制已经实现。

## 总体决定

1. **插件类型不分特权等级**。一个插件有一个身份 `pluginId`，通过 `surfaces` 声明固定 UI 插槽、`providers` 声明无固定 UI 插槽的功能；通过 `permissions` 申请资源操作。Panel 由 Core 在实际用户交互时按需创建，Manifest 无须注册。官方插件与第三方完全一致；不使用 `capabilities`、`scheme`、`writeMode`、`conflictControl` 或 `sync` Manifest 选择开关。
2. **Core 掌握资源与状态**：统一路由、挂载 ID、权限、文件句柄、网络代理、缓存、任务、同步、冲突与持久化。Provider 只提供远端文件语义；UI 只进行显示和操作请求。禁止在 Core 写 WebDAV/Git/SMB/OpenList 等插件 ID 特判。
3. **VFS 是应用内部的资源命名空间**，形式为 `fiex://mount/<Core-id>/...`；Connection 不自动产生挂载；独立 VFS Provider 经 IPC 请求挂载，VFS Provider 经独立 IPC 请求创建云盘持久 mount 或 Git 历史临时 View。当前阶段不提供 OS 级 FUSE/WinFsp 挂载。
4. **一次操作、一份最小授权**：选择文件/目录不自动赋权，后续明确调用才创建 operation scope；后台任务继承并独立持有这一 scope，任务结束立即释放。连接 Provider 的云端文件 ID 不等于宿主磁盘访问授权。
5. **统一流式资源 I/O**：插件开发者优先使用 Rust `Read/Write/Seek`，SDK 负责共享内存与短消息的自动选择；上传可直接传递 *精确文件句柄*。所有传输长度为 u64，有上限、取消与错误传播。
6. **网盘 VFS 的核心闭环**：list/stat/read/open + Core 增量缓存、离线固定、恢复、冲突保留、显式解决；由独立 VFS Provider 提供的网盘写入使用单一 `vfs.writeFile`，Provider 可在内部采用单 PUT、分段、服务器临时文件或原生协议。
7. **协议目录单一事实来源**：通过 schema + 生成器产出 IPC 方法表、Rust SDK 类型、TypeScript UI 类型、Manifest JSON Schema 与契约测试；禁止“文档和代码各自手写一套语义”。
8. **面向实现，而非阶段报告**：每章定义目标接口、必须满足的不变量、模块位置、测试要求；实施顺序见第 07 篇。

## 文档目录（十篇规范 + 仓库根目录 Roadmap）

| 文件 | 用途 |
|---|---|
| [01-ARCHITECTURE.md](01-ARCHITECTURE.md) | 边界、进程、控制流、资源所有权与不变量 |
| [02-MANIFEST-EXTENSIONS.md](02-MANIFEST-EXTENSIONS.md) | 所有 Manifest 字段、所有贡献点、示例与注册流程 |
| [03-IPC-CATALOG.md](03-IPC-CATALOG.md) | **完整 Native IPC 方法清单**：双方方向、参数、响应、错误、权限 |
| [04-SDK-CATALOG.md](04-SDK-CATALOG.md) | **完整 Rust / UI SDK API** 与 IPC 逐项对应 |
| [05-VFS-CONNECTION-SYNC.md](05-VFS-CONNECTION-SYNC.md) | VFS、Connection、缓存、同步、冲突、数据一致性 |
| [06-SECURITY-LIFECYCLE.md](06-SECURITY-LIFECYCLE.md) | 操作授权、沙箱、界面边界、升级安装与任务生命周期 |
| [07-IMPLEMENTATION-PLAN.md](07-IMPLEMENTATION-PLAN.md) | 按依赖拆解的重构任务、验收矩阵、风险、完成定义 |
| [08-UNIFIED-PREVIEW-INFO.md](08-UNIFIED-PREVIEW-INFO.md) | 内置/第三方 Preview 与 Info 共用接口、Registry、Session、流式读取及同形结果 |
| [09-RUNTIME-UI-PANELS.md](09-RUNTIME-UI-PANELS.md) | 通用 PanelHost：所有 UI Surface 均可在实际用户交互时通过 IPC 动态选择并打开页面，无静态 Panel 配置 |
| [../../ROADMAP.md](../../ROADMAP.md) | 整个 Fiex 插件平台按优先级、依赖和验收条件整理的待办事项 |

## 总体实施依赖

```mermaid
flowchart LR
  A["M0 冻结契约与测试夹具"] --> B["M1 Core IPC/授权基建"]
  B --> C["M2 SDK 与 UI Bridge"]
  B --> D["M3 通用 VFS/Connection"]
  C --> D
  D --> E["M4 缓存与双向同步"]
  C --> F["M5 扩展点和官方插件迁移"]
  E --> F
  F --> G["M6 跨平台安全、性能与发布验收"]
```

## 统一 Preview / Info 目标

内置代码高亮文本、图片、音频、视频、PDF、默认文件预览和插件预览**分别为独立模块**，统一注册到 PreviewRegistry；基础 metadata、lofty 音频、MP4/Matroska 视频元数据及第三方 Info **分别实现独立模块**，统一注册到 InfoRegistry。两者均由同一宿主流程按照 Provider Descriptor 匹配、排序、授权、调用、聚合/回退和渲染，不在 `Preview.tsx` 或 `FileInfoDialog.tsx` 里对内置类型与特定插件 ID 写特殊分支。内置进程内 Adapter 与第三方 IPC/沙箱 Adapter **共享接口、不共享信任等级**。具体见 [08 统一预览与属性](08-UNIFIED-PREVIEW-INFO.md)、[根目录 Roadmap](../../ROADMAP.md)。

## Manifest 三层分类和内置来源拆分

`surfaces`：固定宿主 UI 插槽（Connections 添加/管理、Toolbar、ContextMenus、Search、Preview、Info、Decorations、Settings、Themes）。`providers`：无固定 UI 插槽的独立 VFS 文件树（可写云盘/只读 Git 历史）与 Hooks。**Panel 是运行时 UI 服务，不存在静态 Panel 注册表**。旧顶层 `contributions` 无需兼容、直接拒绝。Connection 和 Preview 一样具有固定位置，但 Connection 不自动产生 VFS；网盘必须另行声明 `providers.vfs[]`，点击连接后的导航由插件决定。

一个 Info Surface 下**分别注册** `core.info.metadata`、`core.info.audio`、`core.info.video` 和任意第三方 Info Provider；一个 Preview Surface 下分别注册内置 Text、Image、Audio、Video、PDF、Fallback 和插件 Provider。Core 各来源必须拆到真正独立源代码文件，由各自模块提供匹配、解析与测试，不允许在 `info.rs` / `Preview.tsx` 中继续集中实现。参见 [02](02-MANIFEST-EXTENSIONS.md)、[08](08-UNIFIED-PREVIEW-INFO.md) 和 [Roadmap](../../ROADMAP.md)。

## UI Panel 无静态声明，所有 Surface 在运行时按需创建

`surfaces.connections/toolbar/contextMenus/search/preview/info/settings` 等有 UI 集成位置的插件，在用户实际点击或打开相应界面后，均能通过统一的 `host.ui.open` 请求 Core 创建 Panel。Manifest **不包含** Panel 注册表、`panelId` 或额外的 Panel permission。插件通过 IPC 在当次请求内指定包内 HTML 页面或有界动态 HTML、标题及尺寸。Core 统一管理 PanelSession、iframe 沙箱、交互绑定与权限；普通后台任务/菜单查询/自动 Info/Search 查询不允许主动弹窗。详见 [09 运行时 UI Panel](09-RUNTIME-UI-PANELS.md)。

## 权限、Decoration、Connection 的三个解耦点

`permissions.metadata.enabled` **只有一项**，覆盖文件/文件夹 metadata。独立的受信 `PermissionPolicy` 对插件申请文件资源的请求和 Core 实际返回的数据逐项审核已批准权限与事件/操作 Scope；Core 的事件通知不携带原始资源属性，权限判断不能散落在 Core 各业务组件。

`surfaces.decorations:true` 只是订阅**每次目录切换**，不是取得文件信息的权限。Core 每次只发目录切换事件与当前可见资源 opaque ref；插件自行通过通用文件 IPC 申请 Metadata，独立审核模块检查 `permissions.metadata`、会话作用域及返回值后 Core 才返回；没有这个权限无法产生按文件属性判断的有效 Decoration。

`surfaces.connections[]` 仅注册添加连接的“+”入口和侧栏连接项；点击后插件通过 `connection.create/open` 决定具体导航/面板。网盘还须独立注册 `providers.vfs[]`，Core 不自动挂载 Connection。参见 [02 Manifest](02-MANIFEST-EXTENSIONS.md)、[03 IPC](03-IPC-CATALOG.md)、[05 VFS](05-VFS-CONNECTION-SYNC.md)。

## 权限与搜索的目标约定

所有 `permissions.files.*` 与 `permissions.folders.*` 均为 `{enabled: boolean, ...}` 结构。文件独立声明 `read/write/create/delete/rename`；文件夹独立声明 `read/create/delete/rename`；**文件和文件夹共用唯一的 `permissions.metadata.enabled`**。元数据权限与文件内容读取/目录遍历相互独立。

统一事件链路：插件先注册 Surface → 用户触发 → Core 只通知会话及 opaque 资源引用（不含 Metadata） → 插件通过 `host.fs.stat/list/openRead` 等通用 IPC 按需申请 → 独立 PermissionPolicy 审核权限、当前 EventScope/OperationScope 和 Core 返回数据 → Core 返回结果或 PermissionDenied。不存在 metadata 的单独 Push/Pull 通道；右键 `resolve` 期间可读取本次选择对象的 metadata，但不能读取字节。

`surfaces.search[]` 只含 `id/title/command?` 等基本注册项，**没有 `scope`**。Fiex 搜索始终限定用户当前指定的 root 目录；Everything 使用其全局索引，但查询及 Core 结果过滤都必须遵守该路径范围。

## 动态右键菜单约定

右键菜单不再有静态 `when`、`children` 或每项 `command`：`"surfaces":{"contextMenus":true}` 仅声明参与菜单，用户每次右键时 Core 将本次文件元数据发给每个已启用 Provider 的 `contextMenu.resolve`，由插件返回扁平菜单 `items`（使用 `parentId` 表示多级菜单）；用户点击后才经过独立授权调用 `contextMenu.execute`。具体约定见 [02 Manifest](02-MANIFEST-EXTENSIONS.md)、[03 IPC](03-IPC-CATALOG.md)。

## 统一术语

| 术语 | 唯一含义 |
|---|---|
| Plugin | 经验证且已授权的一个软件包/运行主体 |
| Surface | 宿主固定 UI 插槽；每个插槽可容纳多个 Provider 实现，不授予数据权限 |
| Permission | 声明用户需审批的资源操作类别 |
| OperationScope | 每次动作按已选择资源形成的短生命周期权限 |
| TaskScope | 从 OperationScope 派生，显式延续至任务结束 |
| MountId | Core 生成的持久 Connection 或 View 身份，不可由插件指定 |
| NodeId | Provider 挂载内的资源标识，不是可信本地路径 |
| Revision | Provider 在同一 NodeId 下表示内容版本的 *不透明字符串* |
| Baseline | Core 已物化版本的内容哈希 + 对应远端 Revision |
| Conflict | Core 同时存在不可安全自动合并的本地与云端更改 |

## 不允许的设计回退

不再使用仓库内的旧 `history://` 路由、插件分配 scheme、顶层 `capabilities` 或任意隐式授权当前目录。不得用“已有 permission”替代“一次用户操作的选中资源”；不得仅凭本地 mtime/size 决定可以安全覆盖；不得把比较后上传称为原子 CAS。目标 Manifest/IPC 版本为一次性升级，不为尚未发布的兼容负担长期维护双实现。
