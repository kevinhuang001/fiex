# 04 · Rust SDK 与 UI SDK

## 1. SDK 定位与功能边界

Fiex SDK 提供**符合 Rust IO 生态惯例的 Core 受控资源对象**。插件可以将 SDK 的文件流和网络流传给接受标准 Trait 或可注入 Transport 的第三方库；所有真实文件、Socket、网络目标和句柄仍由 Core Broker 管理，权限由独立 PermissionPolicy 审核。

- **默认同步 API**：只依赖 `std::io` Trait，不强制插件依赖 Tokio。
- **可选 Tokio API**：Cargo feature `tokio-io` 暴露真正非阻塞的异步资源流，适配 Tokio 生态。
- **不伪装 OS 句柄**：不会把 SDK 资源强行变成 `std::fs::File`、`std::net::TcpStream`、`tokio::net::TcpStream` 或实现虚假的 `AsRawFd/AsRawSocket`。
- **HTTP 仍提供高级 Client**，TCP 只提供通用字节流；不会为 IMAP/SMTP/SMB/ZIP 分别添加专用 Core 协议。

标准库和 Tokio 版本**共用同一个 Core ResourceBroker、权限规则、资源 ID 和传输协议**；增加异步 SDK 并不允许插件建立自己的 OS Socket。

## 2. Rust SDK 的调用入口

```rust
// 示意接口：目标规范，非当前仓库已实现签名
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

`RequestContext`、`TaskContext` 的 `files()/http()/tcp()` 封装权限上下文和 IPC，不暴露缓冲 ID、HostProxy 或资源授权实现细节。资源流可在有效 TaskScope 内持有；原请求结束不意味着后台任务凭空获权。

## 3. 标准 IO Trait 合同

| SDK 对象 | 默认同步 Trait | 可选 `tokio-io` 对象及 Trait | 限制 |
|---|---|---|---|
| `FileReader` | `std::io::Read + Seek + Send` | `AsyncFileReader: tokio::io::AsyncRead + AsyncSeek + Send` | 当前文件、只读、可 Seek |
| `FileWriter` | `std::io::Write + Seek + Send` | `AsyncFileWriter: tokio::io::AsyncWrite + AsyncSeek + Send` | staged 写入；必须显式 `commit` |
| `TcpStream` | `std::io::Read + Write + Send` | `AsyncTcpStream: tokio::io::AsyncRead + AsyncWrite + Send` | 双向字节流；不支持 Seek |
| `HttpResponse` | `std::io::Read + Send` | `AsyncHttpResponse: tokio::io::AsyncRead + Send` | 响应体单向流，按需消费 |
| HTTP request body | `body_reader(FileReader)`；可扩展通用 `Read + Send` 上传流 | `body_async_reader(AsyncRead + Send + Unpin + 'static)` | 通过 Broker 流式上传，不整体序列化 |

异步对象的类型名与精确泛型可由 SDK 实现微调，但**稳定的公开合同是上述 Trait 与对应资源语义**。`BufRead` / `AsyncBufRead` 不需 SDK 自己实现：插件可包装 `std::io::BufReader` / `tokio::io::BufReader`。`futures::io` 的 `AsyncRead/Write` 可通过可选 `tokio-util::compat` 等适配器接入，不必为所有异步运行时建立独立 Broker。

### 同步文件与 ZIP 库

```rust
use std::io::{self, Write};

// 以下是调用模式示例；具体路径/Ref 以获准 Scope 为准。
let files = ctx.files();
let input = files.open_read("input.zip")?;
let mut archive = zip::ZipArchive::new(input)?; // Read + Seek

let writer = files.begin_write("result.zip", true)?;
let mut output = zip::ZipWriter::new(writer);    // Write + Seek
output.start_file("hello.txt", zip::write::SimpleFileOptions::default())?;
output.write_all(b"hello")?;
let writer = output.finish()?;
writer.commit()?;                               // Core 文件事务正式提交
```

可以接入接受 `Read`、`Write`、`Seek` 或自定义 FS/Transport Trait 的库；要求真实 `std::fs::File`、原生文件描述符或直接扫描宿主目录的库，**不能自动兼容**。

### 同步 TCP 与协议库

```rust
use std::io::{Read, Write};
let mut stream = ctx.tcp().connect("nas.example.com", 445)?;
stream.write_all(b"...")?;
let mut received = [0_u8; 4096];
let count = stream.read(&mut received)?;
```

现有 SMB 的方式是将 SDK `TcpStream` 包进实现第三方库 `SmbTransport` 的 Adapter，再调用 `smb::Connection::from_transport(...)`；不是修改 Rust 标准库 Socket。第三方库如果写死 `std::net::TcpStream` 或自己调用 `connect()`，SDK 无法无修改接管。

## 4. Tokio 可选 Feature 与异步调用

```toml
[dependencies]
fiex-plugin-sdk = { path = "../sdk/rust", features = ["tokio-io"] }
tokio = { version = "1", features = ["rt", "macros", "io-util"] }
```

```rust
// 目标 API 示例：需要异步 IPC 实现后才能使用
use tokio::io::{AsyncReadExt, AsyncWriteExt};

let mut tcp = ctx.tcp().connect_async("imap.example.com", 993).await?;
tcp.write_all(b"...").await?;
let n = tcp.read(&mut buffer).await?;

let mut file = ctx.files().open_read_async("report.pdf").await?;
let mut out = ctx.files().begin_write_async("report-copy.pdf", true).await?;
tokio::io::copy(&mut file, &mut out).await?;
out.commit().await?;
```

实现要求：

1. `connect_async`、`open_read_async`、`begin_write_async`、`send_async` 在等待 Core 响应时不阻塞 Tokio worker；异步 IO 的 `poll_read/write/flush/shutdown/seek` 必须正确保存等待状态并注册 Waker。
2. Native IPC 使用**一个双向多路复用调度器**按 request ID 关联 Response；可在插件 Handler 处理期间接受 Core Response。没有依赖单个同步读循环串行处理所有 RPC。
3. Core 文件/网络 I/O 可通过异步 Socket/文件任务或有界专用 blocking pool 实现，禁止直接在 async 任务上执行阻塞 syscall；SDK 不能在 `poll_*` 内直接调用现有同步 `ContextInner::call`。
4. 每条流限制未完成请求数和缓冲区大小，支持取消、背压、超时；取消/权限撤销时唤醒等待任务并清理 Core Handle。
5. Feature **增量编译**：不启用 `tokio-io` 时，无 Tokio 公开依赖或额外 runtime 要求；启用后由插件负责拥有 Tokio Runtime，不由 SDK 在每个连接上创建 Runtime。
6. 同步 `FileWriter::commit()` 与异步 `AsyncFileWriter::commit().await` 均完成同一类事务：仅 `flush` 不表示已提交，未提交 Drop 要撤销 staged 写入。

**异步适配不是** `poll_read` 内调用阻塞 `read()` 或在 Tokio worker 上 `block_on`。仅当实现真正的异步请求调度和安全唤醒后，才可声称 Tokio IO Trait 已支持。

## 5. Core TCP：全双工、半关闭与 TLS

- Core 同一连接的**读和写必须独立推进**：读操作等待远端数据时不能持有阻止写入的共享 Mutex。实现可分离 Socket 的 ReadHalf/WriteHalf（或并发任务/专用阻塞 worker），并分别保持连接所有权。
- SDK 支持连接句柄的 `clone`/安全 split，用于协议库并发收发；克隆不允许复用已关闭连接，最终释放关闭 Core 句柄。
- 规定 `read=0`/EOF、短读短写、部分写入、半关闭（TCP Shutdown::Write）和完整关闭的行为；`shutdown` 不是仅调用 `flush`。
- TCP `peer_addr()` 表示 Core 已实际连接的目标；连接和后续操作绑定插件实例、Grant、Session/Task Scope，限制连接数、字节数、空闲期限。
- TLS 由插件按需要在 SDK TCP 流上叠加 `rustls` 或其他接受通用同步/异步 IO 的实现。邮箱客户端支持隐式 TLS 与 STARTTLS（在同一流上升级），Core 无需解析或解密 TLS。
- 不能把 TLS 支持等同于域名授权。Core 必须在连接前检查原始 Host、解析后的实际 IP、端口与网络策略，并考虑 IPv4/IPv6、私网/环回、DNS 重绑定和重定向策略。证书与主机名验证不得默认关闭。

## 6. HTTP：高层 Client + 可复用 Body 流

```rust
// 同步用法
let mut response = ctx.http()
    .get("https://api.example.com/items")
    .send()?;
std::io::copy(&mut response, &mut output)?;
```

高层 `get/post/put/delete/request` 支持 Header、超时、HTTP 状态、错误映射、受控重定向、Range、认证 Header 和响应流。`body_reader(FileReader)` 对传输本次获准的 Core 文件可直接使用**精确只读 Handle**，无须复制全文件或授予源文件目录权限。泛型 `Read` 与 `AsyncRead` 请求体是增强目标，需要受限流传输和背压。

`tokio-io` 可额外提供 `send_async` 与可 `AsyncRead` 的响应体。插件如使用 reqwest 的**普通默认 Client**会自己建立 OS Socket，无法自动经过 Fiex Broker；只有支持自定义连接器/传输后端的库才能通过 Adapter 复用 Fiex TCP。HTTP 业务应优先使用 SDK 高层 HTTP Client。

## 7. 其余 Core API

| SDK | IPC | 语义 |
|---|---|---|
| `ctx.files().metadata/stat_batch/list` | `host.fs.stat/statBatch/list` | Scope 限定，按文件授权读元数据或目录 |
| `ctx.files().open_read/begin_write/create/remove/rename` | `host.fs.*` | Core 控制源/目标及事务 |
| `ctx.tcp().connect` | `host.net.tcp.open/read/write/close` | 与 HTTP 共享网络 Grant |
| `ctx.http().get/post/put/delete/request` | `host.net.http.*` | 高层 HTTP 与请求/响应流 |
| `ctx.vfs().open_view` | `host.vfs.openView` | 经 Core 创建 Mount/View |
| `ctx.ui().open_panel` | `host.ui.open` | 非 Connection 主导航的受控 UI |
| `ctx.task().spawn/cancel/progress` | Task Broker | 继承已批准 Scope 的任务 |
| `ctx.storage()/secrets()/process()` | 对应 Core Broker | 插件隔离的存储、密钥和命令 |

## 8. 标准事件与 UI SDK

标准 Info / Preview / Decoration Handler 接收 Core 已完成权限投影的 Context：

```rust
// 示意领域类型
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
    visible: Vec<VisibleEntry>,
}
```

Info/Preview 内置与插件返回**相同** `InfoResponse/PreviewView` Core 领域 DTO；读资源引用只允许在被授权时读取本次文件，按需流式访问。自定义 `contextMenu.resolve` 默认只接到当前选中文件 opaque refs，额外 Metadata/文件读写继续通过通用 SDK 申请。

`connection.create/open` 必须返回 `NavigationTarget::{Vfs,PluginView}`：前者由 Core Explorer 渲染，后者用插件包 sandboxed iframe **完整替换右侧主内容区**。其他明确用户触发的 UI 仍可调用通用 `ctx.ui().open_panel`，但不是 Connection 导航返回类型。

UI SDK 在专有 MessagePort 中按允许列表请求 Native Plugin 和 Core 服务；没有原生 Socket、FS 或 Tauri 直接访问权。Rust SDK 和 UI Bridge 均使用相同的 Scope、权限投影和流生命周期规则。

## 9. 第三方库兼容性与验收

| 第三方库接口 | Fiex 接入方式 |
|---|---|
| `std::io::Read/Write/Seek` 泛型 | 直接传 `FileReader/FileWriter/TcpStream` |
| `tokio::io::AsyncRead/Write/Seek` 泛型 | 启用 `tokio-io`，直接传对应 `Async*` 对象 |
| `futures::io::AsyncRead/Write` 泛型 | 使用可选 Compat Adapter |
| 自定义 Transport / FileSystem Trait | 插件提供轻量 Adapter 映射到 SDK IO |
| 要求 `std::fs::File` / `std::net::TcpStream` / raw FD | 不能直接替换，需库支持其他 Backend |
| Reqwest/其他自动直连的高层 Client | 不会自动走 Core；优先 SDK HTTP 或自定义 Connector |

必须增加可复现的 SDK/Core/官方插件集成测试：ZIP `Read+Seek`、ZIP `Write+Seek+commit`；SMB `SmbTransport`；Tokio 的 `tokio::io::copy` 与 async TCP 双向收发；同步阻塞读与并发写；TLS 握手、STARTTLS、正常与异常 EOF、半关闭、取消、断网、连接替换、多个 task；大于 5 GiB 的流和常量内存；错误域名/IP/未授权/过期 Scope。测试应涵盖 Windows/macOS/Linux 的差异。