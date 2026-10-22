# 08 · SDK、语言策略与第三方开发指南

[索引](README.md) · [上一篇：安全和打包](07-SECURITY-PACKAGING.md) · [下一篇：迁移](09-MIGRATION-TESTING.md)

## 1. 核心决策

**官方第一版提供两套 SDK：Rust Native `fiex-sdk` 和浏览器端 npm 包 `@fiex/ui-sdk`。两种线协议各自版本化；Native Plugin IPC 仍须语言无关。** Native SDK 是协议的便利封装，**不是**插件唯一入口；UI SDK 封装 iframe → UI Host 的受控消息桥，不能直接访问 Native IPC。第三方可以用 Rust、Go、C/C++ 或打包了运行时的 Python/JS 构建插件，只要交付目标平台能直接启动的独立可执行文件；Fiex **不**安装或管理 Python、Node.js、Bun、Deno 等语言运行时。

原生编译优先用于官方 Native 插件以降低体积与启动开销，但不强制只用 native 语言；Python/PyInstaller 或 JS/Bun/Deno 自包含构建应被协议测试支持。插件二进制的系统库依赖仍需作者注明。

**插件 UI** 可以使用 React、Vue、Svelte 或纯 HTML/JS，经开发者构建后交付静态资源，以 iframe 加载。Node.js/npm 只用于可选的开发构建，Fiex 运行时不提供 Node。纯 UI 包可没有 Native 二进制。浏览器 API 与权限的唯一公开入口是 [11 UI SDK](11-UI-SDK.md)。

## 2. Rust Native SDK 计划模块

```text
fiex-sdk/
  runtime/     # stdin/stdout framing, JSON-RPC, heartbeat, errors
  plugin/      # Plugin trait, initialize/shutdown, manifest
  vfs/         # List, Node, FileSystemProvider, stream API
  ui/          # Toolbar/ContextMenu 贡献以及静态 iframe 资源声明
  tasks/       # task creation, progress, cancellation tokens
  host/        # mounted data, authorized context, transfer requests
  auth/        # consented secret reference / connection validation
  testing/     # fake host, in-memory FS, contract fixtures
```

SDK 暴露异步 API，但不规定最终只能使用 Tokio。异步生命周期由 SDK runtime 管理；所有跨进程调用依旧遵循 [IPC 文档](04-IPC-PROTOCOL.md)。

## 3. 最小 Rust 插件（伪代码）

> 示例是**拟议 API**，不是当前可编译的 crate；定版前应由样例工程的编译测试保证签名准确。

```rust
use fiex_sdk::prelude::*;

struct ExampleFs;

#[async_trait]
impl FileSystemProvider for ExampleFs {
    async fn list(&self, req: ListRequest) -> Result<ListPage, PluginError> {
        Ok(ListPage::new(vec![
            Node::directory("projects", req.directory_id, "Projects")
        ]))
    }

    async fn stat(&self, req: StatRequest) -> Result<Node, PluginError> {
        // 查询或构造节点元数据；不需要处理 Core 缓存
        todo!()
    }
}

#[tokio::main]
async fn main() -> Result<(), PluginError> {
    PluginBuilder::new("org.example.memory")
        .filesystem(ExampleFs)
        .toolbar("example.open", "我的文件", "example.panel")
        .panel_schema("example.panel", include_str!("panel.json"))
        .run_stdio()
        .await
}
```

插件实现需要的是资源树、命令和业务协议。缓存、离线可用、同步规则、任务 ID 管理和 UI 渲染由 Core 实现。官方 Git 插件同样调用公共 SDK，不能直接链接 `fiex-core`。

## 4. UI SDK 单独提供的能力

`@fiex/ui-sdk` 通过 npm 发布，面向浏览器 TypeScript/JavaScript，框架无关，提供 `ready`、`context`、`plugin.invoke`、`events`、`vfs`、`tasks`、`panel`、`theme`，自动完成握手、实例绑定、消息限额、取消与会话释放。UI Bridge v1 与 Native Plugin IPC v1 **相互独立**。React hooks 仅能作为可选适配层。

```bash
npm install @fiex/ui-sdk
npm run build  # 开发者执行，将 dist/index.html 与 JS/CSS 打入插件包
```

## 5. SDK 对开发者的保证

- 自动处理 JSON-RPC 帧、请求 ID、日志分离、协议握手、结构化错误、数据流票据、版本兼容。
- 自动把用户取消映射为任务作用域的取消 token，并阻止终态重复报告。
- 提供合法节点/文件名/目录分页验证器、受限 UI builder、manifest/schema 检查器。
- 默认禁用未声明能力的调用；所有 host API 仍在 Core 作最终鉴权，SDK 检查不是安全边界。
- 提供 FakeHost、内存中的 VFS、模拟掉线/超时/取消、消息 golden files、跨平台打包辅助。
- 公开生成的 JSON Schema/类型定义（可用 TS/Go 等实现），不要求其他语言直接解析 Rust 类型。

## 6. 开发、打包、调试步骤

建议未来开发流程：

```text
cargo install fiex-plugin-cli          # 拟议命令，尚不存在
fiex-plugin new sample --lang rust      # 生成 manifest、SDK 样例和测试
cargo build --release --target ...
fiex-plugin validate ./plugin          # schema/ABI/capability/路径检查
fiex-plugin pack ./plugin -o sample.fiexplugin
fiex-plugin dev --host ./fiex           # 开发模式启动真实独立进程
```

开发模式应仍执行相同协议与最小权限规则；日志通过 stderr/宿主日志面板查看，不从 stdout 打印。CLI 无法可用前，CI 可以直接编译样例并调用协议契约测试器。

## 7. API/SDK 版本策略

Rust Native SDK Cargo 版本、浏览器 SDK npm 包版本、Native IPC 版本和 UI Bridge 版本**彼此独立**。宿主与插件在 `plugin.initialize` 协商协议范围；SDK 可比宿主新，但只能调用协商后的方法。v1 的字段和能力扩展应以可选字段兼容，不允许静默改变 `fs.commitWrite` 的覆盖语义。

必须至少提供一个**非 Rust 的协议测试插件**（可以是 Go 小程序或其他实现），用于验证“无需链接官方 SDK 仍可运行”的关键承诺；没有该测试，不能声称协议语言无关已验证。
