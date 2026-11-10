# Native 插件 IPC 协议参考

本页描述 Fiex Core 与 Rust native 插件进程之间的当前 IPC。目标 Core 主分支目前固定 manifest schema 2、protocol 7.0 和 SDK crate 0.3.4；Core 的 submodule commit 以[代码基线](/reference/source-baseline)为准。协议实现源文件为 Core 锁定的 `sdk/rust/src/lib.rs`、`sdk/rust/src/runtime/` 和 `src-tauri/src/plugins/host/process/rpc/`。

## 边界与方向

Native 插件是独立进程，Core 管理其启动、标准输入/输出和生命周期。方向如下：

| 方向 | 通道 | 用途 |
| --- | --- | --- |
| Core → 插件 | Core 写插件 stdin | 生命周期请求和标准 provider 事件 |
| 插件 → Core | 插件写 stdout | Provider 响应，以及 SDK 发出的 Host API 调用 |
| Core → 插件 | Core 写插件 stdin | 对 Host API 调用的 JSON-RPC response |

SDK runtime 一条读线程接收帧，业务请求并发运行，单一写锁串行化完整 response frame。Core 也通过单一插件进程连接匹配 request ID 与响应。插件 `stdout` 只能包含协议帧；诊断日志写入 `stderr`。

UI 插件页面使用另一套 UI bridge，页面调用通过受控的 Tauri/iframe bridge 路由；不要将 UI bridge 方法与 native JSON-RPC 混成一个接口。UI 侧接口见 [UI SDK IPC audit](https://github.com/kevinhuang001/fiex-sdk/blob/24664a69bb411d0b2608461ae08a588f28cbe385/docs/ipc-audit.md)。

## 帧格式

每个消息由 ASCII 头部、空行和 UTF-8 JSON body 组成：

```text
Content-Length: 59\r\n
\r\n
{"jsonrpc":"2.0","id":1,"method":"plugin.ping","params":{}}
```

规则：

- `Content-Length` 是 JSON body 的 **UTF-8 字节数**，不是字符数。
- 头部行以 CRLF 结束；空 CRLF 结束 header。
- 单帧 body 上限 **4 MiB**，header 上限 **8192 bytes**。
- 缺失、重复或非法 Content-Length、超大帧、截断 body、无效 JSON 均按协议错误处理。
- 每条帧后不需要额外分隔符；接收方按 body 字节数读取，之后立即读取下一帧。
- 不要通过 stdout 打印 banner、进度或日志。一个多余的 `println!` 就会让 Core 把日志误认为下一帧。

SDK 已提供 `read_frame` / `write_frame`；常规插件不应自行重写传输层。

## JSON-RPC 消息

请求的基本形状：

```json
{
  "jsonrpc": "2.0",
  "id": 12,
  "method": "connection.validate",
  "params": {},
  "locale": "zh-CN"
}
```

Core 的标准事件可带顶层 `locale`，SDK 将它写入该请求的 `RequestContext`，并让派生的后台 task 继承当前 locale。`plugin.initialize` 使用 `params.protocolVersion`；一般事件所需数据由相应 provider DTO 定义。

有 ID 的请求必须回复相同 ID。成功使用 `result`；失败使用 `error`：

```json
{"jsonrpc":"2.0","id":12,"result":{"valid":true}}
```

```json
{"jsonrpc":"2.0","id":12,"error":{"code":-32602,"message":"missing connection"}}
```

不要把 response 当成通知，也不要为同一 request ID 回复多次。SDK 处理请求并发和响应关联。

## 初始化与生命周期

Core 首次调用 provider 前发送：

```json
{"jsonrpc":"2.0","id":1,"method":"plugin.initialize","params":{"protocolVersion":"7.0"}}
```

SDK 要求协议版本严格匹配。初始化成功的回复至少包括：

```json
{
  "pluginId": "org.example.hello",
  "protocolVersion": "7.0",
  "sdkVersion": "0.3.4",
  "ready": true,
  "concurrentRequests": true,
  "tasks": true
}
```

`Providers` builder 会在这个结果中补充已注册 provider 的能力。VFS 插件还会报告 `vfs.operations`、`vfs.discovery`、`vfs.preflightOps` 和每个已注册 VFS provider 的能力；Core 校验插件 ID、版本并记录能力声明。VFS 声明缺字段或不满足 discovery contract 时，Core 不会把它当作静态能力默许。

| 方法 | 方向 | 行为 |
| --- | --- | --- |
| `plugin.initialize` | Core → 插件 | 首个业务调用前握手；版本不支持时拒绝并关闭该进程 |
| `plugin.ping` | Core → 插件 | 初始化后检查进程是否响应，结果为 `{ "alive": true }` |
| `plugin.shutdown` | Core → 插件 | 请求优雅关闭；插件回复后停止读循环和后台任务 |
| `plugin.cancel` | Core → 插件 | 以 `params.taskId` 请求取消 SDK 后台 task；取消是协作式的 |

SDK 运行时最多并发处理 64 个普通请求，并限制 8 个后台任务。较长的请求使用 task 生命周期并检查取消标记；不能假设客户端断开就能中断 Rust 代码。

## Core 调用插件的标准事件

这些方法由 SDK `Providers` 映射到类型化 trait。所有者仍应以锁定 SDK 里的 DTO 为准。

| 方法 | 处理者 | 说明 |
| --- | --- | --- |
| `connection.validate` | `ConnectionProvider::validate` | 校验用户配置或远端连接 |
| `connection.open` / `connection.create` | `ConnectionProvider` | 打开已有连接或创建连接；可返回 VFS 或插件视图 |
| `info.compute` | `InfoProvider::compute` | 只计算 Core 请求的 field keys；字段静态声明在 manifest |
| `preview.open` | `PreviewProvider::open` | 返回包内预览页面或 PreviewView |
| `decorations.resolve` | `DecorationProvider::resolve` | 生成请求对象的动态装饰 |
| `contextMenu.resolve` / `contextMenu.execute` | `ContextMenuProvider` | 分别列出与执行文件菜单动作 |
| `connectionContextMenu.resolve/execute` | Connection Context Menu provider | 列出与执行连接入口动作 |
| `search.query` | `SearchProvider::query` | 返回插件搜索结果 |
| `hooks.invoke` | `HookProvider::invoke` | 处理已声明 hook 事件 |
| `toolbar.invoke` | `ToolbarProvider::invoke` | 执行工具栏命令或返回面板动作 |

当前协议没有旧式 `info.describe`；Info 字段必须在 manifest 的 `extensions.info.sections` 声明。旧的 `fs.*` provider 命名和 UI 侧 VFS bridge 也不属于当前协议。

## VFS provider 方法

Core 使用统一的 `vfs.*` namespace 调用连接挂载和插件虚拟树。`vfs.discover` 根据当前连接、mount 和资源返回已实现操作中的当前可用子集。

| 操作 | 方法 |
| --- | --- |
| 发现能力 | `vfs.discover` |
| 附加动态预检 | `vfs.preflight` |
| 枚举/元数据/搜索 | `vfs.list`、`vfs.stat`、`vfs.search` |
| 读取 | `vfs.openRead`、`vfs.readStream`、`vfs.closeRead` |
| 写入 | `vfs.beginWrite`、`vfs.writeStream`、`vfs.commit`、`vfs.abort` |
| 修改结构 | `vfs.mkdir`、`vfs.rename`、`vfs.trash`、`vfs.remove` |

读取和写入是多次有界请求组成的流；Core/SDK 关联 stream ID、mount owner 与偏移。SDK provider 用普通 `Read` / `Write` 和 commit callback 实现，不应自行分配 wire stream ID。写入只有 `commit` 才发布；abort 或未提交关闭应丢弃暂存数据。VFS 完整要求见[虚拟文件系统 VFS](/developer/plugin-development/vfs)。

## 插件调用 Core 的 Host API

插件通常通过 SDK 类型化 API 调用 Host，不要手写以下 wire method。SDK 在每个调用的 `params.authorization` 中附加当前 operation/task 身份；Core 根据该身份恢复原始 scope，并校验插件声明、用户 grant、对象归属和目标资源。

### 文件资源：`host.fs.*`

SDK `Files` 面向 Core 提供的 `ResourceRef`，并不接受任意本机路径。当前公开操作包括：

| 目的 | Host method（SDK 内部使用） |
| --- | --- |
| 解析可信映射、派生子资源 | `host.fs.resolveMapped`、`host.fs.child` |
| 目录/元数据 | `host.fs.list`、`host.fs.stat`、`host.fs.statBatch` |
| 读取流 | `host.fs.openRead`、`host.fs.readOpen`、`host.fs.closeRead` |
| 写入暂存 | `host.fs.beginWrite`、`host.fs.writeChunk` / `writeBuffer`、`host.fs.seekWrite`、`host.fs.commit`、`host.fs.abort` |
| 目录创建与修改 | `host.fs.mkdir`、`host.fs.remove`、`host.fs.rename` |

小文件 `Files::read` 有 64 MiB 上限；更大的数据用 `open_read()` 流式处理。`ResourceRef` 是本次操作范围内的资源身份，不是权限 bearer token；调用仍需权限和 scope 通过。

### 网络与认证

| SDK 服务 | Host namespace | 关键行为 |
| --- | --- | --- |
| HTTP | `host.net.http.*` | Core 建立连接、执行重定向/域名策略并提供有界响应流；大 request body 可流式上传 |
| TCP | `host.net.tcp.*` | Core brokered TCP stream；使用 byte-based read/write 与 deadline |
| OAuth/browser auth | `host.oauth.authorize` | 通过 Core 授权流程完成浏览器认证，不自行启动本地 callback listener |
| aria2 下载 | `host.aria2.*` | 可选的 Core 下载服务，返回流式结果 |

网络权限按目标 host 校验。不要用 SDK 外的 socket/HTTP client 绕过 Core；平台沙箱也会限制直接网络访问。

### 存储、凭据、进程和任务

- `host.storage.*`：插件拥有命名空间的持久化数据，支持读、写、删除、列键和 CAS/update；不要存储敏感凭据。
- `host.secret.*`：由 Core/系统凭据后端管理可用性、读取、写入和删除；区分“凭据缺失”和“凭据服务不可用”。
- `host.process.run`：运行 manifest 中明确允许并经用户同意的外部命令。参数是分离的 argv，不是拼接 shell 字符串；命令在当前用户权限下运行，具有文件 API 之外的风险。
- `host.task.start/progress/finish`：由 SDK task API 管理后台工作和授权生命周期；Core 可用 `plugin.cancel` 请求协作取消。
- `host.vfs.openView`：由 SDK `open_vfs_view` 打开一个 Core 托管的插件视图。
- `host.error.report` / `host.log.report`：结构化上报错误或日志；不要携带访问令牌和文件私密内容。

`host.buffer.*` 是 SDK 内部用于共享内存传输的实现细节，不是插件业务 API。不要直接调用或依赖其 descriptor 生命周期。

## 权限上下文与资源模型

一个插件事件请求可以绑定到文件、目录、连接或 VFS mount。每次 Host call 的 `authorization` 指向当前 operation 或由该 operation 创建的 task。Core 用这段上下文恢复 scope，因此同时运行的两个请求不能相互借用对方的 `ResourceRef`、句柄或连接权限。

重要边界：

- Manifest 能力 + 用户批准 + 本次 Operation Scope 是独立检查。
- `metadata` 不自动提供目录列举或文件内容读取。
- provider 的 `operations()`/`discover()` 是功能能力，不等同于用户授权。
- `ResourceRef` 只可用于相应的当前请求/任务，不应缓存后跨会话使用。
- Core 错误/过期授权必须按失败处理，不要改用未经授权的宿主路径补救。

## 错误处理

SDK 公共 `RpcError` 当前包含这些 JSON-RPC error code：

| code | 名称 | 常见用法 |
| --- | --- | --- |
| `-32601` | Method not found | 插件不支持请求方法或 provider 未注册 |
| `-32602` | Invalid params | 缺少字段、格式不正确或资源不属于当前调用 |
| `-32603` | Internal error | 内部执行失败、依赖服务失败或错误响应 |
| `-32800` | Cancelled | 后台操作观察到协作取消 |

Core 对 Host API 的授权、权限、scope 和宿主执行错误可能使用 `-32001`，并在 message 中给出具体原因。不要用未知错误当作成功，也不要把敏感参数拼进错误文本。

## 变更兼容规则

协议版本是严格匹配，不会尝试自动转换旧 DTO。更改字段、错误语义、capability negotiation、Host namespace 或权限行为时，应同步：

1. SDK shared contracts 和 Rust/UI SDK；
2. Core SDK submodule 与 host dispatch；
3. 官方插件和第三方迁移说明；
4. protocol/reference 文档及 Core 的 IPC 集成覆盖。

具体 wire DTO 的权威位置是 Core 当前锁定的 SDK commit，而不是另一个仓库可能更靠前的 `main`。
