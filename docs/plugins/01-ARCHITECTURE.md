# 01 · 插件平台架构

## 1. 分层

```text
Fiex Desktop / Sidebar / TabHost
    |
    +-- ExtensionRegistry       统一注册 extensions.* 的事件处理器
    +-- ContextBuilder          标准事件生成候选输入，复用 Explorer/VFS Metadata
    +-- InfoRegistry            Core 内置与第三方共用 InfoResponse
    +-- PreviewRegistry         Core 内置与第三方共用 PreviewView
    +-- ContentHost             原生 Explorer 或完整 iframe PluginView
    +-- VfsRouter/MountRegistry Core 分配 mountId，管理 VFS 导航与资源身份
    +-- ResourceBroker          文件、网络、命令、缓存、流与句柄
    +-- PluginRuntime           进程、双向 RPC、任务、取消、超时
    |
    +-- PermissionPolicy        独立受信 Rust 模块/crate
         [grants, session scope, resource identity, DTO projection]
    |
    +-- Native Plugin Process
         +-- Rust SDK (std IO + opt-in tokio-io / files/http/tcp/vfs/ui/task)
         +-- Plugin-owned HTML iframe (MessagePort → authorized bridge)
```

Core 业务模块产生事件与候选资源数据；PermissionPolicy 统一执行授权和字段裁剪；PluginRuntime 只发送已经审核的数据。ResourceBroker 在使用任何宿主资源前再次向 PermissionPolicy 请求操作许可。PermissionPolicy 不加载第三方业务代码，也不由插件提供授权决定。

## 2. Manifest 与扩展注册

插件 Manifest 使用唯一的 `extensions` 对象按类型注册：`contextMenus`、`decorations`、`info`、`preview`、`connections`、`vfs`、`search`、`toolbar`、`themes`、`settings`、`hooks`。每种扩展点有固定的事件与响应合同，不额外注册数据 Context 类型。

`permissions.metadata.enabled` 同时适用于文件和文件夹。文件与文件夹的读取、创建、删除、重命名等操作权限按真实对象类型分别声明。每次访问均需用户批准并符合当前事件/操作的 Scope。

## 3. 两种事件数据合同

### 标准资源事件：Info / Preview / Decoration

Core 在触发时构建标准 ResourceContext（Metadata 依据 Grant 和 Scope 逐项裁剪），一次请求交给 Provider。Info/Preview 需要读取文件内容时获得**绑定目标文件与会话的受限只读流引用**；文件字节分块传输，不把大文件内联到 JSON，也不自动授权尚未获批的文件读权限。Decoration 仅针对本次目录中已显示的成员，不能借订阅事件枚举整个磁盘。

内置 Provider 调用进程内 Adapter，第三方调用 Native RPC；两者均由统一 Registry 调度并返回相同业务 DTO。

### 自定义交互：ContextMenu / PluginView

右键事件提供本次目标和选择集合的 opaque ref；插件可在 MenuSession 中通过 `host.fs.stat` 或 `host.fs.statBatch` 申请当前所选对象的 Metadata。只有用户点击有效 action 后，才能按所确认资源取得读写 OperationScope。自定义 PluginView 的资源访问同样经 UI Bridge → Native SDK → ResourceBroker/PermissionPolicy；加载 HTML 并不自动赋予文件读取。

## 4. Connection / VFS / ContentHost

```text
User clicks Connection
   → Core connection.open(session, connectionId)
   → Plugin returns NavigationTarget
      ├── Vfs { mountId, nodeRef? }
      │      → Core ExplorerView renders VFS nodes
      └── PluginView { packageEntry, state? }
             → ContentHost replaces ENTIRE right pane with sandboxed iframe
```

**导航目标仅有 VFS 与 PluginView 两种。** 这里的“整个右侧”包括目录列表、面包屑、文件工具栏和资源视图，不包含左侧连接导航、全局窗口框架或应用级 TabHost。Connection 项的点击不能返回弹窗、窄 Panel、外部网页跳转或任意宿主路径。登录、配置、账号管理等连接相关 UI 也使用右侧完整 PluginView 承载。

Connection 只注册加号入口和侧栏项；它不产生 VFS。网盘必须同时注册 `extensions.connections` 与 `extensions.vfs`。单独的 VFS 可以用于 Git 历史等无连接入口的文件树；单独的 Connection 可以完全基于 iframe。

ContentHost 将 VFS 与 PluginView 作为一等导航视图，统一支持后退、前进、标签页、会话关闭、插件停用后的失效与恢复。PluginView 只能从所属插件包加载受校验的 HTML。

## 5. 并发与性能

- Native RPC 必须支持双向、可重入、request-id 关联和取消；多个扩展 Handler 并发执行，禁止锁跨越 await 导致嵌套请求死锁。
- SDK 默认使用标准同步 `Read/Write/Seek`，可选 `tokio-io` 提供真正非阻塞的 `AsyncRead/AsyncWrite/AsyncSeek`。异步 IPC 支持 Waker、取消、背压；Core TCP 的读写独立推进，不允许阻塞读独占写锁。
- 两类 SDK 流共用 Core Broker、Grant、Scope、StreamRegistry，不等于 `std::fs::File` 或原生 `TcpStream`，不能伪造原始 OS Handle。
- 标准 Context 优先使用 Core 已有的 Explorer/VFS Metadata 缓存，但无论缓存命中与否都要按插件独立审核 Grant、Scope 与输出字段。
- 自定义 ContextMenu 多选调用 `host.fs.statBatch`；原生菜单先绘制，动态项在有效 MenuSession 内异步合并。保持鼠标悬停与键盘焦点，丢弃过期响应。
- 所有流、列表、菜单与 iframe 消息有字节数、并发数、期限与取消上限。远端 VFS 性能波动不能冻结宿主 UI。

## 6. Core 内部模块划分

`plugin_host/{manifest,registry,runtime,rpc,sessions}`、`permission_policy/{grants,scope,projection,decision}`、`broker/{fs,net,process,buffer,stream_scheduler}`、`content_host/{navigation,explorer,plugin_view}`、`vfs/{router,mounts,views}`、`remote/{connections,cache,sync,conflicts}`、`info/{mod,metadata,audio,video,types}`、`preview/{mod,text,image,audio,video,pdf,fallback}`。

允许代码模块根据职责拆分，但 Registry、PermissionPolicy 和业务 Provider 不互相硬编码插件 ID。
