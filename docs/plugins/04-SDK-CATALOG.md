# 04 · SDK

## Rust SDK

源码：`sdk/rust`。SDK 只服务 native 插件进程。

```rust
fn run(ctx: sdk::RequestContext) -> Result<Value, RpcError> {
    match ctx.method() {
        "hello.greet" => Ok(json!({ "message": "hello" })),
        _ => Err(RpcError::method_not_found(ctx.method())),
    }
}

fn main() {
    sdk::serve("org.example.hello", run).unwrap();
}
```

### 文件

```rust
let files = ctx.files();
let text = files.read_to_string("notes.txt")?;
let mut input = files.open_read(&target.reference)?;
let mut output = files.begin_write("copy.bin", false)?;
std::io::copy(&mut input, &mut output)?;
output.commit()?;
```

- 路径必须是 Core 当前操作/任务授权范围内的路径；
- 大文件用流，不整文件读入内存；
- 写入只有 `commit` 后才替换目标；
- `secret` 字段由 Core 钥匙串保存，SDK 不直接读本地文件。

### HTTP / TCP

```rust
let text = ctx.http().get("https://example.com/info").send()?.text()?;

let mut stream = ctx.tcp().connect("127.0.0.1", 445)?;
stream.write_all(b"ping")?;
stream.read_exact(&mut response)?;
```

HTTP 和 TCP 共用 Manifest 的 `network` 授权。插件不建立原始 socket；`ctx.tcp()` 返回的是 Core Broker 包装后的标准流。SSH 插件即用此方式建立 TCP，再由 `russh` 完成 SSH 握手。

`TcpStream` 实现 `Read + Write + Send + 'static`，支持同一连接读写独立推进、`shutdown` 和关闭。

### 任务

```rust
let task = ctx.spawn_task(|task| {
    if task.is_cancelled() {
        return Err(RpcError::cancelled());
    }
    Ok(Value::Null)
})?;
```

`spawn_internal_task` 用于内部长期流（例如 SSH 终端），不进入用户任务中心。

### Provider

文件系统插件只实现 `FileSystemProvider` 的方法，并用属性宏标注 `impl`：

```rust
#[fiex_sdk::filesystem]
impl FileSystemProvider for GoogleDrive {
    fn list(&self, request: &FsRequest<'_>) -> Result<FsListing, RpcError> { … }
    fn stat(&self, request: &FsRequest<'_>) -> Result<FsEntry, RpcError> { … }
    fn read(&self, request: &FsRequest<'_>) -> Result<FsRead, RpcError> { … }
    fn write(&self, request: &FsRequest<'_>) -> Result<FsWrite, RpcError> { … }
}
```

宏只把实现块里出现的方法计入能力，`plugin.initialize` 自动上报
`fs.operations`；Core 据此决定是否下发 `fs.*`。协议的完整定义见
[10-UNIFIED-FS](10-UNIFIED-FS.md)。

### 其他 Host API

- `ctx.storage()`：类型化插件命名空间（含 CAS）；
- `ctx.secrets()`：系统钥匙串（区分拒绝/不可用/缺失）；
- `ctx.process_run`：已声明的外部命令；
- `ctx.invoke`：调用 Host RPC；
- `ctx.open_vfs_view`：打开自己注册的 VFS 视图。

## UI SDK

插件 iframe 通过 MessagePort 调用受控 bridge；它不能直接访问 Tauri、文件系统或 Keyring。面板入口 HTML 必须是包内静态文件并通过 Core 的 CSP/路径校验。

## 工具链

- `cargo build --release` 构建 native 二进制；
- `scripts/package-fiex-plugin.py` 生成 `.fiexplugin`；
- `plugin_smoke`、`scripts/test-plugin-process.py` 和 CI 集成测试验证真实进程握手；
- `official/ssh` 的集成测试还会连接真实 sshd，验证 TCP Broker、SSH 握手、PTY 和命令输出。
