# 07 · Fiex 插件平台实施计划

本任务书以 [01 架构](01-ARCHITECTURE.md) 和 [02 Manifest](02-MANIFEST-EXTENSIONS.md) 为规范来源。所有代码实现以最终合同为准；本文件列具体工作与验收，不记录设计讨论的修改过程。

## 1. 交付范围

涉及 `fiex`（Core / Desktop）、`fiex-sdk`（Rust 和 UI SDK）、`fiex-official-plugins`（标准插件实现）。涉及全部扩展点、权限审核、受限宿主资源代理、Connection 导航、VFS 云盘同步以及 Info/Preview 统一 Provider。

| 里程碑 | 依赖 | 输出 |
|---|---|---|
| M0 Schema 与合同 | 无 | extensions Manifest / Native IPC / UI Bridge 的单一 Schema 与 fixture |
| M1 PermissionPolicy 与 Broker | M0 | 独立授权模块、ResourceRef/Scope、文件网络/任务受控 API |
| M2 ExtensionRegistry 与标准事件 | M1 | ContextMenu、Info、Preview、Decoration、Search、Toolbar、Hooks、Themes、Settings |
| M3 ContentHost 与 Connection | M1–M2 | VFS 或全宽 iframe 二分支导航、标签与生命周期 |
| M4 VFS/离线缓存 | M1–M3 | MountRegistry、Provider 读写流、Pin、同步、冲突处理 |
| M5 SDK/官方插件 | M1–M4 | `std::io` 与可选 Tokio IO、TLS/Transport Adapter、云盘及 Git Provider |
| M6 发布验收 | 全部 | 跨平台格式、静态检查、单元/集成/E2E、安全与性能基准 |

## 2. M0 · 协议合同

- 编写并校验 `manifest.schema.json`：唯一 `extensions` 扩展注册对象、独立 `permissions`、标准事件字段、统一对象 ID。
- 定义版本化 JSON-RPC method Schema，明确 `InfoContext`、`PreviewContext`、`DecorationContext`、`MenuResponse`、`InfoResponse`、`PreviewView`、`NavigationTarget`。
- `NavigationTarget` 的唯一合法枚举是 `vfs` 和 `pluginView`；`pluginView` 只能绑定插件包 HTML。
- Rust/TypeScript 生成与 Schema/fixture 进行一致性测试；Core 原生领域 DTO 和插件回包使用同一模型。
- 创建 Mock Core、Mock Native Plugin 与 Mock iframe Bridge，以便不连接真实云端即可测试每个事件和错误路径。

**验收**：所有示例 Manifest、请求/响应和导航枚举通过 Schema；不允许出现第三种 Connection 导航结果。

## 3. M1 · 独立权限与宿主资源

- 单独实现可信 `PermissionPolicy`：`project_event`、`authorize_call`、`audit_response`。Scope 绑定 PluginInstance、SessionId、ResourceRef、实际资源类型和操作。
- 统一 `permissions.metadata.enabled` 管理文件与文件夹基础属性；文件、目录的读取、创建、删除、重命名与写入权限按真实对象类型核验。
- ResourceBroker 封装文件列表/Metadata、流式 Reader/Writer、网络 HTTP/TCP、命令、存储、密钥和任务生命周期。大文件使用有界缓冲。
Core TCP 必须让同一连接的读和写独立推进，并支持半关闭、取消、流回收和统一网络 IP 审核；异步 IPC Handler 可以同时等待并收到 Broker Response。
- Core 生成标准事件候选 Context 后由 PermissionPolicy 按 grant/Scope 做字段投影；自定义菜单和 UI 通过同一 Broker 按需申请资源。
- Stream/Handle/Task 随 OperationScope、插件实例和取消/关闭事件撤销；强制防路径逃逸、符号链接、替换竞态、跨账户及重复提交。

**验收**：选择 A 不可读 B；Metadata 不授予文件内容读权限；没有 `files.read` 时标准 Info/Preview 不含 read 引用；授权撤销后旧 stream 不可继续使用。

## 4. M2 · ExtensionRegistry、事件与性能

| 功能 | 实施 | 验收 |
|---|---|---|
| 注册表 | Registry 统一读取 `extensions.*`，注册多个内置和第三方来源 | 不硬编码官方插件 ID；卸载即注销 |
| Info / Preview | Core ContextBuilder 提供经审核的 Metadata 和受限可读资源引用；各来源共享公共 Rust DTO | 标准事件一次带齐本次获准上下文；大文件流按需读取 |
| Decoration | 目录切换给当前可见成员的已授权 Metadata | 未授权仍通知但不给属性；不越界枚举 |
| ContextMenu | `resolve` 仅 opaque refs；可调用单项/批量 stat；`execute` 获得真实点击后的 OperationScope | 无静态业务匹配；动态菜单可超时降级 |
| `statBatch` | 一个 RPC 批量请求并按输入对应逐项回报授权结果 | 256 项初始候选限额，重复/无权/过期对象不泄漏 |
| Metadata Cache | Core 复用有效 Explorer/VFS 属性并在返回前审核 | 改名、写入、revision 变更、撤权后不读到错误数据 |
| 嵌套 IPC | Handler 内发起 `host.*` 请求，Core 可重入异步响应 | 多插件并发不死锁，不串 Session/Request ID |
| 渐进菜单 | 原生菜单首帧先显示，动态 items 按 Session 合并 | 悬停/焦点不跳动；超时/取消丢弃迟到结果 |
| 搜索/工具栏/主题/钩子 | 对应标准事件、受限资源范围与 UI 渲染 | 搜索被限制到用户当前 root |

软响应预算 120 ms、硬截止 250 ms 仅作为跨平台压测候选值；必须提供真实基准再决定阈值。

## 5. M3 · Connection + ContentHost

- Core 左侧“+ 添加连接”和已保存连接列表项触发 `connection.create/open`；插件获得当前 InteractionSession 后选择精确 NavigationTarget。
- `kind=vfs`：只导航到属于该插件/连接的有效 VFS mount，原生 Explorer 在右侧渲染。
- `kind=pluginView`：ContentHost 用受校验的插件包 iframe **完整替换右侧主内容区**，包括目录面包屑、列表、文件操作工具栏与区域内预览。保留左侧栏及全局 TabHost。
- Connection 登录和配置使用完整 PluginView；Connection 点击不打开弹窗、部分宽度 Panel 或任意外部 URL。
- Core 管理导航历史、标签页状态、恢复检查、连接删除、插件禁用及 MessagePort/iframe 回收。
- 其他明确用户交互 Surface 可使用独立的通用 `host.ui.open`；不可把这种 Panel 混入 Connection 的导航结果。

**验收**：一个 iframe-only Connection 和一个网盘 VFS Connection 都可点击；任一账户和插件不能打开其他 Provider 的 HTML/mount；TabHost 的后退/前进正确恢复二者。

## 6. M4 · VFS 与可靠双向同步

- 使用 Core 分配的 Mount ID/URI、Provider 命名空间内的 Node ID；VFS 分页、stat、Range 流与写回均受限、可取消。
- Core 维护 RemoteOnly/Downloading/CachedClean/CachedDirty/Uploading/Conflict 状态；支持按需下载、Pin、目录排除、离线打开、释放本地空间、恢复联机同步。
- 下载锁定 revision、精确字节数、完整 hash、开始/结束版本复核；异常时丢弃 staging。
- 上传先生成不可变本地快照，按远端真实 CAS 保障分级处理；弱条件写不能静默覆盖。
- 破坏性缓存处理之前以实际 hash 确认本地未编辑；冲突总能保留两份；持久化原子化/断电恢复。
- 两个账户与大量分页并发使用独立缓存/凭据/版本基线，不因同名 Node ID 混淆。

**验收**：远端传输中改版、断线、同大小同 mtime 本地修改、冲突保留、释放缓存、离线 Pin、跨账户隔离和进程崩溃均有 E2E。

## 7. M5 · SDK 和官方插件

- SDK 默认提供 `FileReader: Read+Seek`、`FileWriter: Write+Seek`、`TcpStream: Read+Write`、`HttpResponse: Read`，保留 `stat_batch`、任务、存储、凭据、VFS/UI，隐藏底层 IPC/共享缓冲。`FileWriter` 必须显式 `commit`。
- Cargo 可选 Feature `tokio-io` 提供 `AsyncFileReader/AsyncFileWriter/AsyncTcpStream/AsyncHttpResponse`，分别实现对应 Tokio `AsyncRead/AsyncWrite/AsyncSeek`；没有启用 Feature 时不引入强制 Tokio 依赖或启动运行时。
- 新增真正异步的 Native IPC 请求多路复用、`poll_*` 状态/Waker、限额缓冲、取消、背压、Deadline 和 Scope 失效清理；严禁在 `poll_*` 中调用阻塞同步 RPC。
- Core TCP 支持 clone/split、双向并发、短写、半关闭与 Close；协议库可在 SDK TCP 流上叠加同步/异步 TLS 和 STARTTLS，并验证证书、主机名。
- HTTP 保持高层 SDK Client 和 FileReader 精确句柄上传，补充通用 `Read` / `AsyncRead` 请求体与同步/异步 HTTP 响应流。
- 提供 ZIP 标准流、SMB 自定义 Transport、Tokio `io::copy`、异步 TCP 和 TLS/STARTTLS 的可编译示例。第三方库若强制使用原生 `std::fs::File`/`std::net::TcpStream`，须自行寻找可注入 Backend，SDK 不伪造底层句柄。
- UI SDK 用 MessagePort 访问批准的 Bridge API，提供稳定 Init/Close/Navigation 与用户交互请求；禁止宿主 DOM/Tauri 直通。
- Archive、GPG、Git、Everything、Office/Data Preview/Info、Vista/Win11 Themes、Google Drive、WebDAV、OpenList、SMB 等官方实现使用相同合同和测试。
- Git 历史提供 VFS；网盘提供 Connection+VFS；纯账户管理可以只提供 iframe Connection；Info 与 Preview 的内置/插件结果通过同一 Registry 和 DTO 合并。

## 8. M6 · 发布验证

1. Windows/macOS/Linux 运行 Rust fmt、clippy、测试及 UI typecheck/build；提交前执行本地适用测试，CI 检查跨平台覆盖。
2. E2E 测试每个 `extensions` 类型的注册、触发、授权拒绝、超时、禁用、卸载、恢复和错误提示。
3. 性能基准覆盖单文件、100/1000 多选、10 个并发插件、冷热 Metadata 缓存、右键菜单首帧/增量合并 p50/p95/p99、慢/离线 VFS、5 GiB 以上流和数千文件目录。
同时验证 Tokio Worker 不因同步 IPC 阻塞、TCP 全双工无读写互锁、同步/异步缓冲有界，记录两类 IO 吞吐、内存峰值与取消延迟。
4. 安全负面测试覆盖非法 Connection 导航、非插件所属 iframe/mount、文件/目录越权、命令/网络越权、Symlink、Token 泄露、Session 重放与缓存误授权。
包含直接 OS Socket 旁路、DNS 解析后 IP/IPv6/内网访问、证书验证/STARTTLS、Scope 过期的 Tokio Pending 任务、未提交 FileWriter 回滚及跨进程 Handle 重放。
5. 核对文档、Schema、SDK、Core 和官方插件合同一致；只有实际验证的任务才能标记完成。

## 完成定义

Manifest 只有 `extensions` 一个扩展注册对象；PermissionPolicy 与 Core 业务层解耦；标准 Info/Preview/Decoration 一次事件包含获准标准 Context；自定义菜单和 iframe 使用按需 ResourceBroker；**连接点击只能进入 VFS 或完全替换右侧的插件 iframe**；所有跨平台、安全、性能和同步验收有可复现结果；SDK 的默认同步 `std::io` 与可选 Tokio IO 使用同一 Core PermissionPolicy 和 ResourceBroker。