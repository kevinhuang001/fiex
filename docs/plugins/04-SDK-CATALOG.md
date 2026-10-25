# 04 · Rust SDK 与 UI SDK

## 1. Rust SDK 总入口

```rust
sdk::serve("org.example.plugin", |ctx| {
    match ctx.method() {
        "contextMenu.resolve" => menu::resolve(ctx),
        "contextMenu.execute" => menu::execute(ctx),
        "info.resolve" => info::resolve(ctx),
        "preview.open" => preview::open(ctx),
        "decorations.resolve" => decorations::resolve(ctx),
        "connection.create" => connection::create(ctx),
        "connection.open" => connection::open(ctx),
        _ => Err(RpcError::method_not_found(ctx.method()))
    }
});
```

以上是**目标示例**，具体泛型和 Rust 类型由统一协议 Schema 约束。SDK 隐藏手工 JSON-RPC ID 管理、HostProxy 生命周期、低层读写缓冲以及取消处理，支持并发 Handler 和在 Handler 中嵌套调用 Host API。

## 2. 核心 API

| SDK | IPC | 语义 |
|---|---|---|
| `ctx.files().metadata(ref)` | `host.fs.stat` | 本次 Scope 内单个资源属性 |
| `ctx.files().stat_batch(&refs)` | `host.fs.statBatch` | 按项授权的批量 Metadata |
| `ctx.files().list(directory)` | `host.fs.list` | 授权目录浏览 |
| `ctx.files().open_read(ref)` | `host.fs.openRead` | 返回有界可 Seek 的 FileReader |
| `ctx.files().begin_write(ref)` | `host.fs.beginWrite` | 开始现有文件写事务 |
| `ctx.files().create(parent,name)` | `host.fs.beginWrite(create)` | 仅创建不存在的文件 |
| `ctx.files().remove/rename/mkdir` | 对应 `host.fs.*` | 按真实类型和目标授权 |
| `ctx.http().get/post/put/delete/request` | `host.net.http.*` | 受限高层 HTTP |
| `ctx.tcp().connect(host,port)` | `host.net.tcp.*` | 低层 TCP 流，复用 network 授权 |
| `ctx.vfs().open_view(...)` | `host.vfs.openView` | 申请 Core 创建受控 VFS Mount/View |
| `ctx.ui().open_panel(...)` | `host.ui.open` | 其他用户交互的通用 Panel |
| `ctx.task().spawn/cancel/progress` | Task Broker | 可取消任务、进度、会话继承 |
| `ctx.storage()/secrets()/process()` | 对应 Host Broker | 命名空间化受控存储、凭据、命令 |

文件 Reader/Writer 应实现标准 `Read/Seek/Write`（可根据异步模式提供相应接口），尽可能不泄漏 Buffer/Proxy/句柄细节。短数据使用有界 IPC，长流通过复用共享缓冲或有界分块；整文件读取必须设置显式大小上限。自动资源释放、任务取消与会话失效都要关闭 Host 句柄。

## 3. 标准 Context

标准 Info / Preview / Decoration Handler 接收 Core 已经完成权限投影的类型：

```rust
// 示例类型，并非既有 SDK 实现
struct ResourceContext {
    reference: ResourceRef,
    metadata: Option<FileMetadata>,
    read: Option<ScopedReadResource>,
}
struct InfoContext {
    session: SessionId,
    resource: ResourceContext,
}
struct DecorationContext {
    session: SessionId,
    directory: ResourceRef,
    visible: Vec<VisibleEntry>, // ref + approved metadata?
}
```

`ScopedReadResource` 是当前目标文件的受控读取引用，读取字节由 FileReader/ResourceStream 懒加载。未批准 Metadata 或 Files Read 时，相应字段缺失；插件不得将其当作无限访问任意文件的 Capability。

内置 Info/Preview 的 Core 模块直接生成公共 `InfoResponse`/`PreviewView`；外部插件可通过通用 IPC JSON 返回，Core 解码成**完全相同的 Rust 领域类型**，不要求 SDK 维护第二套 `PluginInfoResponse` 等模型。

## 4. 自定义菜单与 UI

`contextMenu.resolve` 的 Request 只有本次 MenuSession 和选择 ref；确实需要属性才用 `ctx.files().metadata/stat_batch`。用户点击菜单 action 后，Core 提供新的 OperationScope；插件使用通用 File API 完成读取、创建等操作，无需向 Core 硬编码“压缩”“解压”的业务类型。

连接条目回调应返回：

```rust
enum NavigationTarget {
    Vfs { mount_id: MountId, node_ref: Option<ResourceRef> },
    PluginView { entry: PackageEntry, state: JsonValue },
}
```

`PluginView` 指向本插件包内受校验 HTML，交由宿主 ContentHost 将**整个右侧主内容区**替换成 iframe。Connection 的 `create/open` **不能**返回通用 PanelSpec；通用 `ctx.ui().open_panel` 是其他 Surface 的 UI 能力。

## 5. 插件 iframe 的 UI SDK

UI SDK 从宿主启动时得到隔离的 MessagePort，与 Core 的 UI Bridge 使用显式允许列表。它可以接收视图 init/update/close、请求自有插件处理器、反馈尺寸/聚焦或向 Native Plugin 请求获准数据，不能直接访问本地 FS、Tauri、DOM 或任意 IPC 方法。

主内容 PluginView 是导航视图而非弹窗：Core 处理 tab、前进后退、恢复、生命周期；插件保留自己的页面内部状态，并以受限 `state` 数据恢复。Connection View 占满右侧而不影响全局 Sidebar。

## 6. 并发和性能合同

- SDK 允许 Handler 内部异步 Host RPC，Host 响应在 Handler 返回之前就可被接收；Request ID、插件实例和会话三者匹配。
- `stat_batch` 有界自动拆批，结果顺序与输入对应，逐项返回 PermissionDenied/Unavailable，不静默跳过。
- 文件 Reader 的生命周期不依赖临时 `Files` 变量，但严格绑定 Core Operation/Task 授权。
- 不在 SDK 缓存未获授权的全局宿主 Metadata；Core 的候选缓存读取仍会先经 PermissionPolicy 审核。
- SDK / UI Bridge 对响应大小、超时、取消、重试和进度均有清晰错误处理，不阻塞 UI 线程。