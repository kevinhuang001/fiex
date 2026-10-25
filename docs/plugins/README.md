# Fiex 插件平台设计

本目录是 Fiex 插件平台的目标规范。所有插件通过 Manifest 注册扩展，通过 Core Broker 访问宿主资源；Core 负责统一 UI、资源身份、连接管理和安全边界。

## 一、架构约定

1. Manifest 只有一个扩展点注册对象 **`extensions`**，不对 UI 插槽和后端 Provider 再做顶层分类。授权单独使用 `permissions`。
2. **注册扩展点不等于授权**。受信任且独立于 Core 业务模块的 `PermissionPolicy` 决定每次事件附带哪些字段、每次资源请求允许哪些操作。
3. **标准事件**（Info、Preview、Decoration）拥有由 Core 定义的输入合同：在触发时交付已批准的 Metadata 和本次资源引用；需要文件内容时交付有作用域的只读流访问能力，不预加载整个文件。
4. **自定义右键菜单和插件自定义 UI** 根据当前用户操作取得不透明资源引用，需要元数据或文件读写时再通过统一文件 SDK 向 Core 申请。对于自定义 UI，插件可以在有效用户会话中选择页面和视图，但不能获取任意宿主文件。
5. **Connection 是入口，不是文件系统。** 点击加号或侧栏连接项时，插件必须选择 **VFS 导航**或**完全替换右侧主内容区的 iframe** 之一。Connection 不提供弹窗、侧面板或外部 URL 作为导航结果；要实现网盘，插件还必须注册 `extensions.vfs`。
6. VFS 由 Core 分配 `mountId` 并统一处理导航、流式读写、缓存、离线固定、冲突及恢复。它不等于操作系统的 FUSE/WinFsp 挂载。
7. Info 和 Preview 的内置与第三方实现使用同一 Registry、领域模型与渲染器：内置直接构建领域响应，第三方经 IPC 反序列化为同样的 Rust 类型。
8. Native 插件是受限独立进程；插件 HTML 使用 sandboxed iframe 和 MessagePort。插件 UI 不直接触达宿主 DOM、Tauri API、Keyring 或原始本地路径。
9. SDK 默认符合 `std::io::{Read,Write,Seek}`，可选 `tokio-io` 提供非阻塞异步文件、TCP、HTTP 流。两种 API 均使用 Core Broker，不能替换写死的原生 `std::net::TcpStream`/`std::fs::File`。

## 二、典型路径

| 场景 | Core → 插件 | 插件 → Core |
|---|---|---|
| 用户右键 | 选中对象 ref + MenuSession | 可按需申请 Metadata；返回扁平菜单项；点击后按操作申请资源 |
| Info / Preview | 标准 Context：ref、获准 Metadata、受限可读资源引用 | 返回统一 InfoResponse / PreviewView；文件字节通过受限流读取 |
| Decoration | 目录及当前可见成员 ref、获准 Metadata | 返回当前可见成员的角标/样式 |
| Connection 点击 | connectionId + 用户 InteractionSession | 返回 **VFS** 或 **全宽主内容 iframe** 的导航目标 |
| 自定义插件 UI | 插件选择要显示的页面 | 通过 UI Bridge/SDK 请求本次授权范围内的数据 |
| 第三方 IO 库 | 标准 `Read/Write/Seek` 或可注入 Transport | 可直接传入 SDK 文件/TCP 流；异步库可选 `tokio-io`，权限仍由 Core 管理 |

## 三、规范索引

- [01 架构与模块](01-ARCHITECTURE.md)
- [02 Manifest / extensions](02-MANIFEST-EXTENSIONS.md)
- [03 Native IPC 协议](03-IPC-CATALOG.md)
- [04 Rust 与 UI SDK](04-SDK-CATALOG.md)
- [05 Connection、VFS 和同步](05-VFS-CONNECTION-SYNC.md)
- [06 权限、安全与生命周期](06-SECURITY-LIFECYCLE.md)
- [07 实施与验收](07-IMPLEMENTATION-PLAN.md)
- [08 Info / Preview 统一合同](08-UNIFIED-PREVIEW-INFO.md)
- [09 主内容 iframe 与运行时 UI](09-RUNTIME-UI-PANELS.md)
- [根目录 Roadmap](../../ROADMAP.md)

上述均为**目标设计**；未列为已完成的实现。