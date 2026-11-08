# 当前架构

本页依据 Core 的 Cargo 工作区、Rust 模块和 SDK 子模块描述代码现状。具体函数签名和 DTO 以对应源码为准。

## 请求路径

1. React 前端管理窗口、视图和交互，通过类型化 Tauri command 请求桌面能力。
2. Tauri 命令层把 UI 请求交给 Core 服务或平台适配器。
3. Core 解析资源位置与能力，经过 provider 注册表选择内置 provider 或插件代理。
4. 文件和 VFS 请求由 Core 的 broker/范围检查执行；插件 provider 通过宿主 IPC 转发到独立进程。
5. Core 校验结果并投影到 UI 所需模型。

## Rust 工作区

Core 的 Cargo workspace 包含主应用 crate、foundation crate、platform crate 和 built-in crate。

| 类别       | 当前 crate        | 主要职责                             |
| ---------- | ----------------- | ------------------------------------ |
| Foundation | fiex-fs           | 共享文件系统/provider 能力基础       |
| Foundation | fiex-resource     | 资源引用及身份能力                   |
| Foundation | fiex-runtime      | provider 运行时相关基础              |
| Foundation | fiex-storage      | Core 与插件隔离的持久化存储基础      |
| Platform   | fiex-platform     | 跨平台系统服务适配                   |
| Built-in   | fiex-actions      | 内置操作与动作 provider              |
| Built-in   | fiex-applications | 应用列表、应用文件夹与导入资源       |
| Built-in   | fiex-decorations  | 文件装饰 provider                    |
| Built-in   | fiex-info         | 文件信息 provider                    |
| Built-in   | fiex-local-fs     | 本地文件系统 provider                |
| Built-in   | fiex-preview      | 内置预览 provider                    |
| Built-in   | fiex-recent       | Recent 虚拟视图与移除记录动作        |
| Built-in   | fiex-search       | 本地搜索 provider                    |
| Built-in   | fiex-trash        | Trash/Recycle Bin provider           |
| Built-in   | fiex-usb-fs       | 不支持原生驱动时的可移动文件系统读取 |

主应用的 Tauri 命令、provider 路由、插件宿主和功能编排位于 src-tauri/src。Rust 工作区 crate 与应用模块通过依赖组合；它们不是独立桌面进程。

## Provider 与插件

SDK 的 provider contracts 是 Core 与插件共享的协议边界。Core 的注册表记录 native/plugin provider，事件路由选择候选并进行范围、授权和结果校验。native provider 可在 Core 进程中执行；插件 provider 通过宿主进程与插件进程之间的 JSON-RPC 通信实现同一类契约。

Core 统一拥有 Fiex 虚拟位置标识和插件挂载视图身份。插件声明 VFS 能力并返回目录/文件操作；它不自行注册新的 Fiex URI scheme。文件读写通过流式 API 传输，大文件操作避免先把整个文件载入内存。

## 前端与基础设施

- src/components：浏览器、设置、对话框和插件 UI 宿主。
- src/hooks：窗口状态、导航、文件操作和平台交互。
- src/lib：前端类型、命令封装和共享模型。
- src-tauri/src/desktop：Tauri command 适配、窗口生命周期与调度。
- src-tauri/src/core：文件系统、传输、搜索、标签、卷和终端等 Core 功能。
- src-tauri/src/plugins：Manifest、插件进程、注册表、事件、资源、权限和 VFS 宿主。
- src-tauri/src/remote：远程连接元数据、本地缓存与同步。
- src-tauri/src/infrastructure：缓存、日志、错误与应用状态。
