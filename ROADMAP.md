# Fiex 插件平台 Roadmap

[设计总纲](docs/plugins/README.md) · [Manifest](docs/plugins/02-MANIFEST-EXTENSIONS.md) · [实施计划](docs/plugins/07-IMPLEMENTATION-PLAN.md)

## 最终结构

```text
Manifest
├─ identity / runtime
├─ permissions
└─ extensions
   ├─ contextMenus
   ├─ decorations
   ├─ info / preview
   ├─ connections / vfs
   ├─ toolbar / search
   └─ themes / settings / hooks
```

- `extensions` 只注册扩展点，`permissions` 只声明申请的资源授权；PermissionPolicy 独立验证 Scope 和交付数据。
- Info、Preview、Decoration 为**标准事件**：Core 直接附带已批准 Metadata；如适用，提供当前文件的受限只读流引用而非完整文件字节。
- ContextMenu 与自定义 HTML UI 根据真实用户操作，按需通过 SDK 申请文件 Metadata 或读写权限。
- Connection 点击的导航返回值**只有两种**：`vfs`（Core ExplorerView）或 `pluginView`（完整替换右侧主内容区的插件 iframe）。Connection 本身不提供 VFS，网盘必须独立注册 `extensions.vfs`。
- Core 统一管理 VFS identity、流式读写、缓存、离线固定、同步、冲突；UI 插件不能越过受控 MessagePort 和 ResourceBroker。
- Rust SDK 默认实现同步标准 `Read/Write/Seek`；可选 `tokio-io` Feature 提供非阻塞 `AsyncRead/AsyncWrite/AsyncSeek` 文件、TCP、HTTP 流。均通过 Core Broker 与 PermissionPolicy，不伪造 OS Socket 或文件句柄。

## M0 · 契约

- [ ] 编写 `extensions` Manifest、Native IPC、UI Bridge 的 Schema 和示例。
- [ ] 定义标准 Context/Result DTO 与 `NavigationTarget::{vfs,pluginView}`。
- [ ] 建立 Rust、TypeScript、Mock Plugin 合同测试。

## M1 · PermissionPolicy 与 ResourceBroker

- [ ] 构建独立可信的 PermissionPolicy，统一审核 Grant、Scope、事件字段与资源请求/返回值。
- [ ] 文件与文件夹共用 `permissions.metadata.enabled`；文件/目录其他操作权限逐项分离。
- [ ] 文件、网络、命令、存储、凭据与任务 API 全部经 Core Broker；支持有界流和作用域撤销。
- [ ] Core TCP 支持独立读写、真正全双工、半关闭、取消与连接回收；校验 DNS、实际 IP、IPv4/IPv6 与网络目标。
- [ ] 覆盖跨文件、跨目录、软链接、会话重放和权限撤销负面测试。

## M2 · 扩展事件

- [ ] 统一 ExtensionRegistry，内置和插件 Info/Preview 共享业务类型、Adapter、排序和 UI 渲染。
- [ ] 标准 Info/Preview/Decoration 的获准 Metadata 在首次事件中直接提供；文件读取使用会话限定的懒加载流。
- [ ] ContextMenu 查询只传选择的 opaque ref；插件按需使用 `host.fs.stat/statBatch`；点击真实 action 后创建执行 Scope。
- [ ] 实现 statBatch、Core Metadata 有效缓存、可重入双向 IPC、原生菜单首帧与动态结果增量合并。
- [ ] Search、Toolbar、Themes、Settings、Hooks 各自具备事件与权限合同。

## M3 · Connection 与主内容区

- [ ] Connection 加号与侧栏触发 `connection.create/open`，Core 根据插件返回的导航目标打开视图。
- [ ] `vfs` 导航在原生 Explorer 中浏览虚拟文件树；须有真实 VFS Provider 和合法 Mount。
- [ ] `pluginView` 将右侧目录区域（包括面包屑、文件工具栏、列表等）**完整替换为 sandboxed iframe**；左侧 Sidebar、全局 TabHost 保留。
- [ ] Connection 导航严格拒绝 dialog/panel/external URL 等第三种类型。
- [ ] 支持导航历史、标签页状态、iframe 生命周期、撤权失效和恢复验证。

## M4 · VFS 与同步

- [ ] Core MountRegistry 统一 Node 身份、分页、Stat、流式 Range 读取及可写操作。
- [ ] 按需缓存、Pin/排除、离线访问、释放空间、重连同步及冲突保留。
- [ ] 验证远端 revision/CAS、传输 hash、本地脏文件保护、原子事务和断电恢复。
- [ ] Google Drive、WebDAV、OpenList、SMB、Git VFS 通过统一合同测试。

## M5 · SDK 与官方插件

- [ ] 默认 SDK 文件/TCP/HTTP 资源实现 `std::io::{Read,Write,Seek}` 的适用 Trait，可直接注入 ZIP 和第三方自定义 Transport。
- [ ] 可选 `tokio-io` Feature 提供非阻塞异步文件/TCP/HTTP 流；实现异步 IPC Request/Response、Waker、背压、取消，未启用时不强制依赖 Tokio。
- [ ] TCP 支持第三方库基于 SDK 流实现 TLS/STARTTLS；HTTP 保留高级 Client、精确 FileReader 句柄上传、通用同步/异步 Body 流。
- [ ] UI SDK 通过受限 MessagePort/iframe Bridge 访问已授权服务；主内容 PluginView 和其他 Panel 共用沙箱边界。
- [ ] 官方 Archive/GPG/Git/Everything/Preview/Info/主题与云盘插件完整联调。

## M6 · 测试与发布

- [ ] Windows/macOS/Linux 完成 fmt、clippy、typecheck、单元、集成和 E2E。
- [ ] 测试多选 100/1000、10 个插件并发、冷热缓存、慢速 VFS、右键首帧及合并的 p50/p95/p99。
- [ ] 测试 5 GiB+ 流、海量目录、离线与冲突、越权资源/导航、Session 撤销、iframe 沙箱和跨账号隔离。
- [ ] 测试 ZIP 标准 IO、SMB Transport、Tokio `io::copy`、TCP 并发读写、TLS/STARTTLS、EOF/半关闭，以及 async poll 非阻塞与撤权唤醒。
- [ ] 每项检查在代码、Schema、文档和真实运行结果一致后再勾选。

## 规范目录

| 文档 | 主题 |
|---|---|
| [01](docs/plugins/01-ARCHITECTURE.md) | 架构、模块和标准事件 |
| [02](docs/plugins/02-MANIFEST-EXTENSIONS.md) | `extensions` 与权限声明 |
| [03](docs/plugins/03-IPC-CATALOG.md) | IPC、标准 Context 与 VFS 方法 |
| [04](docs/plugins/04-SDK-CATALOG.md) | Rust/UI SDK |
| [05](docs/plugins/05-VFS-CONNECTION-SYNC.md) | 连接、文件树、云盘缓存和同步 |
| [06](docs/plugins/06-SECURITY-LIFECYCLE.md) | PermissionPolicy、沙箱、安全 |
| [07](docs/plugins/07-IMPLEMENTATION-PLAN.md) | 里程碑、验收和交付 |
| [08](docs/plugins/08-UNIFIED-PREVIEW-INFO.md) | Info 与 Preview |
| [09](docs/plugins/09-RUNTIME-UI-PANELS.md) | 完整右侧 PluginView、通用 Panel |