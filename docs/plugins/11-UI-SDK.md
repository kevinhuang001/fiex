# 11 · @fiex/ui-sdk 与 iframe Host Bridge v1（提案）

[索引](README.md) · [UI 扩展](05-UI-EXTENSIONS.md) · [Native IPC](04-IPC-PROTOCOL.md) · [SDK 概览](08-SDK.md)

> **重要**：以下 API 属于拟议设计，包尚未发布。本文件规定插件 iframe UI 访问 Fiex Host 和自身 Native 后端的公开契约。JS SDK 不依赖 Node.js runtime；使用 npm/pnpm/yarn 只是开发者构建时的选择。

## 1. 两套 SDK，两个版本化通道

| SDK                           | 执行环境             | 传输边界                             | 协议                 |
| ----------------------------- | -------------------- | ------------------------------------ | -------------------- |
| `fiex-sdk`（Rust crate）      | 插件独立 Native 进程 | Core ↔ plugin process                | Native Plugin IPC v1 |
| `@fiex/ui-sdk`（npm package） | iframe 中的浏览器 JS | iframe ↔ UI Host（隔离 MessagePort） | UI Bridge v1         |

**UI Bridge 与 Native Plugin IPC 不能混为一套线协议**：二者可共用资源引用和错误分类的概念，但有独立版本协商、权限作用域和生命周期。iframe 不能直接打开 Native IPC socket/stdio，不能直接调用 Tauri invoke。

```text
Plugin iframe (@fiex/ui-sdk)
     ⇅ Window.postMessage bootstrap → dedicated MessagePort
Fiex UI Host (iframe registry; authenticate & route)
     ⇅ Tauri-internal channel
Fiex Core (permission/context/task/VFS)
     ⇅ JSON-RPC over native process stdio, when needed
Plugin Native backend (fiex-sdk or third-party implementation)
```

纯 UI 插件不需要 Native 进程，可使用受限 Host API；含 VFS/后端命令时必须交付独立 Native 二进制。

## 2. 前端开发与打包

```bash
npm install @fiex/ui-sdk
npm run build  # 由插件开发者执行，产物交付给 Fiex
```

以 Vite 为例：`base: "./"`，构建输出包含 `index.html`、本地 JS/CSS/assets，禁止远程 dev server、远程 CDN 和 Node 专用运行时 API。SDK 可提供 TypeScript 类型、自动 Bridge 握手、取消/超时封装、React hook 适配（独立可选包）和开发测试模拟 Host。**SDK 必须是框架无关的**，不能强迫第三方使用 React。

```typescript
import { fiex } from '@fiex/ui-sdk'

await fiex.ready()

const context = await fiex.context.get()
const result = await fiex.plugin.invoke<{ branch: string }>('git.status', {
  directory: context.directory,
})
const unsubscribe = fiex.events.on('git.statusChanged', (event) => {
  console.log(event)
})

await fiex.panel.resize({ width: 420, height: 560 })
// iframe 关闭前解除订阅
unsubscribe()
```

`plugin.invoke` 只允许请求 **本插件**的 namespace 下、manifest 声明的命令；UI-only 插件无 Native 命令处理器时返回 `unsupported`，不能偷偷路由到其他插件。

## 3. UI Bridge 的会话握手

1. Fiex 安装时确定不可被插件伪造的 `pluginId`、资源完整性与该 iframe 的**独立 origin**；打开面板时生成不复用的 `panelInstanceId` 与绑定窗口/挂载/选择范围的权限上下文。
2. 主界面等待指定 iframe `load`，验证 `event.source === iframe.contentWindow` 且符合受信资源 origin；通过 `postMessage` 引导建立一条 **MessageChannel**，并转交仅属于该实例的 `MessagePort`。握手携带一次性随机 `nonce`，端口消息必须回显会话 ID/nonce 摘要才能标记 ready。
3. 只有成功验证的端口可执行 `ui.bridge.request`；所有后续调用使用该专用端口，不接受页面对主窗口的任意广播。端口权限绑定进程中的宿主记录而非信任 payload 内的 `pluginId` 或 `mountId`。
4. 限定消息频率、payload 大小、最大并发、握手超时和请求超时。关闭/卸载/权限撤销时关闭端口，清理 subscriptions，拒绝迟到响应。重新打开建立**新会话**。
5. **不可把 nonce 当作永久密钥**；随机值只绑定握手。UI 桥最终鉴权依赖宿主保存的 session、来源、iframe 实例、用户授权及 Core 对节点/命令的二次检查。
6. 若不同平台的资源来源具有 opaque origin，设计需要专门的 origin/source 校验实现并通过安全测试；不可仅凭消息中的 token、`"*"` targetOrigin 或 `event.data.pluginId` 识别身份。

提案中 UI Bridge 对外版本为 `"1.0"`；主/次版本遵循 [版本策略](08-SDK.md)，不等于 Native IPC `"2.0"` 的同一个约束。

## 4. Wire envelope（内部线格式草案）

传输统一使用结构化可序列化对象；字段名和错误码在冻结协议前由 JSON Schema 校验。

请求：

```json
{
  "bridgeVersion": "1.0",
  "sessionId": "panel-18",
  "requestId": "ui-42",
  "type": "request",
  "method": "plugin.invoke",
  "params": { "command": "git.status", "arguments": {} }
}
```

成功响应：

```json
{
  "bridgeVersion": "1.0",
  "sessionId": "panel-18",
  "requestId": "ui-42",
  "type": "response",
  "result": { "branch": "main" }
}
```

失败响应：

```json
{
  "bridgeVersion": "1.0",
  "sessionId": "panel-18",
  "requestId": "ui-42",
  "type": "response",
  "error": {
    "code": "permission_denied",
    "message": "Not authorized for this resource"
  }
}
```

事件使用 `type: "event"`、`subscriptionId`、`name`、`payload`；请求取消使用 `type: "cancel"`、对应 `requestId`。Bridge 事件必须订阅后才能接收，且命名空间范围被验证。`MessagePort` 的 structure clone 不等于可以直接 transfer 未授权 File/Blob/MessagePort 对象；v1 只允许 JSON 兼容的值、受限文件资源引用与显式授权的二进制数据接口。

## 5. 公共 TypeScript API（v1 拟议）

```typescript
type FiexResourceRef = {
  mountId: string
  nodeId: string
  version?: string
}

interface FiexUiSdk {
  ready(): Promise<void>
  context: {
    get(): Promise<ExplorerContext> // 最小授权快照
    subscribe(handler: (ctx: ExplorerContext) => void): () => void
  }
  plugin: {
    invoke<T>(command: string, args?: unknown, options?: { signal?: AbortSignal }): Promise<T>
  }
  vfs: {
    stat(ref: FiexResourceRef): Promise<FileStat>
    list(ref: FiexResourceRef, options?: { cursor?: string; limit?: number }): Promise<ListPage>
    open(ref: FiexResourceRef): Promise<void> // 由宿主打开、非任意 OS 文件句柄
  }
  events: {
    on<T>(name: string, handler: (event: T) => void): () => void
  }
  tasks: {
    list(): Promise<TaskSummary[]>
    cancel(taskId: string): Promise<void> // 仅可取消授权范围内任务
    create(options: { title: string; cancellable?: boolean }): Promise<TaskSummary>
  }
  panel: {
    resize(size: { width?: number; height?: number }): Promise<void>
    close(): Promise<void>
  }
  theme: {
    get(): Promise<ThemeSnapshot>
    subscribe(handler: (theme: ThemeSnapshot) => void): () => void
  }
}
```

`ExplorerContext` 只包含当前已授权 selection、directory、window/pane ID 和最小能力信息，不提供全局可遍历系统路径。事件与上下文权限可变；所有操作必须重新校验。

`tasks.create` **只创建当前 UI 会话作用域的可见短任务**，关闭 iframe 时由 Host 终止或置为中断；不能伪造后台完成。持续运行的 Git Push/同步/大文件传输等任务必须由 **Native 插件或 Core** 创建和拥有，iframe 只能列举/观察/申请取消有权访问的任务。

`panel.resize` 是请求，实际大小始终受宿主布局和用户选择限制。`panel.close` 只能关闭自己的实例。未来增加新宿主 API 时必须是可选/按能力协商，不能默默扩大旧插件的访问范围。

## 6. 静态资源与运行环境边界

- 浏览器 UI 不允许 Node.js `require/process/fs`；不得假定系统提供 npm、Node、Python。
- `ui/main/index.html` 由 Core 加载为**插件隔离来源**，只允许本包静态 JS/CSS/字体图片，不要给 iframe 主窗口的 Tauri capability。
- `window.postMessage` 仅作引导，之后用已绑定的 `MessagePort`；宿主验证 iframe identity、会话及操作授权。
- 默认不允许任意 fetch 外网、打开文件系统路径、访问其他插件资源；如果未来开放浏览器直接网络 API，需要独立权限模型、CSP/服务端资源策略，不应借 JS SDK 绕过 Native plugin 网络权限。
- 插件界面可以读取 Fiex 注入的安全主题 CSS 变量，不能编辑主窗口的 DOM 或强制覆盖宿主样式。

## 7. UI-only、Native-only、Hybrid 三种包

| 形态        | 必需资源                    | 可以调用                                       |
| ----------- | --------------------------- | ---------------------------------------------- |
| UI-only     | manifest + 编译好的 `ui/`   | 授权后的 UI Host API；无 plugin Native command |
| Native-only | manifest + `bin/<platform>` | Native IPC 提供 VFS/任务/后端命令；无 iframe   |
| Hybrid      | manifest + `bin/` + `ui/`   | UI Host API + 本插件 Native command            |

Native 插件不得注入其 Native 进程中的网页服务来替换包内已编译静态页面。任何功能插件若想暴露虚拟目录树，必须在 Native 端实现规范 VFS 提供者或由 Core 提供明确的受控虚拟数据注册能力（后者不是 v1 必需）。

## 8. 向后兼容和验收

- 发布 `@fiex/ui-sdk` 前，固定 UI Bridge `major/minor`、所有方法 schema、错误码、取消语义、panel lifecycle、主题/资源引用类型，配合契约测试。
- SDK 包版本与 UI Bridge 版本独立。新版 SDK 必须在旧宿主协商后的最小能力集内运行或给出 `unsupported`；不得因 SDK 升级强制修改旧插件 UI。
- Windows、macOS、Linux 每平台测试预编译 React/Vite UI（ESM 静态资源）、纯 HTML UI、双面板与多窗口、关闭后迟到响应、iframe 尝试调用 Tauri、跨插件数据请求、CSP 阻断远程脚本。
- 一个非官方 UI-only 示例包必须无需 Native 进程就能安装与工作，证明加载器没有隐藏的官方/Native 必填分支。
